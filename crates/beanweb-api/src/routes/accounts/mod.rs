//! Account routes - Account list and tree view
//!
//! Features:
//! - List all accounts with hierarchical tree structure
//! - Tree view with expandable/collapsible nodes
//! - Multi-currency balance display with detail tooltips
//! - Show/hide closed accounts toggle
//! - Account search and filtering
//! - Account detail page with transactions
//!
//! Structure:
//! - api.rs: JSON API and HTMX endpoints
//! - page.rs: Full page rendering

pub mod api;
pub mod page;

pub use api::{
    api_accounts,
    htmx_accounts_list,
    htmx_account_suggest,
    htmx_account_transactions_list,
    AccountAmount,
    AccountListItem,
    AccountTreeNode,
};
pub use page::{
    page_accounts,
    page_account_detail,
    render_account_transactions_paginated,
    get_posting_amount_for_account,
};
