//! Commodities API endpoints
//!
//! JSON API for commodities data (if needed in the future)

use crate::AppState;

/// Placeholder for future JSON API endpoint
/// Example: GET /api/commodities - returns commodity balances as JSON
pub async fn api_commodities(state: axum::extract::State<AppState>) -> String {
    let ledger = state.ledger.read().await;

    // Calculate balances
    let balances = calculate_all_commodity_balances(&ledger);

    // Return as JSON
    serde_json::to_string(&balances).unwrap_or_default()
}

// Re-export shared calculation logic
pub(crate) use super::page::calculate_all_commodity_balances;

use super::page::CommodityBalance;
