//! Error types for beanweb-api

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Not found: {resource}")]
    NotFound { resource: String },

    #[error("Bad request: {message}")]
    BadRequest { message: String },

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Internal server error")]
    InternalError,
}
