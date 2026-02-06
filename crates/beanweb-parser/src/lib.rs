//! Beancount parser implementation
//!
//! A lightweight Beancount file parser using regex.

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

pub mod error;
pub mod types;
pub mod directives;
pub mod parser;

pub use error::ParseError;
pub use parser::{SimpleBeancountParser, extract_time_from_meta};

// Re-export commonly used types
pub use types::{
    SpanInfo, Account, Amount, Cost, Price, Date, StringValue, AccountType, Meta,
};
pub use directives::{
    SpannedDirective, Directive, Transaction, OpenDirective, CloseDirective,
    BalanceDirective, PadDirective, CommodityDirective, DocumentDirective,
    PriceDirective, EventDirective, NoteDirective, OptionDirective, IncludeDirective,
    CustomDirective, CommentDirective, Posting,
};

// ==================== Utility Functions ====================

/// Generate a short hash (8 characters) from content
pub fn short_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    let hash = hasher.finish();

    // Take first 8 characters of hex hash
    format!("{:x}", hash)[..8].to_string()
}

/// Generate unique transaction ID from source, line number, and content
pub fn generate_txn_id(source: Option<&str>, line: usize, content: &str) -> String {
    let source_part = source.unwrap_or("unknown").replace("/", "-").replace(":", "-");
    let hash = short_hash(content);
    format!("txn-{}:{}:{}", source_part, line, hash)
}

// ==================== Parser Trait ====================

/// Parser reference type
pub type ParserRef = Arc<dyn BeancountParserTrait>;

/// Trait for Beancount parsers
#[async_trait]
pub trait BeancountParserTrait: Send + Sync {
    /// Parse a Beancount file and return directives
    async fn parse(&self, content: &str) -> Result<Vec<SpannedDirective>, ParseError>;

    /// Parse from a file path (recursive, handles includes)
    async fn parse_file(&self, path: PathBuf) -> Result<Vec<SpannedDirective>, ParseError>;

    /// Parse from a file path with base directory for resolving includes
    async fn parse_file_with_base(&self, path: PathBuf, base_dir: PathBuf) -> Result<Vec<SpannedDirective>, ParseError>;
}

/// Default parser implementation
#[derive(Debug, Default)]
pub struct DefaultBeancountParser;

#[async_trait]
impl BeancountParserTrait for DefaultBeancountParser {
    async fn parse(&self, content: &str) -> Result<Vec<SpannedDirective>, ParseError> {
        SimpleBeancountParser::parse(content)
            .map_err(|e| ParseError::SyntaxError {
                location: "parse".to_string(),
                message: e.to_string(),
            })
    }

    async fn parse_file(&self, path: PathBuf) -> Result<Vec<SpannedDirective>, ParseError> {
        let base_dir = path.parent().unwrap_or(&PathBuf::from(".")).to_path_buf();
        self.parse_file_with_base(path, base_dir).await
    }

    async fn parse_file_with_base(&self, path: PathBuf, base_dir: PathBuf) -> Result<Vec<SpannedDirective>, ParseError> {
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| ParseError::IoError(e))?;

        // Get the relative path from the data directory for source tracking
        let source_path = path.to_string_lossy().to_string();

        // First pass: parse and collect all directives
        let all_directives = SimpleBeancountParser::parse_with_source(&content, Some(&source_path))
            .map_err(|e| ParseError::SyntaxError {
                location: source_path.clone(),
                message: e.to_string(),
            })?;

        // Second pass: handle includes recursively
        let mut processed_directives = Vec::new();
        for directive in all_directives {
            match directive.data {
                Directive::Include(include) => {
                    let include_path = &include.path;

                    // Check if it's a glob pattern (contains * or ?)
                    if include_path.contains('*') || include_path.contains('?') {
                        // Use glob to expand the pattern
                        let pattern = base_dir.join(include_path);
                        let pattern_str = pattern.to_string_lossy();

                        if let Ok(paths) = glob::glob(&pattern_str) {
                            for entry in paths.flatten() {
                                if entry.is_file() {
                                    let included_directives = self.parse_file_with_base(
                                        entry.clone(),
                                        entry.parent().unwrap_or(&base_dir).to_path_buf()
                                    ).await
                                    .map_err(|e| ParseError::SyntaxError {
                                        location: entry.to_string_lossy().to_string(),
                                        message: e.to_string(),
                                    })?;
                                    processed_directives.extend(included_directives);
                                }
                            }
                        }
                    } else {
                        // Resolve include path relative to base directory
                        let included_path = base_dir.join(include_path);
                        if included_path.exists() {
                            // Recursively parse included file
                            let included_directives = self.parse_file_with_base(
                                included_path.clone(),
                                included_path.parent().unwrap_or(&base_dir).to_path_buf()
                            ).await
                            .map_err(|e| ParseError::SyntaxError {
                                location: included_path.to_string_lossy().to_string(),
                                message: e.to_string(),
                            })?;
                            processed_directives.extend(included_directives);
                        }
                    }
                },
                _ => {
                    processed_directives.push(directive);
                }
            }
        }

        Ok(processed_directives)
    }
}
