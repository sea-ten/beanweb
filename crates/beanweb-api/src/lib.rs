//! HTTP API server with HTMX support
//!
//! Routes are organized into modules:
//! - routes::transactions: Transaction list, search, pagination
//! - routes::accounts: Account list, tree view
//! - routes::reports: Balance and income-expense reports
//! - routes::settings: Configuration display

pub mod error;
pub mod routes;

use axum::{
    routing::{get, put, post},
    Router,
};
use tokio::net::TcpListener;
use beanweb_core::{Ledger, LedgerOperations};
use beanweb_config::Config;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use error::ApiError;

/// Application state
#[derive(Clone)]
pub struct AppState {
    pub ledger: Arc<RwLock<Ledger>>,
    pub config: Config,
}

/// Create the application router
pub fn create_router(state: AppState) -> Router {
    // Import route handlers
    use routes::transactions::{api_transactions, api_transaction_detail, htmx_transactions_list, htmx_transactions_filter, htmx_transaction_detail, page_transactions, page_transaction_create, htmx_transaction_create_form, htmx_transaction_store};
    use routes::accounts::{api_accounts, htmx_accounts_list, htmx_account_suggest, page_accounts, page_account_detail, htmx_account_transactions_list};
    // NOTE: 报表功能已禁用
    // use routes::reports::{api_balance_report, api_income_expense, page_reports, htmx_reports_overview, htmx_reports_balance, htmx_reports_income_expense, htmx_reports_category};
    use routes::settings::{api_settings, api_settings_metadata, page_settings};
    use routes::time::{api_time_range, api_set_time_range, api_time_range_options, api_time_range_months, api_time_range_years};
    use routes::files::{api_files_list, api_file_content, api_file_save, page_files, page_file_edit};
    // NOTE: 货币功能已禁用
    // use crate::routes::commodities::page_commodities;

    Router::new()
        // API endpoints
        .route("/api/health", get(health_check))
        .route("/api/accounts", get(api_accounts))
        .route("/api/transactions", get(api_transactions))
        .route("/api/transactions/:id", get(api_transaction_detail))
        .route("/api/summary", get(api_summary))
        // NOTE: 报表API已禁用
        // .route("/api/reports/balance", get(api_balance_report))
        // .route("/api/reports/income-expense", get(api_income_expense))
        .route("/api/settings", get(api_settings))
        .route("/api/settings/metadata", get(api_settings_metadata))
        .route("/api/time-range", get(api_time_range))
        .route("/api/time-range", post(api_set_time_range))
        .route("/api/time-range/options", get(api_time_range_options))
        .route("/api/time-range/months", get(api_time_range_months))
        .route("/api/time-range/years", get(api_time_range_years))
        .route("/api/files", get(api_files_list))
        .route("/api/files/*path", get(api_file_content))
        .route("/api/files/*path", put(api_file_save))
        .route("/api/reload", post(api_reload))
        // HTMX page routes
        .route("/", get(index_page))
        .route("/dashboard", get(page_dashboard))
        .route("/accounts", get(page_accounts))
        .route("/accounts/:name", get(page_account_detail))
        .route("/transactions", get(page_transactions))
        // NOTE: 报表页面已禁用
        // .route("/reports", get(page_reports))
        // .route("/reports/overview", get(htmx_reports_overview))
        // .route("/reports/balance", get(htmx_reports_balance))
        // .route("/reports/income-expense", get(htmx_reports_income_expense))
        // .route("/reports/category", get(htmx_reports_category))
        .route("/files", get(page_files))
        .route("/files/*path", get(page_file_edit))
        // NOTE: 货币页面已禁用
        // .route("/commodities", get(page_commodities))
        .route("/settings", get(page_settings))
        // HTMX partial routes (for tab content)
        .route("/accounts/list", get(htmx_accounts_list))
        .route("/accounts/suggest", get(htmx_account_suggest))
        .route("/accounts/:name/transactions/list", get(htmx_account_transactions_list))
        .route("/transactions/list", get(htmx_transactions_list))
        .route("/transactions/filter", get(htmx_transactions_filter))
        .route("/transactions/:id/detail", get(htmx_transaction_detail))
        // NOTE: 编辑功能已禁用
        // .route("/transactions/:id/edit", get(page_transaction_edit))
        // .route("/transactions/:id/edit/form", get(htmx_transaction_edit_form))
        // .route("/transactions/:id", put(htmx_transaction_update))
        // Transaction create routes
        .route("/transactions/create", get(page_transaction_create))
        .route("/transactions/create/form", get(htmx_transaction_create_form))
        .route("/transactions", post(htmx_transaction_store))
        .with_state(state)
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Get ledger summary (JSON API)
async fn api_summary(state: axum::extract::State<AppState>) -> String {
    let ledger = state.ledger.read().await;
    let summary = ledger.summary();
    serde_json::to_string(&summary).unwrap_or_default()
}

// ==================== Template Functions ====================

/// Base HTML template
pub fn base_html(title: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{} - Beanweb</title>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <script src="https://cdn.tailwindcss.com"></script>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/tailwindcss@2.2.19/dist/tailwind.min.css">
    <style>
        .htmx-indicator {{ opacity: 0; transition: opacity 0.3s; }}
        .htmx-request .htmx-indicator {{ opacity: 1; }}
        .htmx-request.htmx-indicator {{ opacity: 1; }}
    </style>
</head>
<body class="bg-gray-50 text-gray-900">
    {}
</body>
</html>"#,
        title, content
    )
}

/// Navigation sidebar (without time selector - for backward compatibility)
pub fn nav_sidebar(current_path: &str) -> String {
    let links = [
        ("/", "仪表盘", "dashboard"),
        ("/accounts", "账户", "accounts"),
        ("/transactions", "流水", "transactions"),
        // NOTE: 货币功能已禁用
        // ("/commodities", "货币", "commodities"),
        // NOTE: 报表功能已禁用
        // ("/reports", "报表", "reports"),
        ("/files", "文件", "files"),
        ("/settings", "设置", "settings"),
    ];

    let mut nav = String::from("<div class='bg-white border-r h-screen flex flex-col'><div class='p-4 border-b'><h1 class='text-xl font-bold text-indigo-600'>Beanweb</h1></div><ul class='flex-1 py-2 space-y-1 px-2'>");

    for (path, label, id) in &links {
        let is_active = if *path == "/" {
            current_path == "/"
        } else {
            current_path.starts_with(path)
        };
        let active_class = if is_active { "bg-indigo-50 text-indigo-600" } else { "text-gray-600 hover:bg-gray-50" };
        let icon = match *id {
            "dashboard" => "📊",
            "accounts" => "💰",
            "transactions" => "📋",
            "reports" => "📈",
            "files" => "📄",
            "settings" => "⚙️",
            _ => "📄",
        };
        nav.push_str(&format!(
            r#"<li><a href='{}' class='flex items-center gap-2 px-3 py-2 rounded-lg {}'>{}<span>{}</span></a></li>"#,
            path, active_class, icon, label
        ));
    }
    nav.push_str("</ul></div>");
    nav
}

/// Page-level time selector - For placing at top of pages
/// With year/month dropdowns that can be combined, plus custom date picker with drill-down
pub fn page_time_selector(current_range: &str, _start_date: &str, _end_date: &str) -> String {
    // Parse current year and month from range string
    let (selected_year, selected_month) = if current_range.starts_with("year:") {
        let year = current_range.strip_prefix("year:").unwrap_or("");
        if year.ends_with("-01") || year.ends_with("-02") || year.ends_with("-03") ||
           year.ends_with("-04") || year.ends_with("-05") || year.ends_with("-06") ||
           year.ends_with("-07") || year.ends_with("-08") || year.ends_with("-09") ||
           year.ends_with("-10") || year.ends_with("-11") || year.ends_with("-12") {
            // Format: year:2026-01 (year + month)
            let parts: Vec<&str> = year.split('-').collect();
            if parts.len() == 2 {
                (parts[0], parts[1])
            } else {
                (year, "")
            }
        } else {
            (year, "")
        }
    } else if current_range.starts_with("month:") {
        let month = current_range.strip_prefix("month:").unwrap_or("");
        ("", month)
    } else {
        ("", "")
    };

    // Parse custom range
    let (custom_start, custom_end) = if current_range.starts_with("custom:") {
        if let Some(custom_str) = current_range.strip_prefix("custom:") {
            let parts: Vec<&str> = custom_str.split(',').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                ("".to_string(), "".to_string())
            }
        } else {
            ("".to_string(), "".to_string())
        }
    } else {
        ("".to_string(), "".to_string())
    };

    let show_custom = if current_range.starts_with("custom:") { "display: block" } else { "display: none" };

    format!(r#"
        <div class='flex items-center gap-3 mb-4 p-3 bg-white rounded-lg border shadow-sm' id='page-time-selector'>
            <span class='text-sm font-medium text-gray-600 flex-shrink-0'>时间:</span>
            <!-- Year selector: 全部 or specific year -->
            <select id='year-select' class='px-2 py-1.5 text-sm border rounded-lg bg-white min-w-[100px]' onchange='handleYearChange()'>
                <option value='' {}>年份</option>
                <option value='all' {}>全部</option>
                <!-- Years loaded dynamically -->
                <optgroup label='选择年份' id='year-options'>
                    <option value='' disabled>加载中...</option>
                </optgroup>
            </select>
            <span class='text-gray-400' id='month-separator' style='display: none;'>-</span>
            <!-- Month selector: cascading, enabled only when year selected -->
            <select id='month-select' class='px-2 py-1.5 text-sm border rounded-lg bg-white min-w-[90px]' onchange='handleMonthChange()' disabled style='background-color: #f9fafb;'>
                <option value='all' {}>月份</option>
                <option value='' {}>全年</option>
                <option value='01' {}>1月</option>
                <option value='02' {}>2月</option>
                <option value='03' {}>3月</option>
                <option value='04' {}>4月</option>
                <option value='05' {}>5月</option>
                <option value='06' {}>6月</option>
                <option value='07' {}>7月</option>
                <option value='08' {}>8月</option>
                <option value='09' {}>9月</option>
                <option value='10' {}>10月</option>
                <option value='11' {}>11月</option>
                <option value='12' {}>12月</option>
            </select>
            <div class='flex-1'></div>
            <button onclick='toggleTimeSelector()' class='px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50 flex items-center gap-1'>
                <svg xmlns='http://www.w3.org/2000/svg' class='h-4 w-4' fill='none' viewBox='0 0 24 24' stroke='currentColor'>
                    <path stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z'/>
                </svg>
                自定义
            </button>
        </div>
        <!-- Time selector dropdown (hidden by default) -->
        <div id='time-selector-dropdown' class='hidden mb-4 p-4 bg-white rounded-lg border shadow-lg' style='position: absolute; z-index: 50; min-width: 360px;'>
            <div class='flex items-center justify-between mb-3'>
                <h4 class='font-medium'>自定义日期范围</h4>
                <button onclick='toggleTimeSelector()' class='text-gray-400 hover:text-gray-600'>
                    <svg xmlns='http://www.w3.org/2000/svg' class='h-5 w-5' fill='none' viewBox='0 0 24 24' stroke='currentColor'>
                        <path stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M6 18L18 6M6 6l12 12'/>
                    </svg>
                </button>
            </div>
            <!-- Step 1: Select Year -->
            <div id='custom-step1' class='space-y-3'>
                <label class='text-sm font-medium text-gray-700'>选择年份</label>
                <select id='custom-year-select' class='w-full px-3 py-2 border rounded-lg bg-white' onchange='customYearChanged()'>
                    <option value=''>请选择年份</option>
                    <optgroup label='可用年份' id='custom-year-options'>
                        <option value='' disabled>加载中...</option>
                    </optgroup>
                </select>
            </div>
            <!-- Step 2: Select Month (hidden initially) -->
            <div id='custom-step2' class='space-y-3' style='display: none;'>
                <div class='flex items-center justify-between'>
                    <label class='text-sm font-medium text-gray-700'>选择月份(从当年1月到所选月份)</label>
                    <button onclick='backToYearSelect()' class='text-xs text-indigo-600 hover:underline'>返回年份</button>
                </div>
                <div class='grid grid-cols-4 gap-2'>
                    <button onclick='selectCustomMonth("01")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>1月</button>
                    <button onclick='selectCustomMonth("02")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>2月</button>
                    <button onclick='selectCustomMonth("03")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>3月</button>
                    <button onclick='selectCustomMonth("04")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>4月</button>
                    <button onclick='selectCustomMonth("05")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>5月</button>
                    <button onclick='selectCustomMonth("06")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>6月</button>
                    <button onclick='selectCustomMonth("07")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>7月</button>
                    <button onclick='selectCustomMonth("08")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>8月</button>
                    <button onclick='selectCustomMonth("09")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>9月</button>
                    <button onclick='selectCustomMonth("10")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>10月</button>
                    <button onclick='selectCustomMonth("11")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>11月</button>
                    <button onclick='selectCustomMonth("12")' class='px-2 py-2 text-sm border rounded hover:bg-gray-50'>12月</button>
                </div>
                <p id='custom-selected-date' class='text-sm text-gray-500 text-center'></p>
            </div>
            <!-- Direct date input (alternative) -->
            <div class='mt-3 pt-3 border-t'>
                <label class='text-xs text-gray-500 mb-1 block'>或者直接输入日期范围</label>
                <div class='flex items-center gap-2 mb-2'>
                    <input type='date' id='custom-start' value='{}' class='flex-1 px-2 py-1.5 text-sm border rounded'>
                    <span class='text-gray-400'>至</span>
                    <input type='date' id='custom-end' value='{}' class='flex-1 px-2 py-1.5 text-sm border rounded'>
                </div>
                <button onclick='applyCustomRange()' class='w-full px-3 py-2 bg-indigo-600 text-white text-sm rounded-lg hover:bg-indigo-700'>
                    应用
                </button>
            </div>
        </div>
        <script>
        let customSelectedYear = '';

        // Fetch available years for custom picker
        function loadCustomYearOptions() {{
            fetch('/api/time-range/years')
                .then(r => r.json())
                .then(years => {{
                    const yearGroup = document.getElementById('custom-year-options');
                    if (yearGroup && years.length > 0) {{
                        yearGroup.innerHTML = years.map(y =>
                            `<option value="${{y}}">${{y}} 年</option>`
                        ).join('');
                    }}
                }})
                .catch(err => console.error('Failed to load years:', err));
        }}

        // Fetch available years and populate main selector
        function loadYearOptions() {{
            fetch('/api/time-range/years')
                .then(r => r.json())
                .then(years => {{
                    const yearGroup = document.getElementById('year-options');
                    if (yearGroup && years.length > 0) {{
                        const currentYear = document.getElementById('year-select').value.split('-')[0];
                        yearGroup.innerHTML = years.map(y =>
                            `<option value="${{y}}" ${{y === currentYear ? 'selected' : ''}}>${{y}} 年</option>`
                        ).join('');
                        // If a year is already selected, trigger change to update month selector
                        if (document.getElementById('year-select').value && document.getElementById('year-select').value !== 'all') {{
                            handleYearChange();
                        }}
                    }}
                }})
                .catch(err => console.error('Failed to load years:', err));
        }}

        // Handle year selection change - enables month selector
        function handleYearChange() {{
            const yearSelect = document.getElementById('year-select');
            const monthSelect = document.getElementById('month-select');
            const separator = document.getElementById('month-separator');
            const year = yearSelect.value;

            if (year === '' || year === 'all') {{
                // Reset to all - no time filter
                monthSelect.value = '';
                monthSelect.disabled = true;
                monthSelect.style.backgroundColor = '#f9fafb';
                separator.style.display = 'none';
                setTimeRange('all');
            }} else {{
                // Year selected - enable month selector
                monthSelect.disabled = false;
                monthSelect.style.backgroundColor = '';
                separator.style.display = 'inline';
                // Default to full year, user can select specific month
                monthSelect.value = 'all';
            }}
        }}

        // Handle month selection change
        function handleMonthChange() {{
            const yearSelect = document.getElementById('year-select');
            const monthSelect = document.getElementById('month-select');
            const year = yearSelect.value;
            const month = monthSelect.value;

            if (!year || year === 'all') {{
                // Should not happen due to disabled state, but handle gracefully
                return;
            }}

            if (month === '' || month === 'all') {{
                // Full year selected
                setTimeRange('year:' + year);
            }} else {{
                // Specific month selected
                setTimeRange('year:' + year + '-' + month);
            }}
        }}

        function setTimeRange(range) {{
            let apiRange = range;
            if (range === 'all') {{
                apiRange = 'all';
            }} else if (range.match(/^\d{4}$/)) {{
                // 4-digit year -> add year: prefix
                apiRange = 'year:' + range;
            }} else if (range.includes('-') && range.length === 7) {{
                // year-month format (like 2025-03)
                apiRange = 'year:' + range;
            }}

            console.log('[DEBUG] setTimeRange: original=' + range + ', api=' + apiRange);
            htmx.ajax('POST', '/api/time-range?range=' + encodeURIComponent(apiRange),
                {{ target: 'body', swap: 'none' }}).then(() => {{
                    window.location.reload();
                }}).catch(err => {{
                    console.error('Failed to set time range:', err);
                    window.location.reload();
                }});
        }}

        function toggleTimeSelector() {{
            const dropdown = document.getElementById('time-selector-dropdown');
            if (dropdown) {{
                dropdown.classList.toggle('hidden');
                if (!dropdown.classList.contains('hidden')) {{
                    // Reset to step 1 when opening
                    document.getElementById('custom-step1').style.display = 'block';
                    document.getElementById('custom-step2').style.display = 'none';
                    loadCustomYearOptions();
                }}
            }}
        }}

        // Custom date picker drill-down functions
        function customYearChanged() {{
            const yearSelect = document.getElementById('custom-year-select');
            customSelectedYear = yearSelect.value;
            if (customSelectedYear) {{
                document.getElementById('custom-step1').style.display = 'none';
                document.getElementById('custom-step2').style.display = 'block';
                document.getElementById('custom-selected-date').textContent = '已选择: ' + customSelectedYear + ' 年';
            }}
        }}

        function backToYearSelect() {{
            document.getElementById('custom-step1').style.display = 'block';
            document.getElementById('custom-step2').style.display = 'none';
            customSelectedYear = '';
        }}

        function selectCustomMonth(month) {{
            if (customSelectedYear) {{
                const range = customSelectedYear + '-' + month;
                setTimeRange(range);
                toggleTimeSelector(); // Close dropdown
            }}
        }}

        function applyCustomRange() {{
            const start = document.getElementById('custom-start').value;
            const end = document.getElementById('custom-end').value;
            if (start && end) {{
                htmx.ajax('POST', '/api/time-range?range=custom:' + start + ',' + end,
                    {{ target: 'body', swap: 'none' }}).then(() => {{
                        window.location.reload();
                    }}).catch(err => {{
                        console.error('Failed to set custom range:', err);
                        window.location.reload();
                    }});
            }}
        }}

        // Initialize on load
        document.addEventListener('DOMContentLoaded', function() {{
            // Load year options first, then sync state
            loadYearOptions();

            // If a year is already selected in the dropdown, enable month selector
            const yearSelect = document.getElementById('year-select');
            if (yearSelect && yearSelect.value && yearSelect.value !== 'all') {{
                handleYearChange();
            }} else {{
                // Ensure month selector is disabled when no year selected
                const monthSelect = document.getElementById('month-select');
                const separator = document.getElementById('month-separator');
                if (monthSelect) {{
                    monthSelect.disabled = true;
                    monthSelect.style.backgroundColor = '#f9fafb';
                }}
                if (separator) {{
                    separator.style.display = 'none';
                }}
            }}
        }});

        // Close dropdown when clicking outside
        document.addEventListener('click', function(e) {{
            const dropdown = document.getElementById('time-selector-dropdown');
            const toggle = document.querySelector('button[onclick="toggleTimeSelector()"]');
            if (dropdown && !dropdown.classList.contains('hidden') && !dropdown.contains(e.target) && (!toggle || !toggle.contains(e.target))) {{
                dropdown.classList.add('hidden');
            }}
        }});
        </script>
    "#,
        if selected_year.is_empty() { "selected" } else { "" },
        if selected_year == "all" { "selected" } else { "" },
        if selected_month.is_empty() { "selected" } else { "" },
        if selected_month == "all" { "selected" } else { "" },
        if selected_month == "01" { "selected" } else { "" },
        if selected_month == "02" { "selected" } else { "" },
        if selected_month == "03" { "selected" } else { "" },
        if selected_month == "04" { "selected" } else { "" },
        if selected_month == "05" { "selected" } else { "" },
        if selected_month == "06" { "selected" } else { "" },
        if selected_month == "07" { "selected" } else { "" },
        if selected_month == "08" { "selected" } else { "" },
        if selected_month == "09" { "selected" } else { "" },
        if selected_month == "10" { "selected" } else { "" },
        if selected_month == "11" { "selected" } else { "" },
        if selected_month == "12" { "selected" } else { "" },
        custom_start,
        custom_end
    )
}

/// Header bar - Wrapper for main content area
pub fn header_bar(current_path: &str) -> String {
    format!(r#"<div class='flex flex-col h-screen'>
    <div class='flex flex-1 overflow-hidden'>
        <aside class='w-64 flex-shrink-0'>{}</aside>
        <main class='flex-1 overflow-auto bg-gray-50 p-6'>"#,
        nav_sidebar(current_path))
}

/// Check if request is from HTMX (partial page update)
fn is_htmx_request(headers: &axum::http::HeaderMap) -> bool {
    headers.get("hx-request").is_some()
}

/// Wrap content for full page or HTMX partial (with time range)
pub fn page_response(headers: &axum::http::HeaderMap, title: &str, current_path: &str, inner_content: &str) -> String {
    page_response_with_time(headers, title, current_path, inner_content, "month")
}

/// Wrap content for full page or HTMX partial with time range
pub fn page_response_with_time(headers: &axum::http::HeaderMap, title: &str, current_path: &str, inner_content: &str, time_range: &str) -> String {
    if is_htmx_request(headers) {
        // HTMX partial - just the content area (no sidebar for partial updates)
        format!(r#"<div class='flex flex-col h-screen'>
    <div class='flex flex-1 overflow-hidden'>
        <main class='flex-1 overflow-auto bg-gray-50 p-6'>{}</main>
    </div>
</div>"#,
            inner_content)
    } else {
        // Full page - wrap with base HTML and sidebar (without time selector)
        base_html(title, &format!(r#"<div class='flex flex-col h-screen'>
    <div class='flex flex-1 overflow-hidden'>
        <aside class='w-64 flex-shrink-0'>{}</aside>
        <main class='flex-1 overflow-auto bg-gray-50 p-6'>{}</main>
    </div>
</div>"#,
            nav_sidebar(current_path), inner_content))
    }
}

/// Index page with navigation
async fn index_page(
    state: axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Html<String> {
    let ledger = state.ledger.read().await;
    let stats = ledger.transaction_stats();
    let balance_report = ledger.balance_report();
    let income_expense = ledger.income_expense_report();
    let time_range = ledger.time_context().range.to_string();

    let top_assets: Vec<String> = balance_report.entries.iter().filter(|e| e.account_type == beanweb_core::AccountType::Assets).take(5).map(|e| {
        format!("<div class='flex justify-between py-2 border-b'><span>{}</span><span class='font-medium'>{}</span></div>", e.account, e.balance)
    }).collect();

    let net_income_value: f64 = income_expense.net_income.parse().unwrap_or(0.0);

    let inner_content = format!(
        r#"<div class='mb-6'><h2 class='text-2xl font-bold'>仪表盘</h2></div>
        <div class='grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6'>
            <div class='bg-green-50 p-4 rounded-lg border border-green-200'><p class='text-sm text-green-600'>总资产</p><p class='text-2xl font-bold text-green-700'>{}</p></div>
            <div class='bg-red-50 p-4 rounded-lg border border-red-200'><p class='text-sm text-red-600'>总负债</p><p class='text-2xl font-bold text-red-700'>{}</p></div>
            <div class='bg-blue-50 p-4 rounded-lg border border-blue-200'><p class='text-sm text-blue-600'>总收入</p><p class='text-2xl font-bold text-blue-700'>{}</p></div>
            <div class='bg-yellow-50 p-4 rounded-lg border border-yellow-200'><p class='text-sm text-yellow-600'>总支出</p><p class='text-2xl font-bold text-yellow-700'>{}</p></div>
        </div>
        <div class='grid grid-cols-1 lg:grid-cols-2 gap-6'>
            <div class='bg-white rounded-xl shadow-sm p-6'>
                <h3 class='text-lg font-semibold mb-4'>资产排名</h3>
                <div class='space-y-1'>{}</div>
            </div>
            <div class='bg-white rounded-xl shadow-sm p-6'>
                <h3 class='text-lg font-semibold mb-4'>本月统计</h3>
                <div class='grid grid-cols-2 gap-4'>
                    <div class='text-center p-4 bg-gray-50 rounded-lg'><p class='text-sm text-gray-600'>交易数</p><p class='text-xl font-bold'>{}</p></div>
                    <div class='text-center p-4 bg-gray-50 rounded-lg'><p class='text-sm text-gray-600'>条目数</p><p class='text-xl font-bold'>{}</p></div>
                    <div class='text-center p-4 bg-gray-50 rounded-lg'><p class='text-sm text-gray-600'>净资产</p><p class='text-xl font-bold text-indigo-600'>{}</p></div>
                    <div class='text-center p-4 bg-gray-50 rounded-lg'><p class='text-sm text-gray-600'>收支结余</p><p class='text-xl font-bold {}'>{}</p></div>
                </div>
            </div>
        </div>"#,
        balance_report.total_assets,
        balance_report.total_liabilities,
        income_expense.total_income,
        income_expense.total_expenses,
        top_assets.join(""),
        stats.total_transactions,
        stats.total_postings,
        balance_report.net_worth,
        if net_income_value < 0.0 { "text-red-600" } else { "text-green-600" },
        income_expense.net_income
    );

    axum::response::Html(page_response_with_time(&headers, "仪表盘", "/dashboard", &inner_content, &time_range))
}

/// Dashboard page (alias for index)
async fn page_dashboard(
    state: axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Html<String> {
    index_page(state, headers).await
}

/// Start the HTTP server
///
/// This is the main entry point for the Beanweb server.
/// It creates the router, binds to the address, and starts listening for requests.
///
/// # Arguments
///
/// * `config` - The application configuration
/// * `ledger` - The shared ledger state
pub async fn start_server(config: Config, ledger: Arc<RwLock<beanweb_core::Ledger>>) {
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let state = AppState { ledger, config };

    let router = create_router(state);

    let listener = TcpListener::bind(&addr).await.unwrap();
    eprintln!("[INFO] Starting Beanweb server on http://{}", addr);
    eprintln!("[INFO] Available routes:");
    eprintln!("[INFO]   - / (Dashboard)");
    eprintln!("[INFO]   - /accounts (Account management)");
    eprintln!("[INFO]   - /transactions (Transaction list)");
    eprintln!("[INFO]   - /reports (Financial reports)");
    eprintln!("[INFO]   - /settings (Configuration)");
    eprintln!("[INFO]   - /api/* (JSON API endpoints)");

    match axum::serve(listener, router).await
    {
        Ok(_) => eprintln!("[INFO] Server stopped gracefully"),
        Err(e) => eprintln!("[ERROR] Server error: {}", e),
    }
}

/// Reload ledger API endpoint
async fn api_reload(state: axum::extract::State<AppState>) -> String {
    let mut ledger = state.ledger.write().await;
    match ledger.reload().await {
        Ok(_) => r#"{"success": true, "message": "账本已重新加载"}"#.to_string(),
        Err(e) => format!(r#"{{"success": false, "message": "{}"}}"#, e),
    }
}
