//! Error types for beanweb-config

use thiserror::Error;
use serde::{Deserialize, Serialize};

/// Error codes for configuration errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfigErrorCode {
    /// File not found
    FileNotFound,
    /// Invalid YAML format
    InvalidYaml,
    /// Missing required field
    MissingField,
    /// Invalid field value
    InvalidValue,
    /// IO error
    IoError,
    /// Validation error
    ValidationError,
}

impl std::fmt::Display for ConfigErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigErrorCode::FileNotFound => write!(f, "FILE_NOT_FOUND"),
            ConfigErrorCode::InvalidYaml => write!(f, "INVALID_YAML"),
            ConfigErrorCode::MissingField => write!(f, "MISSING_FIELD"),
            ConfigErrorCode::InvalidValue => write!(f, "INVALID_VALUE"),
            ConfigErrorCode::IoError => write!(f, "IO_ERROR"),
            ConfigErrorCode::ValidationError => write!(f, "VALIDATION_ERROR"),
        }
    }
}

/// Detailed error information for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigErrorDetails {
    /// Error code
    pub code: ConfigErrorCode,
    /// Human-readable message
    pub message: String,
    /// Field path (for field-specific errors)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Expected value (for validation errors)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// Actual value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
    /// Suggestions for resolution
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
}

impl ConfigErrorDetails {
    /// Create a new error detail
    pub fn new(code: ConfigErrorCode, message: String) -> Self {
        Self {
            code,
            message,
            field: None,
            expected: None,
            actual: None,
            suggestions: vec![],
        }
    }

    /// Add field information
    pub fn with_field(mut self, field: String) -> Self {
        self.field = Some(field);
        self
    }

    /// Add expected/actual values
    pub fn with_values(mut self, expected: String, actual: String) -> Self {
        self.expected = Some(expected);
        self.actual = Some(actual);
        self
    }

    /// Add a suggestion
    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        self.suggestions.push(suggestion);
        self
    }
}

impl std::fmt::Display for ConfigErrorDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        if let Some(ref field) = self.field {
            write!(f, "\nField: {}", field)?;
        }
        if let (Some(ref expected), Some(ref actual)) = (self.expected.as_ref(), self.actual.as_ref()) {
            write!(f, "\nExpected: {}, Actual: {}", expected, actual)?;
        }
        if !self.suggestions.is_empty() {
            write!(f, "\nSuggestions:")?;
            for suggestion in &self.suggestions {
                write!(f, "\n  - {}", suggestion)?;
            }
        }
        Ok(())
    }
}

/// Severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigErrorSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl std::fmt::Display for ConfigErrorSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigErrorSeverity::Debug => write!(f, "debug"),
            ConfigErrorSeverity::Info => write!(f, "info"),
            ConfigErrorSeverity::Warning => write!(f, "warning"),
            ConfigErrorSeverity::Error => write!(f, "error"),
            ConfigErrorSeverity::Critical => write!(f, "critical"),
        }
    }
}

/// Configuration error type
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Invalid YAML format")]
    InvalidYaml,

    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid field value: {field} - {reason}")]
    InvalidValue { field: String, reason: String },

    #[error("IO error occurred")]
    IoError,

    #[error("Validation error: {message}")]
    ValidationError { message: String },
}

impl ConfigError {
    /// Get the error code
    pub fn code(&self) -> ConfigErrorCode {
        match self {
            ConfigError::FileNotFound { .. } => ConfigErrorCode::FileNotFound,
            ConfigError::InvalidYaml => ConfigErrorCode::InvalidYaml,
            ConfigError::MissingField { .. } => ConfigErrorCode::MissingField,
            ConfigError::InvalidValue { .. } => ConfigErrorCode::InvalidValue,
            ConfigError::IoError => ConfigErrorCode::IoError,
            ConfigError::ValidationError { .. } => ConfigErrorCode::ValidationError,
        }
    }

    /// Get the severity level
    pub fn severity(&self) -> ConfigErrorSeverity {
        match self {
            ConfigError::FileNotFound { .. } => ConfigErrorSeverity::Error,
            ConfigError::InvalidYaml => ConfigErrorSeverity::Error,
            ConfigError::MissingField { .. } => ConfigErrorSeverity::Error,
            ConfigError::InvalidValue { .. } => ConfigErrorSeverity::Error,
            ConfigError::IoError { .. } => ConfigErrorSeverity::Error,
            ConfigError::ValidationError { .. } => ConfigErrorSeverity::Warning,
        }
    }

    /// Convert to detailed error info
    pub fn to_details(&self) -> ConfigErrorDetails {
        let mut details = ConfigErrorDetails::new(
            self.code(),
            self.to_string(),
        );

        match self {
            ConfigError::FileNotFound { path: _ } => {
                details = details.with_suggestion(
                    "Check if the config file path is correct.".to_string()
                );
                details = details.with_suggestion(
                    "Use --config flag to specify the config file path.".to_string()
                );
            }
            ConfigError::MissingField { field } => {
                details = details.with_field(field.clone());
                details = details.with_suggestion(
                    format!("Add the '{}' field to your config file.", field)
                );
                details = details.with_suggestion(
                    "See the default_config.yaml for reference.".to_string()
                );
            }
            ConfigError::InvalidValue { field, reason } => {
                details = details.with_field(field.clone());
                details = details.with_suggestion(reason.clone());
                details = details.with_suggestion(
                    "Check the valid values for this field in the documentation.".to_string()
                );
            }
            ConfigError::ValidationError { message } => {
                details = details.with_suggestion(message.clone());
            }
            _ => {}
        }

        details
    }
}

/// Result type with ConfigError
pub type ConfigResult<T> = Result<T, ConfigError>;
