//! Transactions page rendering - Full page endpoints
//!
//! Endpoints:
//! - page_transactions: Main transactions list page
//! - page_transaction_edit: Transaction edit modal
//! - page_transaction_create: Transaction create modal
//!
//! Helper functions:
//! - render_transaction_detail: Render transaction detail view
//! - render_edit_form: Render form-based edit interface
//! - generate_transaction_text: Generate Beancount text format

use crate::AppState;

/// Transactions page - Main page with search and pagination controls
/// NOTE: This page respects current time context - shows all by default, filtered when user selects time
pub async fn page_transactions(
    state: axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Html<String> {
    let ledger = state.ledger.read().await;
    let stats = ledger.transaction_stats();
    let time_context = ledger.time_context();
    let time_range = time_context.range.to_string();

    // Use current time context - show filtered or all based on user's selection
    let is_all_range = matches!(time_context.range, beanweb_config::TimeRange::All);

    // Get data based on time context
    let (count, postings, display_start, display_end) = if is_all_range {
        // No time filter - show all transactions
        let all_count = ledger.transactions_count();
        let all_postings: usize = ledger.transactions(10000, 0).iter().map(|t| t.posting_count()).sum();
        let start = stats.date_range_start.clone().unwrap_or_else(|| "-".to_string());
        let end = stats.date_range_end.clone().unwrap_or_else(|| "-".to_string());
        (all_count, all_postings, start, end)
    } else {
        // Time filter active - show filtered transactions
        let filtered_count = ledger.filtered_transactions_count();
        let filtered_postings: usize = ledger.filtered_transactions(10000, 0).iter().map(|t| t.posting_count()).sum();
        let start = time_context.start_date().map(|d| d.to_string()).unwrap_or_else(|| "-".to_string());
        let end = time_context.end_date().map(|d| d.to_string()).unwrap_or_else(|| "-".to_string());
        (filtered_count, filtered_postings, start, end)
    };

    let inner_content = format!(
        r#"<div class='relative'>
            {}
        </div>
        <div class='flex items-center justify-between mb-4'>
            <h2 class='text-2xl font-bold'>交易流水</h2>
            <div class='flex gap-2'>
                <button onclick='reloadLedger()' class='px-4 py-2 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 flex items-center gap-2' title='重新加载账本'>
                    <svg xmlns='http://www.w3.org/2000/svg' class='h-5 w-5' fill='none' viewBox='0 0 24 24' stroke='currentColor'>
                        <path stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15'/>
                    </svg>
                    Reload
                </button>
                <button hx-get='/transactions/create' hx-swap='beforeend' hx-target='body'
                    class='px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 flex items-center gap-2'>
                    <svg xmlns='http://www.w3.org/2000/svg' class='h-5 w-5' fill='none' viewBox='0 0 24 24' stroke='currentColor'>
                        <path stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M12 4v16m8-8H4'/>
                    </svg>
                    新建
                </button>
                <input type='text' name='q' placeholder='搜索...'
                    hx-get='/transactions/list' hx-target='#transactions-content' hx-trigger='keyup changed delay:500ms'
                    class='px-4 py-2 border rounded-lg w-48'>
                <select name='limit' hx-get='/transactions/list' hx-target='#transactions-content' hx-trigger='change'
                    class='px-4 py-2 border rounded-lg' onchange='this.form.requestSubmit()'>
                    <option value='10'>10 条</option>
                    <option value='25'>25 条</option>
                    <option value='50' selected>50 条</option>
                    <option value='100'>100 条</option>
                </select>
            </div>
        </div>
        <div class='grid grid-cols-2 md:grid-cols-4 gap-3 mb-4'>
            <div class='bg-indigo-50 p-3 rounded-lg border border-indigo-100'><p class='text-xs text-indigo-600'>交易数</p><p class='text-xl font-bold'>{}</p></div>
            <div class='bg-purple-50 p-3 rounded-lg border border-purple-100'><p class='text-xs text-purple-600'>条目数</p><p class='text-xl font-bold'>{}</p></div>
            <div class='bg-green-50 p-3 rounded-lg border border-green-100'><p class='text-xs text-green-600'>开始</p><p class='text-sm font-medium truncate'>{}</p></div>
            <div class='bg-orange-50 p-3 rounded-lg border border-orange-100'><p class='text-xs text-orange-600'>结束</p><p class='text-sm font-medium truncate'>{}</p></div>
        </div>
        <div id='transactions-content' hx-get='/transactions/list?limit=50' hx-trigger='load' class='bg-white rounded-xl shadow-sm p-6'>
            <p class='text-gray-500 text-center'>加载中...</p>
        </div>
        <script>
        function reloadLedger() {{
            fetch('/api/reload', {{method: 'POST'}})
                .then(r => r.json())
                .then(data => {{
                    if (data.success) {{
                        window.location.reload();
                    }} else {{
                        alert('重新加载失败: ' + data.message);
                    }}
                }})
                .catch(e => alert('重新加载失败: ' + e));
        }}
        </script>"#,
        crate::page_time_selector(&time_range, &display_start, &display_end),
        count,
        postings,
        display_start,
        display_end
    );

    axum::response::Html(crate::page_response_with_time(&headers, "交易流水", "/transactions", &inner_content, &time_range))
}

// NOTE: 编辑功能已禁用
// /// Get transaction for editing - modal overlay that loads content via HTMX
// pub async fn page_transaction_edit(
//     state: axum::extract::State<AppState>,
//     headers: axum::http::HeaderMap,
//     path: axum::extract::Path<String>,
// ) -> axum::response::Html<String> {
//     let ledger = state.ledger.read().await;
//     let transaction_id = path.0;
//     let transaction = ledger.transaction(&transaction_id);
//
//     match transaction {
//         Some(tx) => {
//             let edit_mode = &state.config.journal.edit_mode;
//             let initial_mode = match edit_mode {
//                 beanweb_config::EditMode::Form => "form",
//                 beanweb_config::EditMode::Text => "text",
//             };
//             let toggle_label = if initial_mode == "form" { "文本模式" } else { "表单模式" };
//             let toggle_mode = if initial_mode == "form" { "text" } else { "form" };
//
//             let inner_content = format!(
//                 r#"<!-- 编辑模态框 -->
// <div id='edit-modal-{}' class='fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center' onclick='if(event.target.id === "edit-modal-{}") closeEditModal()'>
//     <div class='bg-white rounded-xl shadow-2xl w-full max-w-4xl max-h-[90vh] overflow-hidden' onclick='event.stopPropagation()'>
//         <div class='flex items-center justify-between px-6 py-4 border-b'>
//             <h2 class='text-xl font-bold'>编辑交易</h2>
//             <div class='flex items-center gap-2'>
//                 <button onclick="switchEditMode('{}', '{}')" class='px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50'>
//                     切换到 {}
//                 </button>
//                 <button onclick='closeEditModal()' class='text-gray-500 hover:text-gray-700 p-2'>
//                     <svg class='w-6 h-6' fill='none' stroke='currentColor' viewBox='0 0 24 24'>
//                         <path stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M6 18L18 6M6 6l12 12'/>
//                     </svg>
//                 </button>
//             </div>
//         </div>
//         <div class='p-6 overflow-y-auto max-h-[calc(90vh-140px)]' id='edit-form-container'>
//             <div hx-get='/transactions/{}/edit/form?mode={}' hx-trigger='load' hx-target='this' hx-swap='innerHTML'>
//                 <div class='flex items-center justify-center py-12'>
//                     <div class='animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-600'></div>
//                 </div>
//             </div>
//         </div>
//     </div>
// </div>
// <script>
// function closeEditModal() {{
//     const modal = document.getElementById('edit-modal-{}') || document.querySelector('[id^="edit-modal-"]');
//     if (modal) {{ modal.remove(); }}
// }}
// function switchEditMode(mode, txId) {{
//     const headerContainer = document.querySelector('#edit-modal-{{}}').querySelector('.flex.items-center.gap-2');
//     if (headerContainer) {{
//         const toggleBtn = headerContainer.querySelector('button[onclick^="switchEditMode"]');
//         if (toggleBtn) {{
//             const newMode = mode === 'form' ? 'text' : 'form';
//             const newLabel = mode === 'form' ? '文本模式' : '表单模式';
//             toggleBtn.setAttribute('onclick', "switchEditMode('" + newMode + "', '" + txId + "')");
//             toggleBtn.textContent = '切换到 ' + newLabel;
//         }}
//     }}
//     const container = document.getElementById('edit-form-container');
//     if (container) {{
//         container.innerHTML = '<div class="flex items-center justify-center py-12"><div class="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-600"></div></div>';
//         htmx.ajax('GET', '/transactions/' + txId + '/edit/form?mode=' + mode, {{target: container}});
//     }}
// }}
// document.addEventListener('keydown', function(e) {{
//     if (e.key === 'Escape') closeEditModal();
// }});
// </script>"#,
//                 tx.id, tx.id, toggle_mode, tx.id, toggle_label,
//                 tx.id, initial_mode,
//                 tx.id
//             );
//
//             axum::response::Html(inner_content)
//         }
//         None => {
//             let error_html = format!(
//                 r#"<div class='fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center' onclick='if(event.target === this) closeEditModal()'>
//     <div class='bg-red-50 border border-red-200 rounded-lg p-6 text-center' onclick='event.stopPropagation()'>
//         <h3 class='text-lg font-medium text-red-800 mb-2'>未找到交易记录</h3>
//         <p class='text-red-600 mb-4'>交易 ID: {}</p>
//         <button onclick='closeEditModal()' class='px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700'>关闭</button>
//     </div>
// </div>
// <script>function closeEditModal() {{ const m = document.querySelector('.fixed.inset-0.bg-black'); if(m) m.remove(); }}</script>"#,
//                 transaction_id
//             );
//             axum::response::Html(error_html)
//         }
//     }
// }

/// Get transaction create modal
pub async fn page_transaction_create(
    state: axum::extract::State<AppState>,
) -> axum::response::Html<String> {
    let edit_mode = &state.config.journal.edit_mode;
    let initial_mode = match edit_mode {
        beanweb_config::EditMode::Form => "form",
        beanweb_config::EditMode::Text => "text",
    };
    let toggle_label = if initial_mode == "form" { "文本模式" } else { "表单模式" };
    let toggle_mode = if initial_mode == "form" { "text" } else { "form" };

    let inner_content = format!(
        r#"<!-- 新建交易模态框 -->
<div id='create-modal' class='fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center' onclick='if(event.target.id === "create-modal") closeCreateModal()'>
    <div class='bg-white rounded-xl shadow-2xl w-full max-w-4xl max-h-[90vh] overflow-hidden' onclick='event.stopPropagation()'>
        <div class='flex items-center justify-between px-6 py-4 border-b'>
            <h2 class='text-xl font-bold'>新建交易</h2>
            <div class='flex items-center gap-2'>
                <button onclick="switchCreateMode('{}')" class='px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50'>
                    切换到 {}
                </button>
                <button onclick='closeCreateModal()' class='text-gray-500 hover:text-gray-700 p-2'>
                    <svg class='w-6 h-6' fill='none' stroke='currentColor' viewBox='0 0 24 24'>
                        <path stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M6 18L18 6M6 6l12 12'/>
                    </svg>
                </button>
            </div>
        </div>
        <div class='p-6 overflow-y-auto max-h-[calc(90vh-140px)]' id='create-form-container'>
            <div hx-get='/transactions/create/form?mode={}' hx-trigger='load' hx-target='this' hx-swap='innerHTML'>
                <div class='flex items-center justify-center py-12'>
                    <div class='animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-600'></div>
                </div>
            </div>
        </div>
    </div>
</div>
<script>
function closeCreateModal() {{
    const modal = document.getElementById('create-modal');
    if (modal) {{ modal.remove(); }}
}}
function switchCreateMode(mode) {{
    const container = document.getElementById('create-form-container');
    if (container) {{
        container.innerHTML = '<div class="flex items-center justify-center py-12"><div class="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-600"></div></div>';
        htmx.ajax('GET', '/transactions/create/form?mode=' + mode, {{target: container}});
    }}
}}
document.addEventListener('keydown', function(e) {{
    if (e.key === 'Escape') closeCreateModal();
}});
</script>"#,
        toggle_mode, toggle_label, initial_mode
    );

    axum::response::Html(inner_content)
}

/// Render transaction detail (postings)
pub fn render_transaction_detail(tx: &beanweb_core::Transaction) -> String {
    let mut known_total: f64 = 0.0;
    let mut has_known_amount = false;
    let mut common_currency = String::new();

    for p in &tx.postings {
        if !p.amount.is_empty() {
            // Use total_value to handle @ price syntax
            let (amount, currency) = p.total_value("CNY");
            if amount != 0.0 {
                known_total += amount;
                has_known_amount = true;
                if common_currency.is_empty() && !currency.is_empty() {
                    common_currency = currency;
                }
            }
        }
    }

    let missing_amount = if has_known_amount { -known_total } else { 0.0 };
    let detail_id = format!("tx-detail-{}", tx.id);
    // NOTE: "收起"按钮已移除 - 用户可以直接点击交易记录来展开/收起详情
    // 如果需要单独收起，可以点击其他交易或再次点击当前交易

    let mut html = format!(
        r#"<div class='mt-2 p-4 bg-gray-50 rounded-lg border border-indigo-200'>
        <div class='flex items-center gap-2 mb-3'>
            <span class='text-sm text-gray-500'>点击交易查看详情</span>
        </div>
        <div class='space-y-2'>"#,
    );

    for (_, posting) in tx.postings.iter().enumerate() {
        let (display_amount, amount_class) = if posting.amount.is_empty() || posting.amount == "0" || posting.amount == "-0" {
            if has_known_amount {
                let prefix = if missing_amount < 0.0 { "-" } else { "" };
                let amount_val = missing_amount.abs();
                let currency = if common_currency.is_empty() {
                    posting.currency.clone()
                } else {
                    common_currency.clone()
                };
                let suffix = if currency.is_empty() { String::new() } else { format!(" {}", currency) };
                (format!("{}{:.2}{}", prefix, amount_val, suffix), "text-indigo-600 font-medium")
            } else {
                ("-".to_string(), "text-gray-400")
            }
        } else {
            // Use total_value to handle @ price syntax for display
            let (amount_value, currency) = posting.total_value("CNY");
            let color_class = if amount_value < 0.0 {
                "text-red-600"
            } else if amount_value > 0.0 {
                "text-green-600"
            } else {
                "text-gray-400"
            };
            let prefix = if amount_value < 0.0 { "-" } else { "" };
            let suffix = if currency.is_empty() { String::new() } else { format!(" {}", currency) };
            // Keep the original price info if present
            let original_price_suffix = if posting.amount.contains("@ ") && posting.amount != format!("{} {}", amount_value, currency) {
                // Extract the original @ price part
                if let Some(at_pos) = posting.amount.find("@ ") {
                    let after_at = &posting.amount[at_pos..];
                    format!(" {}", after_at)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            (format!("{}{:.2}{}{}", prefix, amount_value.abs(), suffix, original_price_suffix), color_class)
        };

        let account_link = format!(r#"<a href="/accounts/{}" class='text-indigo-600 hover:underline truncate' title='{}'>{}</a>"#,
            urlencoding::encode(&posting.account), posting.account, posting.account);

        html.push_str(&format!(
            r#"<div class='flex items-center justify-between py-2 border-b border-gray-200 last:border-0'>
                <div class='flex items-center gap-2 flex-1 min-w-0'>
                    <span class='text-gray-400 text-sm flex-shrink-0'></span>
                    <span class='truncate text-gray-700'>{}</span>
                </div>
                <span class='font-medium flex-shrink-0 {}'>{}</span>
            </div>"#,
            account_link,
            amount_class,
            display_amount
        ));
    }

    if !tx.tags.is_empty() || !tx.links.is_empty() {
        html.push_str(r#"<div class='mt-3 flex flex-wrap gap-2'>"#);
        for tag in &tx.tags {
            html.push_str(&format!(r#"<span class='px-2 py-1 bg-blue-100 text-blue-700 rounded text-xs'>#{}</span>"#, tag));
        }
        for link in &tx.links {
            html.push_str(&format!(r#"<span class='px-2 py-1 bg-purple-100 text-purple-700 rounded text-xs'>^{}</span>"#, link));
        }
        html.push_str("</div>");
    }

    if tx.metadata.is_object() && !tx.metadata.as_object().unwrap().is_empty() {
        html.push_str(r#"<div class='mt-3 pt-3 border-t border-gray-200'><h5 class='text-xs font-medium text-gray-500 mb-2'>元数据</h5><div class='text-xs text-gray-600 font-mono'>"#);
        for (key, value) in tx.metadata.as_object().unwrap() {
            html.push_str(&format!("{}: {}, ", key, value));
        }
        html.push_str("</div></div>");
    }

    html.push_str("</div></div>");
    html
}

// NOTE: 编辑功能已禁用，以下函数已注释
// /// Render form-based edit interface with account search, tags/links, and preview
// pub fn render_edit_form(tx: &beanweb_core::Transaction) -> String {
//     let date = &tx.date;
//     let payee = &tx.payee;
//     let narration = &tx.narration;
//     let flag = tx.flag.as_deref().unwrap_or("*");
//
//     let tags_value = tx.tags.join(" ");
//     let links_value = tx.links.join(" ");
//
//     let postings_html: String = tx.postings.iter().enumerate().map(|(i, p)| {
//         format!(
//             r#"<div class='flex items-center gap-2 mb-2'>
//                 <div class='relative flex-1'>
//                     <input type='text' name='posting_{}_account' value='{}' class='w-full px-3 py-2 border rounded-lg pl-8' placeholder='搜索账户...'>
//                     <svg class='w-4 h-4 absolute left-2.5 top-3 text-gray-400' fill='none' stroke='currentColor' viewBox='0 0 24 24'>
//                         <path stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z'/>
//                     </svg>
//                 </div>
//                 <input type='text' name='posting_{}_amount' value='{}' class='w-32 px-3 py-2 border rounded-lg' placeholder='金额'>
//                 <button type='button' onclick='this.parentElement.remove()' class='px-3 py-2 text-red-500 hover:text-red-700 hover:bg-red-50 rounded-lg'>删除</button>
//             </div>"#,
//             i, p.account, i, i, i, p.amount
//         )
//     }).collect();
//
//     let initial_count = tx.postings.len();
//     let preview_text = generate_transaction_text(tx);
//     let escaped_preview = preview_text
//         .replace("&", "&amp;")
//         .replace("<", "&lt;")
//         .replace(">", "&gt;")
//         .replace("\"", "&quot;");
//
//     format!(...)
// }
//
// /// Generate Beancount text format for a transaction
// pub fn generate_transaction_text(tx: &beanweb_core::Transaction) -> String {
//     let flag = tx.flag.as_deref().unwrap_or("");
//     let flag_str = if flag.is_empty() { String::new() } else { format!("{} ", flag) };
//
//     let mut header_parts = Vec::new();
//     header_parts.push(tx.date.clone());
//     if !flag_str.is_empty() {
//         header_parts.push(flag_str.trim().to_string());
//     }
//     if !tx.payee.is_empty() {
//         header_parts.push(format!("\"{}\"", tx.payee));
//     }
//     if !tx.narration.is_empty() {
//         header_parts.push(format!("\"{}\"", tx.narration));
//     }
//     for tag in &tx.tags {
//         header_parts.push(format!("#{}", tag));
//     }
//     for link in &tx.links {
//         header_parts.push(format!("^{}", link));
//     }
//
//     let header = header_parts.join(" ");
//     let mut lines = Vec::new();
//     lines.push(header);
//
//     for p in &tx.postings {
//         let indent = "    ";
//         let amount = if p.amount.is_empty() { String::new() } else { format!(" {}", p.amount) };
//         lines.push(format!("{}{}{}", indent, p.account, amount));
//     }
//
//     lines.join("\n")
// }
