//! axga TUI mode — beautiful terminal interface.
//!
//! Layout: Status bar | Chat with bullets | Border-decorated input
//! Theme: Semantic color tokens, dark mode by default

use axga_tui::app::{App, ChatLine, InputMode, PendingPrompt};
use axga_tui::theme;
use axga_core::{Conversation, ToolRegistry, run_turn_streaming, continue_turn_streaming, simple_chat, StreamHandler, load_config, save_config, PermissionManager, PermissionMode};
use axga_core::tools::ask_user::parse_ask_user_result;
use axga_shared::types::{AgentMessage, ToolCall};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
pub async fn run_tui(
    mut provider: String,
    base_url: Option<&str>,
    mut model: String,
    system_prompt: Option<&str>,
    max_turns: usize,
    dangerous: bool,
    yolo: bool,
) -> anyhow::Result<()> {
    let api_key = resolve_api_key(&provider);
    let mut registry = axga_core::build_default_registry(
        dangerous,
        Some(&provider),
        Some(&model),
        api_key.as_deref(),
        base_url,
    )?;
    let mut conversation = Conversation::new();
    let mut terminal = ratatui::init();
    let th = theme::dark_theme();
    let mut app = App::new(&model, th);

    // Permission manager
    let mode = if yolo { PermissionMode::Auto } else { PermissionMode::Manual };
    let permissions = Arc::new(PermissionManager::new(mode));

    // Welcome
    app.chat_lines.push(ChatLine::Info(format!("axga v{} — {} / {}", env!("CARGO_PKG_VERSION"), provider, model)));
    app.chat_lines.push(ChatLine::Info("Type a message, press Enter. i=insert  Esc=normal  :q=quit  :tools=list".into()));
    app.chat_lines.push(ChatLine::Spacer);

    // Git branch detection
    if let Ok(output) = std::process::Command::new("git").args(["branch", "--show-current"]).output() {
        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !branch.is_empty() {
                app.status.git_branch = Some(branch);
            }
        }
    }

    // Set cwd and permission mode on status line
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_str = cwd.to_string_lossy().replace('\\', "/");
        // Strip Windows \\?\ prefix if present
        let cleaned = cwd_str.strip_prefix("//?/").unwrap_or(&cwd_str);
        app.status.cwd = cleaned.to_string();
    }
    app.status.permission_mode = if yolo { "AUTO".into() } else { "MANUAL".into() };

    let result = tui_loop(
        &mut terminal, &mut app,
        &mut provider, base_url, &mut model,
        system_prompt, max_turns,
        &mut registry, &mut conversation,
        permissions,
    ).await;

    ratatui::restore();
    result.map_err(|e| anyhow::anyhow!("{e}"))
}

fn resolve_api_key(provider: &str) -> Option<String> {
    // 1. Try config file
    if let Some(config) = load_config() {
        if let Some(ref key) = config.provider.api_key {
            return Some(key.clone());
        }
    }
    // 2. Try environment per provider
    match provider {
        "openai" | "deepseek" => std::env::var("OPENAI_API_KEY").ok()
            .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok()),
        "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
        _ => None,
    }
}

/// StreamHandler implementation that renders to the TUI in real-time.
struct TuiStreamHandler<'a, 'b> {
    app: &'a mut App,
    terminal: &'b mut DefaultTerminal,
    /// Index in chat_lines of the current streaming assistant line.
    streaming_line_idx: Option<usize>,
    /// Accumulated assistant text for the current turn.
    accumulated_text: String,
    /// Tool calls tracked during this turn (name → detail shown).
    tools_seen: Vec<String>,
    /// If an ask_user_question tool was executed, holds (tool_call_id, questions_json).
    ask_user_pending: Option<(String, String)>,
}

impl StreamHandler for TuiStreamHandler<'_, '_> {
    fn on_thinking(&mut self) {
        self.app.is_streaming = true;
        // Push an empty assistant line as a streaming placeholder
        self.app.chat_lines.push(ChatLine::Assistant(String::new()));
        self.streaming_line_idx = Some(self.app.chat_lines.len() - 1);
        let _ = self.terminal.draw(|f| self.app.render(f));
    }

    fn on_text_delta(&mut self, text: &str) {
        self.accumulated_text.push_str(text);
        // Update the streaming line in-place
        if let Some(idx) = self.streaming_line_idx {
            if idx < self.app.chat_lines.len() {
                self.app.chat_lines[idx] = ChatLine::Assistant(self.accumulated_text.clone());
            }
        }
        let _ = self.terminal.draw(|f| self.app.render(f));
    }

    fn on_tool_call_delta(&mut self, _id: &str, _name: &str, _args: &str) {
        // Tool call deltas arrive during streaming — we defer showing until execution
    }

    fn on_tool_call_result(&mut self, name: &str, result: &str) {
        // Check if this is an ask_user_question result — don't show inline
        if name == "ask_user_question" {
            if let Some(parsed) = parse_ask_user_result(result) {
                let questions_json = parsed["questions"].to_string();
                // tool_call_id is not directly available in the handler — we'll
                // store the questions and detect the tool_call_id from the
                // conversation in post-turn processing.
                self.ask_user_pending = Some((String::new(), questions_json));
                self.tools_seen.push(name.to_string());
                return;
            }
        }
        // Show tool result
        let detail = if result.len() > 80 {
            format!("{}...", &result[..80])
        } else {
            result.to_string()
        };
        self.app.chat_lines.push(ChatLine::Tool {
            name: name.to_string(),
            detail,
        });
        self.tools_seen.push(name.to_string());
        let _ = self.terminal.draw(|f| self.app.render(f));
    }

    fn on_done(&mut self) {
        self.app.is_streaming = false;
        // If no text accumulated and we have a streaming placeholder, remove it
        if self.accumulated_text.is_empty() {
            if let Some(idx) = self.streaming_line_idx {
                if idx < self.app.chat_lines.len() {
                    self.app.chat_lines.remove(idx);
                }
            }
        }
        let _ = self.terminal.draw(|f| self.app.render(f));
    }
}

/// No-op stream handler for when we just need to execute tools without real-time rendering.
struct NoopStreamHandler;

impl StreamHandler for NoopStreamHandler {
    fn on_thinking(&mut self) {}
    fn on_text_delta(&mut self, _text: &str) {}
    fn on_tool_call_delta(&mut self, _id: &str, _name: &str, _args: &str) {}
    fn on_tool_call_result(&mut self, _name: &str, _result: &str) {}
    fn on_done(&mut self) {}
}

#[allow(clippy::too_many_arguments)]
async fn tui_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    provider: &mut String,
    base_url: Option<&str>,
    model: &mut String,
    system_prompt: Option<&str>,
    max_turns: usize,
    registry: &mut ToolRegistry,
    conversation: &mut Conversation,
    permissions: Arc<PermissionManager>,
) -> anyhow::Result<()> {
    let mut spinner_tick: usize = 0;
    let mut pending_approvals_stack: Vec<ToolCall> = Vec::new();
    let mut resolved_calls: Vec<ToolCall> = Vec::new();
    let mut noop_handler = NoopStreamHandler;

    loop {
        app.spinner_idx = spinner_tick;
        spinner_tick = spinner_tick.wrapping_add(1);

        terminal.draw(|f| app.render(f))?;

        if app.exit {
            break;
        }

        // Poll for events (100ms tick for spinner animation)
        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;

            match ev {
                Event::Key(key) => {
                    // Global: Ctrl+C → quit (press twice if streaming)
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        if app.is_streaming {
                            app.chat_lines.push(ChatLine::Info("Press Ctrl+C again to force quit".into()));
                            app.is_streaming = false;
                        } else {
                            app.exit = true;
                            break;
                        }
                        continue;
                    }

                    // Global scroll — works in any mode
                    match key.code {
                        KeyCode::Up => { app.scroll_by(-1); continue; }
                        KeyCode::Down => { app.scroll_by(1); continue; }
                        KeyCode::PageUp => { app.scroll_by(-10); continue; }
                        KeyCode::PageDown => { app.scroll_by(10); continue; }
                        _ => {}
                    }

                    match app.mode {
                        InputMode::Insert => {
                            match key.code {
                                KeyCode::Esc => app.mode = InputMode::Normal,
                                KeyCode::PageUp | KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => { app.scroll_by(-10); }
                                KeyCode::PageDown | KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => { app.scroll_by(10); }
                                KeyCode::Up => { app.scroll_by(-1); }
                                KeyCode::Down => { app.scroll_by(1); }
                                KeyCode::Enter => {
                                    let input = std::mem::take(&mut app.input);
                                    app.cursor_pos = 0;
                                    if input.trim().is_empty() { continue; }

                                    // Interactive wizard: provider setup
                                    match std::mem::replace(&mut app.pending_prompt, PendingPrompt::None) {
                                        PendingPrompt::ApiKey { provider: p, model: pref_model } => {
                                            if let Some(mut config) = load_config() {
                                                config.provider.api_key = Some(input.clone());
                                                config.provider.provider_type = Some(p.clone());
                                                let _ = save_config(&config);
                                            }
                                            app.chat_lines.push(ChatLine::Info(format!("API key saved for {p}")));
                                            if let Some(m) = pref_model {
                                                *provider = p;
                                                *model = m;
                                                app.status.model = model.clone();
                                                conversation.reset();
                                                app.chat_lines.push(ChatLine::Info(format!("Switched to {provider} / {model}")));
                                                app.chat_lines.push(ChatLine::Info("Conversation reset. Ready to chat!".into()));
                                            } else {
                                                let models = match p.as_str() {
                                                    "deepseek" => "deepseek-v4-flash / deepseek-v4-pro",
                                                    "openai" => "gpt-4o-mini / gpt-4o / o1-mini",
                                                    _ => "gpt-4o-mini",
                                                };
                                                app.chat_lines.push(ChatLine::Info(format!("Select model for {p}: {models}")));
                                                app.chat_lines.push(ChatLine::Info("Type model name and press Enter:".into()));
                                                app.pending_prompt = PendingPrompt::Model { provider: p };
                                            }
                                            continue;
                                        }
                                        PendingPrompt::Model { provider: p } => {
                                            *provider = p.clone();
                                            *model = input.clone();
                                            app.status.model = input;
                                            conversation.reset();
                                            app.chat_lines.push(ChatLine::Info(format!("Switched to {p} / {model}")));
                                            app.chat_lines.push(ChatLine::Info("Ready to chat!".into()));
                                            continue;
                                        }
                                        PendingPrompt::ApprovalDialog { tool_name, tool_args, tool_id } => {
                                            match input.trim().to_lowercase().as_str() {
                                                "y" | "yes" => {
                                                    permissions.approve(&tool_name);
                                                    app.chat_lines.push(ChatLine::Info(format!("Approved: {tool_name}")));
                                                }
                                                "n" | "no" => {
                                                    permissions.deny(&tool_name);
                                                    app.chat_lines.push(ChatLine::Info(format!("Denied: {tool_name}")));
                                                }
                                                "a" | "all" => {
                                                    permissions.approve_all();
                                                    pending_approvals_stack.clear();
                                                    app.chat_lines.push(ChatLine::Info("Approved all — switched to Auto mode".into()));
                                                }
                                                _ => {
                                                    app.chat_lines.push(ChatLine::Error("Type 'y' (approve), 'n' (deny), or 'a' (approve all)".to_string()));
                                                    app.pending_prompt = PendingPrompt::ApprovalDialog { tool_name, tool_args, tool_id };
                                                    continue;
                                                }
                                            }
                                            // Pop next pending approval or execute resolved if stack is empty
                                            if let Some(next) = pending_approvals_stack.pop() {
                                                let args_display = serde_json::to_string_pretty(&next.arguments)
                                                    .unwrap_or_else(|_| String::new());
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Pending approval: {} <- {} ({})",
                                                    next.name, &args_display[..std::cmp::min(80, args_display.len())],
                                                    if args_display.len() > 80 { "..." } else { "" }
                                                )));
                                                app.pending_prompt = PendingPrompt::ApprovalDialog {
                                                    tool_name: next.name,
                                                    tool_args: args_display,
                                                    tool_id: next.id,
                                                };
                                            } else {
                                                // All resolved — continue the agent turn
                                                let api_key = resolve_api_key(provider);
                                                match axga_core::continue_turn_streaming(
                                                    provider.as_str(), api_key.as_deref(), base_url, model.as_str(),
                                                    conversation, registry, system_prompt, max_turns,
                                                    &mut noop_handler, Some(permissions.clone()),
                                                    std::mem::take(&mut resolved_calls),
                                                ).await {
                                                    Ok(turn) => {
                                                        if !turn.final_text.is_empty() {
                                                            app.chat_lines.push(ChatLine::Assistant(turn.final_text));
                                                        }
                                                        for tc in &turn.tool_calls_made {
                                                            app.chat_lines.push(ChatLine::Tool { name: tc.clone(), detail: "executed".into() });
                                                        }
                                                        app.status.tokens_used = app.status.tokens_used.saturating_add(turn.total_tokens);
                                                        app.update_context(conversation.len());
                                                        // Handle any new pending approvals
                                                        if !turn.pending_approvals.is_empty() {
                                                            resolved_calls = turn.pending_approvals.clone();
                                                            pending_approvals_stack = turn.pending_approvals.into_iter().rev().collect();
                                                            if let Some(next) = pending_approvals_stack.pop() {
                                                                let args_display = serde_json::to_string_pretty(&next.arguments)
                                                                    .unwrap_or_else(|_| String::new());
                                                                app.chat_lines.push(ChatLine::Info(format!(
                                                                    "Pending approval: {} <- {}",
                                                                    next.name, &args_display[..std::cmp::min(80, args_display.len())]
                                                                )));
                                                                app.pending_prompt = PendingPrompt::ApprovalDialog {
                                                                    tool_name: next.name,
                                                                    tool_args: args_display,
                                                                    tool_id: next.id,
                                                                };
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        app.chat_lines.push(ChatLine::Error(format!("{e}")));
                                                    }
                                                }
                                                app.chat_lines.push(ChatLine::Spacer);
                                            }
                                            continue;
                                        }
                                        PendingPrompt::AskUser { questions_json, tool_call_id } => {
                                            // User answered the question — push answer as tool result
                                            let answer_json = serde_json::json!({
                                                "answer": input.trim(),
                                                "questions": serde_json::from_str::<serde_json::Value>(&questions_json).unwrap_or_default()
                                            });
                                            app.chat_lines.push(ChatLine::Info(format!("Answer recorded: {}", input.trim())));
                                            conversation.push(AgentMessage::Tool {
                                                tool_call_id,
                                                content: answer_json.to_string(),
                                            });

                                            // Continue the agent turn
                                            let api_key = resolve_api_key(provider);
                                            match continue_turn_streaming(
                                                provider.as_str(), api_key.as_deref(), base_url, model.as_str(),
                                                conversation, registry, system_prompt, max_turns,
                                                &mut NoopStreamHandler, Some(permissions.clone()),
                                                Vec::new(),
                                            ).await {
                                                Ok(turn) => {
                                                    if !turn.final_text.is_empty() {
                                                        app.chat_lines.push(ChatLine::Assistant(turn.final_text));
                                                    }
                                                    for tc in &turn.tool_calls_made {
                                                        app.chat_lines.push(ChatLine::Tool { name: tc.clone(), detail: "executed".into() });
                                                    }
                                                    app.status.tokens_used = app.status.tokens_used.saturating_add(turn.total_tokens);
                                                    app.update_context(conversation.len());
                                                }
                                                Err(e) => {
                                                    app.chat_lines.push(ChatLine::Error(format!("{e}")));
                                                }
                                            }
                                            app.chat_lines.push(ChatLine::Spacer);
                                            continue;
                                        }
                                        PendingPrompt::None => {}
                                    }

                                    // Check for slash commands
                                    if input.starts_with('/') {
                                        let full = input.strip_prefix('/').unwrap_or(&input).trim();
                                        let (cmd, args) = full.split_once(' ').unwrap_or((full, ""));

                                        match cmd {
                                            "quit" | "exit" | "q" => { app.exit = true; break; }
                                            "clear" | "new" => {
                                                conversation.reset();
                                                app.chat_lines.clear();
                                                app.chat_lines.push(ChatLine::Info("Conversation cleared. New session.".into()));
                                            }
                                            "tools" => {
                                                app.chat_lines.push(ChatLine::Info(format!("{} tools available:", registry.len())));
                                                for name in registry.names() {
                                                    if let Some(tool) = registry.get(name) {
                                                        app.chat_lines.push(ChatLine::Info(format!("  {} — {}", tool.name(), tool.description())));
                                                    }
                                                }
                                            }
                                            "help" | "?" | "h" => {
                                                app.chat_lines.push(ChatLine::Info("╭─ Slash Commands ──────────────────────────╮".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /quit, /q       Exit axga                │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /clear, /new    New session              │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /tools          List tools               │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /help, /?       Show this help           │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /history        Session stats            │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /status         Runtime status           │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /usage          Token usage              │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /undo [N]       Undo last N turns (default 1) │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /compact        LLM context compaction        │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /version        Show version             │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /export <file>  Export to markdown       │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /title <text>   Set session title        │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /provider [m]   Show/switch provider/model│".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /apikey <key>   Save API key to config     │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /yolo           Auto-approve all tools    │".into()));
                                                app.chat_lines.push(ChatLine::Info("│ /manual         Switch to manual approval │".into()));
                                                app.chat_lines.push(ChatLine::Info("╰──────────────────────────────────────────╯".into()));
                                                app.chat_lines.push(ChatLine::Info("Keys: ↑↓ scroll | Esc normal | i insert | Ctrl+C quit".into()));
                                            }
                                            "history" => {
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Session: {} messages, {} turns", conversation.len(), conversation.turn_count()
                                                )));
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Tokens used: {}", app.status.tokens_used
                                                )));
                                            }
                                            "status" => {
                                                app.chat_lines.push(ChatLine::Info(format!("Provider:  {provider}")));
                                                app.chat_lines.push(ChatLine::Info(format!("Model:     {model}")));
                                                app.chat_lines.push(ChatLine::Info(format!("Max turns: {max_turns}")));
                                                app.chat_lines.push(ChatLine::Info(format!("Messages:  {}", conversation.len())));
                                                app.chat_lines.push(ChatLine::Info(format!("Turns:     {}", conversation.turn_count())));
                                                app.chat_lines.push(ChatLine::Info(format!("Tokens:    {}", app.status.tokens_used)));
                                                app.chat_lines.push(ChatLine::Info(format!("Tools:     {} loaded", registry.len())));
                                                app.chat_lines.push(ChatLine::Info(format!("Version:   axga v{}", env!("CARGO_PKG_VERSION"))));
                                            }
                                            "usage" => {
                                                let tokens = app.status.tokens_used;
                                                let max_ctx = 32_768u32;
                                                let pct = if max_ctx > 0 { (tokens as f64 / max_ctx as f64 * 100.0) as u32 } else { 0 };
                                                app.chat_lines.push(ChatLine::Info(format!("Tokens: {tokens}/{max_ctx} ({pct}%)")));
                                                app.chat_lines.push(ChatLine::Info(format!("Messages: {}", conversation.len())));
                                                app.chat_lines.push(ChatLine::Info(format!("Turns: {}", conversation.turn_count())));
                                            }
                                            "undo" => {
                                                let n: usize = args.parse().unwrap_or(1);
                                                let removed = conversation.undo(n);
                                                // Also trim chat_lines: remove last N user lines + everything after
                                                let mut user_count = 0;
                                                let mut cut_idx = app.chat_lines.len();
                                                for i in (0..app.chat_lines.len()).rev() {
                                                    if matches!(app.chat_lines[i], ChatLine::User(_)) {
                                                        user_count += 1;
                                                        if user_count >= n {
                                                            cut_idx = i;
                                                            break;
                                                        }
                                                    }
                                                }
                                                app.chat_lines.truncate(cut_idx);
                                                app.chat_lines.push(ChatLine::Info(format!("Undo: removed {removed} messages")));
                                            }
                                            "compact" => {
                                                let n = (conversation.len() / 2).min(10);
                                                if n == 0 {
                                                    app.chat_lines.push(ChatLine::Info("Nothing to compact".into()));
                                                } else {
                                                    // Collect the oldest n messages for summarization
                                                    let old_messages: Vec<AgentMessage> =
                                                        conversation.messages().take(n).cloned().collect();
                                                    let mut prompt = String::from(
                                                        "Please summarize this conversation concisely, preserving key decisions, context, and action items:\n\n",
                                                    );
                                                    for msg in &old_messages {
                                                        match msg {
                                                            AgentMessage::User { content } => {
                                                                prompt.push_str(&format!("User: {content}\n"));
                                                            }
                                                            AgentMessage::Assistant { content } => {
                                                                if let Some(ref t) = content.text {
                                                                    prompt.push_str(&format!("Assistant: {t}\n"));
                                                                }
                                                                if let Some(ref calls) = content.tool_calls {
                                                                    for tc in calls {
                                                                        prompt.push_str(&format!("[Tool: {}]\n", tc.name));
                                                                    }
                                                                }
                                                            }
                                                            AgentMessage::System { content } => {
                                                                prompt.push_str(&format!("System: {content}\n"));
                                                            }
                                                            AgentMessage::Tool { tool_call_id, content } => {
                                                                let snippet = if content.len() > 200 {
                                                                    &content[..200]
                                                                } else {
                                                                    content
                                                                };
                                                                prompt.push_str(&format!("Tool[{tool_call_id}]: {snippet}\n"));
                                                            }
                                                        }
                                                    }

                                                    let compact_messages = vec![AgentMessage::User {
                                                        content: prompt,
                                                    }];
                                                    let api_key = resolve_api_key(provider);

                                                    // Block on the async LLM call from within the TUI event loop
                                                    let result = tokio::task::block_in_place(|| {
                                                        tokio::runtime::Handle::current().block_on(
                                                            simple_chat(
                                                                provider, api_key.as_deref(), base_url, model,
                                                                &compact_messages, None,
                                                            ),
                                                        )
                                                    });

                                                    match result {
                                                        Ok(summary) => {
                                                            let before = conversation.len();
                                                            let replaced = conversation.compact(summary);
                                                            app.chat_lines.push(ChatLine::Info(format!(
                                                                "Compacted: {before} → {} messages (replaced {replaced})",
                                                                conversation.len()
                                                            )));
                                                        }
                                                        Err(e) => {
                                                            app.chat_lines.push(ChatLine::Error(format!(
                                                                "Compaction failed: {e}"
                                                            )));
                                                        }
                                                    }
                                                }
                                            }
                                            "version" => {
                                                app.chat_lines.push(ChatLine::Info(format!("axga v{} (rustc {})", env!("CARGO_PKG_VERSION"), option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown"))));
                                            }
                                            "export" => {
                                                let path = if args.is_empty() { "axga-export.md" } else { args };
                                                let mut md = String::new();
                                                md.push_str("# axga Session Export\n\n");
                                                md.push_str(&format!("Provider: {} | Model: {} | Tokens: {}\n\n", provider, model, app.status.tokens_used));
                                                for line in &app.chat_lines {
                                                    match line {
                                                        ChatLine::User(t) => md.push_str(&format!("**You:** {t}\n\n")),
                                                        ChatLine::Assistant(t) => md.push_str(&format!("{t}\n\n")),
                                                        ChatLine::Tool { name, detail } => md.push_str(&format!("*[tool: {name} → {detail}]*\n\n")),
                                                        ChatLine::Error(t) => md.push_str(&format!("*Error: {t}*\n\n")),
                                                        _ => {}
                                                    }
                                                }
                                                match std::fs::write(path, &md) {
                                                    Ok(_) => app.chat_lines.push(ChatLine::Info(format!("Exported to {path}"))),
                                                    Err(e) => app.chat_lines.push(ChatLine::Error(format!("Export failed: {e}"))),
                                                }
                                            }
                                            "title" => {
                                                if args.is_empty() {
                                                    app.chat_lines.push(ChatLine::Info("Usage: /title <your session title>".into()));
                                                } else {
                                                    app.chat_lines.push(ChatLine::Info(format!("Session title set to: {args}")));
                                                }
                                            }
                                            "provider" => {
                                                let parts: Vec<&str> = args.split_whitespace().collect();
                                                if parts.is_empty() {
                                                    app.chat_lines.push(ChatLine::Info(format!("Current: provider={provider}, model={model}")));
                                                    app.chat_lines.push(ChatLine::Info("Usage: /provider <name> [model]".into()));
                                                    app.chat_lines.push(ChatLine::Info("  /provider deepseek".into()));
                                                    app.chat_lines.push(ChatLine::Info("  /provider deepseek deepseek-v4-flash".into()));
                                                    app.chat_lines.push(ChatLine::Info("  /provider openai gpt-4o-mini".into()));
                                                    app.chat_lines.push(ChatLine::Info("".into()));
                                                    app.chat_lines.push(ChatLine::Info("Set API key:".into()));
                                                    app.chat_lines.push(ChatLine::Info("  /apikey sk-...".into()));
                                                } else {
                                                    let new_provider = parts[0].to_string();
                                                    let new_model = parts.get(1).map(|s| s.to_string());
                                                    *provider = new_provider.clone();
                                                    if resolve_api_key(&new_provider).is_none() {
                                                        app.chat_lines.push(ChatLine::Info(format!("No API key found for {new_provider}.")));
                                                        app.chat_lines.push(ChatLine::Info("Paste your API key and press Enter:".into()));
                                                        app.pending_prompt = PendingPrompt::ApiKey { provider: new_provider, model: new_model };
                                                    } else {
                                                        let m = new_model.unwrap_or_else(|| match provider.as_str() {
                                                            "deepseek" => "deepseek-v4-flash".into(),
                                                            "openai" => "gpt-4o-mini".into(),
                                                            _ => "gpt-4o-mini".into(),
                                                        });
                                                        *model = m;
                                                        app.status.model = model.clone();
                                                        conversation.reset();
                                                        app.chat_lines.push(ChatLine::Info(format!("Switched to {provider} / {model}")));
                                                        app.chat_lines.push(ChatLine::Info("Conversation reset.".into()));
                                                    }
                                                }
                                            }
                                            "apikey" => {
                                                if args.is_empty() {
                                                    app.chat_lines.push(ChatLine::Info("Usage: /apikey <your-api-key>".into()));
                                                    app.chat_lines.push(ChatLine::Info("  /apikey sk-...".into()));
                                                    app.chat_lines.push(ChatLine::Info("The key is saved to ~/.config/axga/config.toml".into()));
                                                } else if let Some(mut config) = load_config() {
                                                    config.provider.api_key = Some(args.to_string());
                                                    match save_config(&config) {
                                                        Ok(_) => {
                                                            app.chat_lines.push(ChatLine::Info("API key saved to ~/.config/axga/config.toml".into()));
                                                            app.chat_lines.push(ChatLine::Info(format!("Provider: {provider}, key: {}...", &args[..std::cmp::min(10, args.len())])));
                                                        }
                                                        Err(e) => app.chat_lines.push(ChatLine::Error(format!("Failed to save: {e}"))),
                                                    }
                                                } else {
                                                    let config = axga_core::Config {
                                                        provider: axga_core::config::ProviderSection {
                                                            provider_type: Some(provider.clone()),
                                                            model: Some(model.clone()),
                                                            api_key: Some(args.to_string()),
                                                            base_url: None,
                                                            system_prompt: None,
                                                            max_turns: Some(max_turns),
                                                        },
                                                        ..Default::default()
                                                    };
                                                    match save_config(&config) {
                                                        Ok(_) => app.chat_lines.push(ChatLine::Info("API key saved to ~/.config/axga/config.toml".into())),
                                                        Err(e) => app.chat_lines.push(ChatLine::Error(format!("Failed to save: {e}"))),
                                                    }
                                                }
                                            }
                                            "yolo" => {
                                                permissions.set_mode(PermissionMode::Auto);
                                                app.status.permission_mode = "AUTO".into();
                                                app.chat_lines.push(ChatLine::Info("YOLO mode: auto-approving all tools".into()));
                                            }
                                            "manual" => {
                                                permissions.set_mode(PermissionMode::Manual);
                                                app.status.permission_mode = "MANUAL".into();
                                                app.chat_lines.push(ChatLine::Info("Manual mode: asking before write/shell tools".into()));
                                            }
                                            _ => {
                                                app.chat_lines.push(ChatLine::Error(format!("Unknown: /{cmd}. Try /help")));
                                            }
                                        }
                                        app.scroll_to_bottom();
                                        continue;
                                    }

                                    // Push user message
                                    app.chat_lines.push(ChatLine::User(input.clone()));

                                    // Run agent with real-time streaming
                                    let mut handler = TuiStreamHandler {
                                        app: &mut *app,
                                        terminal: &mut *terminal,
                                        streaming_line_idx: None,
                                        accumulated_text: String::new(),
                                        tools_seen: Vec::new(),
                                        ask_user_pending: None,
                                    };
                                    let result = run_turn_streaming(provider.as_str(), resolve_api_key(provider).as_deref(), base_url, model.as_str(),
                                        conversation, &input, registry, system_prompt, max_turns, &mut handler, Some(permissions.clone())).await;

                                    let handler_accumulated = std::mem::take(&mut handler.accumulated_text);
                                    let handler_tools_seen = std::mem::take(&mut handler.tools_seen);
                                    let handler_ask_user = std::mem::take(&mut handler.ask_user_pending);
                                    let handler_streaming_idx = handler.streaming_line_idx;
                                    // handler goes out of scope here, releasing app/terminal borrows

                                    match result {
                                        Ok(turn) => {
                                            // Tool results were already pushed by handler during streaming.
                                            // Push any remaining tool calls that weren't shown.
                                            for tc in &turn.tool_calls_made {
                                                if !handler_tools_seen.contains(tc) {
                                                    app.chat_lines.push(ChatLine::Tool {
                                                        name: tc.clone(),
                                                        detail: "executed".into(),
                                                    });
                                                }
                                            }
                                            // Text was streamed — only push fallback if somehow missing
                                            if !turn.final_text.is_empty() && handler_accumulated.is_empty() {
                                                app.chat_lines.push(ChatLine::Assistant(turn.final_text));
                                            }
                                            app.status.tokens_used = app.status.tokens_used.saturating_add(turn.total_tokens);
                                            app.update_context(conversation.len());

                                            // Handle pending approvals
                                            if !turn.pending_approvals.is_empty() {
                                                app.is_streaming = false;
                                                resolved_calls = turn.pending_approvals.clone();
                                                pending_approvals_stack = turn.pending_approvals.into_iter().rev().collect();
                                                if let Some(first) = pending_approvals_stack.pop() {
                                                    let args_display = serde_json::to_string_pretty(&first.arguments)
                                                        .unwrap_or_else(|_| String::new());
                                                    app.chat_lines.push(ChatLine::Info(format!(
                                                        "Tool needs approval: {} <- {}",
                                                        first.name,
                                                        &args_display[..std::cmp::min(80, args_display.len())]
                                                    )));
                                                    app.pending_prompt = PendingPrompt::ApprovalDialog {
                                                        tool_name: first.name,
                                                        tool_args: args_display,
                                                        tool_id: first.id,
                                                    };
                                                }
                                            }

                                            // Handle ask_user_question pending
                                            if let Some((_, questions_json)) = handler_ask_user {
                                                // Find the tool_call_id from the conversation
                                                let tool_call_id = conversation.messages()
                                                    .filter_map(|m| {
                                                        if let AgentMessage::Tool { tool_call_id, content } = m {
                                                            if content.starts_with("__AXGA_ASK_USER__") {
                                                                Some(tool_call_id.clone())
                                                            } else { None }
                                                        } else { None }
                                                    })
                                                    .last()
                                                    .unwrap_or_default();

                                                app.is_streaming = false;
                                                app.chat_lines.push(ChatLine::Info("Answer the following questions:".into()));
                                                app.pending_prompt = PendingPrompt::AskUser {
                                                    questions_json,
                                                    tool_call_id,
                                                };
                                            }
                                        }
                                        Err(e) => {
                                            app.is_streaming = false;
                                            // Remove the streaming placeholder if still there
                                            if let Some(idx) = handler_streaming_idx {
                                                if idx < app.chat_lines.len() {
                                                    app.chat_lines.remove(idx);
                                                }
                                            }
                                            app.chat_lines.push(ChatLine::Error(format!("{e}")));
                                        }
                                    }

                                    app.chat_lines.push(ChatLine::Spacer);
                                    app.scroll_to_bottom();
                                }
                                KeyCode::Backspace => {
                                    if app.cursor_pos > 0 {
                                        app.input.remove(app.cursor_pos - 1);
                                        app.cursor_pos -= 1;
                                    }
                                }
                                KeyCode::Delete => {
                                    if app.cursor_pos < app.input.len() {
                                        app.input.remove(app.cursor_pos);
                                    }
                                }
                                KeyCode::Left => {
                                    if app.cursor_pos > 0 { app.cursor_pos -= 1; }
                                }
                                KeyCode::Right => {
                                    if app.cursor_pos < app.input.len() { app.cursor_pos += 1; }
                                }
                                KeyCode::Home => app.cursor_pos = 0,
                                KeyCode::End => app.cursor_pos = app.input.len(),
                                KeyCode::Char(c) => {
                                    app.input.insert(app.cursor_pos, c);
                                    app.cursor_pos += 1;
                                }
                                _ => {}
                            }
                        }
                        InputMode::Normal => {
                            match key.code {
                                KeyCode::Char('i') => app.mode = InputMode::Insert,
                                KeyCode::Char('a') => {
                                    app.mode = InputMode::Insert;
                                    app.cursor_pos = app.cursor_pos.saturating_add(1).min(app.input.len());
                                }
                                KeyCode::Char(':') => {
                                    app.mode = InputMode::Command;
                                    app.input.clear();
                                    app.input.push(':');
                                    app.cursor_pos = 1;
                                }
                                KeyCode::Char('q') => app.exit = true,
                                KeyCode::Char('G') => app.scroll_to_bottom(),
                                KeyCode::Char('g') => {
                                    if app.pending_gg {
                                        app.scroll_to_top();
                                        app.pending_gg = false;
                                    } else {
                                        app.pending_gg = true;
                                    }
                                }
                                KeyCode::Up | KeyCode::Char('k') => app.scroll_by(-1),
                                KeyCode::Down | KeyCode::Char('j') => app.scroll_by(1),
                                KeyCode::Enter => {
                                    // Submit from normal mode
                                    let input = std::mem::take(&mut app.input);
                                    if !input.trim().is_empty() {

                                        // Handle pending prompts from normal mode too
                                        if let PendingPrompt::ApprovalDialog { tool_name, tool_args, tool_id } = std::mem::replace(&mut app.pending_prompt, PendingPrompt::None) {
                                            match input.trim().to_lowercase().as_str() {
                                                "y" | "yes" => {
                                                    permissions.approve(&tool_name);
                                                    app.chat_lines.push(ChatLine::Info(format!("Approved: {tool_name}")));
                                                }
                                                "n" | "no" => {
                                                    permissions.deny(&tool_name);
                                                    app.chat_lines.push(ChatLine::Info(format!("Denied: {tool_name}")));
                                                }
                                                "a" | "all" => {
                                                    permissions.approve_all();
                                                    pending_approvals_stack.clear();
                                                    app.chat_lines.push(ChatLine::Info("Approved all — switched to Auto mode".into()));
                                                }
                                                _ => {
                                                    app.chat_lines.push(ChatLine::Error("Type 'y' (approve), 'n' (deny), or 'a' (approve all)".into()));
                                                    app.pending_prompt = PendingPrompt::ApprovalDialog { tool_name, tool_args, tool_id };
                                                    continue;
                                                }
                                            }
                                            if let Some(next) = pending_approvals_stack.pop() {
                                                let args_display = serde_json::to_string_pretty(&next.arguments)
                                                    .unwrap_or_else(|_| String::new());
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Pending approval: {} <- {}",
                                                    next.name, &args_display[..std::cmp::min(80, args_display.len())]
                                                )));
                                                app.pending_prompt = PendingPrompt::ApprovalDialog {
                                                    tool_name: next.name,
                                                    tool_args: args_display,
                                                    tool_id: next.id,
                                                };
                                            } else {
                                                let api_key = resolve_api_key(provider);
                                                match continue_turn_streaming(
                                                    provider.as_str(), api_key.as_deref(), base_url, model.as_str(),
                                                    conversation, registry, system_prompt, max_turns,
                                                    &mut noop_handler, Some(permissions.clone()),
                                                    std::mem::take(&mut resolved_calls),
                                                ).await {
                                                    Ok(turn) => {
                                                        if !turn.final_text.is_empty() {
                                                            app.chat_lines.push(ChatLine::Assistant(turn.final_text));
                                                        }
                                                        for tc in &turn.tool_calls_made {
                                                            app.chat_lines.push(ChatLine::Tool { name: tc.clone(), detail: "executed".into() });
                                                        }
                                                        app.status.tokens_used = app.status.tokens_used.saturating_add(turn.total_tokens);
                                                        app.update_context(conversation.len());
                                                        if !turn.pending_approvals.is_empty() {
                                                            resolved_calls = turn.pending_approvals.clone();
                                                            pending_approvals_stack = turn.pending_approvals.into_iter().rev().collect();
                                                            if let Some(next) = pending_approvals_stack.pop() {
                                                                let args_display = serde_json::to_string_pretty(&next.arguments)
                                                                    .unwrap_or_else(|_| String::new());
                                                                app.chat_lines.push(ChatLine::Info(format!(
                                                                    "Pending approval: {} <- {}",
                                                                    next.name, &args_display[..std::cmp::min(80, args_display.len())]
                                                                )));
                                                                app.pending_prompt = PendingPrompt::ApprovalDialog {
                                                                    tool_name: next.name,
                                                                    tool_args: args_display,
                                                                    tool_id: next.id,
                                                                };
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        app.chat_lines.push(ChatLine::Error(format!("{e}")));
                                                    }
                                                }
                                                app.chat_lines.push(ChatLine::Spacer);
                                            }
                                            continue;
                                        }

                                        // Handle AskUserQuestion prompt
                                        if let PendingPrompt::AskUser { questions_json, tool_call_id } = std::mem::replace(&mut app.pending_prompt, PendingPrompt::None) {
                                            let answer_json = serde_json::json!({
                                                "answer": input.trim(),
                                                "questions": serde_json::from_str::<serde_json::Value>(&questions_json).unwrap_or_default()
                                            });
                                            app.chat_lines.push(ChatLine::Info(format!("Answer recorded: {}", input.trim())));
                                            conversation.push(AgentMessage::Tool {
                                                tool_call_id,
                                                content: answer_json.to_string(),
                                            });

                                            let api_key = resolve_api_key(provider);
                                            match continue_turn_streaming(
                                                provider.as_str(), api_key.as_deref(), base_url, model.as_str(),
                                                conversation, registry, system_prompt, max_turns,
                                                &mut noop_handler, Some(permissions.clone()),
                                                Vec::new(),
                                            ).await {
                                                Ok(turn) => {
                                                    if !turn.final_text.is_empty() {
                                                        app.chat_lines.push(ChatLine::Assistant(turn.final_text));
                                                    }
                                                    for tc in &turn.tool_calls_made {
                                                        app.chat_lines.push(ChatLine::Tool { name: tc.clone(), detail: "executed".into() });
                                                    }
                                                    app.status.tokens_used = app.status.tokens_used.saturating_add(turn.total_tokens);
                                                    app.update_context(conversation.len());
                                                }
                                                Err(e) => {
                                                    app.chat_lines.push(ChatLine::Error(format!("{e}")));
                                                }
                                            }
                                            app.chat_lines.push(ChatLine::Spacer);
                                            continue;
                                        }

                                        app.chat_lines.push(ChatLine::User(input.clone()));

                                        let mut handler = TuiStreamHandler {
                                            app: &mut *app,
                                            terminal: &mut *terminal,
                                            streaming_line_idx: None,
                                            accumulated_text: String::new(),
                                            tools_seen: Vec::new(),
                                            ask_user_pending: None,
                                        };

                                        let result = run_turn_streaming(provider.as_str(), resolve_api_key(provider).as_deref(), base_url, model.as_str(),
                                            conversation, &input, registry, system_prompt, max_turns, &mut handler, Some(permissions.clone())).await;

                                        let handler_accumulated = std::mem::take(&mut handler.accumulated_text);
                                        let handler_tools_seen = std::mem::take(&mut handler.tools_seen);
                                        let handler_ask_user = std::mem::take(&mut handler.ask_user_pending);
                                        let handler_streaming_idx = handler.streaming_line_idx;
                                        // handler goes out of scope here, releasing app/terminal borrows

                                        match result {
                                            Ok(turn) => {
                                                for tc in &turn.tool_calls_made {
                                                    if !handler_tools_seen.contains(tc) {
                                                        app.chat_lines.push(ChatLine::Tool { name: tc.clone(), detail: "executed".into() });
                                                    }
                                                }
                                                if !turn.final_text.is_empty() && handler_accumulated.is_empty() {
                                                    app.chat_lines.push(ChatLine::Assistant(turn.final_text));
                                                }
                                                app.status.tokens_used = app.status.tokens_used.saturating_add(turn.total_tokens);
                                                app.update_context(conversation.len());

                                                if !turn.pending_approvals.is_empty() {
                                                    app.is_streaming = false;
                                                    resolved_calls = turn.pending_approvals.clone();
                                                    pending_approvals_stack = turn.pending_approvals.into_iter().rev().collect();
                                                    if let Some(first) = pending_approvals_stack.pop() {
                                                        let args_display = serde_json::to_string_pretty(&first.arguments)
                                                            .unwrap_or_else(|_| String::new());
                                                        app.chat_lines.push(ChatLine::Info(format!(
                                                            "Tool needs approval: {} <- {}",
                                                            first.name, &args_display[..std::cmp::min(80, args_display.len())]
                                                        )));
                                                        app.pending_prompt = PendingPrompt::ApprovalDialog {
                                                            tool_name: first.name,
                                                            tool_args: args_display,
                                                            tool_id: first.id,
                                                        };
                                                    }
                                                }

                                                // Handle ask_user_question pending
                                                if let Some((_, questions_json)) = handler_ask_user {
                                                    let tool_call_id = conversation.messages()
                                                        .filter_map(|m| {
                                                            if let AgentMessage::Tool { tool_call_id, content } = m {
                                                                if content.starts_with("__AXGA_ASK_USER__") {
                                                                    Some(tool_call_id.clone())
                                                                } else { None }
                                                            } else { None }
                                                        })
                                                        .last()
                                                        .unwrap_or_default();

                                                    app.is_streaming = false;
                                                    app.chat_lines.push(ChatLine::Info("Answer the following questions:".into()));
                                                    app.pending_prompt = PendingPrompt::AskUser {
                                                        questions_json,
                                                        tool_call_id,
                                                    };
                                                }
                                            }
                                            Err(e) => {
                                                app.is_streaming = false;
                                                if let Some(idx) = handler_streaming_idx {
                                                    if idx < app.chat_lines.len() {
                                                        app.chat_lines.remove(idx);
                                                    }
                                                }
                                                app.chat_lines.push(ChatLine::Error(format!("{e}")));
                                            }
                                        }
                                        app.chat_lines.push(ChatLine::Spacer);
                                        app.scroll_to_bottom();
                                    }
                                }
                                _ => {
                                    app.pending_gg = false;
                                }
                            }
                        }
                        InputMode::Command => {
                            match key.code {
                                KeyCode::Esc => {
                                    app.mode = InputMode::Normal;
                                    app.input.clear();
                                    app.cursor_pos = 0;
                                }
                                KeyCode::Enter => {
                                    let cmd = std::mem::take(&mut app.input);
                                    let cmd_clean = cmd.strip_prefix(':').unwrap_or(&cmd).trim();
                                    match cmd_clean {
                                        "q" | "quit" => app.exit = true,
                                        "clear" => {
                                            conversation.reset();
                                            app.chat_lines.clear();
                                            app.chat_lines.push(ChatLine::Info("Conversation cleared.".into()));
                                        }
                                        "tools" => {
                                            app.chat_lines.push(ChatLine::Info("Available tools:".into()));
                                            for name in registry.names() {
                                                if let Some(tool) = registry.get(name) {
                                                    app.chat_lines.push(ChatLine::Info(format!(
                                                        "  {} — {}", tool.name(), tool.description()
                                                    )));
                                                }
                                            }
                                        }
                                        "history" => {
                                            app.chat_lines.push(ChatLine::Info(format!(
                                                "{} messages, {} turns", conversation.len(), conversation.turn_count()
                                            )));
                                        }
                                        _ => {
                                            app.chat_lines.push(ChatLine::Error(format!("Unknown: {cmd_clean}")));
                                        }
                                    }
                                    app.mode = InputMode::Normal;
                                }
                                KeyCode::Backspace => {
                                    if app.cursor_pos > 0 {
                                        app.input.remove(app.cursor_pos - 1);
                                        app.cursor_pos -= 1;
                                    }
                                }
                                KeyCode::Char(c) => {
                                    app.input.insert(app.cursor_pos, c);
                                    app.cursor_pos += 1;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    Ok(())
}
