use std::fmt;

#[derive(Debug)]
pub enum GiantError {
    ConfigError(String),
    CertError(String),
    ProxyError(String),
    RuleError(String),
    IoError(std::io::Error),
    ApiError(String),
}

impl fmt::Display for GiantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GiantError::ConfigError(msg) => write!(f, "config error: {}", msg),
            GiantError::CertError(msg) => write!(f, "cert error: {}", msg),
            GiantError::ProxyError(msg) => write!(f, "proxy error: {}", msg),
            GiantError::RuleError(msg) => write!(f, "rule error: {}", msg),
            GiantError::IoError(err) => write!(f, "io error: {}", err),
            GiantError::ApiError(msg) => write!(f, "api error: {}", msg),
        }
    }
}

impl std::error::Error for GiantError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GiantError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for GiantError {
    fn from(err: std::io::Error) -> Self {
        GiantError::IoError(err)
    }
}

impl From<regex::Error> for GiantError {
    fn from(err: regex::Error) -> Self {
        GiantError::RuleError(err.to_string())
    }
}

impl From<toml::de::Error> for GiantError {
    fn from(err: toml::de::Error) -> Self {
        GiantError::ConfigError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, GiantError>;
