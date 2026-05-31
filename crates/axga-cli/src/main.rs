//! `axga` - AI Coding Agent CLI.
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
mod telegram;
mod tui_mode;

#[derive(Parser)]
#[command(name = "axga", version, about = "AI coding agent for 1GB VPS")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Single-shot prompt (streams response to stdout).
    #[arg(short, long)]
    prompt: Option<String>,

    /// Model to use. Defaults to the provider's recommended model.
    #[arg(short, long)]
    model: Option<String>,

    /// Provider name. Run `axga models` for supported providers.
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

    // Telegram Bot
    /// Start Telegram bot mode (requires --key).
    #[arg(long)]
    telegram: bool,

    /// Telegram bot token from @BotFather.
    #[arg(long)]
    key: Option<String>,

    /// Run onboarding wizard.
    #[arg(long)]
    onboard: bool,

    // Agent Spawning
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

    let log_level = if cli.verbose {
        "axga=debug"
    } else {
        "axga=info"
    };
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
        if cli.telegram {
            let token = cli.key.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "--key <bot_token> required for --telegram. Get one from @BotFather."
                )
            })?;
            let model = resolve_cli_model(&cli)?;
            if cli.onboard {
                cmd_onboard(&cli).await?;
                println!();
            }
            telegram::run_telegram_bot(
                &cli.provider,
                None,
                &model,
                token,
                cli.system_prompt.as_deref(),
            )
            .await
        } else if cli.onboard {
            cmd_onboard(&cli).await
        } else if let Some(ref spawn_prompt) = cli.spawn {
            cmd_spawn(&cli, spawn_prompt)
        } else {
            match cli.command {
                Some(Commands::Models) => cmd_models().await,
                Some(Commands::Config) => cmd_config().await,
                Some(Commands::Doctor) => cmd_doctor().await,
                None => {
                    if let Some(ref prompt) = cli.prompt {
                        cmd_single_shot(prompt, &cli).await
                    } else {
                        cmd_interactive(&cli).await
                    }
                }
            }
        }
    })
}

async fn cmd_models() -> anyhow::Result<()> {
    println!("Supported providers:");
    for provider in axga_core::provider_specs() {
        println!(
            "  {:<10} default: {:<28} models: {}",
            provider.name,
            provider.default_model,
            provider.models.join(", ")
        );
    }
    println!();
    println!("Set provider:  axga --provider <name>");
    println!("Set model:     axga --model <model-id>");
    Ok(())
}

async fn cmd_config() -> anyhow::Result<()> {
    println!("axga configuration:");
    println!("  Config dir:  ~/.config/axga/");
    println!("  Providers:");
    for provider in axga_core::provider_specs() {
        println!(
            "    {:<10} key: {:<20} base: {}",
            provider.name,
            provider.api_key_env.unwrap_or("(none)"),
            provider.default_base_url.unwrap_or("(native)")
        );
    }
    Ok(())
}

async fn cmd_doctor() -> anyhow::Result<()> {
    println!("axga doctor - diagnostics:");
    println!("  Rust:        {}", rustc_version());
    println!(
        "  CWD:         {}",
        std::env::current_dir().unwrap_or_default().display()
    );
    println!(
        "  OpenAI key:  {}",
        if std::env::var("OPENAI_API_KEY").is_ok() {
            "set"
        } else {
            "not set"
        }
    );
    println!(
        "  Anthropic:   {}",
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            "set"
        } else {
            "not set"
        }
    );
    Ok(())
}

async fn cmd_single_shot(prompt: &str, cli: &Cli) -> anyhow::Result<()> {
    use axga_core::tools::{code, fetch_url, fs, memctrl, shell, web_search};
    use axga_core::{Conversation, ToolRegistry, run_turn};

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

    let model = resolve_cli_model(cli)?;
    let mut conversation = Conversation::new();

    tracing::info!(provider = %cli.provider, model = %model, "single-shot");

    let result = run_turn(
        &cli.provider,
        None,
        cli.base_url.as_deref(),
        &model,
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
                eprintln!(
                    "\n[tools used: {} | {} tokens]",
                    turn.tool_calls_made.join(", "),
                    turn.total_tokens
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn cmd_interactive(cli: &Cli) -> anyhow::Result<()> {
    let model = resolve_cli_model(cli)?;

    tui_mode::run_tui(
        &cli.provider,
        None,
        cli.base_url.as_deref(),
        &model,
        cli.system_prompt.as_deref(),
        cli.max_turns,
    )
    .await
}

async fn cmd_onboard(cli: &Cli) -> anyhow::Result<()> {
    println!("AXGA Onboarding Wizard");
    println!();
    println!("Setup options:");
    println!("  axga --onboard --telegram --key <token>");
    println!("    Start Telegram bot with your token");
    println!("  axga --spawn \"your prompt\"");
    println!("    Spawn sub-agent with prompt");
    println!();
    println!("Get a Telegram token:");
    println!("  1. Open @BotFather on Telegram");
    println!("  2. Send /newbot");
    println!("  3. Copy the token");

    if let Some(ref token) = cli.key {
        if cli.telegram {
            println!(
                "\nStarting Telegram bot with token: {}...",
                &token[..8.min(token.len())]
            );
        }
    } else if cli.telegram {
        println!("\n--telegram requires --key <bot_token>");
    }

    Ok(())
}

fn cmd_spawn(cli: &Cli, prompt: &str) -> anyhow::Result<()> {
    let current_exe = std::env::current_exe()?;
    let provider = cli.provider.clone();
    let model = resolve_cli_model(cli)?;

    println!("Spawning sub-agent with prompt: {prompt}");
    println!("Provider: {provider}, Model: {model}");

    let terminal = if std::env::var("TMUX").is_ok() {
        "tmux"
    } else if cfg!(target_os = "macos") {
        "osascript"
    } else {
        "gnome-terminal"
    };

    match terminal {
        "tmux" => {
            let cmd = format!(
                "tmux split-window -h -c \"$(pwd)\" \"{} --provider {} --model {} --prompt '{}'\"",
                current_exe.display(),
                provider,
                model,
                prompt
            );
            println!("Spawning via tmux: {cmd}");
            std::process::Command::new("bash")
                .arg("-c")
                .arg(&cmd)
                .spawn()?;
            Ok(())
        }
        "osascript" => {
            let cmd = format!(
                "tell application \"Terminal\" to do script \"cd '$(pwd)' && {} --provider {} --model {} --prompt '{}'\"",
                current_exe.display(),
                provider,
                model,
                prompt
            );
            std::process::Command::new("osascript")
                .arg("-e")
                .arg(&cmd)
                .spawn()?;
            Ok(())
        }
        _ => {
            let cmd = format!(
                "gnome-terminal -- bash -c \"{} --provider {} --model {} --prompt '{}'; exec \\$SHELL\"",
                current_exe.display(),
                provider,
                model,
                prompt
            );
            std::process::Command::new("bash")
                .arg("-c")
                .arg(&cmd)
                .spawn()?;
            Ok(())
        }
    }
}

fn resolve_cli_model(cli: &Cli) -> anyhow::Result<String> {
    match &cli.model {
        Some(model) => Ok(model.clone()),
        None => Ok(axga_core::default_model_for_provider(&cli.provider)?.to_string()),
    }
}

fn rustc_version() -> String {
    option_env!("CARGO_PKG_RUST_VERSION")
        .unwrap_or("unknown")
        .to_string()
}
