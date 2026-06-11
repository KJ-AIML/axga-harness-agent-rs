use crate::Cli;
use axga_core::{Config, save_config};
use axga_core::config::ProviderSection;
use std::io::{self, Write};

pub async fn cmd_onboard(_cli: &Cli) -> anyhow::Result<()> {
    let stdin = io::stdin();

    println!("╔══════════════════════════════════════════════╗");
    println!("║        AXGA Onboarding Wizard               ║");
    println!("╚══════════════════════════════════════════════╝");
    println!();
    println!("Let's set up your API keys and choose a mode.");
    println!();

    // Step 1: API key
    print!("  Do you have a DeepSeek API key? (Y/n): ");
    io::stdout().flush()?;
    let mut buf = String::new();
    stdin.read_line(&mut buf)?;
    let has_deepseek = buf.trim().is_empty() || buf.trim().to_lowercase().starts_with('y');

    let provider = if has_deepseek {
        print!("  DeepSeek API key: ");
        io::stdout().flush()?;
        let mut key = String::new();
        stdin.read_line(&mut key)?;
        let key = key.trim();
        if !key.is_empty() {
            unsafe { std::env::set_var("DEEPSEEK_API_KEY", key) };
            let config = Config {
                provider: ProviderSection {
                    provider_type: Some("deepseek".into()),
                    model: Some("deepseek-v4-flash".into()),
                    api_key: Some(key.to_string()),
                    base_url: None,
                    system_prompt: None,
                    max_turns: Some(10),
                },
                ..Default::default()
            };
            save_config(&config)?;
            println!("  ✅ Saved to ~/.config/axga/config.toml");
        }
        "deepseek"
    } else {
        print!("  OpenAI API key: ");
        io::stdout().flush()?;
        let mut key = String::new();
        stdin.read_line(&mut key)?;
        let key = key.trim();
        if !key.is_empty() {
            unsafe { std::env::set_var("OPENAI_API_KEY", key) };
            let config = Config {
                provider: ProviderSection {
                    provider_type: Some("openai".into()),
                    model: Some("gpt-4o-mini".into()),
                    api_key: Some(key.to_string()),
                    base_url: None,
                    system_prompt: None,
                    max_turns: Some(10),
                },
                ..Default::default()
            };
            save_config(&config)?;
            println!("  ✅ Saved to ~/.config/axga/config.toml");
        }
        "openai"
    };
    println!();

    // Step 2: Mode
    println!("  How would you like to run axga?");
    println!("    [1] Interactive TUI (default)");
    println!("    [2] Telegram bot");
    println!("    [3] Discord bot");
    println!("    [4] Background daemon");
    print!("  Choice [1]: ");
    io::stdout().flush()?;
    let mut choice = String::new();
    stdin.read_line(&mut choice)?;
    let choice = choice.trim();
    println!();

    match choice {
        "2" => {
            print!("  Telegram bot token (from @BotFather): ");
            io::stdout().flush()?;
            let mut token = String::new();
            stdin.read_line(&mut token)?;
            let token = token.trim();
            if !token.is_empty() {
                println!();
                println!("  ✅ Telegram bot configured!");
                println!();
                println!("  Start the bot:");
                println!("    axga --telegram --key {token} --provider {provider}");
                println!();
                println!("  Or as a background daemon:");
                println!("    axga --telegram --key {token} --provider {provider} --daemon");
            }
        }
        "3" => {
            println!();
            println!("  ═══ How to get a Discord Bot Token ═══");
            println!("  1. Go to https://discord.com/developers/applications");
            println!("  2. Click 'New Application' → name it (e.g. 'axga')");
            println!("  3. Go to 'Bot' tab → 'Add Bot' → 'Reset Token' → COPY");
            println!("  4. Enable 'Message Content Intent' (required!)");
            println!("  5. Go to 'OAuth2' → 'URL Generator'");
            println!("     - Scope: 'bot'");
            println!("     - Permissions: Send Messages, Read Message History");
            println!("  6. Open the generated URL → invite bot to your server");
            println!();
            print!("  Paste your bot token here: ");
            io::stdout().flush()?;
            let mut token = String::new();
            stdin.read_line(&mut token)?;
            let token = token.trim();
            if !token.is_empty() {
                println!();
                println!("  ✅ Discord bot configured!");
                println!();
                println!("  Start the bot:");
                println!("    axga --discord --key {token} --provider {provider}");
                println!();
                println!("  Make sure your bot is invited to a server first!");
            }
        }
        "4" => {
            println!("  ✅ Background daemon mode selected.");
            println!();
            println!("  Install systemd service:");
            println!("    sudo cp scripts/axga-bot.service /etc/systemd/system/");
            println!("    sudo systemctl enable --now axga-bot");
            println!();
            println!("  Or run in background:");
            println!("    nohup axga --telegram --key TOKEN --provider {provider} &");
        }
        _ => {
            println!("  ✅ All set! You can now run:");
            println!("    axga --provider {provider}");
            println!();
            println!("  Or for more options:");
            println!("    axga --help");
        }
    }

    Ok(())
}
