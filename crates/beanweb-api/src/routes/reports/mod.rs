//! Report routes - Balance and income-expense reports
//!
//! Structure:
//! - api.rs: JSON API and HTMX endpoints
//! - page.rs: Full page rendering

pub mod api;
pub mod page;

pub use api::{
    api_balance_report,
    api_income_expense,
    htmx_reports_overview,
    htmx_reports_balance,
    htmx_reports_income_expense,
    htmx_reports_category,
};

pub use page::page_reports;
