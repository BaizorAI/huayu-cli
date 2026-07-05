use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuazhenConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default = "default_tool")]
    pub active_tool: String,
}

fn default_base_url() -> String {
    "https://baizor.com".to_string()
}
fn default_model() -> String {
    "huazhen-fable-5".to_string()
}
fn default_tool() -> String {
    "codex".to_string()
}

impl Default for HuazhenConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: default_base_url(),
            default_model: default_model(),
            active_tool: default_tool(),
        }
    }
}

/// Root config directory: $HUAZHEN_CONFIG_DIR or ~/.huazhen/
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("HUAZHEN_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".huazhen")
}

/// Isolated codex config dir used by huazhen (injected as CODEX_HOME).
pub fn codex_home() -> PathBuf {
    config_dir().join("codex")
}

/// Isolated claude config dir used by huazhen (injected as CLAUDE_CONFIG_DIR).
pub fn claude_config_dir() -> PathBuf {
    config_dir().join("claude")
}

pub fn load() -> HuazhenConfig {
    let path = config_dir().join(CONFIG_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => HuazhenConfig::default(),
    }
}

pub fn save(cfg: &HuazhenConfig) -> Result<(), AppError> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(AppError::Io)?;
    let text = serde_json::to_string_pretty(cfg)?;
    std::fs::write(dir.join(CONFIG_FILE), text).map_err(AppError::Io)
}

/// Write the minimal codex config files into ~/.huazhen/codex/.
pub fn write_codex_config(cfg: &HuazhenConfig) -> Result<(), AppError> {
    let home = codex_home();
    std::fs::create_dir_all(&home).map_err(AppError::Io)?;

    // Keep config.toml minimal. The openai base URL is set at spawn time via
    // `-c openai_base_url="..."` (top-level field, not model_providers.openai which
    // is reserved). --ignore-user-config is also passed so this file is never loaded.
    let config_toml = format!("model = \"{}\"\n", cfg.default_model);
    std::fs::write(home.join("config.toml"), config_toml).map_err(AppError::Io)?;

    // auth.json provides the API key; codex reads this from CODEX_HOME.
    let auth = serde_json::json!({ "OPENAI_API_KEY": cfg.api_key });
    std::fs::write(
        home.join("auth.json"),
        serde_json::to_string_pretty(&auth)?,
    )
    .map_err(AppError::Io)?;

    Ok(())
}

/// Write the minimal claude settings into ~/.huazhen/claude/.
pub fn write_claude_config(cfg: &HuazhenConfig) -> Result<(), AppError> {
    let dir = claude_config_dir();
    std::fs::create_dir_all(&dir).map_err(AppError::Io)?;

    let base = cfg.base_url.trim_end_matches('/');
    let settings = serde_json::json!({
        "env": {
            "ANTHROPIC_AUTH_TOKEN": cfg.api_key,
            "ANTHROPIC_BASE_URL": format!("{}/v1", base),
            "ANTHROPIC_MODEL": cfg.default_model,
        }
    });
    std::fs::write(dir.join("settings.json"), serde_json::to_string_pretty(&settings)?).map_err(AppError::Io)?;

    Ok(())
}
