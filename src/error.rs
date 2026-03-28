use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("GitHub API error: {0}")]
    Api(#[from] ApiError),

    #[error("Authentication error: {0}")]
    Auth(#[from] AuthError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Filter error: {0}")]
    Filter(#[from] regex::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Terminal error: {0}")]
    Terminal(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("HTTP {status}: {message}")]
    HttpStatus { status: u16, message: String },

    #[error("GraphQL error: {message}")]
    GraphQL { message: String },

    #[error("Invalid response format")]
    InvalidResponse,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("GitHub token not found. Set GH_TOKEN environment variable or run 'gh auth login'")]
    TokenNotFound,

    #[error("Failed to read config file: {0}")]
    ConfigReadFailed(String),
}

pub type Result<T> = std::result::Result<T, Error>;
