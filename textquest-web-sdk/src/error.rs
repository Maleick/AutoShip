use std::fmt;

/// Result type for TextQuest SDK operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for TextQuest SDK operations
#[derive(Debug)]
pub enum Error {
    /// HTTP client error
    Http(String),
    /// Serialization/deserialization error
    Serde(String),
    /// API returned an error response
    ApiError { status: u16, message: String },
    /// Connection error
    Connection(String),
    /// Invalid configuration
    Configuration(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Http(msg) => write!(f, "HTTP error: {}", msg),
            Error::Serde(msg) => write!(f, "Serialization error: {}", msg),
            Error::ApiError { status, message } => {
                write!(f, "API error ({}): {}", status, message)
            }
            Error::Connection(msg) => write!(f, "Connection error: {}", msg),
            Error::Configuration(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Http(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serde(err.to_string())
    }
}
