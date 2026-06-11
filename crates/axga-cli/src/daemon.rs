//! Background daemon mode — runs axga without a TUI.
//!
//! Usage: axga --daemon [--telegram --key TOKEN | --discord --key TOKEN]
//!
//! Keeps the agent alive even when the terminal is closed.
//! Combine with nohup or systemd for production use.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Run axga as a background daemon.
/// Listens on stdin for prompts and responds via stdout.
/// Gracefully shuts down on SIGTERM/SIGINT.
pub async fn run_daemon(
    provider: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model: &str,
    system_prompt: Option<&str>,
    max_turns: usize,
    dangerous: bool,
) -> anyhow::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Handle Ctrl+C gracefully
    ctrlc_handler(r)?;

    let permissions = Arc::new(axga_core::PermissionManager::new(
        if dangerous { axga_core::PermissionMode::Auto } else { axga_core::PermissionMode::Manual }
    ));

    println!("🔄 axga daemon started (PID: {})", std::process::id());
    println!("   Provider: {provider}");
    println!("   Model:    {model}");
    println!("   Mode:     {}", if dangerous { "Auto (--dangerous)" } else { "Manual" });
    println!();

    // Read prompts from stdin (one per line)
    let stdin = std::io::BufRead::lines(std::io::BufReader::new(std::io::stdin()));

    for line_res in stdin {
        if !running.load(Ordering::SeqCst) {
            println!("Shutting down...");
            break;
        }

        let line = line_res?;
        let prompt = line.trim();
        if prompt.is_empty() { continue; }

        let mut conversation = axga_core::Conversation::new();
        let registry = axga_core::build_default_registry(
            dangerous, Some(provider), Some(model),
            api_key, base_url, None,
        )?;

        struct DaemonHandler;
        impl axga_core::StreamHandler for DaemonHandler {
            fn on_text_delta(&mut self, text: &str) { print!("{text}"); }
            fn on_tool_call_delta(&mut self, _id: &str, name: &str, _args: &str) {
                print!("\n⚙ {name} → ");
            }
            fn on_tool_call_result(&mut self, _name: &str, _result: &str) {}
            fn on_thinking(&mut self) {}
            fn on_done(&mut self) { println!(); }
        }

        let result = axga_core::run_turn_streaming(
            provider, api_key, base_url, model,
            &mut conversation, prompt, &registry,
            system_prompt, max_turns, &mut DaemonHandler,
            Some(permissions.clone()),
        ).await;

        match result {
            Ok(turn) => {
                if !turn.final_text.is_empty() {
                    println!("{}", turn.final_text);
                }
                for tc in &turn.tool_calls_made {
                    println!("[tool: {tc}]");
                }
                if !turn.tool_calls_made.is_empty() {
                    println!();
                }
            }
            Err(e) => eprintln!("Error: {e}"),
        }
    }

    println!("Daemon stopped.");
    Ok(())
}

#[cfg(unix)]
fn ctrlc_handler(running: Arc<AtomicBool>) -> anyhow::Result<()> {
    let mut signals = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::spawn(async move {
        let _ = signals.recv().await;
        running.store(false, Ordering::SeqCst);
    });
    Ok(())
}

#[cfg(not(unix))]
fn ctrlc_handler(_running: Arc<AtomicBool>) -> anyhow::Result<()> {
    Ok(())
}
