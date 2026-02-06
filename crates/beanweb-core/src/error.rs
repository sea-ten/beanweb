//! Error types for beanweb-core
//!
//! This module provides comprehensive error handling for the core ledger
//! functionality, including error codes, detailed messages, and suggestions.

use thiserror::Error;
use serde::{Deserialize, Serialize};
use std::io;

/// Error codes for programmatic error handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// Ledger not loaded
    NotLoaded,
    /// Account not found
    AccountNotFound,
    /// Transaction not found
    TransactionNotFound,
    /// Parse error
    ParseError,
    /// Validation error
    ValidationError,
    /// IO error
    IoError,
    /// Configuration error
    ConfigError,
    /// File not found
    FileNotFound,
    /// Invalid data format
    InvalidFormat,
    /// Duplicate entry
    DuplicateEntry,
    /// Operation not supported
    NotSupported,
    /// Unauthorized access
    Unauthorized,
    /// Internal error
    InternalError,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::NotLoaded => write!(f, "NOT_LOADED"),
            ErrorCode::AccountNotFound => write!(f, "ACCOUNT_NOT_FOUND"),
            ErrorCode::TransactionNotFound => write!(f, "TRANSACTION_NOT_FOUND"),
            ErrorCode::ParseError => write!(f, "PARSE_ERROR"),
            ErrorCode::ValidationError => write!(f, "VALIDATION_ERROR"),
            ErrorCode::IoError => write!(f, "IO_ERROR"),
            ErrorCode::ConfigError => write!(f, "CONFIG_ERROR"),
            ErrorCode::FileNotFound => write!(f, "FILE_NOT_FOUND"),
            ErrorCode::InvalidFormat => write!(f, "INVALID_FORMAT"),
            ErrorCode::DuplicateEntry => write!(f, "DUPLICATE_ENTRY"),
            ErrorCode::NotSupported => write!(f, "NOT_SUPPORTED"),
            ErrorCode::Unauthorized => write!(f, "UNAUTHORIZED"),
            ErrorCode::InternalError => write!(f, "INTERNAL_ERROR"),
        }
    }
}

/// Detailed error information for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    /// Error code
    pub code: ErrorCode,
    /// Human-readable message
    pub message: String,
    /// Additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Suggestions for resolution
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
    /// Source file (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Line number (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Column number (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

impl ErrorDetails {
    /// Create a new error detail
    pub fn new(code: ErrorCode, message: String) -> Self {
        Self {
            code,
            message,
            details: None,
            suggestions: vec![],
            file: None,
            line: None,
            column: None,
        }
    }

    /// Add detail information
    pub fn with_detail(mut self, detail: serde_json::Value) -> Self {
        self.details = Some(detail);
        self
    }

    /// Add a suggestion
    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Add source location
    pub fn with_location(mut self, file: String, line: u32, column: u32) -> Self {
        self.file = Some(file);
        self.line = Some(line);
        self.column = Some(column);
        self
    }
}

impl std::fmt::Display for ErrorDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        if let Some(ref details) = self.details {
            write!(f, "\nDetails: {}", details)?;
        }
        if !self.suggestions.is_empty() {
            write!(f, "\nSuggestions:")?;
            for suggestion in &self.suggestions {
                write!(f, "\n  - {}", suggestion)?;
            }
        }
        if let (Some(ref file), Some(line)) = (&self.file, self.line) {
            write!(f, "\nLocation: {}:{}", file, line)?;
        }
        Ok(())
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorSeverity {
    /// Debug information
    Debug,
    /// Informational
    Info,
    /// Warning - operation may be affected
    Warning,
    /// Error - operation failed
    Error,
    /// Critical - application may be unstable
    Critical,
}

impl std::fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorSeverity::Debug => write!(f, "debug"),
            ErrorSeverity::Info => write!(f, "info"),
            ErrorSeverity::Warning => write!(f, "warning"),
            ErrorSeverity::Error => write!(f, "error"),
            ErrorSeverity::Critical => write!(f, "critical"),
        }
    }
}

/// Main error type for beanweb-core
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Ledger not loaded")]
    NotLoaded,

    #[error("Account not found: {name}")]
    AccountNotFound { name: String },

    #[error("Transaction not found: {id}")]
    TransactionNotFound { id: String },

    #[error("Parse error: {message}")]
    ParseError { message: String },

    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("IO error occurred")]
    IoError,

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Invalid format: {message}")]
    InvalidFormat { message: String },

    #[error("Duplicate entry: {entry}")]
    DuplicateEntry { entry: String },

    #[error("Operation not supported: {operation}")]
    NotSupported { operation: String },

    #[error("Unauthorized access")]
    Unauthorized,

    #[error("Internal error: {message}")]
    InternalError { message: String },
}

impl CoreError {
    /// Get the error code
    pub fn code(&self) -> ErrorCode {
        match self {
            CoreError::NotLoaded => ErrorCode::NotLoaded,
            CoreError::AccountNotFound { .. } => ErrorCode::AccountNotFound,
            CoreError::TransactionNotFound { .. } => ErrorCode::TransactionNotFound,
            CoreError::ParseError { .. } => ErrorCode::ParseError,
            CoreError::ValidationError { .. } => ErrorCode::ValidationError,
            CoreError::IoError => ErrorCode::IoError,
            CoreError::ConfigError { .. } => ErrorCode::ConfigError,
            CoreError::FileNotFound { .. } => ErrorCode::FileNotFound,
            CoreError::InvalidFormat { .. } => ErrorCode::InvalidFormat,
            CoreError::DuplicateEntry { .. } => ErrorCode::DuplicateEntry,
            CoreError::NotSupported { .. } => ErrorCode::NotSupported,
            CoreError::Unauthorized => ErrorCode::Unauthorized,
            CoreError::InternalError { .. } => ErrorCode::InternalError,
        }
    }

    /// Get the severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            CoreError::NotLoaded => ErrorSeverity::Warning,
            CoreError::AccountNotFound { .. } => ErrorSeverity::Info,
            CoreError::TransactionNotFound { .. } => ErrorSeverity::Info,
            CoreError::ParseError { .. } => ErrorSeverity::Error,
            CoreError::ValidationError { .. } => ErrorSeverity::Warning,
            CoreError::IoError => ErrorSeverity::Error,
            CoreError::ConfigError { .. } => ErrorSeverity::Critical,
            CoreError::FileNotFound { .. } => ErrorSeverity::Error,
            CoreError::InvalidFormat { .. } => ErrorSeverity::Error,
            CoreError::DuplicateEntry { .. } => ErrorSeverity::Warning,
            CoreError::NotSupported { .. } => ErrorSeverity::Warning,
            CoreError::Unauthorized => ErrorSeverity::Warning,
            CoreError::InternalError { .. } => ErrorSeverity::Critical,
        }
    }

    /// Convert to detailed error info
    pub fn to_details(&self) -> ErrorDetails {
        let mut details = ErrorDetails::new(
            self.code(),
            self.to_string(),
        );

        match self {
            CoreError::AccountNotFound { name } => {
                details = details.with_suggestion(format!(
                    "Check if the account '{}' exists in your ledger file.", name
                ));
                details = details.with_suggestion(
                    "Use the /api/accounts endpoint to list all accounts.".to_string()
                );
            }
            CoreError::TransactionNotFound { id } => {
                details = details.with_suggestion(
                    "Check if the transaction ID is correct.".to_string()
                );
                details = details.with_suggestion(
                    "Use the /api/transactions endpoint to list all transactions.".to_string()
                );
            }
            CoreError::ParseError { message } => {
                details = details.with_detail(serde_json::json!({ "parse_message": message }));
                details = details.with_suggestion(
                    "Check the syntax of your Beancount file.".to_string()
                );
                details = details.with_suggestion(
                    "Ensure all transactions have balanced postings.".to_string()
                );
            }
            CoreError::ValidationError { message } => {
                details = details.with_detail(serde_json::json!({ "validation_message": message }));
                details = details.with_suggestion(
                    "Review the validation message for specific requirements.".to_string()
                );
            }
            CoreError::FileNotFound { path } => {
                details = details.with_suggestion(
                    "Check if the file path is correct.".to_string()
                );
                details = details.with_suggestion(
                    "Ensure the file exists and is readable.".to_string()
                );
            }
            CoreError::NotSupported { operation } => {
                details = details.with_suggestion(
                    format!("The operation '{}' is not yet implemented.", operation)
                );
                details = details.with_suggestion(
                    "This feature may be added in a future version.".to_string()
                );
            }
            _ => {}
        }

        details
    }
}

/// Result type with CoreError
pub type CoreResult<T> = Result<T, CoreError>;

impl From<io::Error> for CoreError {
    fn from(_error: io::Error) -> Self {
        CoreError::IoError
    }
}

/// Error context for reporting
#[derive(Debug, Clone, Default)]
pub struct ErrorContext {
    /// Request ID for tracing
    pub request_id: Option<String>,
    /// User ID (if authenticated)
    pub user_id: Option<String>,
    /// Operation being performed
    pub operation: String,
    /// Additional context data
    pub data: serde_json::Value,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(operation: String) -> Self {
        Self {
            request_id: None,
            user_id: None,
            operation,
            data: serde_json::json!({}),
        }
    }

    /// Add request ID
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Add user ID
    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Add context data
    pub fn with_data(mut self, key: &str, value: serde_json::Value) -> Self {
        self.data[key] = value;
        self
    }
}

/// Error logger trait
pub trait ErrorLogger {
    /// Log an error
    fn log_error(&self, error: &CoreError, context: &ErrorContext);
    /// Log a warning
    fn log_warning(&self, message: &str, context: &ErrorContext);
    /// Log debug information
    fn log_debug(&self, message: &str, context: &ErrorContext);
}

/// Default error logger using log crate
#[derive(Default)]
pub struct DefaultErrorLogger;

impl ErrorLogger for DefaultErrorLogger {
    fn log_error(&self, error: &CoreError, context: &ErrorContext) {
        log::error!(
            target: "beanweb::error",
            "ERROR [{}] {} - Operation: {} - Request: {:?}",
            error.code(),
            error.to_details(),
            context.operation,
            context.request_id
        );
    }

    fn log_warning(&self, message: &str, context: &ErrorContext) {
        log::warn!(
            target: "beanweb::error",
            "WARNING: {} - Operation: {} - Request: {:?}",
            message,
            context.operation,
            context.request_id
        );
    }

    fn log_debug(&self, message: &str, context: &ErrorContext) {
        log::debug!(
            target: "beanweb::error",
            "DEBUG: {} - Operation: {} - Request: {:?}",
            message,
            context.operation,
            context.request_id
        );
    }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;
    use beanweb_config::error::{ConfigError, ConfigErrorCode, ConfigErrorSeverity};

    #[test]
    fn test_error_code_display() {
        assert_eq!(ErrorCode::NotLoaded.to_string(), "NOT_LOADED");
        assert_eq!(ErrorCode::AccountNotFound.to_string(), "ACCOUNT_NOT_FOUND");
        assert_eq!(ErrorCode::ParseError.to_string(), "PARSE_ERROR");
    }

    #[test]
    fn test_error_severity_display() {
        assert_eq!(ErrorSeverity::Debug.to_string(), "debug");
        assert_eq!(ErrorSeverity::Warning.to_string(), "warning");
        assert_eq!(ErrorSeverity::Error.to_string(), "error");
        assert_eq!(ErrorSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn test_core_error_code() {
        let error = CoreError::AccountNotFound { name: "Test".to_string() };
        assert_eq!(error.code(), ErrorCode::AccountNotFound);

        let error = CoreError::NotLoaded;
        assert_eq!(error.code(), ErrorCode::NotLoaded);
    }

    #[test]
    fn test_core_error_severity() {
        let error = CoreError::NotLoaded;
        assert_eq!(error.severity(), ErrorSeverity::Warning);

        let error = CoreError::ConfigError { message: "test".to_string() };
        assert_eq!(error.severity(), ErrorSeverity::Critical);

        let error = CoreError::IoError;
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_error_details_account_not_found() {
        let error = CoreError::AccountNotFound {
            name: "Assets:Checking".to_string()
        };
        let details = error.to_details();

        assert_eq!(details.code, ErrorCode::AccountNotFound);
        assert!(!details.suggestions.is_empty());
        assert!(details.message.contains("Assets:Checking"));
    }

    #[test]
    fn test_error_details_parse_error() {
        let error = CoreError::ParseError {
            message: "Invalid syntax".to_string()
        };
        let details = error.to_details();

        assert_eq!(details.code, ErrorCode::ParseError);
        assert!(details.details.is_some());
    }

    #[test]
    fn test_error_context() {
        let context = ErrorContext::new("test_operation".to_string())
            .with_request_id("req-123".to_string())
            .with_user_id("user-456".to_string())
            .with_data("key", serde_json::json!("value"));

        assert_eq!(context.operation, "test_operation");
        assert_eq!(context.request_id, Some("req-123".to_string()));
        assert_eq!(context.user_id, Some("user-456".to_string()));
    }

    #[test]
    fn test_error_details_builder() {
        let details = ErrorDetails::new(
            ErrorCode::ValidationError,
            "Validation failed".to_string()
        )
        .with_detail(serde_json::json!({"field": "amount"}))
        .with_suggestion("Check the value".to_string())
        .with_location("test.beancount".to_string(), 10, 5);

        assert_eq!(details.code, ErrorCode::ValidationError);
        assert!(details.details.is_some());
        assert_eq!(details.suggestions.len(), 1);
        assert_eq!(details.file, Some("test.beancount".to_string()));
        assert_eq!(details.line, Some(10));
    }

    #[test]
    fn test_config_error_code() {
        let error = ConfigError::FileNotFound {
            path: "/path/to/config.yaml".to_string()
        };
        assert_eq!(error.code(), ConfigErrorCode::FileNotFound);

        let error = ConfigError::MissingField {
            field: "server.port".to_string()
        };
        assert_eq!(error.code(), ConfigErrorCode::MissingField);
    }

    #[test]
    fn test_config_error_severity() {
        let error = ConfigError::FileNotFound {
            path: "/path/to/config.yaml".to_string()
        };
        assert_eq!(error.severity(), ConfigErrorSeverity::Error);

        let error = ConfigError::ValidationError {
            message: "Port must be positive".to_string()
        };
        assert_eq!(error.severity(), ConfigErrorSeverity::Warning);
    }
}
