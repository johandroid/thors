use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Missing required configuration: {0}")]
    MissingConfig(String),
    #[error("Environment variable {0} is set but empty")]
    EmptyEnv(String),
    #[error("Environment variable {0} contains invalid Unicode")]
    InvalidEnv(String),
    #[error("Secret file {0} is empty")]
    EmptySecret(String),
    #[error("Failed to read secret file {path}: {source}")]
    SecretRead {
        path: String,
        source: std::io::Error,
    },
    #[error("Server error: {0}")]
    Server(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AppError>;
