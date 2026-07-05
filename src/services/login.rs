use serde::Deserialize;
use std::time::Duration;

pub const LOGIN_TIMEOUT_SECS: u64 = 300;
const POLL_INTERVAL_SECS: u64 = 2;

#[derive(Debug, Clone)]
pub struct LoginOutcome {
    pub api_key: String,
    pub default_model: Option<String>,
}

#[derive(Deserialize)]
struct PollData {
    status: String,
    key: Option<String>,
    #[serde(default)]
    default_model: Option<String>,
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
                                            .default_model
                                            .filter(|v| !v.is_empty()),
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
