//! `axga` — AI Coding Agent CLI.
//!
//! # Memory
//! Custom tokio runtime: 2 worker threads (ADR-004).
//! Binary size target: <10 MB (release, musl, LTO).

// Use mimalloc for better memory efficiency (replaces system allocator).
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use axga_shared::types::SubAgentConfig;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod runtime;
mod tui_mode;
mod telegram;
mod mcp;

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

    /// JSON log format for production.
    #[arg(long)]
    json_log: bool,

    // ── Telegram Bot ──

    /// Start Telegram bot mode (requires --key).
    #[arg(long)]
    telegram: bool,

    /// Telegram bot token from @BotFather.
    #[arg(long)]
    key: Option<String>,

    /// Telegram webhook URL (alternative to long-polling).
    #[arg(long)]
    webhook_url: Option<String>,

    /// Run onboarding wizard.
    #[arg(long)]
    onboard: bool,

    // ── Agent Spawning ──

    /// Spawn a new agent with the given prompt.
    #[arg(long)]
    spawn: Option<String>,

    /// Allow dangerous shell commands (rm, dd, curl|sh, etc).
    #[arg(long)]
    dangerous: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List supported providers and models.
    Models,
    /// Print current configuration.
    Config,
    /// Run memory diagnostics.
    Doctor {
        /// Output in JSON format.
        #[arg(long)]
        json: bool,
    },
    /// Start MCP server (stdio transport, JSON-RPC).
    Mcp,
    /// Run multiple agents with different providers/models.
    Orchestrate {
        /// Path to JSON file with agent configurations.
        #[arg(short, long)]
        config: String,

        /// Input prompt for all agents.
        #[arg(short, long)]
        prompt: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose { "axga=debug" } else { "axga=info" };
    if cli.json_log {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(log_level)
            .with_target(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(log_level)
            .with_target(false)
            .init();
    }

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "axga starting");

    if let Some(ref dir) = cli.dir {
        std::env::set_current_dir(dir)?;
    }

    let rt = runtime::build_runtime()?;
    rt.block_on(async {
        // ── Telegram Bot Mode ──
        if cli.telegram {
            let token = cli.key.as_deref()
                .ok_or_else(|| anyhow::anyhow!("--key <bot_token> required for --telegram. Get one from @BotFather."))?;
            let api_key = match cli.provider.as_str() {
                "openai" | "deepseek" => std::env::var("OPENAI_API_KEY").ok()
                    .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok()),
                "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
                _ => None,
            };
            if cli.onboard { cmd_onboard(&cli).await?; println!(); }
            if let Some(ref webhook_url) = cli.webhook_url {
                telegram::run_telegram_webhook(&cli.provider, api_key.as_deref(), &cli.model, token, cli.system_prompt.as_deref(), webhook_url, cli.dangerous).await
            } else {
                telegram::run_telegram_bot(&cli.provider, api_key.as_deref(), &cli.model, token, cli.system_prompt.as_deref(), cli.dangerous).await
            }
        }
        // ── Onboarding wizard (without telegram) ──
        else if cli.onboard {
            cmd_onboard(&cli).await
        }
        // ── Spawn agent ──
        else if let Some(ref spawn_prompt) = cli.spawn {
            cmd_spawn(&cli, spawn_prompt)
        }
        else {
            match cli.command {
                Some(Commands::Models) => cmd_models().await,
                Some(Commands::Config) => cmd_config().await,
                Some(Commands::Doctor { json }) => cmd_doctor(json).await,
                Some(Commands::Mcp) => cmd_mcp(cli.dangerous).await,
                Some(Commands::Orchestrate { config, prompt }) => cmd_orchestrate(&config, &prompt, cli.dangerous).await,
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

async fn cmd_doctor(json: bool) -> anyhow::Result<()> {
    if json {
        let info = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "rust_version": rustc_version(),
            "cwd": std::env::current_dir().unwrap_or_default().display().to_string(),
            "openai_key": std::env::var("OPENAI_API_KEY").is_ok(),
            "deepseek_key": std::env::var("DEEPSEEK_API_KEY").is_ok(),
            "anthropic_key": std::env::var("ANTHROPIC_API_KEY").is_ok(),
            "memctrl_db": std::path::Path::new(".memctrl/memories.db").exists(),
            "config_file": std::path::Path::new(".config/axga/config.toml").exists()
                || std::path::Path::new("axga.toml").exists(),
            "pid": std::process::id(),
        });
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("axga doctor — diagnostics:");
        println!("  Rust:        {}", rustc_version());
        println!("  CWD:         {}", std::env::current_dir().unwrap_or_default().display());
        println!("  OpenAI key:  {}", if std::env::var("OPENAI_API_KEY").is_ok() { "set" } else { "not set" });
        println!("  DeepSeek:    {}", if std::env::var("DEEPSEEK_API_KEY").is_ok() { "set" } else { "not set" });
        println!("  Anthropic:   {}", if std::env::var("ANTHROPIC_API_KEY").is_ok() { "set" } else { "not set" });
        println!("  MemCtrl DB:  {}", if std::path::Path::new(".memctrl/memories.db").exists() { "found" } else { "not found" });
    }
    Ok(())
}

async fn cmd_mcp(dangerous: bool) -> anyhow::Result<()> {
    let registry = axga_core::build_default_registry(dangerous)?;
    mcp::run_mcp_server("mcp", None, "any", &registry).await
}

async fn cmd_orchestrate(config_path: &str, prompt: &str, dangerous: bool) -> anyhow::Result<()> {
    use axga_core::Orchestrator;

    let config_file = std::fs::read_to_string(config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file '{config_path}': {e}"))?;
    let agent_configs: Vec<SubAgentConfig> = serde_json::from_str(&config_file)
        .map_err(|e| anyhow::anyhow!("Failed to parse config file '{config_path}': {e}"))?;

    let registry = axga_core::build_default_registry(dangerous)?;
    let orch = Orchestrator::new(registry);

    let config_inputs: Vec<(SubAgentConfig, String)> = agent_configs
        .into_iter()
        .map(|c| (c, prompt.to_string()))
        .collect();

    println!("Spawning {} agents...", config_inputs.len());
    let results = orch.spawn_all(config_inputs).await;

    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(r) => {
                println!("\n─── Agent {i} ───");
                println!("Response: {}", r.response);
                println!("Tokens: {} | Turns: {}", r.tokens_used, r.turns_taken);
                if let Some(ref err) = r.error {
                    eprintln!("Error: {err}");
                }
            }
            Err(e) => {
                eprintln!("Agent {i} failed: {e}");
            }
        }
    }

    Ok(())
}

async fn cmd_single_shot(prompt: &str, cli: &Cli) -> anyhow::Result<()> {
    use axga_core::{Conversation, run_turn};

    // Build tool registry
    let registry = axga_core::build_default_registry(cli.dangerous)?;
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
            eprintln!("Error: {e}");
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
        cli.dangerous,
    )
    .await
}

async fn cmd_onboard(cli: &Cli) -> anyhow::Result<()> {
    println!("╔══════════════════════════════════════════╗");
    println!("║        AXGA Onboarding Wizard           ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║                                          ║");
    println!("║  Setup options:                          ║");
    println!("║                                          ║");
    println!("║  axga --onboard --telegram --key <token> ║");
    println!("║    → Start Telegram bot with your token  ║");
    println!("║                                          ║");
    println!("║  axga --spawn \"your prompt\"             ║");
    println!("║    → Spawn sub-agent with prompt         ║");
    println!("║                                          ║");
    println!("║  Get a Telegram token:                   ║");
    println!("║    1. Open @BotFather on Telegram        ║");
    println!("║    2. Send /newbot                       ║");
    println!("║    3. Copy the token                     ║");
    println!("║                                          ║");
    println!("╚══════════════════════════════════════════╝");

    if let Some(ref token) = cli.key {
        if cli.telegram {
            println!("\n→ Starting Telegram bot with token: {}...", &token[..8.min(token.len())]);
        }
    } else if cli.telegram {
        println!("\n→ --telegram requires --key <bot_token>");
    }

    Ok(())
}

fn cmd_spawn(cli: &Cli, prompt: &str) -> anyhow::Result<()> {
    let current_exe = std::env::current_exe()?;
    let provider = cli.provider.clone();
    let model = cli.model.clone();

    println!("Spawning sub-agent with prompt: {prompt}");
    println!("Provider: {provider}, Model: {model}");

    // Detect terminal
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
                current_exe.display(), provider, model, prompt
            );
            println!("→ Spawning via tmux: {cmd}");
            std::process::Command::new("bash").arg("-c").arg(&cmd).spawn()?;
            Ok(())
        }
        "osascript" => {
            let cmd = format!(
                "tell application \"Terminal\" to do script \"cd '$(pwd)' && {} --provider {} --model {} --prompt '{}'\"",
                current_exe.display(), provider, model, prompt
            );
            std::process::Command::new("osascript").arg("-e").arg(&cmd).spawn()?;
            Ok(())
        }
        _ => {
            let cmd = format!(
                "gnome-terminal -- bash -c \"{} --provider {} --model {} --prompt '{}'; exec \\$SHELL\"",
                current_exe.display(), provider, model, prompt
            );
            std::process::Command::new("bash").arg("-c").arg(&cmd).spawn()?;
            Ok(())
        }
    }
}

fn rustc_version() -> String {
    option_env!("CARGO_PKG_RUST_VERSION")
        .unwrap_or("unknown")
        .to_string()
}
