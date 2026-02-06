//! Reports page rendering - Full page endpoints

use crate::AppState;
use beanweb_core::AccountType;

pub fn render_reports_overview(ledger: &beanweb_core::Ledger) -> String {
    let balance_report = ledger.balance_report();
    let income_expense = ledger.income_expense_report();

    // Group balance entries by account type
    let assets: Vec<_> = balance_report.entries.iter()
        .filter(|e| matches!(e.account_type, AccountType::Assets))
        .collect();
    let liabilities: Vec<_> = balance_report.entries.iter()
        .filter(|e| matches!(e.account_type, AccountType::Liabilities))
        .collect();

    let mut html = String::from(r#"<div class='grid grid-cols-1 md:grid-cols-2 gap-6'>"#);

    // Assets section
    html.push_str(r#"<div class='bg-white rounded-xl shadow-sm p-6'><h3 class='text-lg font-bold mb-4'>资产</h3><div class='space-y-2'>"#);
    for entry in &assets {
        html.push_str(&format!(r#"<div class='flex justify-between py-2 border-b'><span>{}</span><span class='font-medium'>{}</span></div>"#, entry.account, entry.balance));
    }
    html.push_str("</div></div>");

    // Liabilities section
    html.push_str(r#"<div class='bg-white rounded-xl shadow-sm p-6'><h3 class='text-lg font-bold mb-4'>负债</h3><div class='space-y-2'>"#);
    for entry in &liabilities {
        html.push_str(&format!(r#"<div class='flex justify-between py-2 border-b'><span>{}</span><span class='font-medium'>{}</span></div>"#, entry.account, entry.balance));
    }
    html.push_str("</div></div>");

    // Income section
    html.push_str(r#"<div class='bg-white rounded-xl shadow-sm p-6'><h3 class='text-lg font-bold mb-4 text-green-600'>收入</h3><div class='space-y-2'>"#);
    for entry in &income_expense.income_entries {
        html.push_str(&format!(r#"<div class='flex justify-between py-2 border-b'><span>{}</span><span class='font-medium text-green-600'>{}</span></div>"#, entry.account, entry.amount));
    }
    html.push_str("</div></div>");

    // Expenses section
    html.push_str(r#"<div class='bg-white rounded-xl shadow-sm p-6'><h3 class='text-lg font-bold mb-4 text-red-600'>支出</h3><div class='space-y-2'>"#);
    for entry in &income_expense.expense_entries {
        html.push_str(&format!(r#"<div class='flex justify-between py-2 border-b'><span>{}</span><span class='font-medium text-red-600'>{}</span></div>"#, entry.account, entry.amount));
    }
    html.push_str("</div></div></div>");

    html
}

pub fn render_balance_report(ledger: &beanweb_core::Ledger) -> String {
    let balance_report = ledger.balance_report();

    let mut html = String::from(r#"<div class='overflow-x-auto'><table class='w-full'><thead class='bg-gray-50'><tr><th class='px-4 py-2 text-left'>账户</th><th class='px-4 py-2 text-right'>余额</th></tr></thead><tbody>"#);

    // Group by account type
    let types = [
        (AccountType::Assets, "资产"),
        (AccountType::Liabilities, "负债"),
        (AccountType::Equity, "权益"),
    ];

    for (account_type, type_name) in &types {
        let entries: Vec<_> = balance_report.entries.iter()
            .filter(|e| &e.account_type == account_type)
            .collect();

        if !entries.is_empty() {
            html.push_str(&format!(r#"<tr class='bg-gray-100'><td class='px-4 py-2 font-bold' colspan='2'>{}</td></tr>"#, type_name));
            for entry in &entries {
                html.push_str(&format!(r#"<tr class='border-b'><td class='px-4 py-2'>{}</td><td class='px-4 py-2 text-right font-medium'>{}</td></tr>"#,
                    entry.account, entry.balance));
            }
        }
    }
    html.push_str("</tbody></table></div>");
    html
}

pub fn render_income_expense_report(ledger: &beanweb_core::Ledger) -> String {
    let income_expense = ledger.income_expense_report();
    let mut html = String::from(r#"<div class='grid grid-cols-1 md:grid-cols-2 gap-6'><div class='bg-white rounded-xl shadow-sm p-6'><h3 class='text-lg font-bold mb-4 text-green-600'>收入</h3><div class='space-y-2'>"#);

    for entry in &income_expense.income_entries {
        html.push_str(&format!(r#"<div class='flex justify-between py-2 border-b'><span>{}</span><span class='font-medium text-green-600'>{}</span></div>"#, entry.account, entry.amount));
    }
    html.push_str("</div></div><div class='bg-white rounded-xl shadow-sm p-6'><h3 class='text-lg font-bold mb-4 text-red-600'>支出</h3><div class='space-y-2'>");

    for entry in &income_expense.expense_entries {
        html.push_str(&format!(r#"<div class='flex justify-between py-2 border-b'><span>{}</span><span class='font-medium text-red-600'>{}</span></div>"#, entry.account, entry.amount));
    }
    html.push_str("</div></div></div>");
    html
}

pub fn render_category_details(ledger: &beanweb_core::Ledger, category: &str) -> String {
    let transactions = ledger.transactions(1000, 0);
    let filtered: Vec<_> = transactions.iter()
        .filter(|tx| tx.postings.iter().any(|p| p.account.starts_with(category)))
        .collect();

    if filtered.is_empty() {
        return format!(r#"<div class='text-center py-12 text-gray-500'><p>暂无 {} 相关交易</p></div>"#, category);
    }

    let total: f64 = filtered.iter()
        .flat_map(|tx| tx.postings.iter())
        .filter(|p| p.account.starts_with(category))
        .filter_map(|p| p.amount.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()))
        .sum();

    let mut html = format!(r#"<div class='mb-4'><h3 class='text-lg font-bold'>{}</h3><p class='text-gray-500'>共 {} 笔交易，总额: {:.2}</p></div>"#, category, filtered.len(), total);

    for tx in filtered.iter().take(20) {
        let amount: f64 = tx.postings.iter()
            .filter(|p| p.account.starts_with(category))
            .filter_map(|p| p.amount.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()))
            .sum();
        html.push_str(&format!(
            r#"<div class='border rounded-lg p-3 mb-2 hover:bg-gray-50'>
                <div class='flex justify-between'>
                    <span class='text-gray-500'>{}</span>
                    <span class='font-medium {}'>{:.2}</span>
                </div>
                <div class='text-sm text-gray-700'>{}</div>
            </div>"#,
            tx.date,
            if amount < 0.0 { "text-red-600" } else { "text-green-600" },
            amount,
            if tx.payee.is_empty() { &tx.narration } else { &tx.payee }
        ));
    }

    if filtered.len() > 20 {
        html.push_str(&format!(r#"<div class='text-center text-gray-500 mt-4'>显示前 20 笔，共 {} 笔</div>"#, filtered.len()));
    }

    html
}

pub async fn page_reports(
    state: axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Html<String> {
    let ledger = state.ledger.read().await;
    let time_range = ledger.time_context().range.to_string();
    let start_date = ledger.time_context().start_date().map(|d| d.to_string()).unwrap_or_else(|| "-".to_string());
    let end_date = ledger.time_context().end_date().map(|d| d.to_string()).unwrap_or_else(|| "-".to_string());

    let inner_content = format!(
        r#"<div class='mb-6'><h2 class='text-2xl font-bold'>报表</h2></div>
        {}
        <div class='mb-4 flex gap-2'>
            <button hx-get='/reports/overview' hx-target='#reports-content' class='px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700'>概览</button>
            <button hx-get='/reports/balance' hx-target='#reports-content' class='px-4 py-2 border rounded-lg hover:bg-gray-50'>资产负债表</button>
            <button hx-get='/reports/income-expense' hx-target='#reports-content' class='px-4 py-2 border rounded-lg hover:bg-gray-50'>收支报表</button>
        </div>
        <div id='reports-content' hx-get='/reports/overview' hx-trigger='load' class='bg-white rounded-xl shadow-sm p-6'>
            <p class='text-gray-500 text-center'>加载中...</p>
        </div>"#,
        crate::page_time_selector(&time_range, &start_date, &end_date)
    );

    axum::response::Html(crate::page_response_with_time(&headers, "报表", "/reports", &inner_content, &time_range))
}
