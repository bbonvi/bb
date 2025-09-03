use thiserror::Error;

/// Domain-specific errors for CLI operations
#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum CliError {
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    #[error("Operation cancelled by user")]
    UserCancelled,

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("Validation error: {field}: {message}")]
    Validation { field: String, message: String },

    #[error("Operation not supported: {operation}")]
    NotSupported { operation: String },

    #[error("Database operation failed: {message}")]
    Database { message: String },

    #[error("Metadata operation failed: {message}")]
    Metadata { message: String },

    #[error("Storage operation failed: {message}")]
    Storage { message: String },

    #[error("Network operation failed: {message}")]
    Network { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl CliError {
    /// Create an invalid input error
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput { message: message.into() }
    }

    /// Create a validation error
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation { 
            field: field.into(), 
            message: message.into() 
        }
    }

    /// Create a configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration { message: message.into() }
    }

    /// Create a not supported error
    pub fn not_supported(operation: impl Into<String>) -> Self {
        Self::NotSupported { operation: operation.into() }
    }

    /// Create a database error
    pub fn database(message: impl Into<String>) -> Self {
        Self::Database { message: message.into() }
    }

    /// Create a metadata error
    pub fn metadata(message: impl Into<String>) -> Self {
        Self::Metadata { message: message.into() }
    }

    /// Create a storage error
    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage { message: message.into() }
    }

    /// Create a network error
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network { message: message.into() }
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal { message: message.into() }
    }
}

/// Result type for CLI operations
pub type CliResult<T> = Result<T, CliError>;

impl From<anyhow::Error> for CliError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal { message: err.to_string() }
    }
}

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => Self::Storage { 
                message: "File not found".to_string() 
            },
            std::io::ErrorKind::PermissionDenied => Self::Storage { 
                message: "Permission denied".to_string() 
            },
            _ => Self::Storage { message: err.to_string() }
        }
    }
}

impl From<serde_json::Error> for CliError {
    fn from(err: serde_json::Error) -> Self {
        Self::InvalidInput { 
            message: format!("JSON error: {}", err) 
        }
    }
}
