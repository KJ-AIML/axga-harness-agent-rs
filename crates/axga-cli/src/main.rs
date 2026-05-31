//! `axga` - AI Coding Agent CLI.
//!
//! # Memory
//! Custom tokio runtime: 2 worker threads (ADR-004).
//! Binary size target: <10 MB (release, musl, LTO).

// Use mimalloc for better memory efficiency (replaces system allocator).
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use axga_core::config::{ProviderSection, TelegramSection};
use axga_core::{Config, ProviderKind};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
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

    /// Model to use. Defaults to config or provider recommendation.
    #[arg(short, long)]
    model: Option<String>,

    /// Provider name. Run `axga models` for supported providers.
    #[arg(short = 'P', long)]
    provider: Option<String>,

    /// System prompt override.
    #[arg(long)]
    system_prompt: Option<String>,

    /// OpenAI API base URL (for compatible providers).
    #[arg(long)]
    base_url: Option<String>,

    /// Max conversation turns before summarization.
    #[arg(long)]
    max_turns: Option<usize>,

    /// Working directory.
    #[arg(short, long)]
    dir: Option<PathBuf>,

    /// Verbose output.
    #[arg(short, long)]
    verbose: bool,

    // Telegram Bot
    /// Start Telegram bot mode (requires --key or saved telegram token).
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

#[derive(Debug, Clone)]
struct EffectiveConfig {
    provider: String,
    model: String,
    api_key: Option<String>,
    base_url: Option<String>,
    system_prompt: Option<String>,
    max_turns: usize,
    telegram_token: Option<String>,
    telegram_allowed_users: Vec<i64>,
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
        let mut saved_config = axga_core::load_config().unwrap_or_default();

        if cli.onboard {
            saved_config = cmd_onboard(&cli, &saved_config).await?;
            if !cli.telegram {
                return Ok(());
            }
            println!();
        }

        match cli.command {
            Some(Commands::Models) => return cmd_models().await,
            Some(Commands::Config) => return cmd_config(&saved_config).await,
            Some(Commands::Doctor) => return cmd_doctor().await,
            None => {}
        }

        let effective = resolve_effective_config(&cli, &saved_config)?;

        if cli.telegram {
            let token = effective.telegram_token.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "--telegram requires --key <bot_token> or a saved [telegram] token. Run axga --onboard."
                )
            })?;
            telegram::run_telegram_bot(telegram::TelegramBotConfig {
                provider: &effective.provider,
                api_key: effective.api_key.as_deref(),
                base_url: effective.base_url.as_deref(),
                model: &effective.model,
                token,
                allowed_users: &effective.telegram_allowed_users,
                system_prompt: effective.system_prompt.as_deref(),
                max_turns: effective.max_turns,
            })
            .await
        } else if let Some(ref spawn_prompt) = cli.spawn {
            cmd_spawn(&effective, spawn_prompt)
        } else if let Some(ref prompt) = cli.prompt {
            cmd_single_shot(prompt, &effective).await
        } else {
            cmd_interactive(&effective).await
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

async fn cmd_config(config: &Config) -> anyhow::Result<()> {
    println!("axga configuration:");
    println!("  Config file: {}", axga_core::config_file_path().display());
    println!("  Provider:");
    println!(
        "    provider: {}",
        config
            .provider
            .provider_type
            .as_deref()
            .unwrap_or("(default)")
    );
    println!(
        "    model:    {}",
        config
            .provider
            .model
            .as_deref()
            .unwrap_or("(provider default)")
    );
    println!(
        "    base URL: {}",
        config
            .provider
            .base_url
            .as_deref()
            .unwrap_or("(provider default)")
    );
    println!(
        "    API key:  {}",
        if config.provider.api_key.is_some() {
            "saved"
        } else {
            "not saved"
        }
    );
    println!(
        "    system:   {}",
        if config.provider.system_prompt.is_some() {
            "set"
        } else {
            "not set"
        }
    );
    println!(
        "    turns:    {}",
        config
            .provider
            .max_turns
            .map(|turns| turns.to_string())
            .unwrap_or_else(|| "10".to_string())
    );
    println!("  Telegram:");
    println!(
        "    token:    {}",
        if config.telegram.is_some() {
            "saved"
        } else {
            "not saved"
        }
    );
    Ok(())
}

async fn cmd_doctor() -> anyhow::Result<()> {
    println!("axga doctor - diagnostics:");
    println!("  Rust:        {}", rustc_version());
    println!(
        "  CWD:         {}",
        std::env::current_dir().unwrap_or_default().display()
    );
    for provider in axga_core::provider_specs() {
        if let Some(env) = provider.api_key_env {
            println!(
                "  {:<10} {}",
                format!("{env}:"),
                if std::env::var(env).is_ok() {
                    "set"
                } else {
                    "not set"
                }
            );
        }
    }
    Ok(())
}

async fn cmd_single_shot(prompt: &str, config: &EffectiveConfig) -> anyhow::Result<()> {
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

    let mut conversation = Conversation::new();

    tracing::info!(provider = %config.provider, model = %config.model, "single-shot");

    let result = run_turn(
        &config.provider,
        config.api_key.as_deref(),
        config.base_url.as_deref(),
        &config.model,
        &mut conversation,
        prompt,
        &registry,
        config.system_prompt.as_deref(),
        config.max_turns,
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

async fn cmd_interactive(config: &EffectiveConfig) -> anyhow::Result<()> {
    tui_mode::run_tui(
        &config.provider,
        config.api_key.as_deref(),
        config.base_url.as_deref(),
        &config.model,
        config.system_prompt.as_deref(),
        config.max_turns,
    )
    .await
}

async fn cmd_onboard(cli: &Cli, existing: &Config) -> anyhow::Result<Config> {
    println!("AXGA Onboarding Wizard");
    println!();
    println!("Providers:");
    for provider in axga_core::provider_specs() {
        println!(
            "  {:<10} default model: {}",
            provider.name, provider.default_model
        );
    }
    println!();

    let default_provider = cli
        .provider
        .clone()
        .or_else(|| existing.provider.provider_type.clone())
        .unwrap_or_else(|| "openai".to_string());
    let provider = prompt_default("Provider", &default_provider)?;
    let spec = axga_core::provider_spec(&provider)
        .ok_or_else(|| anyhow::anyhow!("unknown provider: {provider}"))?;

    let default_model = cli
        .model
        .clone()
        .or_else(|| existing.provider.model.clone())
        .unwrap_or_else(|| spec.default_model.to_string());
    println!("Models: {}", spec.models.join(", "));
    let model = prompt_default("Model", &default_model)?;

    let mut api_key = existing.provider.api_key.clone();
    if spec.requires_api_key {
        if let Some(env) = spec.api_key_env {
            let state = if std::env::var(env).is_ok() {
                "set"
            } else {
                "not set"
            };
            println!("Env {env}: {state}");
        }
        if api_key.is_some() {
            println!("Stored API key: set");
        }
        let entered = prompt_optional("API key (blank keeps saved/env)")?;
        if let Some(value) = entered {
            api_key = Some(value);
        } else if api_key.is_none()
            && spec
                .api_key_env
                .is_none_or(|env| std::env::var(env).is_err())
        {
            anyhow::bail!("No API key provided and provider env var is not set");
        }
    } else {
        api_key = None;
    }

    let base_url = if spec.kind == ProviderKind::OpenAiCompatible {
        let default_base_url = cli
            .base_url
            .clone()
            .or_else(|| existing.provider.base_url.clone())
            .or_else(|| spec.default_base_url.map(ToOwned::to_owned))
            .unwrap_or_default();
        Some(prompt_default("Base URL", &default_base_url)?)
    } else {
        None
    };

    let system_prompt = prompt_optional_with_default(
        "System prompt",
        cli.system_prompt
            .as_deref()
            .or(existing.provider.system_prompt.as_deref()),
    )?;

    let default_turns = cli
        .max_turns
        .or(existing.provider.max_turns)
        .unwrap_or(10)
        .to_string();
    let max_turns = prompt_default("Max turns", &default_turns)?
        .parse::<usize>()
        .map_err(|_| anyhow::anyhow!("max turns must be a positive integer"))?;

    validate_provider_credentials(&provider, api_key.as_deref(), base_url.as_deref(), &model)
        .await?;

    let configure_telegram = cli.telegram
        || prompt_yes_no("Configure Telegram bot token?", existing.telegram.is_some())?;
    let telegram = if configure_telegram {
        let token = if let Some(token) = &cli.key {
            token.clone()
        } else {
            prompt_default(
                "Telegram token",
                existing.telegram.as_ref().map(|_| "(saved)").unwrap_or(""),
            )?
        };
        let token = if token == "(saved)" {
            existing
                .telegram
                .as_ref()
                .map(|telegram| telegram.token.clone())
                .ok_or_else(|| anyhow::anyhow!("no saved Telegram token to keep"))?
        } else {
            token
        };
        validate_telegram_token(&token).await?;
        Some(TelegramSection {
            token,
            allowed_users: existing
                .telegram
                .as_ref()
                .map(|section| section.allowed_users.clone())
                .unwrap_or_default(),
        })
    } else {
        None
    };

    let config = Config {
        provider: ProviderSection {
            provider_type: Some(provider),
            model: Some(model),
            api_key,
            base_url,
            system_prompt,
            max_turns: Some(max_turns),
        },
        session: existing.session.clone(),
        telegram,
        tools: existing.tools.clone(),
        memory: existing.memory.clone(),
    };

    axga_core::save_config(&config)?;
    println!(
        "Saved config to {}",
        axga_core::config_file_path().display()
    );
    Ok(config)
}

fn cmd_spawn(config: &EffectiveConfig, prompt: &str) -> anyhow::Result<()> {
    let current_exe = std::env::current_exe()?;

    println!("Spawning sub-agent with prompt: {prompt}");
    println!("Provider: {}, Model: {}", config.provider, config.model);

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
                config.provider,
                config.model,
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
                config.provider,
                config.model,
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
                config.provider,
                config.model,
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

fn resolve_effective_config(cli: &Cli, saved: &Config) -> anyhow::Result<EffectiveConfig> {
    let provider = cli
        .provider
        .clone()
        .or_else(|| saved.provider.provider_type.clone())
        .unwrap_or_else(|| "openai".to_string());
    let saved_provider_matches = saved
        .provider
        .provider_type
        .as_deref()
        .is_none_or(|saved_provider| saved_provider.eq_ignore_ascii_case(&provider));
    let model = cli
        .model
        .clone()
        .or_else(|| {
            saved_provider_matches
                .then(|| saved.provider.model.clone())
                .flatten()
        })
        .unwrap_or(axga_core::default_model_for_provider(&provider)?.to_string());

    Ok(EffectiveConfig {
        provider,
        model,
        api_key: saved_provider_matches
            .then(|| saved.provider.api_key.clone())
            .flatten(),
        base_url: cli.base_url.clone().or_else(|| {
            saved_provider_matches
                .then(|| saved.provider.base_url.clone())
                .flatten()
        }),
        system_prompt: cli
            .system_prompt
            .clone()
            .or_else(|| saved.provider.system_prompt.clone()),
        max_turns: cli.max_turns.or(saved.provider.max_turns).unwrap_or(10),
        telegram_token: cli
            .key
            .clone()
            .or_else(|| saved.telegram.as_ref().map(|section| section.token.clone())),
        telegram_allowed_users: saved
            .telegram
            .as_ref()
            .map(|section| section.allowed_users.clone())
            .unwrap_or_default(),
    })
}

async fn validate_provider_credentials(
    provider: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model: &str,
) -> anyhow::Result<()> {
    let resolved = axga_core::resolve_provider(provider, api_key, base_url)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;

    match resolved.spec.kind {
        ProviderKind::OpenAiCompatible => {
            let base_url = resolved
                .base_url
                .ok_or_else(|| anyhow::anyhow!("base URL required for provider {provider}"))?;
            let mut request = client.get(format!("{}/models", base_url.trim_end_matches('/')));
            if let Some(api_key) = resolved.api_key {
                request = request.header("Authorization", format!("Bearer {api_key}"));
            }
            let response = request.send().await?;
            if !response.status().is_success() {
                anyhow::bail!("provider validation failed: HTTP {}", response.status());
            }
        }
        ProviderKind::Anthropic => {
            let api_key = resolved
                .api_key
                .ok_or_else(|| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;
            let response = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&serde_json::json!({
                    "model": model,
                    "max_tokens": 1,
                    "messages": [{"role": "user", "content": "ping"}]
                }))
                .send()
                .await?;
            if !response.status().is_success() {
                anyhow::bail!("provider validation failed: HTTP {}", response.status());
            }
        }
    }

    Ok(())
}

async fn validate_telegram_token(token: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let response: serde_json::Value = client
        .get(format!("https://api.telegram.org/bot{token}/getMe"))
        .send()
        .await?
        .json()
        .await?;

    if response["ok"].as_bool().unwrap_or(false) {
        Ok(())
    } else {
        anyhow::bail!("Telegram token validation failed")
    }
}

fn prompt_default(label: &str, default: &str) -> anyhow::Result<String> {
    let input = prompt_line(&format!("{label} [{default}]: "))?;
    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input)
    }
}

fn prompt_optional(label: &str) -> anyhow::Result<Option<String>> {
    let input = prompt_line(&format!("{label}: "))?;
    Ok((!input.is_empty()).then_some(input))
}

fn prompt_optional_with_default(
    label: &str,
    default: Option<&str>,
) -> anyhow::Result<Option<String>> {
    match default {
        Some(default) => {
            let input = prompt_line(&format!("{label} [{default}]: "))?;
            if input.is_empty() {
                Ok(Some(default.to_string()))
            } else {
                Ok(Some(input))
            }
        }
        None => prompt_optional(label),
    }
}

fn prompt_yes_no(label: &str, default: bool) -> anyhow::Result<bool> {
    let suffix = if default { "Y/n" } else { "y/N" };
    let input = prompt_line(&format!("{label} [{suffix}]: "))?;
    if input.is_empty() {
        return Ok(default);
    }
    Ok(matches!(input.to_ascii_lowercase().as_str(), "y" | "yes"))
}

fn prompt_line(prompt: &str) -> anyhow::Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn rustc_version() -> String {
    option_env!("CARGO_PKG_RUST_VERSION")
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli() -> Cli {
        Cli {
            command: None,
            prompt: None,
            model: None,
            provider: None,
            system_prompt: None,
            base_url: None,
            max_turns: None,
            dir: None,
            verbose: false,
            telegram: false,
            key: None,
            onboard: false,
            spawn: None,
        }
    }

    #[test]
    fn saved_config_is_used_when_flags_are_omitted() {
        let saved = Config {
            provider: ProviderSection {
                provider_type: Some("deepseek".into()),
                model: Some("deepseek-chat".into()),
                api_key: Some("stored-key".into()),
                base_url: Some("http://proxy.local/v1".into()),
                system_prompt: Some("system".into()),
                max_turns: Some(4),
            },
            telegram: Some(TelegramSection {
                token: "bot-token".into(),
                allowed_users: vec![],
            }),
            ..Default::default()
        };

        let effective = resolve_effective_config(&cli(), &saved).unwrap();

        assert_eq!(effective.provider, "deepseek");
        assert_eq!(effective.model, "deepseek-chat");
        assert_eq!(effective.api_key.as_deref(), Some("stored-key"));
        assert_eq!(effective.base_url.as_deref(), Some("http://proxy.local/v1"));
        assert_eq!(effective.system_prompt.as_deref(), Some("system"));
        assert_eq!(effective.max_turns, 4);
        assert_eq!(effective.telegram_token.as_deref(), Some("bot-token"));
        assert!(effective.telegram_allowed_users.is_empty());
    }

    #[test]
    fn cli_flags_override_saved_config() {
        let mut cli = cli();
        cli.provider = Some("openai".into());
        cli.model = Some("gpt-4o".into());
        cli.base_url = Some("http://override.local/v1".into());
        cli.system_prompt = Some("override".into());
        cli.max_turns = Some(2);
        cli.key = Some("cli-token".into());

        let saved = Config {
            provider: ProviderSection {
                provider_type: Some("deepseek".into()),
                model: Some("deepseek-chat".into()),
                api_key: Some("stored-key".into()),
                base_url: Some("http://proxy.local/v1".into()),
                system_prompt: Some("system".into()),
                max_turns: Some(4),
            },
            telegram: Some(TelegramSection {
                token: "saved-token".into(),
                allowed_users: vec![],
            }),
            ..Default::default()
        };

        let effective = resolve_effective_config(&cli, &saved).unwrap();

        assert_eq!(effective.provider, "openai");
        assert_eq!(effective.model, "gpt-4o");
        assert_eq!(effective.api_key.as_deref(), None);
        assert_eq!(
            effective.base_url.as_deref(),
            Some("http://override.local/v1")
        );
        assert_eq!(effective.system_prompt.as_deref(), Some("override"));
        assert_eq!(effective.max_turns, 2);
        assert_eq!(effective.telegram_token.as_deref(), Some("cli-token"));
    }

    #[test]
    fn provider_override_does_not_reuse_saved_provider_scoped_fields() {
        let mut cli = cli();
        cli.provider = Some("openai".into());

        let saved = Config {
            provider: ProviderSection {
                provider_type: Some("deepseek".into()),
                model: Some("deepseek-chat".into()),
                api_key: Some("stored-key".into()),
                base_url: Some("http://proxy.local/v1".into()),
                system_prompt: None,
                max_turns: None,
            },
            ..Default::default()
        };

        let effective = resolve_effective_config(&cli, &saved).unwrap();

        assert_eq!(effective.provider, "openai");
        assert_eq!(effective.model, "gpt-4o-mini");
        assert!(effective.api_key.is_none());
        assert!(effective.base_url.is_none());
    }
}
