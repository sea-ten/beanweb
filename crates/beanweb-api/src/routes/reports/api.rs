//! Reports API endpoints - JSON API and HTMX partial responses

use crate::AppState;
use axum::extract::Query;

// Re-export all API functions from the original module

pub async fn api_balance_report(state: axum::extract::State<AppState>) -> String {
    let ledger = state.ledger.read().await;
    serde_json::to_string(&ledger.balance_report()).unwrap_or_default()
}

pub async fn api_income_expense(state: axum::extract::State<AppState>) -> String {
    let ledger = state.ledger.read().await;
    serde_json::to_string(&ledger.income_expense_report()).unwrap_or_default()
}

pub async fn htmx_reports_overview(state: axum::extract::State<AppState>) -> String {
    let ledger = state.ledger.read().await;
    super::page::render_reports_overview(&ledger)
}

pub async fn htmx_reports_balance(state: axum::extract::State<AppState>) -> String {
    let ledger = state.ledger.read().await;
    super::page::render_balance_report(&ledger)
}

pub async fn htmx_reports_income_expense(state: axum::extract::State<AppState>) -> String {
    let ledger = state.ledger.read().await;
    super::page::render_income_expense_report(&ledger)
}

pub async fn htmx_reports_category(state: axum::extract::State<AppState>, query: Query<std::collections::HashMap<String, String>>) -> String {
    let ledger = state.ledger.read().await;
    let category = query.0.get("category").map(|s| s.as_str()).unwrap_or("");
    super::page::render_category_details(&ledger, category)
}
