//! `axga` — AI Coding Agent CLI.
//!
//! # Memory
//! Custom tokio runtime: 2 worker threads (ADR-004).
//! Binary size target: <10 MB (release, musl, LTO).

// Use mimalloc for better memory efficiency (replaces system allocator).
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod runtime;
mod tui_mode;
mod telegram;

#[derive(Parser)]
#[command(name = "axga", version, about = "AI coding agent for 1GB VPS")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Single-shot prompt (streams response to stdout).
    #[arg(short, long)]
    prompt: Option<String>,

    /// Model to use.
    #[arg(short, long, default_value = "gpt-4o-mini")]
    model: String,

    /// Provider: openai, anthropic.
    #[arg(short = 'P', long, default_value = "openai")]
    provider: String,

    /// System prompt override.
    #[arg(long)]
    system_prompt: Option<String>,

    /// OpenAI API base URL (for compatible providers).
    #[arg(long)]
    base_url: Option<String>,

    /// Max conversation turns before summarization.
    #[arg(long, default_value = "10")]
    max_turns: usize,

    /// Working directory.
    #[arg(short, long)]
    dir: Option<PathBuf>,

    /// Verbose output.
    #[arg(short, long)]
    verbose: bool,

    // ── Telegram Bot ──

    /// Start Telegram bot mode (requires --key).
    #[arg(long)]
    telegram: bool,

    /// Telegram bot token from @BotFather.
    #[arg(long)]
    key: Option<String>,

    /// Run onboarding wizard.
    #[arg(long)]
    onboard: bool,

    // ── Agent Spawning ──

    /// Spawn a new agent with the given prompt.
    #[arg(long)]
    spawn: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// List supported providers and models.
    Models,
    /// Print current configuration.
    Config,
    /// Run memory diagnostics.
    Doctor,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose { "axga=debug" } else { "axga=info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_target(false)
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "axga starting");

    if let Some(ref dir) = cli.dir {
        std::env::set_current_dir(dir)?;
    }

    let rt = runtime::build_runtime()?;
    rt.block_on(async {
        match cli.command {
            Some(Commands::Models) => cmd_models().await,
            Some(Commands::Config) => cmd_config().await,
            Some(Commands::Doctor) => cmd_doctor().await,
            None => {
                if let Some(ref prompt) = cli.prompt {
                    cmd_single_shot(&prompt, &cli).await
                } else {
                    cmd_interactive(&cli).await
                }
            }
        }
    })
}

async fn cmd_models() -> anyhow::Result<()> {
    println!("Supported providers:");
    println!("  openai      — gpt-4o, gpt-4o-mini, gpt-4.1, o3-mini");
    println!("  deepseek    — deepseek-chat, deepseek-reasoner");
    println!("  anthropic   — claude-sonnet-4-20250514, claude-haiku-3-5");
    println!();
    println!("Set provider:  axga --provider <name>");
    println!("Set model:     axga --model <model-id>");
    Ok(())
}

async fn cmd_config() -> anyhow::Result<()> {
    println!("axga configuration:");
    println!("  Config dir:  ~/.config/axga/");
    println!("  Env vars:    OPENAI_API_KEY, ANTHROPIC_API_KEY, OPENAI_BASE_URL");
    Ok(())
}

async fn cmd_doctor() -> anyhow::Result<()> {
    println!("axga doctor — diagnostics:");
    println!("  Rust:        {}", rustc_version());
    println!("  CWD:         {}", std::env::current_dir().unwrap_or_default().display());
    println!("  OpenAI key:  {}", if std::env::var("OPENAI_API_KEY").is_ok() { "set" } else { "not set" });
    println!("  Anthropic:   {}", if std::env::var("ANTHROPIC_API_KEY").is_ok() { "set" } else { "not set" });
    Ok(())
}

async fn cmd_single_shot(prompt: &str, cli: &Cli) -> anyhow::Result<()> {
    use axga_core::{Conversation, ToolRegistry, run_turn};
    use axga_core::tools::{fs, shell, code, memctrl};

    // Build tool registry
    let mut registry = ToolRegistry::new();
    registry.register(fs::ReadFileTool)?;
    registry.register(fs::WriteFileTool)?;
    registry.register(fs::ListDirectoryTool)?;
    registry.register(shell::ShellTool)?;
    registry.register(code::GrepTool)?;
    registry.register(code::GlobTool)?;
    registry.register(code::DiffTool)?;
    registry.register(memctrl::MemCtrlTool)?;

    let api_key = match cli.provider.as_str() {
        "openai" | "deepseek" => std::env::var("OPENAI_API_KEY").ok()
            .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok()),
        "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
        _ => None,
    };

    let mut conversation = Conversation::new();

    tracing::info!(provider = %cli.provider, model = %cli.model, "single-shot");

    let result = run_turn(
        &cli.provider,
        api_key.as_deref(),
        cli.base_url.as_deref(),
        &cli.model,
        &mut conversation,
        prompt,
        &registry,
        cli.system_prompt.as_deref(),
        cli.max_turns,
    )
    .await;

    match result {
        Ok(turn) => {
            println!("{}", turn.final_text);
            if !turn.tool_calls_made.is_empty() {
                eprintln!("\n[tools used: {} | {} tokens]",
                    turn.tool_calls_made.join(", "),
                    turn.total_tokens);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn cmd_interactive(cli: &Cli) -> anyhow::Result<()> {
    let api_key = match cli.provider.as_str() {
        "openai" | "deepseek" => std::env::var("OPENAI_API_KEY").ok()
            .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok()),
        "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
        _ => None,
    };

    tui_mode::run_tui(
        &cli.provider,
        api_key.as_deref(),
        cli.base_url.as_deref(),
        &cli.model,
        cli.system_prompt.as_deref(),
        cli.max_turns,
    )
    .await
}

fn rustc_version() -> String {
    option_env!("CARGO_PKG_RUST_VERSION")
        .unwrap_or("unknown")
        .to_string()
}
