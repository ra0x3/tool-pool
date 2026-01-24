//! Error types for the configuration system

use thiserror::Error;

/// Result type alias
pub type Result<T> = std::result::Result<T, ConfigError>;

/// Configuration error type
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration parsing error: {0}")]
    ParseError(String),

    #[error("Configuration validation error: {0}")]
    ValidationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: String, found: String },

    #[error("Invalid runtime type: {0}")]
    InvalidRuntimeType(String),

    #[error("Invalid transport type: {0}")]
    InvalidTransportType(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid value for {field}: {value}")]
    InvalidValue { field: String, value: String },

    #[error("Policy error: {0}")]
    PolicyError(#[from] mcpkit_rs_policy::PolicyError),

    #[error("Configuration not found at path: {0}")]
    NotFound(String),

    #[error("Merge conflict: {0}")]
    MergeConflict(String),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}
