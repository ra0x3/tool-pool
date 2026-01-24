//! Error types for the policy system

use thiserror::Error;

/// Result type alias for policy operations
pub type Result<T> = std::result::Result<T, PolicyError>;

/// Main error type for policy operations
#[derive(Error, Debug)]
pub enum PolicyError {
    /// Policy parsing error
    #[error("Failed to parse policy: {0}")]
    ParseError(String),

    /// Invalid policy format
    #[error("Invalid policy format: {0}")]
    InvalidFormat(String),

    /// Permission denied
    #[error("Permission denied: {action} on {resource}")]
    PermissionDenied {
        /// Action that was denied
        action: String,
        /// Resource that was being accessed
        resource: String,
    },

    /// Resource limit exceeded
    #[error("Resource limit exceeded: {resource} (limit: {limit}, requested: {requested})")]
    ResourceLimitExceeded {
        /// Resource type that exceeded limit
        resource: String,
        /// The configured limit
        limit: String,
        /// The amount requested
        requested: String,
    },

    /// Runtime error
    #[error("Runtime error: {0}")]
    RuntimeError(String),

    /// Extension not found
    #[error("Extension not found: {0}")]
    ExtensionNotFound(String),

    /// Invalid extension configuration
    #[error("Invalid extension configuration for {extension}: {message}")]
    InvalidExtension {
        /// Extension name
        extension: String,
        /// Error message
        message: String,
    },

    /// Policy validation error
    #[error("Policy validation failed: {0}")]
    ValidationError(String),

    /// Incompatible runtime
    #[error("Runtime {runtime} is not compatible with configuration")]
    IncompatibleRuntime {
        /// Runtime name
        runtime: String,
    },

    /// Cache error
    #[error("Cache operation failed: {0}")]
    CacheError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// YAML parsing error
    #[error("YAML parsing error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    /// JSON parsing error
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Glob pattern error
    #[error("Glob pattern error: {0}")]
    GlobError(String),

    /// Generic error wrapper
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<globset::Error> for PolicyError {
    fn from(err: globset::Error) -> Self {
        PolicyError::GlobError(err.to_string())
    }
}

impl PolicyError {
    /// Create a parse error with context
    pub fn parse<S: Into<String>>(msg: S) -> Self {
        PolicyError::ParseError(msg.into())
    }

    /// Create a validation error with context
    pub fn validation<S: Into<String>>(msg: S) -> Self {
        PolicyError::ValidationError(msg.into())
    }

    /// Create a runtime error with context
    pub fn runtime<S: Into<String>>(msg: S) -> Self {
        PolicyError::RuntimeError(msg.into())
    }
}
