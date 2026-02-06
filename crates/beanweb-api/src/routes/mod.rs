//! Route modules for the API server
//!
//! All routes are organized into modules for better maintainability:
//! - transactions: Transaction list, search, pagination
//! - accounts: Account list, tree view
//! - settings: Settings page
//! - time: Time range control
//! - files: File editor
//!
//! NOTE: Reports and commodities modules have been disabled (incomplete features)
//!
//! Each module follows a consistent structure:
//! - mod.rs: Module declaration and exports
//! - api.rs: JSON API endpoints
//! - page.rs: HTMX page rendering

pub mod transactions;
pub mod accounts;
// NOTE: 报表功能已禁用
// pub mod reports;
pub mod settings;
pub mod time;
pub mod files;
// NOTE: 货币功能已禁用
// pub mod commodities;
