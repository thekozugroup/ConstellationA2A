use thiserror::Error;

/// Top-level error type for the Constellation SDK.
#[derive(Debug, Error)]
pub enum ConstellationError {
    #[error("Matrix SDK error: {0}")]
    Matrix(#[from] matrix_sdk::Error),

    #[error("Matrix HTTP error: {0}")]
    MatrixHttp(#[from] matrix_sdk::HttpError),

    #[error("Matrix ID parse error: {0}")]
    MatrixId(#[from] matrix_sdk::ruma::IdParseError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Room error: {0}")]
    Room(String),

    #[error("Message error: {0}")]
    Message(String),

    #[error("Task error: {0}")]
    Task(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, ConstellationError>;
