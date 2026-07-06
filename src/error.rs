use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Auth error: please run `huayu login` to re-authenticate")]
    Auth,

    #[error("Tool not found: `{0}` is not installed or not in PATH")]
    ToolNotFound(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Message(String),
}
