//! Transactions API endpoints - JSON API and HTMX partial responses
//!
//! Endpoints:
//! - api_transactions: Get transactions list (JSON)
//! - api_transaction_detail: Get single transaction (JSON)
//! - htmx_transactions_list: Transaction list (HTML fragment)
//! - htmx_transactions_filter: Transaction filter (HTML fragment)
//! - htmx_transaction_detail: Transaction detail (HTML fragment)
//! - htmx_transaction_edit_form: Edit form (HTML fragment)
//! - htmx_transaction_update: Update transaction (HTMX)
//! - htmx_transaction_create_form: Create form (HTML fragment)
//! - htmx_transaction_store: Store new transaction (HTMX)

use crate::AppState;
use beanweb_core::TransactionsResponse;
use axum::extract::Query;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

/// Get transactions with pagination and search (JSON API)
pub async fn api_transactions(
    state: axum::extract::State<AppState>,
    params: Query<HashMap<String, String>>,
) -> String {
    let ledger = state.ledger.read().await;
    let limit = params.get("limit").and_then(|s| s.parse().ok()).unwrap_or(50);
    let offset = params.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0);
    let query = params.get("q").map(|s| s.as_str());

    let transactions = if let Some(q) = query {
        if q.is_empty() {
            ledger.transactions(limit, offset)
        } else {
            let all = ledger.search_transactions(q);
            all.into_iter().skip(offset).take(limit).collect()
        }
    } else {
        ledger.transactions(limit, offset)
    };

    let total_count = if let Some(q) = query {
        if q.is_empty() { ledger.transactions_count() } else { ledger.search_transactions(q).len() }
    } else {
        ledger.transactions_count()
    };

    let response = TransactionsResponse {
        transactions,
        total_count,
        page: offset / limit + 1,
        page_size: limit,
    };
    serde_json::to_string(&response).unwrap_or_default()
}

/// Get single transaction detail (JSON API)
pub async fn api_transaction_detail(
    state: axum::extract::State<AppState>,
    path: axum::extract::Path<String>,
) -> String {
    let ledger = state.ledger.read().await;
    let transaction_id = path.0;
    let transaction = ledger.transaction(&transaction_id);

    match transaction {
        Some(tx) => serde_json::to_string(&tx).unwrap_or_default(),
        None => r#"{"error": "Transaction not found"}"#.to_string(),
    }
}

/// HTMX: Transactions list - Partial page update
/// Supports combined keyword and time filtering:
/// - Keyword only: Search all transactions
/// - Time only: Filter by time range
/// - Both: Apply keyword filter to time-filtered results
pub async fn htmx_transactions_list(
    state: axum::extract::State<AppState>,
    params: Query<HashMap<String, String>>,
) -> String {
    let ledger = state.ledger.read().await;
    let time_context = ledger.time_context();
    let limit = params.get("limit").and_then(|s| s.parse().ok()).unwrap_or(50);
    let offset = params.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0);
    let query = params.get("q").map(|s| s.as_str()).unwrap_or("");

    // Check if time filter is active
    let use_time_filter = !matches!(time_context.range, beanweb_config::TimeRange::All);

    // Determine data source based on filters
    let base_transactions: Vec<beanweb_core::Transaction> = if query.is_empty() {
        // No keyword - use time filter if active
        if use_time_filter {
            ledger.filtered_transactions(10000, 0)
        } else {
            ledger.transactions(10000, 0)
        }
    } else if query.is_empty() && use_time_filter {
        // Keyword empty, time filter active
        ledger.filtered_transactions(10000, 0)
    } else if query.is_empty() {
        // No filters
        ledger.transactions(10000, 0)
    } else {
        // Has keyword - need to check time filter too
        if use_time_filter {
            // Apply keyword filter to time-filtered results
            let time_filtered = ledger.filtered_transactions(10000, 0);
            let search_results = ledger.search_transactions(query);
            let search_accounts: std::collections::HashSet<String> =
                search_results.iter().flat_map(|t| t.postings.iter().map(|p| p.account.clone())).collect();
            time_filtered.into_iter()
                .filter(|t| {
                    // Match if any posting account matches or transaction metadata matches
                    t.postings.iter().any(|p| search_accounts.contains(&p.account)) ||
                    t.payee.to_lowercase().contains(&query.to_lowercase()) ||
                    t.narration.to_lowercase().contains(&query.to_lowercase()) ||
                    t.tags.iter().any(|tag| tag.to_lowercase().contains(&query.to_lowercase()))
                })
                .collect()
        } else {
            // Only keyword filter
            ledger.search_transactions(query)
        }
    };

    let mut transactions = base_transactions;

    transactions.sort_by(|a, b| {
        match b.date.cmp(&a.date) {
            std::cmp::Ordering::Equal => b.time.cmp(&a.time),
            other => other
        }
    });

    let total_count = transactions.len();
    let transactions: Vec<_> = transactions.into_iter().skip(offset).take(limit).collect();

    let current_page = offset / limit + 1;
    let total_pages = (total_count + limit - 1) / limit;

    if transactions.is_empty() {
        return r#"<div class='text-center py-12 text-gray-500'><p>暂无交易记录</p></div>"#.to_string();
    }

    let mut html = String::from("<div id='tx-list-container' class='space-y-2'>");
    for tx in &transactions {
        let flag = tx.flag.as_deref().unwrap_or("");
        let flag_color = match flag {
            "*" => "#10B981",
            "!" => "#F59E0B",
            _ => "#6B7280",
        };

        // Smart parsing of payee and narration
        // Handle cases where user entered "payee: narration" without quotes
        let (desc, narration): (String, String) = if tx.payee.is_empty() && tx.narration.contains(": ") {
            // Likely entered as "工资: 2026-01-30" without quotes
            let parts: Vec<&str> = tx.narration.split(": ").collect();
            if parts.len() >= 2 {
                (parts[0].to_string(), parts[1..].join(": "))
            } else {
                (tx.narration.clone(), String::new())
            }
        } else if tx.payee.is_empty() {
            // Only narration, no payee
            (tx.narration.clone(), String::new())
        } else if tx.narration.is_empty() {
            // Only payee, no narration
            (tx.payee.clone(), String::new())
        } else {
            // Both payee and narration present
            (tx.payee.clone(), tx.narration.clone())
        };

        let narration_display = if narration.is_empty() {
            String::new()
        } else {
            format!(" - {}", narration)
        };

        let (amount_display, amount_color, display_currency) = calculate_tx_amount(tx);

        let currency_suffix = if display_currency.is_empty() {
            String::new()
        } else {
            format!(" {}", display_currency)
        };

        let tags_brief: Vec<String> = tx.tags.iter().take(2).map(|tag| format!("#{}", tag)).collect();
        let tags_brief_str = if tags_brief.is_empty() {
            String::new()
        } else {
            format!(" <span class='text-gray-400 text-sm'>{}</span>", tags_brief.join(" "))
        };

        let detail_id = format!("tx-detail-{}", tx.id);
        let datetime = if tx.has_time() {
            format!("{} <span class='text-gray-400'>{}</span>", tx.date, tx.time)
        } else {
            tx.date.clone()
        };

        html.push_str(&format!(
            r#"<div class='border border-l-4 rounded-r-lg p-3 hover:bg-gray-50 transition cursor-pointer' onclick='toggleDetail("{}")'>
                <div class='flex items-center justify-between gap-2'>
                    <div class='flex items-center gap-3 flex-1 min-w-0'>
                        <span class='w-1 h-8 rounded flex-shrink-0' style='background:{}'></span>
                        <div class='flex-1 min-w-0'>
                            <div class='text-sm text-gray-500'>{}</div>
                            <div class='font-medium truncate'>{}{}</div>
                        </div>
                    </div>
                    <div class='flex items-center gap-2 flex-shrink-0'>
                        {}
                        <span class='font-medium {}'>{}{}</span>
                    </div>
                </div>
            </div>
            <div id='{}' class='tx-detail-container' style='display:none'></div>"#,
            detail_id, flag_color, datetime, desc, narration_display, tags_brief_str, amount_color, amount_display, currency_suffix, detail_id
        ));
    }
    html.push_str("</div>");

    html.push_str(r#"<script>
    function toggleDetail(id) {
        var el = document.getElementById(id);
        if (el.style.display === 'none') {
            el.style.display = 'block';
            if (el.innerHTML === '') {
                htmx.ajax('GET', '/transactions/' + id.replace('tx-detail-', '') + '/detail', {target: el});
            }
        } else {
            el.style.display = 'none';
            el.innerHTML = '';
        }
    }
    </script>"#);

    let target = "#transactions-content";
    html.push_str(&format!(
        r#"<div class='mt-6 flex items-center justify-between flex-wrap gap-4'>
            <span class='text-sm text-gray-500'>共 {} 条记录，第 {} / {} 页</span>
            <div class='flex items-center gap-2'>
                <button {} onclick='htmx.ajax("GET", "/transactions/list?limit={}&offset=0&q={}", "{}")' class='px-3 py-1 border rounded hover:bg-gray-100'>首页</button>
                <button {} onclick='htmx.ajax("GET", "/transactions/list?limit={}&offset={}&q={}", "{}")' class='px-3 py-1 border rounded hover:bg-gray-100'>上一页</button>
                <span class='text-sm text-gray-600'>第 <input type='number' id='page-jump-input' min='1' max='{}' value='{}' class='w-16 text-center border rounded px-2 py-1'> 页</span>
                <button onclick='const p=document.getElementById("page-jump-input").value; htmx.ajax("GET", "/transactions/list?limit={}&offset=" + (p-1)*{} + "&q={}", "{}")' class='px-3 py-1 border rounded bg-blue-50 hover:bg-blue-100 text-blue-600'>跳转</button>
                <button {} onclick='htmx.ajax("GET", "/transactions/list?limit={}&offset={}&q={}", "{}")' class='px-3 py-1 border rounded hover:bg-gray-100'>下一页</button>
                <button {} onclick='htmx.ajax("GET", "/transactions/list?limit={}&offset={}&q={}", "{}")' class='px-3 py-1 border rounded hover:bg-gray-100'>末页</button>
            </div>
        </div>"#,
        total_count, current_page, total_pages,
        if current_page == 1 { "disabled" } else { "" },
        limit, query, target,
        if current_page == 1 { "disabled" } else { "" },
        limit, offset.saturating_sub(limit), query, target,
        total_pages, current_page,
        limit, limit, query, target,
        if current_page >= total_pages { "disabled" } else { "" },
        limit, offset + limit, query, target,
        if current_page >= total_pages { "disabled" } else { "" },
        limit, (total_pages.saturating_sub(1)) * limit, query, target
    ));

    html.push_str(r#"<style>.disabled{cursor:not-allowed;opacity:0.5;pointer-events:none}</style>"#);
    html
}

/// Transactions filter - Alias for list (used by page size selector)
pub async fn htmx_transactions_filter(
    state: axum::extract::State<AppState>,
    params: Query<HashMap<String, String>>,
) -> String {
    htmx_transactions_list(state, params).await
}

/// HTMX: Transaction detail - Returns expanded detail view
pub async fn htmx_transaction_detail(
    state: axum::extract::State<AppState>,
    path: axum::extract::Path<String>,
) -> String {
    let ledger = state.ledger.read().await;
    let transaction_id = path.0;
    let transaction = ledger.transaction(&transaction_id);

    match transaction {
        Some(tx) => super::page::render_transaction_detail(&tx),
        None => r#"<div class='text-center py-8 text-red-500'>未找到交易记录</div>"#.to_string(),
    }
}

// NOTE: 编辑功能已禁用
// /// HTMX: Get edit form (supports mode switching via query param)
// pub async fn htmx_transaction_edit_form(
//     state: axum::extract::State<AppState>,
//     path: axum::extract::Path<String>,
//     query: axum::extract::Query<std::collections::HashMap<String, String>>,
// ) -> String {
//     let ledger = state.ledger.read().await;
//     let transaction_id = path.0;
//     let transaction = ledger.transaction(&transaction_id);
//
//     match transaction {
//         Some(tx) => {
//             let mode = query.0.get("mode").map(|s| s.as_str()).unwrap_or("form");
//             let text_content = super::page::generate_transaction_text(&tx);
//
//             match mode {
//                 "text" => {
//                     format!(
//                         r#"<form hx-put='/transactions/{}' hx-target='#edit-result' hx-swap='innerHTML'>
//                             <div class='mb-4'>
//                                 <label class='block text-sm font-medium text-gray-700 mb-2'>交易内容 (Beancount 格式)</label>
//                                 <textarea name="content" class="w-full h-64 font-mono text-sm p-4 border rounded-lg focus:ring-2 focus:ring-indigo-500" placeholder="输入交易记录...">{}</textarea>
//                             </div>
//                             <div class='flex items-center gap-4'>
//                                 <button type='submit' class='px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700'>保存更改</button>
//                                 <button type='button' onclick="switchEditMode('form', '{}')" class='px-4 py-2 border rounded-lg hover:bg-gray-50'>切换到 UI表单模式</button>
//                                 <button type='button' onclick='closeEditModal()' class='px-4 py-2 border rounded-lg hover:bg-gray-50'>取消</button>
//                             </div>
//                         </form>
//                         <div id='edit-result' class='mt-4'></div>"#,
//                         transaction_id, text_content, transaction_id
//                     )
//                 }
//                 _ => {
//                     let form_html = super::page::render_edit_form(&tx);
//                     format!(
//                         r#"<form hx-put='/transactions/{}' hx-target='#edit-result' hx-swap='innerHTML'>
//                             {}
//                             <div class='flex items-center gap-4 mt-6 pt-4 border-t'>
//                                 <button type='submit' class='px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700'>保存更改</button>
//                                 <button type='button' onclick='closeEditModal()' class='px-4 py-2 border rounded-lg hover:bg-gray-50'>取消</button>
//                             </div>
//                         </form>
//                         <div id='edit-result' class='mt-4'></div>"#,
//                         transaction_id, form_html
//                     )
//                 }
//             }
//         }
//         None => r#"<div class='text-red-500 p-4'>未找到交易记录</div>"#.to_string(),
//     }
// }

/// Parse transaction ID to extract line number
/// Format: txn-{source}:{line}:{hash} or legacy txn-{line}
fn parse_txn_id(transaction_id: &str) -> (Option<String>, usize) {
    // Try new format: txn-{source}:{line}:{hash}
    if let Some(after_txn) = transaction_id.strip_prefix("txn-") {
        let parts: Vec<&str> = after_txn.rsplitn(2, ':').collect();
        if parts.len() >= 2 {
            // parts[0] = hash, parts[1] = line:source (need to split by : again)
            if let Some((source_part, line_str)) = parts[1].rsplit_once(':') {
                if let Ok(line) = line_str.parse::<usize>() {
                    // Restore source from parts[2..]
                    let source = if parts.len() > 2 {
                        let source_rest: String = parts[2..].iter().rev().map(|s| *s).collect::<Vec<&str>>().join(":");
                        Some(source_rest.replace("-", "/"))
                    } else {
                        None
                    };
                    return (source, line);
                }
            }
        }
    }

    // Legacy format: txn-{line}
    let line_number: usize = transaction_id
        .strip_prefix("txn-")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    (None, line_number)
}

/// NOTE: 编辑功能已禁用
// /// Handle transaction update (save)
// pub async fn htmx_transaction_update(
//     state: axum::extract::State<AppState>,
//     path: axum::extract::Path<String>,
//     body: String,
// ) -> String {
//     let transaction_id = path.0;
//
//     // Parse transaction ID to get source file and line number
//     let (source_from_id, line_number) = parse_txn_id(&transaction_id);
//
//     // Get transaction to find its source file
//     let ledger = state.ledger.read().await;
//     let transaction = ledger.transaction(&transaction_id);
//
//     if transaction.is_none() {
//         return format!(
//             r#"<div class='bg-red-50 border border-red-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-red-600'>✗</span><span class='font-medium text-red-800'>保存失败</span></div><p class='text-sm text-red-600 mt-1'>未找到交易记录: {}</p></div>"#,
//             transaction_id
//         );
//     }
//     let transaction = transaction.unwrap();
//
//     // Determine which file to modify - use transaction.source first, fall back to ID-based extraction
//     let source_file = transaction.source.clone()
//         .or(source_from_id)
//         .unwrap_or_else(|| state.config.data.main_file.clone());
//     let data_path = &state.config.data.path;
//     let target_file_path = data_path.join(&source_file);
//
//     drop(ledger);
//
//     // Parse the form body - handle URL-encoded form data
//     let mut new_content = String::new();
//     let mut found_content = false;
//
//     // Debug: log first part of body
//     let body_preview = if body.len() > 200 { &body[..200] } else { &body };
//     eprintln!("[DEBUG] Transaction update - tx_id: {}, line: {}, source: {}", transaction_id, line_number, source_file);
//
//     // Parse form fields
//     let mut params: HashMap<String, String> = HashMap::new();
//     let mut has_content_field = false;
//
//     // Check if it's JSON format
//     if body.starts_with('{') {
//         // Try to parse as JSON
//         if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
//             for (key, value) in json.as_object().unwrap_or(&serde_json::Map::new()).iter() {
//                 if let Some(s) = value.as_str() {
//                     params.insert(key.clone(), s.to_string());
//                     if key == "content" {
//                         has_content_field = true;
//                     }
//                 }
//             }
//         }
//     } else {
//         // Parse as form-urlencoded
//         for pair in body.split('&') {
//             let parts: Vec<&str> = pair.split('=').collect();
//             if parts.len() >= 2 {
//                 let key = urlencoding::decode(parts[0]).unwrap_or_default().into_owned();
//                 let value_raw = parts[1..].join("=");
//                 let value = urlencoding::decode(&value_raw).unwrap_or_default().into_owned();
//                 params.insert(key.clone(), value.clone());
//                 if key == "content" {
//                     has_content_field = true;
//                 }
//                 eprintln!("[DEBUG] Form field: key={}, value_len={}", key, value.len());
//             }
//         }
//     }
//
//     // Build content from form fields
//     if has_content_field {
//         // Text mode - use content field directly
//         if let Some(content) = params.get("content") {
//             new_content = content.clone();
//             found_content = true;
//         }
//     } else {
//         // Form mode - build Beancount text from fields
//         eprintln!("[DEBUG] Form mode detected, building text from fields...");
//
//         let date = params.get("date").cloned().unwrap_or_default();
//         let flag = params.get("flag").cloned().unwrap_or_default();
//         let payee = params.get("payee").cloned().unwrap_or_default();
//         let narration = params.get("narration").cloned().unwrap_or_default();
//         let tags_str = params.get("tags").cloned().unwrap_or_default();
//         let links_str = params.get("links").cloned().unwrap_or_default();
//
//         // Build header: DATE FLAG "payee" "narration" #tags ^links
//         let mut header_parts = Vec::new();
//         header_parts.push(date.clone());
//         if !flag.is_empty() {
//             header_parts.push(flag);
//         }
//         if !payee.is_empty() {
//             header_parts.push(format!("\"{}\"", payee));
//         }
//         if !narration.is_empty() {
//             header_parts.push(format!("\"{}\"", narration));
//         }
//         // Add tags
//         for tag in tags_str.split_whitespace().filter(|t| t.starts_with('#')) {
//             header_parts.push(tag.to_string());
//         }
//         // Add links
//         for link in links_str.split_whitespace().filter(|l| l.starts_with('^')) {
//             header_parts.push(link.to_string());
//         }
//
//         let header = header_parts.join(" ");
//         let mut lines = Vec::new();
//         lines.push(header);
//
//         // Parse postings
//         let mut posting_accounts: Vec<String> = Vec::new();
//         let mut posting_amounts: Vec<String> = Vec::new();
//
//         for (key, value) in &params {
//             if key.starts_with("posting_") && key.ends_with("_account") {
//                 if !value.is_empty() {
//                     posting_accounts.push(value.clone());
//                 }
//             } else if key.starts_with("posting_") && key.ends_with("_amount") {
//                 if !value.is_empty() {
//                     posting_amounts.push(value.clone());
//                 }
//             }
//         }
//
//         // Build posting lines
//         for (i, account) in posting_accounts.iter().enumerate() {
//             let amount = posting_amounts.get(i).cloned().unwrap_or_default();
//             let posting_line = if amount.is_empty() {
//                 format!("    {}", account)
//             } else {
//                 format!("    {} {}", account, amount)
//             };
//             lines.push(posting_line);
//         }
//
//         if !lines.is_empty() && lines.iter().any(|l| !l.trim().is_empty()) {
//             new_content = lines.join("\n");
//             found_content = true;
//             eprintln!("[DEBUG] Built content from form, {} lines", lines.len());
//         }
//     }
//
//     if !found_content || new_content.trim().is_empty() {
//         return format!(
//             r#"<div class='bg-red-50 border border-red-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-red-600'>✗</span><span class='font-medium text-red-800'>保存失败</span></div><p class='text-sm text-red-600 mt-1'>交易内容不能为空</p></div>"#,
//             has_content_field,
//             new_content.len()
//         );
//     }
//
//     // Read the original file (use the source file)
//     match std::fs::read_to_string(&target_file_path) {
//         Ok(original_content) => {
//             // Split into lines and find the transaction
//             let mut lines: Vec<&str> = original_content.lines().collect();
//             let total_lines = lines.len();
//
//             // Find the transaction by line number
//             let line_index = line_number.saturating_sub(1);
//
//             if line_index >= total_lines {
//                 return format!(r#"<div class='bg-red-50 border border-red-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-red-600'>✗</span><span class='font-medium text-red-800'>保存失败</span></div><p class='text-sm text-red-600 mt-1'>行号超出范围</p></div>"#);
//             }
//
//             // Find and replace transaction
//             let new_lines: Vec<&str> = new_content.lines().collect();
//             lines.splice(line_index..=line_index, new_lines.iter().copied());
//
//             let new_file_content = lines.join("\n");
//             match std::fs::write(&target_file_path, new_file_content) {
//                 Ok(_) => {
//                     let mut ledger = state.ledger.write().await;
//                     let _ = ledger.reload();
//
//                     format!(r#"<div class='bg-green-50 border border-green-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-green-600'>✓</span><span class='font-medium text-green-800'>交易已更新</span></div></div>"#)
//                 }
//                 Err(e) => {
//                     format!(r#"<div class='bg-red-50 border border-red-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-red-600'>✗</span><span class='font-medium text-red-800'>保存失败</span></div><p class='text-sm text-red-600 mt-1'>写入文件失败: {}</p></div>"#, e)
//                 }
//             }
//         }
//         Err(e) => {
//             format!(r#"<div class='bg-red-50 border border-red-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-red-600'>✗</span><span class='font-medium text-red-800'>保存失败</span></div><p class='text-sm text-red-600 mt-1'>无法读取文件: {}</p></div>"#, e)
//         }
//     }
// }

/// Render create form (text or form mode)
pub async fn htmx_transaction_create_form(
    state: axum::extract::State<AppState>,
    query: Query<HashMap<String, String>>,
) -> String {
    let mode = query.0.get("mode").map(|s| s.as_str()).unwrap_or("form");

    match mode {
        "text" => {
            let today = chrono::Local::today().format("%Y-%m-%d").to_string();
            format!(
                r#"<form hx-post='/transactions' hx-target='#create-result' hx-swap='innerHTML'>
                    <div class='mb-4'>
                        <label class='block text-sm font-medium text-gray-700 mb-2'>交易内容 (Beancount 格式)</label>
                        <textarea name="content" class="w-full h-64 font-mono text-sm p-4 border rounded-lg focus:ring-2 focus:ring-indigo-500" placeholder="输入交易记录...">{}  * "交易对象" "摘要"
    Assets:账户    -100.00 CNY
    Expenses:类别    100.00 CNY</textarea>
                    </div>
                    <div class='flex items-center gap-4'>
                        <button type='submit' class='px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700'>保存</button>
                        <button type='button' onclick='closeCreateModal()' class='px-4 py-2 border rounded-lg hover:bg-gray-50'>取消</button>
                    </div>
                </form>
                <div id='create-result' class='mt-4'></div>"#,
                today
            )
        }
        _ => {
            let today = chrono::Local::today().format("%Y-%m-%d").to_string();
            let accounts_json = {
                let ledger = state.ledger.read().await;
                let accounts = ledger.accounts();
                let json = serde_json::to_string(&accounts).unwrap_or_default();
                json.replace("\"", "&quot;").replace("'", "&#39;")
            };
            format!(
                r#"<form hx-post='/transactions' hx-target='#create-result' hx-swap='innerHTML'>
                    <div class='space-y-6'>
                        <div class='grid grid-cols-3 gap-4'>
                            <div>
                                <label class='block text-sm font-medium text-gray-700 mb-1'>日期</label>
                                <input type='date' name='date' value='{}' class='w-full px-3 py-2.5 border rounded-lg focus:ring-2 focus:ring-indigo-500'>
                            </div>
                            <div>
                                <label class='block text-sm font-medium text-gray-700 mb-1'>标记</label>
                                <select name='flag' class='w-full px-3 py-2.5 border rounded-lg focus:ring-2 focus:ring-indigo-500'>
                                    <option value='*'>已确认 (*)</option>
                                    <option value='!'>待确认 (!)</option>
                                    <option value=''>无标记</option>
                                </select>
                            </div>
                        </div>
                        <div>
                            <label class='block text-sm font-medium text-gray-700 mb-1'>交易对象</label>
                            <input type='text' name='payee' value='' class='w-full px-3 py-2.5 border rounded-lg focus:ring-2 focus:ring-indigo-500' placeholder='交易对象'>
                        </div>
                        <div>
                            <label class='block text-sm font-medium text-gray-700 mb-1'>摘要</label>
                            <input type='text' name='narration' value='' class='w-full px-3 py-2.5 border rounded-lg focus:ring-2 focus:ring-indigo-500' placeholder='交易摘要'>
                        </div>
                        <div>
                            <label class='block text-sm font-medium text-gray-700 mb-2'>分录</label>
                            <div class='border rounded-lg p-3 bg-gray-50' id='postings-container'>
                                <div class='flex items-center gap-2 mb-2'>
                                    <div class='relative flex-1'>
                                        <input type='text' name='posting_0_account' value='' class='w-full px-3 py-2.5 border rounded-lg pl-8' placeholder='搜索账户...'
                                            oninput='filterAccount(this, "account-list-0")' onfocus='showAccountList(this, "account-list-0")' onblur='hideAccountListDelayed(this, "account-list-0")'>
                                        <svg class='w-4 h-4 absolute left-2.5 top-3 text-gray-400' fill='none' stroke='currentColor' viewBox='0 0 24 24'>
                                            <path stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z'/>
                                        </svg>
                                        <div id='account-list-0' class='absolute z-10 w-full bg-white border rounded-lg shadow-lg mt-1 max-h-48 overflow-auto hidden'></div>
                                    </div>
                                    <input type='text' name='posting_0_amount' value='' class='w-32 px-3 py-2.5 border rounded-lg' placeholder='金额'>
                                </div>
                                <div class='flex items-center gap-2 mb-2'>
                                    <div class='relative flex-1'>
                                        <input type='text' name='posting_1_account' value='' class='w-full px-3 py-2.5 border rounded-lg pl-8' placeholder='搜索账户...'
                                            oninput='filterAccount(this, "account-list-1")' onfocus='showAccountList(this, "account-list-1")' onblur='hideAccountListDelayed(this, "account-list-1")'>
                                        <svg class='w-4 h-4 absolute left-2.5 top-3 text-gray-400' fill='none' stroke='currentColor' viewBox='0 0 24 24'>
                                            <path stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z'/>
                                        </svg>
                                        <div id='account-list-1' class='absolute z-10 w-full bg-white border rounded-lg shadow-lg mt-1 max-h-48 overflow-auto hidden'></div>
                                    </div>
                                    <input type='text' name='posting_1_amount' value='' class='w-32 px-3 py-2.5 border rounded-lg' placeholder='金额'>
                                </div>
                            </div>
                            <button type='button' onclick='addPosting()' class='mt-2 px-4 py-2 text-sm text-indigo-600 hover:text-indigo-800 hover:bg-indigo-50 rounded-lg border border-indigo-200'>+ 添加分录</button>
                        </div>
                        <div class='grid grid-cols-2 gap-4'>
                            <div>
                                <label class='block text-sm font-medium text-gray-700 mb-1'>标签 (Tags)</label>
                                <input type='text' name='tags' value='' class='w-full px-3 py-2.5 border rounded-lg focus:ring-2 focus:ring-indigo-500' placeholder='#tag1 #tag2' oninput='updatePreview()'>
                            </div>
                            <div>
                                <label class='block text-sm font-medium text-gray-700 mb-1'>链接 (Links)</label>
                                <input type='text' name='links' value='' class='w-full px-3 py-2.5 border rounded-lg focus:ring-2 focus:ring-indigo-500' placeholder='^link1 ^link2' oninput='updatePreview()'>
                            </div>
                        </div>
                    </div>
                    <div class='border-t pt-4'>
                        <label class='block text-sm font-medium text-gray-700 mb-2'>交易预览</label>
                        <pre id='tx-preview' class='bg-gray-900 text-green-400 p-4 rounded-lg text-sm font-mono overflow-x-auto'>2026-01-31 *
    Assets:Account
    Expenses:Category</pre>
                    </div>
                    <div class='flex items-center gap-4 mt-6 pt-4 border-t'>
                        <button type='submit' class='px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700'>保存</button>
                        <button type='button' onclick='closeCreateModal()' class='px-4 py-2 border rounded-lg hover:bg-gray-50'>取消</button>
                    </div>
                </form>
                <div id='create-result' class='mt-4'></div>
                <div id='accounts-data' data-accounts='{}' class='hidden'></div>
                <script>
                    let postingCount = 2;
                    function addPosting() {{
                        const container = document.getElementById('postings-container');
                        if (container) {{
                            const html = `<div class='flex items-center gap-2 mb-2'>
                                <div class='relative flex-1'>
                                    <input type='text' name='posting_` + postingCount + `_account' value='' class='w-full px-3 py-2.5 border rounded-lg pl-8' placeholder='搜索账户...'
                                        oninput='filterAccount(this, "account-list-` + postingCount + `")' onfocus='showAccountList(this, "account-list-` + postingCount + `")' onblur='hideAccountListDelayed(this, "account-list-` + postingCount + `")'>
                                    <svg class='w-4 h-4 absolute left-2.5 top-3 text-gray-400' fill='none' stroke='currentColor' viewBox='0 0 24 24'>
                                        <path stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z'/>
                                    </svg>
                                    <div id='account-list-` + postingCount + `' class='absolute z-10 w-full bg-white border rounded-lg shadow-lg mt-1 max-h-48 overflow-auto hidden'></div>
                                </div>
                                <input type='text' name='posting_` + postingCount + `_amount' value='' class='w-32 px-3 py-2.5 border rounded-lg' placeholder='金额'>
                                <button type='button' onclick='this.parentElement.remove()' class='px-3 py-2 text-red-500 hover:text-red-700 hover:bg-red-50 rounded-lg'>删除</button>
                            </div>`;
                            container.insertAdjacentHTML('beforeend', html);
                            postingCount++;
                        }}
                    }}
                    function showAccountList(input, listId) {{ filterAccount(input, listId); }}
                    function hideAccountListDelayed(input, listId) {{
                        setTimeout(() => {{
                            const list = document.getElementById(listId);
                            if (list) {{ list.classList.add('hidden'); }}
                        }}, 200);
                    }}
                    function filterAccount(input, listId) {{
                        const q = input.value.toLowerCase();
                        const list = document.getElementById(listId);
                        if (!list) return;
                        const dataDiv = document.getElementById('accounts-data');
                        let accounts = [];
                        if (dataDiv && dataDiv.dataset.accounts) {{
                            try {{ accounts = JSON.parse(dataDiv.dataset.accounts); }} catch(e) {{ accounts = []; }}
                        }}
                        const filtered = accounts.filter(a => a.name.toLowerCase().includes(q));
                        if (filtered.length === 0 || q.length === 0) {{ list.classList.add('hidden'); return; }}
                        let html = '';
                        filtered.slice(0, 10).forEach(a => {{
                            const parts = a.name.split(':');
                            const shortName = parts[parts.length - 1];
                            html += `<div class='px-3 py-2 hover:bg-indigo-50 cursor-pointer text-sm border-b last:border-0' data-account='${{a.name}}' onclick='selectAccount(this, "` + listId + `")'>
                                <div class='font-medium'>${{shortName}}</div>
                                <div class='text-xs text-gray-400'>${{a.name}}</div>
                            </div>`;
                        }});
                        list.innerHTML = html;
                        list.classList.remove('hidden');
                    }}
                    function selectAccount(item, listId) {{
                        const list = document.getElementById(listId);
                        const parent = list.parentElement;
                        const input = parent.querySelector('input[type="text"]');
                        const fullAccount = item.getAttribute('data-account');
                        if (input && fullAccount) {{ input.value = fullAccount; }}
                        list.classList.add('hidden');
                        updatePreview();
                    }}
                    function updatePreview() {{
                        const date = document.querySelector('input[name="date"]')?.value || new Date().toISOString().split('T')[0];
                        const flag = document.querySelector('select[name="flag"]')?.value || '*';
                        const flagStr = flag ? flag + ' ' : '';
                        const payee = document.querySelector('input[name="payee"]')?.value || '';
                        const narration = document.querySelector('input[name="narration"]')?.value || '';
                        const tags = document.querySelector('input[name="tags"]')?.value || '';
                        const links = document.querySelector('input[name="links"]')?.value || '';
                        let firstLine = date + ' ' + flagStr;
                        if (payee && narration) {{ firstLine += '"' + payee + '" "' + narration + '"'; }}
                        else if (payee) {{ firstLine += '"' + payee + '"'; }}
                        else if (narration) {{ firstLine += '"' + narration + '"'; }}
                        const extras = [];
                        tags.split(/\s+/).filter(t => t.starts_with('#')).forEach(t => extras.push(t));
                        links.split(/\s+/).filter(l => l.starts_with('^')).forEach(l => extras.push(l));
                        if (extras.length > 0) {{ firstLine += ' ' + extras.join(' '); }}
                        let postings = [];
                        document.querySelectorAll('[name^="posting_"][name$="_account"]').forEach(input => {{
                            const name = input.name;
                            const amountInput = document.querySelector('input[name="' + name.replace('_account', '_amount') + '"]');
                            const account = input.value;
                            const amount = amountInput?.value || '';
                            if (account) {{ postings.push('    ' + account + (amount ? ' ' + amount : '')); }}
                        }});
                        const preview = document.getElementById('tx-preview');
                        if (preview) {{ preview.textContent = firstLine + '\\n' + (postings.length > 0 ? postings.join('\\n') : '    ...'); }}
                    }}
                </script>"#,
                today, accounts_json
            )
        }
    }
}

/// Store new transaction (write to file)
pub async fn htmx_transaction_store(
    state: axum::extract::State<AppState>,
    body: String,
) -> String {
    let mut params: HashMap<String, String> = HashMap::new();
    for pair in body.split('&') {
        let parts: Vec<&str> = pair.split('=').collect();
        if parts.len() == 2 {
            let key = urlencoding::decode(parts[0]).unwrap_or_default().into_owned();
            let value = urlencoding::decode(parts[1]).unwrap_or_default().into_owned();
            params.insert(key, value);
        }
    }

    let default_date = chrono::Local::today().format("%Y-%m-%d").to_string();
    let date = params.get("date").unwrap_or(&default_date).clone();
    let flag = params.get("flag").unwrap_or(&"*".to_string()).clone();
    let payee = params.get("payee").unwrap_or(&String::new()).clone();
    let narration = params.get("narration").unwrap_or(&String::new()).clone();
    let tags_str = params.get("tags").unwrap_or(&String::new()).clone();
    let links_str = params.get("links").unwrap_or(&String::new()).clone();

    let flag_str = if flag.is_empty() { String::new() } else { format!("{} ", flag) };
    let tags: Vec<&str> = tags_str.split_whitespace().filter(|s| s.starts_with('#')).collect();
    let links: Vec<&str> = links_str.split_whitespace().filter(|s| s.starts_with('^')).collect();

    let mut first_line = if payee.is_empty() && narration.is_empty() {
        String::new()
    } else if payee.is_empty() {
        format!("\"{}\"", narration)
    } else if narration.is_empty() {
        format!("\"{}\"", payee)
    } else {
        format!("\"{}\" \"{}\"", payee, narration)
    };

    let mut extras = Vec::new();
    for tag in &tags { extras.push(*tag); }
    for link in &links { extras.push(*link); }
    if !extras.is_empty() {
        first_line.push(' ');
        first_line.push_str(&extras.join(" "));
    }

    fn parse_amount(amount_str: &str) -> (f64, String) {
        let parts: Vec<&str> = amount_str.split_whitespace().collect();
        let num_str = parts.first().map(|s| *s).unwrap_or("");
        let currency = if parts.len() > 1 { parts[1].to_string() } else { String::new() };
        let amount = num_str.parse::<f64>().unwrap_or(0.0);
        (amount, currency)
    }

    struct PostingData { account: String, amount: f64, currency: String }
    let mut postings_data: Vec<PostingData> = Vec::new();
    for (key, value) in &params {
        if key.starts_with("posting_") && key.ends_with("_account") {
            let amount_key = format!("{}_amount", key.strip_suffix("_account").unwrap());
            let amount_str = params.get(&amount_key).unwrap_or(&String::new()).clone();
            if !value.is_empty() {
                let (amount, currency) = parse_amount(&amount_str);
                postings_data.push(PostingData { account: value.clone(), amount, currency });
            }
        }
    }

    let known_amounts: Vec<f64> = postings_data.iter().filter(|p| p.amount != 0.0).map(|p| p.amount).collect();
    let total: f64 = known_amounts.iter().sum();
    let has_zero_amount_postings = postings_data.iter().any(|p| p.amount == 0.0);

    let final_postings: Vec<String>;
    if postings_data.is_empty() {
        return r#"<div class='bg-red-50 border border-red-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-red-600'>✗</span><span class='font-medium text-red-800'>保存失败</span></div><p class='text-sm text-red-600 mt-1'>请至少添加一个分录</p></div>"#.to_string();
    } else if total.abs() < 0.001 {
        final_postings = postings_data.iter().map(|p| {
            let amount_str = if p.amount == 0.0 { String::new() } else if p.currency.is_empty() { format!("{:.2}", p.amount) } else { format!("{} {}", p.amount, p.currency) };
            format!("    {}{}", p.account, if amount_str.is_empty() { String::new() } else { format!(" {}", amount_str) })
        }).collect();
    } else if known_amounts.len() == postings_data.len() - 1 && has_zero_amount_postings {
        let missing_amount = -total;
        let currency = postings_data.iter().find(|p| p.amount != 0.0).map(|p| p.currency.clone()).unwrap_or_default();
        final_postings = postings_data.iter().map(|p| {
            let amount_str = if p.amount == 0.0 {
                if currency.is_empty() { format!("{:.2}", missing_amount) } else { format!("{} {}", missing_amount, currency) }
            } else if p.currency.is_empty() { format!("{:.2}", p.amount) } else { format!("{} {}", p.amount, p.currency) };
            format!("    {}{}", p.account, if amount_str.is_empty() { String::new() } else { format!(" {}", amount_str) })
        }).collect();
    } else if known_amounts.len() == postings_data.len() {
        return format!(r#"<div class='bg-red-50 border border-red-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-red-600'>✗</span><span class='font-medium text-red-800'>金额不平衡</span></div><p class='text-sm text-red-600 mt-1'>分录金额总和不为 0，当前: {:.2}</p></div>"#, total);
    } else {
        return r#"<div class='bg-red-50 border border-red-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-red-600'>✗</span><span class='font-medium text-red-800'>无法自动计算</span></div><p class='text-sm text-red-600 mt-1'>多个分录金额为空，请填写足够的金额使总和为 0</p></div>"#.to_string();
    }

    let created_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let metadata_line = format!("    created_at: \"{}\"", created_at);
    let mut transaction_text = String::new();
    transaction_text.push_str(&format!("{} {} {}", date, flag_str, first_line));
    transaction_text.push('\n');
    transaction_text.push_str(&final_postings.join("\n"));
    transaction_text.push('\n');
    transaction_text.push_str(&metadata_line);
    transaction_text.push('\n');
    transaction_text.push('\n');

    let new_tx_file = &state.config.data.new_transaction_file;
    let file_path = if new_tx_file.is_empty() || new_tx_file == "main.bean" {
        state.config.data.path.join(&state.config.data.main_file)
    } else if new_tx_file.starts_with("/") || new_tx_file.contains(":/") {
        PathBuf::from(new_tx_file)
    } else {
        state.config.data.path.join(new_tx_file)
    };

    match std::fs::OpenOptions::new().append(true).open(&file_path) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(transaction_text.as_bytes()) {
                return format!(r#"<div class='bg-red-50 border border-red-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-red-600'>✗</span><span class='font-medium text-red-800'>保存失败</span></div><p class='text-sm text-red-600 mt-1'>错误: {}</p></div>"#, e);
            }

            // Trigger ledger reload directly
            let mut ledger = state.ledger.write().await;
            if let Err(e) = ledger.reload().await {
                eprintln!("[ERROR] Failed to reload ledger after creating transaction: {}", e);
            }

            format!(r#"<div class='bg-green-50 border border-green-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-green-600'>✓</span><span class='font-medium text-green-800'>交易已创建</span></div><p class='text-sm text-green-600 mt-1'>账本已重新加载</p><script>closeCreateModal(); const txContent = document.getElementById('transactions-content'); if (txContent) {{ htmx.ajax('GET', '/transactions/list?limit=50', {{target: txContent}}); }} else {{ setTimeout(() => window.location.reload(), 300); }}</script></div>"#)
        }
        Err(e) => {
            format!(r#"<div class='bg-red-50 border border-red-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-red-600'>✗</span><span class='font-medium text-red-800'>保存失败</span></div><p class='text-sm text-red-600 mt-1'>无法打开文件: {}</p></div>"#, e)
        }
    }
}

/// Calculate transaction amount for display
fn calculate_tx_amount(tx: &beanweb_core::Transaction) -> (String, String, String) {
    fn is_expenses_account(account: &str) -> bool {
        account.starts_with("Expenses:") || account.starts_with("expenses:")
    }
    fn is_income_account(account: &str) -> bool {
        account.starts_with("Income:") || account.starts_with("income:")
    }
    fn is_assets_account(account: &str) -> bool {
        account.starts_with("Assets:") || account.starts_with("assets:")
    }

    let mut expenses_total: f64 = 0.0;
    let mut income_total: f64 = 0.0;
    let mut assets_total: f64 = 0.0;
    let mut liabilities_total: f64 = 0.0;
    let mut common_currency = String::new();
    let mut has_unknown_amount = false;

    // First pass: calculate known amounts
    for p in &tx.postings {
        if !p.amount.is_empty() {
            // Use total_value to handle @ price syntax (e.g., 800 PI @ 1 CNY)
            let (amount, currency) = p.total_value("CNY");
            if common_currency.is_empty() && !currency.is_empty() {
                common_currency = currency;
            }
            if is_expenses_account(&p.account) {
                expenses_total += amount;
            } else if is_income_account(&p.account) {
                income_total += amount;
            } else if is_assets_account(&p.account) {
                assets_total += amount;
            } else if p.account.starts_with("Liabilities:") || p.account.starts_with("liabilities:") {
                liabilities_total += amount;
            }
        } else {
            // Posting without amount - this is the balancing posting
            has_unknown_amount = true;
        }
    }

    // If there's a balancing posting (no amount), calculate the missing amount
    let display_amount: f64;
    let amount_color: String;
    let display_currency = common_currency;

    if has_unknown_amount {
        // The transaction has a balancing posting without amount
        // Calculate what the missing amount should be to balance the transaction
        let known_total = expenses_total + income_total + assets_total + liabilities_total;
        let balancing_amount = -known_total;

        // Find the balancing posting's account
        let balancing_account = tx.postings.iter()
            .find(|p| p.amount.is_empty())
            .map(|p| p.account.clone())
            .unwrap_or_default();

        // Determine which account type the balancing posting belongs to
        if is_expenses_account(&balancing_account) {
            expenses_total = balancing_amount;
        } else if is_income_account(&balancing_account) {
            income_total = balancing_amount;
        } else if is_assets_account(&balancing_account) {
            assets_total = balancing_amount;
        } else if balancing_account.starts_with("Liabilities:") || balancing_account.starts_with("liabilities:") {
            liabilities_total = balancing_amount;
        }
    }

    // Determine display based on account type
    // For income: negative means money in (green), positive means refund (red)
    // For expenses: positive means money out (red), negative means refund (green)
    // For assets: positive means increase (green), negative means decrease (red)

    if income_total.abs() > 0.001 {
        // Primary transaction is income-related
        if income_total < 0.0 {
            // Normal income (money in) - show positive in green
            display_amount = income_total.abs();
            amount_color = "text-green-600".to_string();
        } else {
            // Refund/return - show as red
            display_amount = income_total.abs();
            amount_color = "text-red-600".to_string();
        }
    } else if expenses_total.abs() > 0.001 {
        // Primary transaction is expense-related
        if expenses_total > 0.0 {
            // Normal expense (money out) - show positive in red
            display_amount = expenses_total.abs();
            amount_color = "text-red-600".to_string();
        } else {
            // Refund - show as green
            display_amount = expenses_total.abs();
            amount_color = "text-green-600".to_string();
        }
    } else if assets_total.abs() > 0.001 {
        // Primary transaction is asset-related
        if assets_total > 0.0 {
            // Asset increase - green
            display_amount = assets_total.abs();
            amount_color = "text-green-600".to_string();
        } else {
            // Asset decrease - red
            display_amount = assets_total.abs();
            amount_color = "text-red-600".to_string();
        }
    } else if liabilities_total.abs() > 0.001 {
        // Primary transaction is liability-related
        if liabilities_total > 0.0 {
            // Liability increase - red
            display_amount = liabilities_total.abs();
            amount_color = "text-red-600".to_string();
        } else {
            // Liability decrease (payoff) - green
            display_amount = liabilities_total.abs();
            amount_color = "text-green-600".to_string();
        }
    } else {
        // Fallback: use total
        let total = expenses_total + income_total + assets_total + liabilities_total;
        display_amount = total.abs();
        if total < 0.0 {
            amount_color = "text-green-600".to_string();
        } else {
            amount_color = "text-red-600".to_string();
        }
    }

    (format!("{:.2}", display_amount), amount_color, display_currency)
}
