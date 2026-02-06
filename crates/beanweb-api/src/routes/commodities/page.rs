//! Commodities page rendering
//!
//! HTMX page endpoints for commodities/multi-currency view

use crate::AppState;

/// Commodity balance entry
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommodityBalance {
    pub commodity: String,
    pub amount: f64,
    pub price_per_unit: Option<f64>,
    pub total_value: f64,
}

/// Calculate all commodity balances across all accounts
pub fn calculate_all_commodity_balances(ledger: &beanweb_core::Ledger) -> Vec<CommodityBalance> {
    let transactions = ledger.transactions(100000, 0);

    // Collect commodity balances per commodity type
    let mut commodity_map: std::collections::HashMap<String, (f64, Option<f64>)> = std::collections::HashMap::new();

    for tx in transactions {
        for posting in &tx.postings {
            // Parse amount and currency
            if !posting.amount.is_empty() {
                let parts: Vec<&str> = posting.amount.split_whitespace().collect();
                if let Some(num_str) = parts.first() {
                    if let Ok(amount) = num_str.parse::<f64>() {
                        let currency = if parts.len() > 1 {
                            parts[1].to_string()
                        } else {
                            posting.currency.clone()
                        };

                        if !currency.is_empty() {
                            // Parse price if present (format: "1 CNY")
                            let price_per_unit = posting.price.as_ref().and_then(|p| {
                                let p_parts: Vec<&str> = p.split_whitespace().collect();
                                p_parts.first().and_then(|s| s.parse().ok())
                            });

                            // Add to commodity map
                            if let Some((current_amount, current_price)) = commodity_map.get(&currency) {
                                let new_amount = current_amount + amount;
                                let new_price = price_per_unit.or(*current_price);
                                commodity_map.insert(currency.clone(), (new_amount, new_price));
                            } else {
                                commodity_map.insert(currency.clone(), (amount, price_per_unit));
                            }
                        }
                    }
                }
            }
        }
    }

    // Convert to sorted vector
    let mut balances: Vec<CommodityBalance> = commodity_map.into_iter()
        .filter(|(_, (amount, _))| *amount != 0.0)
        .map(|(commodity, (amount, price_per_unit))| {
            let total_value = price_per_unit.map(|p| p * amount).unwrap_or(0.0);
            CommodityBalance {
                commodity,
                amount,
                price_per_unit,
                total_value,
            }
        })
        .collect();

    balances.sort_by(|a, b| b.total_value.partial_cmp(&a.total_value).unwrap_or(std::cmp::Ordering::Equal));
    balances
}

/// Commodities page - Shows all commodity/currency total balances
pub async fn page_commodities(
    state: axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Html<String> {
    let ledger = state.ledger.read().await;

    // Calculate all commodity balances
    let all_balances = calculate_all_commodity_balances(&ledger);

    let inner_content = format!(
        r#"<div class='mb-6'>
            <h2 class='text-2xl font-bold'>货币/商品</h2>
            <p class='text-gray-500 mt-1'>各类货币资产总额统计</p>
        </div>
        <div class='bg-white rounded-xl shadow-sm p-6'>
            {}
        </div>"#,
        render_commodity_table(&all_balances)
    );

    axum::response::Html(crate::page_response(&headers, "货币/商品", "/commodities", &inner_content))
}

/// Render commodity table
fn render_commodity_table(balances: &[CommodityBalance]) -> String {
    if balances.is_empty() {
        return r#"<div class='text-center py-12 text-gray-500'><p>暂无货币/商品数据</p></div>"#.to_string();
    }

    let mut html = String::from(
        r#"<div class='overflow-x-auto'>
        <table class='w-full'>
            <thead class='bg-gray-50'>
                <tr>
                    <th class='px-4 py-3 text-left text-sm font-medium text-gray-600'>货币/商品</th>
                    <th class='px-4 py-3 text-right text-sm font-medium text-gray-600'>数量</th>
                    <th class='px-4 py-3 text-right text-sm font-medium text-gray-600'>单价</th>
                    <th class='px-4 py-3 text-right text-sm font-medium text-gray-600'>总价值</th>
                </tr>
            </thead>
            <tbody class='divide-y divide-gray-100'>"#
    );

    for balance in balances {
        let price_display = balance.price_per_unit
            .map(|p| format!("{:.2}", p))
            .unwrap_or_else(|| String::from("-"));
        let total_display = if balance.total_value != 0.0 {
            format!("{:.2}", balance.total_value.abs())
        } else {
            String::from("-")
        };

        html.push_str(&format!(
            r#"<tr class='hover:bg-gray-50'>
                <td class='px-4 py-3 font-medium'>{}</td>
                <td class='px-4 py-3 text-right'>{:.2}</td>
                <td class='px-4 py-3 text-right text-gray-500'>{}</td>
                <td class='px-4 py-3 text-right font-medium'>{}</td>
            </tr>"#,
            balance.commodity,
            balance.amount.abs(),
            price_display,
            total_display
        ));
    }

    html.push_str("</tbody></table></div>");
    html
}
