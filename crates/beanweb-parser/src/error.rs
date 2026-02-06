//! Error types for beanweb-parser

use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Syntax error at {location}: {message}")]
    SyntaxError {
        location: String,
        message: String,
    },

    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Unsupported directive: {directive_type}")]
    UnsupportedDirective { directive_type: String },

    #[error("IO error")]
    IoError(#[from] io::Error),

    #[error("Internal error")]
    InternalError,
}
