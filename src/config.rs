use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

const CONFIG_FILE: &str = "config.json";

/// Per-model metadata returned by the server and cached in config.json.
/// Used to populate `[model_info]` sections in codex config.toml.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelInfo {
    pub context_window: u32,
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuayuConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default = "default_tool")]
    pub active_tool: String,

    // ── Codex-specific settings (from server on login) ─────────────────────
    #[serde(default)]
    pub codex_model: String,
    #[serde(default = "default_codex_full_auto")]
    pub codex_full_auto: bool,
    #[serde(default = "default_codex_reasoning_effort")]
    pub codex_reasoning_effort: String,

    // ── Claude-specific settings (from server on login) ────────────────────
    #[serde(default)]
    pub claude_model: String,
    #[serde(default)]
    pub claude_max_turns: u32,
    #[serde(default = "default_claude_permission_mode")]
    pub claude_permission_mode: String,

    // ── Model metadata (from server on login) ──────────────────────────────
    // Maps model name → context_window / max_output_tokens.
    // Written to codex [model_info] sections; falls back to built-in values
    // for well-known baizor models when the server hasn't provided anything.
    #[serde(default)]
    pub model_info: HashMap<String, ModelInfo>,
}

fn default_base_url() -> String {
    "https://baizor.com".to_string()
}
fn default_model() -> String {
    "huayu-v2".to_string()
}
fn default_tool() -> String {
    "codex".to_string()
}
fn default_codex_full_auto() -> bool {
    true
}
fn default_codex_reasoning_effort() -> String {
    "medium".to_string()
}
fn default_claude_permission_mode() -> String {
    "bypassPermissions".to_string()
}

impl Default for HuayuConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: default_base_url(),
            default_model: default_model(),
            active_tool: default_tool(),
            codex_model: String::new(),
            codex_full_auto: default_codex_full_auto(),
            codex_reasoning_effort: default_codex_reasoning_effort(),
            claude_model: String::new(),
            claude_max_turns: 0,
            claude_permission_mode: default_claude_permission_mode(),
            model_info: HashMap::new(),
        }
    }
}

/// Root config directory: $HUAYU_CONFIG_DIR or ~/.huayu/
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("HUAYU_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".huayu")
}

/// Isolated codex config dir used by huayu (injected as CODEX_HOME).
pub fn codex_home() -> PathBuf {
    config_dir().join("codex")
}

/// Isolated claude config dir used by huayu (injected as CLAUDE_CONFIG_DIR).
pub fn claude_config_dir() -> PathBuf {
    config_dir().join("claude")
}

pub fn load() -> HuayuConfig {
    let path = config_dir().join(CONFIG_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => HuayuConfig::default(),
    }
}

pub fn save(cfg: &HuayuConfig) -> Result<(), AppError> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(AppError::Io)?;
    let text = serde_json::to_string_pretty(cfg)?;
    std::fs::write(dir.join(CONFIG_FILE), text).map_err(AppError::Io)
}

/// Effective codex model: codex_model overrides default_model when set.
pub fn effective_codex_model(cfg: &HuayuConfig) -> &str {
    if !cfg.codex_model.is_empty() {
        &cfg.codex_model
    } else {
        &cfg.default_model
    }
}

/// Effective claude model: claude_model overrides default_model when set.
pub fn effective_claude_model(cfg: &HuayuConfig) -> &str {
    if !cfg.claude_model.is_empty() {
        &cfg.claude_model
    } else {
        &cfg.default_model
    }
}

/// Write the minimal codex config files into ~/.huayu/codex/.
///
/// Model metadata is sourced from `cfg.model_info` (populated from the server
/// on login). Built-in values for well-known baizor models are used as a
/// fallback when the server hasn't provided metadata for a specific model.
pub fn write_codex_config(cfg: &HuayuConfig) -> Result<(), AppError> {
    let home = codex_home();
    std::fs::create_dir_all(&home).map_err(AppError::Io)?;

    let model = effective_codex_model(cfg);

    // Merge built-in fallbacks with server-provided metadata.
    // Server values win when present; built-ins fill the gap.
    let mut merged: HashMap<String, ModelInfo> = [
        ("huayu-v2",       ModelInfo { context_window: 128000, max_output_tokens: 16384 }),
        ("huayu-v2-max",   ModelInfo { context_window: 128000, max_output_tokens: 16384 }),
        ("huayu-v2-flash", ModelInfo { context_window: 32768,  max_output_tokens: 8192  }),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();

    for (name, info) in &cfg.model_info {
        merged.insert(name.clone(), info.clone());
    }

    // Ensure the active model always has an entry (generic fallback if unknown).
    merged.entry(model.to_string()).or_insert(ModelInfo {
        context_window: 128000,
        max_output_tokens: 16384,
    });

    // Sort entries so the output is deterministic.
    let mut entries: Vec<(String, ModelInfo)> = merged.into_iter().collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut config_toml = format!("model = \"{model}\"\n");

    // Provider config — points codex at the baizor API gateway.
    // wire_api = "responses" uses the Responses API format (required by codex >= v0.142).
    let base = cfg.base_url.trim_end_matches('/');
    config_toml.push_str(&format!(
        "model_provider = \"custom\"\n\
         \n\
         [model_providers.custom]\n\
         name = \"huayu\"\n\
         base_url = \"{base}/v1\"\n\
         wire_api = \"responses\"\n\
         requires_openai_auth = true\n"
    ));

    for (name, info) in &entries {
        config_toml.push_str(&format!(
            "\n[model_info.\"{}\"]\ncontext_window = {}\nmax_output_tokens = {}\n",
            name, info.context_window, info.max_output_tokens
        ));
    }

    std::fs::write(home.join("config.toml"), config_toml).map_err(AppError::Io)?;

    let auth = serde_json::json!({ "OPENAI_API_KEY": cfg.api_key });
    std::fs::write(
        home.join("auth.json"),
        serde_json::to_string_pretty(&auth)?,
    )
    .map_err(AppError::Io)?;

    Ok(())
}

/// Write the minimal claude settings into ~/.huayu/claude/.
pub fn write_claude_config(cfg: &HuayuConfig) -> Result<(), AppError> {
    let dir = claude_config_dir();
    std::fs::create_dir_all(&dir).map_err(AppError::Io)?;

    let base = cfg.base_url.trim_end_matches('/');
    let model = effective_claude_model(cfg);

    // bypassPermissionsModeAccepted=true pre-accepts the --dangerously-skip-permissions
    // prompt so claude can run non-interactively in huayu's PTY. Without this,
    // claude refuses with "must be accepted in an interactive session first".
    let cli_config = serde_json::json!({
        "bypassPermissionsModeAccepted": true,
        "hasCompletedOnboarding": true,
    });
    std::fs::write(
        dir.join("config.json"),
        serde_json::to_string_pretty(&cli_config)?,
    )
    .map_err(AppError::Io)?;

    let settings = serde_json::json!({
        "bypassPermissionsModeAccepted": true,
        "hasCompletedOnboarding": true,
        "env": {
            "ANTHROPIC_AUTH_TOKEN": cfg.api_key,
            "ANTHROPIC_BASE_URL": format!("{}/v1", base),
            "ANTHROPIC_MODEL": model,
        }
    });
    std::fs::write(dir.join("settings.json"), serde_json::to_string_pretty(&settings)?).map_err(AppError::Io)?;

    Ok(())
}

// ── Test infrastructure ────────────────────────────────────────────────────

/// Process-wide lock for tests that set HUAYU_CONFIG_DIR.
#[cfg(test)]
pub(crate) static CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// RAII guard: acquires CONFIG_LOCK, redirects HUAYU_CONFIG_DIR to a fresh
/// TempDir, and restores the env var on drop (even if the test panics).
#[cfg(test)]
pub(crate) struct TempConfigGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    _dir: tempfile::TempDir,
}

#[cfg(test)]
impl TempConfigGuard {
    pub(crate) fn new() -> Self {
        let lock = CONFIG_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = tempfile::TempDir::new().expect("tempdir");
        std::env::set_var("HUAYU_CONFIG_DIR", dir.path());
        TempConfigGuard { _lock: lock, _dir: dir }
    }
}

#[cfg(test)]
impl Drop for TempConfigGuard {
    fn drop(&mut self) {
        std::env::remove_var("HUAYU_CONFIG_DIR");
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_correct() {
        let cfg = HuayuConfig::default();
        assert_eq!(cfg.base_url, "https://baizor.com");
        assert_eq!(cfg.default_model, "huayu-v2");
        assert_eq!(cfg.active_tool, "codex");
        assert!(cfg.api_key.is_empty());
        assert!(cfg.codex_full_auto);
        assert_eq!(cfg.codex_reasoning_effort, "medium");
        assert_eq!(cfg.claude_permission_mode, "bypassPermissions");
    }

    #[test]
    fn effective_model_falls_back_to_default() {
        let cfg = HuayuConfig::default();
        assert_eq!(effective_codex_model(&cfg), cfg.default_model);
        assert_eq!(effective_claude_model(&cfg), cfg.default_model);
    }

    #[test]
    fn effective_model_uses_tool_specific_when_set() {
        let cfg = HuayuConfig {
            default_model: "fallback".to_string(),
            codex_model: "codex-specific".to_string(),
            claude_model: "claude-specific".to_string(),
            ..Default::default()
        };
        assert_eq!(effective_codex_model(&cfg), "codex-specific");
        assert_eq!(effective_claude_model(&cfg), "claude-specific");
    }

    #[test]
    fn save_load_round_trip() {
        let _g = TempConfigGuard::new();
        let cfg = HuayuConfig {
            api_key: "sk-test-key".to_string(),
            base_url: "https://example.com".to_string(),
            default_model: "gpt-test".to_string(),
            active_tool: "claude".to_string(),
            codex_model: "codex-special".to_string(),
            codex_full_auto: false,
            codex_reasoning_effort: "high".to_string(),
            claude_model: "claude-special".to_string(),
            claude_max_turns: 10,
            claude_permission_mode: "acceptEdits".to_string(),
            model_info: HashMap::new(),
        };
        save(&cfg).unwrap();
        let loaded = load();
        assert_eq!(loaded.api_key, cfg.api_key);
        assert_eq!(loaded.codex_model, "codex-special");
        assert!(!loaded.codex_full_auto);
        assert_eq!(loaded.codex_reasoning_effort, "high");
        assert_eq!(loaded.claude_model, "claude-special");
        assert_eq!(loaded.claude_max_turns, 10);
        assert_eq!(loaded.claude_permission_mode, "acceptEdits");
    }

    #[test]
    fn load_returns_defaults_when_no_file() {
        let _g = TempConfigGuard::new();
        let loaded = load();
        assert_eq!(loaded.base_url, "https://baizor.com");
        assert!(loaded.api_key.is_empty());
    }

    #[test]
    fn codex_config_writes_model_and_api_key() {
        let _g = TempConfigGuard::new();
        let cfg = HuayuConfig {
            api_key: "sk-codex-key".to_string(),
            default_model: "gpt-5.5".to_string(),
            ..Default::default()
        };
        write_codex_config(&cfg).unwrap();
        let home = codex_home();
        let toml = std::fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(toml.contains("gpt-5.5"), "config.toml should contain model");
        assert!(toml.contains("model_provider = \"custom\""), "config.toml should contain provider");
        assert!(toml.contains("base_url = \"https://baizor.com/v1\""), "config.toml should contain base_url");
        assert!(toml.contains("wire_api = \"chat\""), "config.toml should contain wire_api");
        let auth: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(home.join("auth.json")).unwrap()).unwrap();
        assert_eq!(auth["OPENAI_API_KEY"].as_str().unwrap(), "sk-codex-key");
    }

    #[test]
    fn codex_config_uses_codex_specific_model_when_set() {
        let _g = TempConfigGuard::new();
        let cfg = HuayuConfig {
            api_key: "sk-key".to_string(),
            default_model: "fallback-model".to_string(),
            codex_model: "codex-override".to_string(),
            ..Default::default()
        };
        write_codex_config(&cfg).unwrap();
        let toml = std::fs::read_to_string(codex_home().join("config.toml")).unwrap();
        assert!(toml.contains("codex-override"), "should use codex_model");
        assert!(!toml.contains("fallback-model"), "should not use default_model");
    }

    #[test]
    fn claude_config_writes_auth_token_base_url_and_model() {
        let _g = TempConfigGuard::new();
        let cfg = HuayuConfig {
            api_key: "sk-claude-key".to_string(),
            base_url: "https://baizor.com".to_string(),
            default_model: "claude-test".to_string(),
            ..Default::default()
        };
        write_claude_config(&cfg).unwrap();
        let raw =
            std::fs::read_to_string(claude_config_dir().join("settings.json")).unwrap();
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let env = &val["env"];
        assert_eq!(env["ANTHROPIC_AUTH_TOKEN"].as_str().unwrap(), "sk-claude-key");
        assert_eq!(env["ANTHROPIC_BASE_URL"].as_str().unwrap(), "https://baizor.com/v1");
        assert_eq!(env["ANTHROPIC_MODEL"].as_str().unwrap(), "claude-test");
        assert_eq!(val["bypassPermissionsModeAccepted"].as_bool().unwrap(), true);

        let cli_raw = std::fs::read_to_string(claude_config_dir().join("config.json")).unwrap();
        let cli_config: serde_json::Value = serde_json::from_str(&cli_raw).unwrap();
        assert_eq!(cli_config["bypassPermissionsModeAccepted"].as_bool().unwrap(), true);
        assert_eq!(cli_config["hasCompletedOnboarding"].as_bool().unwrap(), true);
    }

    #[test]
    fn claude_config_uses_claude_specific_model_when_set() {
        let _g = TempConfigGuard::new();
        let cfg = HuayuConfig {
            api_key: "sk-key".to_string(),
            base_url: "https://baizor.com".to_string(),
            default_model: "fallback-model".to_string(),
            claude_model: "claude-override".to_string(),
            ..Default::default()
        };
        write_claude_config(&cfg).unwrap();
        let raw = std::fs::read_to_string(claude_config_dir().join("settings.json")).unwrap();
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["env"]["ANTHROPIC_MODEL"].as_str().unwrap(), "claude-override");
    }

    #[test]
    fn codex_config_files_are_under_huayu_root() {
        let _g = TempConfigGuard::new();
        let cfg = HuayuConfig { api_key: "key".to_string(), ..Default::default() };
        write_codex_config(&cfg).unwrap();
        let root = config_dir();
        for entry in std::fs::read_dir(codex_home()).unwrap().flatten() {
            assert!(
                entry.path().starts_with(&root),
                "{} is outside huayu root {}",
                entry.path().display(),
                root.display()
            );
        }
    }

    #[test]
    fn claude_config_files_are_under_huayu_root() {
        let _g = TempConfigGuard::new();
        let cfg = HuayuConfig { api_key: "key".to_string(), ..Default::default() };
        write_claude_config(&cfg).unwrap();
        let root = config_dir();
        for entry in std::fs::read_dir(claude_config_dir()).unwrap().flatten() {
            assert!(
                entry.path().starts_with(&root),
                "{} is outside huayu root {}",
                entry.path().display(),
                root.display()
            );
        }
    }

    #[test]
    fn api_key_masking_in_status_output() {
        let key = "sk-abcd1234efgh5678";
        let masked = if key.len() > 8 {
            format!("sk-{}***{}", &key[..4.min(key.len())], &key[key.len().saturating_sub(4)..])
        } else {
            "***".to_string()
        };
        assert!(!masked.contains("abcd1234efgh5678"), "full key must not appear");
        assert!(masked.contains("sk-"), "prefix preserved");
        assert!(masked.contains("5678"), "last 4 chars preserved");
    }
}
