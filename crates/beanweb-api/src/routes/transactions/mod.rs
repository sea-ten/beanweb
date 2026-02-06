//! Transaction routes - Transaction list, search, pagination
//!
//! Features:
//! - List transactions with pagination
//! - Search by keyword (payee, narration, account)
//! - HTMX partial page updates
//!
//! NOTE: Transaction edit functionality has been disabled
//!
//! Structure:
//! - api.rs: JSON API and HTMX endpoints
//! - page.rs: Full page rendering

pub mod api;
pub mod page;

pub use api::{
    api_transactions,
    api_transaction_detail,
    htmx_transactions_list,
    htmx_transactions_filter,
    htmx_transaction_detail,
    // NOTE: 编辑功能已禁用
    // htmx_transaction_edit_form,
    // htmx_transaction_update,
    htmx_transaction_create_form,
    htmx_transaction_store,
};

pub use page::{
    page_transactions,
    // NOTE: 编辑功能已禁用
    // page_transaction_edit,
    page_transaction_create,
};
