//! Commodities/Multi-currency routes
//!
//! Features:
//! - Display all commodity/currency total balances
//!
//! Structure:
//! - api.rs: JSON API endpoints (if needed)
//! - page.rs: HTMX page rendering

pub mod api;
pub mod page;

pub use page::page_commodities;
