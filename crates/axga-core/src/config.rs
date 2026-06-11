//! Configuration file support (~/.config/axga/config.toml).
//!
//! Falls back to CLI args and env vars if config file is missing.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub provider: ProviderSection,
    #[serde(default)]
    pub session: SessionSection,
    #[serde(default)]
    pub telegram: Option<TelegramSection>,
    #[serde(default)]
    pub tools: ToolsSection,
    #[serde(default)]
    pub memory: MemorySection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderSection {
    pub provider_type: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSection {
    #[serde(default = "default_sessions_dir")]
    pub dir: String,
    #[serde(default = "default_true")]
    pub auto_save: bool,
}

impl Default for SessionSection {
    fn default() -> Self {
        Self { dir: default_sessions_dir(), auto_save: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramSection {
    pub token: String,
    #[serde(default)]
    pub allowed_users: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolsSection {
    #[serde(default = "default_true")]
    pub read_file: bool,
    #[serde(default = "default_true")]
    pub write_file: bool,
    #[serde(default = "default_true")]
    pub list_directory: bool,
    #[serde(default = "default_true")]
    pub execute_shell: bool,
    #[serde(default = "default_true")]
    pub grep: bool,
    #[serde(default = "default_true")]
    pub glob: bool,
    #[serde(default = "default_true")]
    pub diff: bool,
    #[serde(default = "default_true")]
    pub memctrl: bool,
    #[serde(default)]
    pub web_search: bool,
    #[serde(default)]
    pub fetch_url: bool,
    #[serde(default = "default_true")]
    pub task_list: bool,
    #[serde(default = "default_true")]
    pub task_output: bool,
    #[serde(default = "default_true")]
    pub task_stop: bool,
    #[serde(default)]
    pub image_vision: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySection {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_memctrl_path")]
    pub memctrl_path: String,
}

impl Default for MemorySection {
    fn default() -> Self {
        Self { enabled: true, memctrl_path: default_memctrl_path() }
    }
}

fn default_sessions_dir() -> String { "~/.config/axga/sessions".into() }
fn default_true() -> bool { true }
fn default_memctrl_path() -> String { "memctrl".into() }

/// Load config from standard locations.
pub fn load_config() -> Option<Config> {
    let paths = [
        dirs_config().join("axga").join("config.toml"),
        PathBuf::from("axga.toml"),
        PathBuf::from(".axga.toml"),
    ];

    for path in &paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(config) = toml::from_str::<Config>(&content) {
                tracing::info!(path = %path.display(), "config loaded");
                return Some(config);
            }
        }
    }
    None
}

/// Save a config file.
pub fn save_config(config: &Config) -> std::io::Result<()> {
    let dir = dirs_config().join("axga");
    std::fs::create_dir_all(&dir)?;
    let content = toml::to_string_pretty(config).map_err(std::io::Error::other)?;
    std::fs::write(dir.join("config.toml"), content)
}

fn dirs_config() -> PathBuf {
    if let Ok(dir) = std::env::var("AXGA_CONFIG_DIR") { return PathBuf::from(dir); }
    #[cfg(target_os = "linux")] { PathBuf::from(std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", std::env::var("HOME").unwrap_or_default()))) }
    #[cfg(target_os = "macos")] { PathBuf::from(format!("{}/Library/Application Support", std::env::var("HOME").unwrap_or_default())) }
    #[cfg(target_os = "windows")] { PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into())) }
}
