use serde::Deserialize;
use std::time::Duration;

pub const LOGIN_TIMEOUT_SECS: u64 = 300;
const POLL_INTERVAL_SECS: u64 = 2;

#[derive(Debug, Clone, Default)]
pub struct CodexSettings {
    pub model: Option<String>,
    pub full_auto: Option<bool>,
    pub reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ClaudeSettings {
    pub model: Option<String>,
    pub max_turns: Option<u32>,
    pub permission_mode: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LoginOutcome {
    pub api_key: String,
    pub default_model: Option<String>,
    pub codex: CodexSettings,
    pub claude: ClaudeSettings,
}

#[derive(Deserialize)]
struct PollData {
    status: String,
    key: Option<String>,
    #[serde(default)]
    model: Option<String>,
    // codex fields
    #[serde(default)]
    codex_model: Option<String>,
    #[serde(default)]
    codex_full_auto: Option<bool>,
    #[serde(default)]
    codex_reasoning_effort: Option<String>,
    // claude fields
    #[serde(default)]
    claude_model: Option<String>,
    #[serde(default)]
    claude_max_turns: Option<u32>,
    #[serde(default)]
    claude_permission_mode: Option<String>,
}

#[derive(Deserialize)]
struct PollResponse {
    success: bool,
    data: Option<PollData>,
}

pub struct LoginService;

impl LoginService {
    pub fn generate_token() -> String {
        uuid::Uuid::new_v4().simple().to_string()
    }

    pub fn login_url(base_url: &str, token: &str) -> String {
        format!("{}/code/token?token={}", base_url.trim_end_matches('/'), token)
    }

    pub async fn poll_for_key(base_url: &str, token: &str) -> Result<LoginOutcome, String> {
        let poll_url = format!(
            "{}/api/cli/poll?token={}",
            base_url.trim_end_matches('/'),
            token
        );
        let client = reqwest::Client::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(LOGIN_TIMEOUT_SECS);

        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(format!(
                    "Login timed out after {} seconds.",
                    LOGIN_TIMEOUT_SECS
                ));
            }

            if let Ok(resp) = client.get(&poll_url).send().await {
                if let Ok(body) = resp.json::<PollResponse>().await {
                    if body.success {
                        if let Some(data) = body.data {
                            if data.status == "done" {
                                if let Some(key) = data.key.filter(|k| !k.is_empty()) {
                                    return Ok(LoginOutcome {
                                        api_key: key,
                                        default_model: data
                                            .model
                                            .filter(|v| !v.is_empty()),
                                        codex: CodexSettings {
                                            model: data.codex_model.filter(|v| !v.is_empty()),
                                            full_auto: data.codex_full_auto,
                                            reasoning_effort: data.codex_reasoning_effort.filter(|v| !v.is_empty()),
                                        },
                                        claude: ClaudeSettings {
                                            model: data.claude_model.filter(|v| !v.is_empty()),
                                            max_turns: data.claude_max_turns,
                                            permission_mode: data.claude_permission_mode.filter(|v| !v.is_empty()),
                                        },
                                    });
                                }
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_32_hex_chars() {
        let t = LoginService::generate_token();
        assert_eq!(t.len(), 32);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn tokens_are_unique() {
        assert_ne!(LoginService::generate_token(), LoginService::generate_token());
    }

    #[test]
    fn login_url_format() {
        assert_eq!(
            LoginService::login_url("https://baizor.com", "abc"),
            "https://baizor.com/code/token?token=abc"
        );
        assert_eq!(
            LoginService::login_url("https://baizor.com/", "abc"),
            "https://baizor.com/code/token?token=abc"
        );
    }
}
