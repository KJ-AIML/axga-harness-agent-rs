//! axga TUI mode — ratatui-powered interactive terminal.
//!
//! Launched when no --prompt is given. Full ratatui app with:
//! - Chat pane (conversation history)
//! - Status bar (model, tokens, memory)
//! - Input pane with Insert/Normal/Command modes

use axga_tui::app::{App, InputMode};
use axga_core::{Conversation, ToolRegistry, run_turn};
use axga_core::tools::{fs, shell, code};
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
    // ── Setup ──
    let mut registry = ToolRegistry::new();
    registry.register(fs::ReadFileTool)?;
    registry.register(fs::WriteFileTool)?;
    registry.register(fs::ListDirectoryTool)?;
    registry.register(shell::ShellTool)?;
    registry.register(code::GrepTool)?;
    registry.register(code::GlobTool)?;
    registry.register(code::DiffTool)?;

    let mut conversation = Conversation::new();
    let mut terminal = ratatui::init();
    let mut app = App::new(model);

    app.chat_lines.push("AXGA TUI — type your prompt and press Enter.".into());
    app.chat_lines.push(format!("Provider: {}, Model: {}", provider, model).into());
    app.chat_lines.push(String::new());

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

    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("Error: {}", e);
            Ok(())
        }
    }
}

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
    loop {
        terminal.draw(|f| app.render(f))?;

        if app.exit {
            break;
        }

        // Non-blocking event poll (100ms timeout for smooth rendering)
        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;

            match ev {
                Event::Key(key) => {
                    // Global: Ctrl+C exits
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        app.exit = true;
                        break;
                    }

                    match app.mode {
                        InputMode::Insert => {
                            match key.code {
                                KeyCode::Esc => app.mode = InputMode::Normal,
                                KeyCode::Enter => {
                                    let input = std::mem::take(&mut app.input);
                                    if input.trim().is_empty() {
                                        continue;
                                    }
                                    app.chat_lines.push(format!("> {}", input));

                                    // Run the agent
                                    app.status.model = format!("{} (thinking...)", model);
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
                                            if !turn.final_text.is_empty() {
                                                for line in turn.final_text.lines() {
                                                    app.chat_lines.push(line.to_string());
                                                }
                                            }
                                            if !turn.tool_calls_made.is_empty() {
                                                app.chat_lines.push(format!(
                                                    "  [tools: {}]",
                                                    turn.tool_calls_made.join(", ")
                                                ));
                                            }
                                            app.status.tokens_used = turn.total_tokens;
                                        }
                                        Err(e) => {
                                            app.chat_lines.push(format!("Error: {}", e));
                                        }
                                    }

                                    app.status.model = model.to_string();
                                    // Scroll to bottom
                                    if app.chat_lines.len() > 3 {
                                        app.scroll_offset = (app.chat_lines.len() - 3) as u16;
                                    }
                                }
                                KeyCode::Backspace => {
                                    app.input.pop();
                                }
                                KeyCode::Char(c) => {
                                    app.input.push(c);
                                }
                                _ => {}
                            }
                        }
                        InputMode::Normal => {
                            match key.code {
                                KeyCode::Char('i') => app.mode = InputMode::Insert,
                                KeyCode::Char(':') => {
                                    app.mode = InputMode::Command;
                                    app.input.push(':');
                                }
                                KeyCode::Char('q') => app.exit = true,
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if app.scroll_offset > 0 {
                                        app.scroll_offset -= 1;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    app.scroll_offset += 1;
                                }
                                _ => {}
                            }
                        }
                        InputMode::Command => {
                            match key.code {
                                KeyCode::Esc => {
                                    app.mode = InputMode::Normal;
                                    app.input.clear();
                                }
                                KeyCode::Enter => {
                                    let cmd = std::mem::take(&mut app.input);
                                    let cmd = cmd.strip_prefix(':').unwrap_or(&cmd).trim();
                                    match cmd {
                                        "q" | "quit" => app.exit = true,
                                        "clear" => {
                                            conversation.reset();
                                            app.chat_lines.clear();
                                            app.chat_lines.push("Conversation cleared.".into());
                                        }
                                        "tools" => {
                                            for name in registry.names() {
                                                if let Some(tool) = registry.get(name) {
                                                    app.chat_lines.push(format!(
                                                        "  {} — {}",
                                                        tool.name(),
                                                        tool.description()
                                                    ));
                                                }
                                            }
                                        }
                                        _ => {
                                            app.chat_lines.push(format!("Unknown command: {}", cmd));
                                        }
                                    }
                                    app.mode = InputMode::Normal;
                                }
                                KeyCode::Backspace => {
                                    app.input.pop();
                                }
                                KeyCode::Char(c) => {
                                    app.input.push(c);
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
