//! Time range control routes
//!
//! Provides API endpoints for global time context control

use crate::AppState;
use chrono::Datelike;
use std::collections::HashMap;

/// Get current time range (JSON API)
pub async fn api_time_range(state: axum::extract::State<AppState>) -> String {
    let ledger = state.ledger.read().await;
    let context = ledger.time_context();

    serde_json::to_string(&serde_json::json!({
        "range": context.range.to_string(),
        "start_date": context.start_date().map(|d| d.to_string()),
        "end_date": context.end_date().map(|d| d.to_string())
    })).unwrap_or_default()
}

/// Get available months from ledger data
pub async fn api_time_range_months(state: axum::extract::State<AppState>) -> String {
    let ledger = state.ledger.read().await;
    let transactions = ledger.transactions(10000, 0);

    // Extract unique year-month combinations
    let mut months: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for tx in &transactions {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&tx.date, "%Y-%m-%d") {
            let month = date.format("%Y-%m").to_string();
            months.insert(month);
        }
    }

    // Convert to Vec and reverse (most recent first)
    let mut months_vec: Vec<String> = months.into_iter().collect();
    months_vec.reverse();

    serde_json::to_string(&months_vec).unwrap_or_default()
}

/// Get available years from ledger data
pub async fn api_time_range_years(state: axum::extract::State<AppState>) -> String {
    let ledger = state.ledger.read().await;
    let transactions = ledger.transactions(10000, 0);

    // Extract unique years
    let mut years: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for tx in &transactions {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&tx.date, "%Y-%m-%d") {
            let year = date.format("%Y").to_string();
            years.insert(year);
        }
    }

    // Convert to Vec and reverse (most recent first)
    let mut years_vec: Vec<String> = years.into_iter().collect();
    years_vec.reverse();

    serde_json::to_string(&years_vec).unwrap_or_default()
}

/// Set time range (POST) - supports query params and form body
pub async fn api_set_time_range(
    state: axum::extract::State<AppState>,
    query: axum::extract::Query<HashMap<String, String>>,
    body: String,
) -> String {
    let query_params = &query.0;

    // First try to get from query params
    let mut found_from_body = "".to_string();
    let range_str = if let Some(range) = query_params.get("range") {
        range.as_str()
    } else if body.contains('=') {
        // Try to parse from body
        for pair in body.split('&') {
            let parts: Vec<&str> = pair.split('=').collect();
            if parts.len() == 2 && parts[0] == "range" {
                found_from_body = urlencoding::decode(parts[1]).unwrap_or_default().into_owned();
                break;
            }
        }
        found_from_body.as_str()
    } else if !body.is_empty() {
        // Body is just the range parameter
        body.as_str()
    } else {
        ""
    };

    eprintln!("[DEBUG] api_set_time_range called with range: '{}'", range_str);

    // Set time range on ledger (need write lock for set_custom_range/set_time_range)
    {
        let ledger = state.ledger.write().await;

        if range_str.starts_with("month:") {
            // Format: month:01 (current year)
            if let Some(month_str) = range_str.strip_prefix("month:") {
                // Use current year
                let today = chrono::Utc::now().date_naive();
                let month: u32 = month_str.parse().unwrap_or(1);
                if month >= 1 && month <= 12 {
                    let start = chrono::NaiveDate::from_ymd_opt(today.year(), month, 1).unwrap();
                    let end = chrono::NaiveDate::from_ymd_opt(today.year(), month + 1, 1)
                        .unwrap_or(start)
                        .pred_opt()
                        .unwrap_or(start);
                    ledger.set_custom_range(start, end);
                    eprintln!("[DEBUG] Set month range: {} to {}", start, end);
                }
            }
        } else if range_str.starts_with("year:") {
            // Format: year:2026 or year:2026-03 (year with month)
            // - year:2026 -> full year (Jan 1 to Dec 31)
            // - year:2026-04 -> specific month only (Apr 1 to Apr 30)
            if let Some(year_month) = range_str.strip_prefix("year:") {
                let parts: Vec<&str> = year_month.split('-').collect();
                if let Ok(year) = parts[0].parse::<i32>() {
                    if parts.len() == 2 {
                        // year-month format - specific month only
                        let month: u32 = parts[1].parse().unwrap_or(1);
                        if month >= 1 && month <= 12 {
                            let start = chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap();
                            let end = chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
                                .unwrap_or(start)
                                .pred_opt()
                                .unwrap_or(start);
                            ledger.set_custom_range(start, end);
                            eprintln!("[DEBUG] Set month range: {} to {}", start, end);
                        }
                    } else {
                        // year only - full year from Jan 1 to Dec 31
                        let start = chrono::NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
                        let end = chrono::NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
                        ledger.set_custom_range(start, end);
                        eprintln!("[DEBUG] Set year range: {} to {}", start, end);
                    }
                }
            }
        } else if range_str.starts_with("custom:") {
            // Format: custom:2026-01-01,2026-01-31
            if let Some(custom_str) = range_str.strip_prefix("custom:") {
                let parts: Vec<&str> = custom_str.split(',').collect();
                if parts.len() == 2 {
                    if let (Ok(start), Ok(end)) = (
                        chrono::NaiveDate::parse_from_str(parts[0], "%Y-%m-%d"),
                        chrono::NaiveDate::parse_from_str(parts[1], "%Y-%m-%d")
                    ) {
                        ledger.set_custom_range(start, end);
                        eprintln!("[DEBUG] Set custom range: {} to {}", start, end);
                    }
                }
            }
        } else {
            // Standard ranges
            match range_str {
                "month" => {
                    ledger.set_time_range(beanweb_config::TimeRange::Month);
                    eprintln!("[DEBUG] Set time range: month");
                }
                "quarter" => {
                    ledger.set_time_range(beanweb_config::TimeRange::Quarter);
                    eprintln!("[DEBUG] Set time range: quarter");
                }
                "year" => {
                    ledger.set_time_range(beanweb_config::TimeRange::Year);
                    eprintln!("[DEBUG] Set time range: year");
                }
                "all" => {
                    ledger.set_time_range(beanweb_config::TimeRange::All);
                    eprintln!("[DEBUG] Set time range: all");
                }
                _ => {
                    eprintln!("[DEBUG] Unknown time range: '{}'", range_str);
                }
            }
        }
    }

    format!(r#"{{"success": true, "message": "时间范围已更新"}}"#)
}

/// Available time range options (for UI)
pub async fn api_time_range_options() -> String {
    serde_json::to_string(&serde_json::json!([
        {"value": "month", "label": "本月"},
        {"value": "quarter", "label": "本季度"},
        {"value": "year", "label": "本年"},
        {"value": "all", "label": "全部时间"},
        {"value": "custom", "label": "自定义范围"}
    ])).unwrap_or_default()
}
