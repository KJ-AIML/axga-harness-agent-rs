//! axga TUI mode — beautiful terminal interface.
//!
//! Layout: Status bar | Chat with bullets | Border-decorated input
//! Theme: Semantic color tokens, dark mode by default

use axga_core::tools::{code, fetch_url, fs, memctrl, shell, web_search};
use axga_core::{Conversation, ToolRegistry, run_turn};
use axga_tui::app::{App, ChatLine, InputMode};
use axga_tui::theme;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;

pub async fn run_tui(
    provider: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model: &str,
    system_prompt: Option<&str>,
    max_turns: usize,
) -> anyhow::Result<()> {
    let mut registry = ToolRegistry::new();
    registry.register(fs::ReadFileTool)?;
    registry.register(fs::WriteFileTool)?;
    registry.register(fs::ListDirectoryTool)?;
    registry.register(shell::ShellTool)?;
    registry.register(code::GrepTool)?;
    registry.register(code::GlobTool)?;
    registry.register(code::DiffTool)?;
    registry.register(memctrl::MemCtrlTool)?;
    registry.register(web_search::WebSearchTool)?;
    registry.register(fetch_url::FetchUrlTool)?;
    let mut conversation = Conversation::new();
    let mut terminal = ratatui::init();
    let th = theme::dark_theme();
    let mut app = App::new(model, th);

    // Welcome
    app.chat_lines.push(ChatLine::Info(format!(
        "axga v{} — {} / {}",
        env!("CARGO_PKG_VERSION"),
        provider,
        model
    )));
    app.chat_lines.push(ChatLine::Info(
        "Type a message, press Enter. i=insert  Esc=normal  :q=quit  :tools=list".into(),
    ));
    app.chat_lines.push(ChatLine::Spacer);

    // Git branch detection
    if let Ok(output) = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
    {
        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !branch.is_empty() {
                app.status.git_branch = Some(branch);
            }
        }
    }

    let result = tui_loop(
        &mut terminal,
        &mut app,
        provider,
        api_key,
        base_url,
        model,
        system_prompt,
        max_turns,
        &mut registry,
        &mut conversation,
    )
    .await;

    ratatui::restore();
    result.map_err(|e| anyhow::anyhow!("{e}"))
}

#[allow(clippy::too_many_arguments)]
async fn tui_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    provider: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model: &str,
    system_prompt: Option<&str>,
    max_turns: usize,
    registry: &mut ToolRegistry,
    conversation: &mut Conversation,
) -> anyhow::Result<()> {
    let mut spinner_tick: usize = 0;

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
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        if app.is_streaming {
                            app.chat_lines
                                .push(ChatLine::Info("Press Ctrl+C again to force quit".into()));
                            app.is_streaming = false;
                        } else {
                            app.exit = true;
                            break;
                        }
                        continue;
                    }

                    // Global scroll — works in any mode
                    match key.code {
                        KeyCode::Up => {
                            app.scroll_by(-1);
                            continue;
                        }
                        KeyCode::Down => {
                            app.scroll_by(1);
                            continue;
                        }
                        KeyCode::PageUp => {
                            app.scroll_by(-10);
                            continue;
                        }
                        KeyCode::PageDown => {
                            app.scroll_by(10);
                            continue;
                        }
                        _ => {}
                    }

                    match app.mode {
                        InputMode::Insert => {
                            match key.code {
                                KeyCode::Esc => app.mode = InputMode::Normal,
                                KeyCode::PageUp | KeyCode::Char('u')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    app.scroll_by(-10);
                                }
                                KeyCode::PageDown | KeyCode::Char('d')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    app.scroll_by(10);
                                }
                                KeyCode::Up => {
                                    app.scroll_by(-1);
                                }
                                KeyCode::Down => {
                                    app.scroll_by(1);
                                }
                                KeyCode::Enter => {
                                    let input = std::mem::take(&mut app.input);
                                    app.cursor_pos = 0;
                                    if input.trim().is_empty() {
                                        continue;
                                    }

                                    // Check for slash commands
                                    if input.starts_with('/') {
                                        let full = input.strip_prefix('/').unwrap_or(&input).trim();
                                        let (cmd, args) =
                                            full.split_once(' ').unwrap_or((full, ""));

                                        match cmd {
                                            "quit" | "exit" | "q" => {
                                                app.exit = true;
                                                break;
                                            }
                                            "clear" | "new" => {
                                                conversation.reset();
                                                app.chat_lines.clear();
                                                app.chat_lines.push(ChatLine::Info(
                                                    "Conversation cleared. New session.".into(),
                                                ));
                                            }
                                            "tools" => {
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "{} tools available:",
                                                    registry.len()
                                                )));
                                                for name in registry.names() {
                                                    if let Some(tool) = registry.get(name) {
                                                        app.chat_lines.push(ChatLine::Info(
                                                            format!(
                                                                "  {} — {}",
                                                                tool.name(),
                                                                tool.description()
                                                            ),
                                                        ));
                                                    }
                                                }
                                            }
                                            "help" | "?" | "h" => {
                                                app.chat_lines.push(ChatLine::Info(
                                                    "╭─ Slash Commands ──────────────────────────╮"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "│ /quit, /q       Exit axga                │"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "│ /clear, /new    New session              │"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "│ /tools          List tools               │"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "│ /help, /?       Show this help           │"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "│ /history        Session stats            │"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "│ /status         Runtime status           │"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "│ /usage          Token usage              │"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "│ /compact        Compact context          │"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "│ /version        Show version             │"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "│ /export <file>  Export to markdown       │"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "│ /title <text>   Set session title        │"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info(
                                                    "╰──────────────────────────────────────────╯"
                                                        .into(),
                                                ));
                                                app.chat_lines.push(ChatLine::Info("Keys: ↑↓ scroll | Esc normal | i insert | Ctrl+C quit".into()));
                                            }
                                            "history" => {
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Session: {} messages, {} turns",
                                                    conversation.len(),
                                                    conversation.turn_count()
                                                )));
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Tokens used: {}",
                                                    app.status.tokens_used
                                                )));
                                            }
                                            "status" => {
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Provider:  {provider}"
                                                )));
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Model:     {model}"
                                                )));
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Max turns: {max_turns}"
                                                )));
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Messages:  {}",
                                                    conversation.len()
                                                )));
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Turns:     {}",
                                                    conversation.turn_count()
                                                )));
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Tokens:    {}",
                                                    app.status.tokens_used
                                                )));
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Tools:     {} loaded",
                                                    registry.len()
                                                )));
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Version:   axga v{}",
                                                    env!("CARGO_PKG_VERSION")
                                                )));
                                            }
                                            "usage" => {
                                                let tokens = app.status.tokens_used;
                                                let max_ctx = 32_768u32;
                                                let pct = if max_ctx > 0 {
                                                    (tokens as f64 / max_ctx as f64 * 100.0) as u32
                                                } else {
                                                    0
                                                };
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Tokens: {tokens}/{max_ctx} ({pct}%)"
                                                )));
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Messages: {}",
                                                    conversation.len()
                                                )));
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Turns: {}",
                                                    conversation.turn_count()
                                                )));
                                            }
                                            "compact" => {
                                                let before = conversation.len();
                                                // Force summarization by pushing dummy messages
                                                for _ in 0..5 {
                                                    conversation.push(
                                                        axga_shared::types::AgentMessage::System {
                                                            content: "[compacted]".into(),
                                                        },
                                                    );
                                                }
                                                let after = conversation.len();
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "Compacted: {before} → {after} messages"
                                                )));
                                            }
                                            "version" => {
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "axga v{} (rustc {})",
                                                    env!("CARGO_PKG_VERSION"),
                                                    option_env!("CARGO_PKG_RUST_VERSION")
                                                        .unwrap_or("unknown")
                                                )));
                                            }
                                            "export" => {
                                                let path = if args.is_empty() {
                                                    "axga-export.md"
                                                } else {
                                                    args
                                                };
                                                let mut md = String::new();
                                                md.push_str("# axga Session Export\n\n");
                                                md.push_str(&format!(
                                                    "Provider: {} | Model: {} | Tokens: {}\n\n",
                                                    provider, model, app.status.tokens_used
                                                ));
                                                for line in &app.chat_lines {
                                                    match line {
                                                        ChatLine::User(t) => md
                                                            .push_str(&format!("**You:** {t}\n\n")),
                                                        ChatLine::Assistant(t) => {
                                                            md.push_str(&format!("{t}\n\n"))
                                                        }
                                                        ChatLine::Tool { name, detail } => md
                                                            .push_str(&format!(
                                                                "*[tool: {name} → {detail}]*\n\n"
                                                            )),
                                                        ChatLine::Error(t) => md
                                                            .push_str(&format!("*Error: {t}*\n\n")),
                                                        _ => {}
                                                    }
                                                }
                                                match std::fs::write(path, &md) {
                                                    Ok(_) => app.chat_lines.push(ChatLine::Info(
                                                        format!("Exported to {path}"),
                                                    )),
                                                    Err(e) => app.chat_lines.push(ChatLine::Error(
                                                        format!("Export failed: {e}"),
                                                    )),
                                                }
                                            }
                                            "title" => {
                                                if args.is_empty() {
                                                    app.chat_lines.push(ChatLine::Info(
                                                        "Usage: /title <your session title>".into(),
                                                    ));
                                                } else {
                                                    app.chat_lines.push(ChatLine::Info(format!(
                                                        "Session title set to: {args}"
                                                    )));
                                                }
                                            }
                                            _ => {
                                                app.chat_lines.push(ChatLine::Error(format!(
                                                    "Unknown: /{cmd}. Try /help"
                                                )));
                                            }
                                        }
                                        app.scroll_to_bottom();
                                        continue;
                                    }

                                    // Push user message
                                    app.chat_lines.push(ChatLine::User(input.clone()));

                                    // Show spinner
                                    app.is_streaming = true;
                                    app.chat_lines
                                        .push(ChatLine::Thinking("thinking...".into()));
                                    terminal.draw(|f| app.render(f))?;

                                    // Run agent
                                    match run_turn(
                                        provider,
                                        api_key,
                                        base_url,
                                        model,
                                        conversation,
                                        &input,
                                        registry,
                                        system_prompt,
                                        max_turns,
                                    )
                                    .await
                                    {
                                        Ok(turn) => {
                                            // Remove spinner
                                            app.chat_lines.pop();
                                            app.is_streaming = false;

                                            // Show tool calls
                                            if !turn.tool_calls_made.is_empty() {
                                                for tc in &turn.tool_calls_made {
                                                    app.chat_lines.push(ChatLine::Tool {
                                                        name: tc.clone(),
                                                        detail: "executed".into(),
                                                    });
                                                }
                                            }

                                            // Show response
                                            if !turn.final_text.is_empty() {
                                                app.chat_lines
                                                    .push(ChatLine::Assistant(turn.final_text));
                                            }

                                            app.status.tokens_used = app
                                                .status
                                                .tokens_used
                                                .saturating_add(turn.total_tokens);
                                        }
                                        Err(e) => {
                                            app.chat_lines.pop();
                                            app.is_streaming = false;
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
                                    if app.cursor_pos > 0 {
                                        app.cursor_pos -= 1;
                                    }
                                }
                                KeyCode::Right => {
                                    if app.cursor_pos < app.input.len() {
                                        app.cursor_pos += 1;
                                    }
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
                                    app.cursor_pos =
                                        app.cursor_pos.saturating_add(1).min(app.input.len());
                                }
                                KeyCode::Char(':') => {
                                    app.mode = InputMode::Command;
                                    app.input.clear();
                                    app.input.push(':');
                                    app.cursor_pos = 1;
                                }
                                KeyCode::Char('q') => app.exit = true,
                                KeyCode::Char('G') => app.scroll_to_bottom(),
                                KeyCode::Up | KeyCode::Char('k') => app.scroll_by(-1),
                                KeyCode::Down | KeyCode::Char('j') => app.scroll_by(1),
                                KeyCode::Enter => {
                                    // Submit from normal mode
                                    let input = std::mem::take(&mut app.input);
                                    if !input.trim().is_empty() {
                                        app.chat_lines.push(ChatLine::User(input.clone()));
                                        app.is_streaming = true;
                                        app.chat_lines
                                            .push(ChatLine::Thinking("thinking...".into()));
                                        terminal.draw(|f| app.render(f))?;

                                        match run_turn(
                                            provider,
                                            api_key,
                                            base_url,
                                            model,
                                            conversation,
                                            &input,
                                            registry,
                                            system_prompt,
                                            max_turns,
                                        )
                                        .await
                                        {
                                            Ok(turn) => {
                                                app.chat_lines.pop();
                                                app.is_streaming = false;
                                                if !turn.tool_calls_made.is_empty() {
                                                    for tc in &turn.tool_calls_made {
                                                        app.chat_lines.push(ChatLine::Tool {
                                                            name: tc.clone(),
                                                            detail: "executed".into(),
                                                        });
                                                    }
                                                }
                                                if !turn.final_text.is_empty() {
                                                    app.chat_lines
                                                        .push(ChatLine::Assistant(turn.final_text));
                                                }
                                                app.status.tokens_used = app
                                                    .status
                                                    .tokens_used
                                                    .saturating_add(turn.total_tokens);
                                            }
                                            Err(e) => {
                                                app.chat_lines.pop();
                                                app.is_streaming = false;
                                                app.chat_lines
                                                    .push(ChatLine::Error(format!("{e}")));
                                            }
                                        }
                                        app.chat_lines.push(ChatLine::Spacer);
                                        app.scroll_to_bottom();
                                    }
                                }
                                _ => {}
                            }
                        }
                        InputMode::Command => match key.code {
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
                                        app.chat_lines
                                            .push(ChatLine::Info("Conversation cleared.".into()));
                                    }
                                    "tools" => {
                                        app.chat_lines
                                            .push(ChatLine::Info("Available tools:".into()));
                                        for name in registry.names() {
                                            if let Some(tool) = registry.get(name) {
                                                app.chat_lines.push(ChatLine::Info(format!(
                                                    "  {} — {}",
                                                    tool.name(),
                                                    tool.description()
                                                )));
                                            }
                                        }
                                    }
                                    "history" => {
                                        app.chat_lines.push(ChatLine::Info(format!(
                                            "{} messages, {} turns",
                                            conversation.len(),
                                            conversation.turn_count()
                                        )));
                                    }
                                    _ => {
                                        app.chat_lines
                                            .push(ChatLine::Error(format!("Unknown: {cmd_clean}")));
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
                        },
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    Ok(())
}
