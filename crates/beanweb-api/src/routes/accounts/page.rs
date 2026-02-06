//! Accounts page rendering - Full page endpoints

use crate::AppState;
use axum::extract::{Path, Query};
use std::collections::HashMap;

use super::api::AccountTreeNode;

pub use super::api::AccountTreeNode as AccountNode;

/// Calculate the amount for a specific account in a transaction
/// Handles multiple postings to the same account and empty amounts (inferred from other postings)
pub fn get_posting_amount_for_account(tx: &beanweb_core::Transaction, account_name: &str) -> f64 {
    // First, collect ALL postings to this account
    let account_postings: Vec<_> = tx.postings.iter()
        .filter(|p| p.account == account_name)
        .collect();

    // If there are postings with explicit amounts, sum them all
    let mut explicit_total: f64 = 0.0;
    let mut has_explicit = false;

    for posting in &account_postings {
        if !posting.amount.is_empty() {
            let (val, _) = posting.total_value("CNY");
            explicit_total += val;
            has_explicit = true;
        }
    }

    if has_explicit {
        eprintln!("[DEBUG get_posting_amount] tx={} account={} explicit={}", tx.date, account_name, explicit_total);
        return explicit_total;
    }

    // If no explicit amount, calculate from OTHER postings (Beancount double-entry)
    let mut known_total: f64 = 0.0;
    let mut has_known_amount = false;

    for p in &tx.postings {
        if !p.amount.is_empty() {
            let (amount, _) = p.total_value("CNY");
            if amount != 0.0 {
                known_total += amount;
                has_known_amount = true;
            }
        }
    }

    // For empty amount posting, the amount is the negative of known total
    let result = if has_known_amount {
        -known_total
    } else {
        0.0
    };
    eprintln!("[DEBUG get_posting_amount] tx={} account={} inferred={} (known_total={})", tx.date, account_name, result, known_total);
    result
}

fn format_amount(value: &str) -> String {
    if value.is_empty() { String::new() } else { value.to_string() }
}

/// Parse amount string to f64, handling currency and signs
/// Handles formats like "12,306.11 CNY", "-6307.77 CNY", "100.00"
pub fn parse_amount(amount_str: &str) -> f64 {
    if amount_str.is_empty() {
        return 0.0;
    }
    // Remove commas and extract the first number
    let cleaned: String = amount_str.chars().filter(|&c| c != ',').collect();

    let chars: Vec<char> = cleaned.chars().collect();
    let mut i = 0;
    let mut has_minus = false;

    // Find the start of a number (digit or minus followed by digit)
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() {
            // Found start of number
            let mut num_str = String::new();
            if has_minus {
                num_str.push('-');
            }
            // Collect digits and decimal point
            let mut j = i;
            while j < chars.len() {
                let nc = chars[j];
                if nc.is_ascii_digit() || nc == '.' {
                    num_str.push(nc);
                    j += 1;
                } else {
                    break;
                }
            }
            if !num_str.is_empty() {
                return num_str.parse::<f64>().unwrap_or(0.0);
            }
        } else if c == '-' {
            has_minus = true;
        } else {
            has_minus = false;
        }
        i += 1;
    }
    0.0
}

fn is_negative(value: &str) -> bool {
    value.parse::<f64>().map_or(false, |n| n < 0.0)
}

fn render_account_node(node: &AccountNode, depth: usize, search_term: &str, hide_closed: bool) -> String {
    if hide_closed && node.account_status == "closed" { return String::new(); }

    let node_matches = if search_term.is_empty() {
        true
    } else {
        let search_lower = search_term.to_lowercase();
        node.name.to_lowercase().contains(&search_lower)
            || node.alias.as_ref().map_or(false, |a| a.to_lowercase().contains(&search_lower))
    };

    fn has_matching_descendant(node: &AccountNode, search_lower: &str, hide_closed: bool) -> bool {
        if hide_closed && node.account_status == "closed" { return false; }
        if node.name.to_lowercase().contains(search_lower)
            || node.alias.as_ref().map_or(false, |a| a.to_lowercase().contains(search_lower)) {
            return true;
        }
        if let Some(children) = &node.children {
            children.iter().any(|c| has_matching_descendant(c, search_lower, hide_closed))
        } else {
            false
        }
    }

    let visible_children: Vec<&AccountNode> = node.children.as_ref()
        .map(|children| {
            if search_term.is_empty() {
                children.iter().filter(|child| {
                    let child_not_closed = !hide_closed || child.account_status != "closed";
                    child_not_closed
                }).collect()
            } else {
                let search_lower = search_term.to_lowercase();
                children.iter().filter(|child| {
                    let child_not_closed = !hide_closed || child.account_status != "closed";
                    let child_matches = child.name.to_lowercase().contains(&search_lower)
                        || child.alias.as_ref().map_or(false, |a| a.to_lowercase().contains(&search_lower));
                    child_not_closed && (child_matches || has_matching_descendant(child, &search_lower, hide_closed))
                }).collect()
            }
        }).unwrap_or_default();

    let should_show = if search_term.is_empty() {
        true
    } else if node_matches {
        true
    } else {
        !visible_children.is_empty()
    };

    if !should_show { return String::new(); }

    let has_visible_children = !visible_children.is_empty();
    let display_name = node.alias.as_ref().unwrap_or(&node.short_name);
    let indent_html = if depth > 0 { format!(r#"<span class="inline-block" style="width: {}px"></span>"#, depth * 24) } else { String::new() };
    let toggle_html = if has_visible_children {
        r#"<svg class="w-4 h-4 text-gray-400 mr-1 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/></svg>"#
    } else {
        r#"<span class="w-4 h-4 mr-1 flex-shrink-0"></span>"#
    };
    let account_html = if node.is_real {
        format!(r#"<a href="/accounts/{}" class="text-indigo-600 hover:text-indigo-800 font-medium">{}</a>"#, urlencoding::encode(&node.name), display_name)
    } else {
        format!(r#"<span class="font-medium text-gray-700">{}</span>"#, display_name)
    };
    let closed_badge = if node.account_status == "closed" { String::from(r#"<span class="ml-2 px-2 py-0.5 text-xs border rounded text-gray-500">Close</span>"#) } else { String::new() };

    // Format multi-currency display
    // Show CNY in main column, Other currencies in separate part
    let cny_amount = node.amount.detail.get("CNY").map(|s| s.clone()).unwrap_or_else(|| "0.00".to_string());
    let cny_num: f64 = cny_amount.parse().unwrap_or(0.0);
    let cny_class = if cny_num < 0.0 { "text-red-600" } else if cny_num == 0.0 { "text-gray-400" } else { "text-green-600" };
    let cny_html = format!(r#"<span class="font-medium {}">{}</span>"#, cny_class, cny_amount);

    // Check for other currencies
    let other_currencies: Vec<_> = node.amount.detail.iter()
        .filter(|(k, _)| k != &"CNY")
        .filter(|(_, v)| {
            if let Ok(n) = (*v).parse::<f64>() {
                n != 0.0
            } else {
                !v.is_empty() && v.as_str() != "0.00"
            }
        })
        .collect();

    let other_html = if other_currencies.is_empty() {
        String::new()
    } else {
        let other_text: Vec<String> = other_currencies.iter()
            .map(|(currency, amount)| format!("{}: {}", currency, amount))
            .collect();
        format!(r#"<span class="font-medium text-blue-600 ml-2" title="Other currencies">({})</span>"#, other_text.join(", "))
    };

    // Add currency label (CNY)
    let amount_html = format!(r#"{}{} <span class="text-gray-400 text-sm">CNY</span>"#, cny_html, other_html);

    let row = if has_visible_children && !node.is_real {
        // Virtual node (category) with children - show as collapsible group
        format!(r#"<details class="pl-4" open><summary class="flex items-center py-2 px-3 hover:bg-gray-50 cursor-pointer list-none" data-path="{}"><div class="flex items-center flex-1 min-w-0">{}{}{}{}</div><div class="flex items-center gap-2 flex-shrink-0">{}</div></summary>"#,
            node.path, indent_html, toggle_html, account_html, closed_badge, amount_html)
    } else if node.is_real {
        // Real account (with its own transactions) - show as regular row
        // If it also has children, they will be rendered below
        format!(r#"<div class="flex items-center py-2 px-3 hover:bg-gray-50 border-b border-gray-100" data-path="{}"><div class="flex items-center flex-1 min-w-0">{}{}{}{}</div><div class="flex items-center gap-2 flex-shrink-0">{}</div></div>"#,
            node.path, indent_html, toggle_html, account_html, closed_badge, amount_html)
    } else {
        // Real account without children - show as regular row
        format!(r#"<div class="flex items-center py-2 px-3 hover:bg-gray-50 border-b border-gray-100" data-path="{}"><div class="flex items-center flex-1 min-w-0">{}{}{}{}</div><div class="flex items-center gap-2 flex-shrink-0">{}</div></div>"#,
            node.path, indent_html, toggle_html, account_html, closed_badge, amount_html)
    };

    let mut html = row;
    if has_visible_children {
        for child in visible_children {
            html.push_str(&render_account_node(child, depth + 1, search_term, hide_closed));
        }
        // Close the details tag if this is a virtual node (category)
        if !node.is_real && has_visible_children {
            html.push_str("</details>");
        }
    }
    html
}

pub fn render_accounts_tree(tree: &[AccountNode], search_term: String, hide_closed: bool) -> String {
    if tree.is_empty() { return String::from(r#"<div class="text-center py-12 text-gray-500"><p>暂无账户数据</p></div>"#); }
    let mut html = String::new();
    for node in tree { html.push_str(&render_account_node(node, 0, &search_term, hide_closed)); }
    if html.is_empty() { return String::from(r#"<div class="text-center py-12 text-gray-500"><p>没有找到匹配的账户</p></div>"#); }
    html
}

pub fn render_accounts_summary(tree: &[AccountNode]) -> (String, String, String, String) {
    fn calculate_total(node: &AccountNode, account_type: &str) -> f64 {
        let is_type = node.name.starts_with(account_type);
        let own_balance = node.amount.calculated.number.parse::<f64>().unwrap_or(0.0);
        let children_total: f64 = node.children.as_ref()
            .map(|children| children.iter().map(|c| calculate_total(c, account_type)).sum())
            .unwrap_or(0.0);
        if is_type { own_balance + children_total } else { children_total }
    }
    let total_assets = tree.iter().map(|n| calculate_total(n, "Assets")).sum();
    let total_liabilities = tree.iter().map(|n| calculate_total(n, "Liabilities")).sum();
    let total_income = tree.iter().map(|n| calculate_total(n, "Income")).sum();
    let total_expenses = tree.iter().map(|n| calculate_total(n, "Expenses")).sum();
    let fmt = |v: f64| if v == 0.0 { String::from("0.00") } else { format!("{:.2}", v) };
    (fmt(total_assets), fmt(total_liabilities), fmt(total_income), fmt(total_expenses))
}

/// Calculate balance per currency for an account
/// Uses the same logic as calculate_correct_balance for consistency
fn calculate_balance_per_currency(
    transactions: &[beanweb_core::Transaction],
    balances: &[beanweb_core::BalanceEntry],
    pads: &[beanweb_core::PadEntry],
    all_balances: &[beanweb_core::BalanceEntry],
    account_name: &str,
) -> std::collections::HashMap<String, f64> {
    // Use the same calculation logic as calculate_correct_balance
    // This ensures consistency between overview and detail pages
    let (final_balance, _currency) = calculate_correct_balance(transactions, balances, pads, all_balances, account_name, "CNY");

    let mut result = std::collections::HashMap::new();
    result.insert("CNY".to_string(), final_balance);
    result
}

/// Parse date string to NaiveDate
fn parse_date_string(date_str: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
}

pub async fn page_accounts(
    state: axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    query: Option<Query<HashMap<String, String>>>,
) -> axum::response::Html<String> {
    let ledger = state.ledger.read().await;
    let accounts = ledger.accounts();
    let time_range = ledger.time_context().range.to_string();
    let all_balances = ledger.all_balances();
    let all_pads = ledger.all_pads();

    // For each account, calculate the correct balance using the same logic as calculate_correct_balance
    // This ensures consistency between account detail and overview pages
    let mut account_balances: std::collections::HashMap<String, super::api::AccountAmount> = std::collections::HashMap::new();

    for acc in &accounts {
        let transactions = ledger.transactions_by_account(&acc.name);
        let balances = ledger.balances_by_account(&acc.name);
        let balances_per_currency = calculate_balance_per_currency(&transactions, &balances, &all_pads, &all_balances, &acc.name);

        // Calculate total in primary currency (CNY if present)
        let primary_currency = "CNY";
        let mut total = 0.0f64;
        let mut detail: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        for (currency, amount) in &balances_per_currency {
            let formatted = if *amount == 0.0 { "0.00".to_string() } else { format!("{:.2}", amount) };
            detail.insert(currency.clone(), formatted);
            if currency == primary_currency {
                total += *amount;
            }
        }

        let currency = acc.currency.clone().unwrap_or_else(|| "CNY".to_string());
        account_balances.insert(acc.name.clone(), super::api::AccountAmount {
            calculated: super::api::CalculatedAmount {
                number: if total == 0.0 { "0.00".to_string() } else { format!("{:.2}", total) },
                currency: primary_currency.to_string(),
            },
            detail,
        });
    }

    use super::api::build_account_tree;
    let tree = build_account_tree(&accounts, &account_balances);
    let (total_assets, total_liabilities, total_income, total_expenses) = render_accounts_summary(&tree);

    let search_term = query.as_ref().and_then(|q| q.0.get("search")).map(|s| s.to_lowercase()).unwrap_or_default();
    let hide_closed = query.as_ref().and_then(|q| q.0.get("hide_closed")).map(|s| s == "true").unwrap_or(false);

    let tree_html = render_accounts_tree(&tree, search_term.clone(), hide_closed);
    let header_html = r#"<div class="mb-6"><h2 class="text-2xl font-bold">账户</h2></div>"#;

    let summary_html = format!(r#"<div class="grid grid-cols-1 lg:grid-cols-4 gap-4 mb-6">
            <div class="bg-gradient-to-br from-green-50 to-green-100 p-4 rounded-xl border border-green-200">
                <div class="flex items-center gap-2 mb-2">
                    <svg class="w-5 h-5 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
                    </svg>
                    <p class="text-sm text-green-700 font-medium">总资产</p>
                </div>
                <p class="text-2xl font-bold text-green-800">{}</p>
            </div>
            <div class="bg-gradient-to-br from-red-50 to-red-100 p-4 rounded-xl border border-red-200">
                <div class="flex items-center gap-2 mb-2">
                    <svg class="w-5 h-5 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"/>
                    </svg>
                    <p class="text-sm text-red-700 font-medium">总负债</p>
                </div>
                <p class="text-2xl font-bold text-red-800">{}</p>
            </div>
            <div class="bg-gradient-to-br from-blue-50 to-blue-100 p-4 rounded-xl border border-blue-200">
                <div class="flex items-center gap-2 mb-2">
                    <svg class="w-5 h-5 text-blue-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6"/>
                    </svg>
                    <p class="text-sm text-blue-700 font-medium">总收入</p>
                </div>
                <p class="text-2xl font-bold text-blue-800">{}</p>
            </div>
            <div class="bg-gradient-to-br from-amber-50 to-amber-100 p-4 rounded-xl border border-amber-200">
                <div class="flex items-center gap-2 mb-2">
                    <svg class="w-5 h-5 text-amber-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 9V7a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2m2 4h10a2 2 0 002-2v-6a2 2 0 00-2-2H9a2 2 0 00-2 2v6a2 2 0 002 2zm7-5a2 2 0 11-4 0 2 2 0 014 0z"/>
                    </svg>
                    <p class="text-sm text-amber-700 font-medium">总支出</p>
                </div>
                <p class="text-2xl font-bold text-amber-800">{}</p>
            </div>
        </div>"#, total_assets, total_liabilities, total_income, total_expenses);

    let search_attr = if search_term.is_empty() { String::new() } else {
        let escaped = search_term.replace('{', "{{").replace('}', "}}");
        format!(r#" value="{}""#, escaped)
    };
    let hide_closed_attr = if hide_closed { " checked" } else { "" };

    let input_html = format!(r#"<input type="text" name="search" placeholder="搜索账户..."{} class="w-full pl-10 pr-4 py-2.5 border border-gray-300 rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500">"#,
        search_attr);

    let mut filter_html = format!(r#"<div class="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
            <div class="p-4 border-b border-gray-200 bg-gray-50">
                <div class="flex flex-col sm:flex-row sm:items-center gap-4">
                    <form action="/accounts" method="get" class="flex-1 flex gap-4">
                        <div class="relative flex-1">
                            <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/>
                            </svg>
                            {}
                        </div>
                        <label class="flex items-center gap-2 cursor-pointer select-none">
                            <input type="checkbox" name="hide_closed" value="true"{} class="w-4 h-4 text-indigo-600 rounded border-gray-300 focus:ring-indigo-500">
                            <span class="text-sm text-gray-600">隐藏已关闭账户</span>
                        </label>
                        <button type="submit" class="px-4 py-2.5 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700">搜索</button>
                        <a href="/accounts" class="px-4 py-2.5 border border-gray-300 rounded-lg hover:bg-gray-50 text-gray-700">重置</a>
                    </form>
                </div>
            </div>
            <div class="divide-y divide-gray-100">"#,
        input_html, hide_closed_attr);
    filter_html.push_str(&tree_html);
    filter_html.push_str(r#"</div>
        </div>"#);

    let inner_content = format!("{}{}{}", header_html, summary_html, filter_html);
    axum::response::Html(crate::page_response_with_time(&headers, "账户", "/accounts", &inner_content, &time_range))
}

pub async fn page_account_detail(
    state: axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: Path<String>,
) -> axum::response::Html<String> {
    let ledger = state.ledger.read().await;
    let account_name = path.0;
    let account = ledger.account(&account_name);
    let time_context = ledger.time_context();
    let time_range = time_context.range.to_string();
    let all_balances = ledger.all_balances();
    let all_pads = ledger.all_pads();

    match account {
        Some(acc) => {
            // Use the same calculation logic as render_account_transactions_paginated
            // This ensures the balance display matches the transaction list
            let transactions = ledger.transactions_by_account(&account_name);
            let balances = ledger.balances_by_account(&account_name);
            let default_currency = acc.currency.clone().unwrap_or_else(|| "CNY".to_string());
            let (balance, currency) = calculate_correct_balance(&transactions, &balances, &all_pads, &all_balances, &account_name, &default_currency);
            let balance_display = if balance < 0.0 {
                format!("-{:.2} {}", balance.abs(), currency)
            } else {
                format!("{:.2} {}", balance, currency)
            };
            let encoded_name = urlencoding::encode(&account_name);
            let tx_list_url = format!("/accounts/{}/transactions/list", encoded_name);

            // Get time filter for display
            let display_start = time_context.start_date().map(|d| d.to_string()).unwrap_or_else(|| "-".to_string());
            let display_end = time_context.end_date().map(|d| d.to_string()).unwrap_or_else(|| "-".to_string());
            let time_selector_html = crate::page_time_selector(&time_range, &display_start, &display_end);

            let header_back = r#"<div class="mb-6 flex items-center gap-4">
                <a href="/accounts" class="text-gray-500 hover:text-gray-700 flex items-center gap-1">
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
                    </svg>
                    返回账户
                </a>
            </div>"#;

            let account_info = format!(r#"<div class="mb-6">
                <h2 class="text-2xl font-bold break-all">{}</h2>
                <p class="text-gray-500 mt-1 flex items-center gap-2">
                    <span class="px-2 py-0.5 bg-gray-100 rounded text-sm">{}</span>
                    <span>|</span>
                    <span>余额: <span class="font-medium text-green-600">{}</span></span>
                </p>
            </div>"#, account_name, acc.account_type, balance_display);

            let hx_get_attr = format!("hx-get=\"{}\"", tx_list_url);
            let hx_target_attr = "hx-target=\"#account-tx-list\"".to_string();
            let hx_trigger_input = "hx-trigger=\"keyup changed, delay:500ms\"".to_string();
            let hx_trigger_select = "hx-trigger=\"change\"".to_string();

            let filter_bar = format!(r#"<div class="mb-4 flex items-center justify-between gap-4 flex-wrap">
                <h3 class="text-lg font-semibold">近期交易</h3>
                <div class="flex items-center gap-2">
                    <input type="text" name="q" placeholder="搜索交易..."
                        {} {} {}
                        class="px-3 py-2 border rounded w-48">
                    <select name="limit" {} {} {}
                        class="px-3 py-2 border rounded" onchange="this.form.requestSubmit()">
                        <option value="10">10 条</option>
                        <option value="25">25 条</option>
                        <option value="50" selected>50 条</option>
                        <option value="100">100 条</option>
                    </select>
                </div>
            </div>
            <div class="mb-4">
                <span class="text-sm text-gray-500">共 {} 笔交易</span>
            </div>
            <div id="account-tx-list" {}="{}?limit=50" hx-trigger="load" class="bg-white rounded shadow-sm p-6">
                <p class="text-gray-500 text-center py-8">加载中...</p>
            </div>"#,
                hx_get_attr, hx_target_attr, hx_trigger_input,
                hx_get_attr, hx_target_attr, hx_trigger_select,
                transactions.len(),
                hx_get_attr.split('=').next().unwrap_or("hx-get"), tx_list_url
            );

            let inner_content = format!("{}{}{}{}", time_selector_html, header_back, account_info, filter_bar);

            axum::response::Html(crate::page_response_with_time(&headers, &account_name, &format!("/accounts/{}", encoded_name), &inner_content, &time_range))
        }
        None => {
            let inner_content = format!(r#"<div class="mb-6 flex items-center gap-4">
                <a href="/accounts" class="text-gray-500 hover:text-gray-700 flex items-center gap-1">
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
                    </svg>
                    返回账户
                </a>
            </div>
            <div class="text-center py-12">
                <div class="mb-4">
                    <svg class="w-16 h-16 mx-auto text-gray-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
                    </svg>
                </div>
                <h2 class="text-xl font-bold text-gray-600">未找到账户</h2>
                <p class="text-gray-400 mt-2">{}</p>
            </div>"#, account_name);
            axum::response::Html(crate::page_response(&headers, "账户未找到", &format!("/accounts/{}", urlencoding::encode(&account_name)), &inner_content))
        }
    }
}

/// Render transactions for an account with proper sorting, time display, running balance, and click-to-expand
fn render_account_transactions(transactions: &[beanweb_core::Transaction], account_name: &str, initial_balance: f64) -> String {
    if transactions.is_empty() {
        return String::from(r#"<div class="text-center py-12 text-gray-400"><p>暂无交易记录</p></div>"#);
    }

    eprintln!("[DEBUG render_account_transactions] account={}, initial_balance={}, tx_count={}",
        account_name, initial_balance, transactions.len());

    // First, sort by date ascending (oldest first), then by time
    // This is needed to calculate running balance correctly
    let mut ascending: Vec<_> = transactions.iter().collect();
    ascending.sort_by(|a, b| {
        match a.date.cmp(&b.date) {
            std::cmp::Ordering::Equal => a.time.cmp(&b.time),
            other => other,
        }
    });

    // Calculate running balance in ascending order (oldest -> newest)
    // Start from the given initial balance (not 0!)
    let mut balances: Vec<(f64, &beanweb_core::Transaction)> = Vec::new();
    let mut running_balance = initial_balance;

    for tx in &ascending {
        let amount = get_posting_amount_for_account(tx, account_name);
        running_balance += amount;
        eprintln!("[DEBUG balance calc] date={}, amount={}, running_balance={}", tx.date, amount, running_balance);
        balances.push((running_balance, *tx));
    }

    // Now reverse for display (newest first)
    balances.reverse();

    render_balances(&balances, account_name)
}

/// Calculate the correct running balance for an account (uses the same logic as render_account_transactions_paginated)
/// Returns the final balance and the primary currency after processing all balances and transactions
pub fn calculate_correct_balance(
    account_transactions: &[beanweb_core::Transaction],
    balances: &[beanweb_core::BalanceEntry],
    _pads: &[beanweb_core::PadEntry],
    _all_balances: &[beanweb_core::BalanceEntry],
    account_name: &str,
    default_currency: &str,
) -> (f64, String) {
    // Note: We don't use source_pads here because synthetic transactions already include Pad effects
    // The pads parameter is kept for API compatibility but not used

    // If no transactions and no balances, return 0
    if account_transactions.is_empty() && balances.is_empty() {
        return (0.0, default_currency.to_string());
    }

    // Collect all currencies used in this account
    let mut currencies: std::collections::HashSet<String> = std::collections::HashSet::new();
    currencies.insert(default_currency.to_string());

    // Add currencies from balance entries
    for balance in balances {
        if !balance.currency.is_empty() {
            currencies.insert(balance.currency.clone());
        }
    }

    // Add currencies from transactions
    for tx in account_transactions {
        for posting in &tx.postings {
            if posting.account == account_name && !posting.currency.is_empty() {
                currencies.insert(posting.currency.clone());
            }
        }
    }

    // Build timeline items - track balances per currency
    let mut timeline: Vec<TimelineItem> = Vec::new();

    // Add balance entries
    for balance in balances {
        let amount = parse_amount(&balance.amount);
        timeline.push(TimelineItem {
            date: balance.date.clone(),
            time: String::new(),
            item_type: TimelineItemType::Balance,
            posting_amount: amount,
            running_balance: amount,
            description: format!("Balance: {}", balance.amount),
        });
    }

    // Note: We don't add source_pads here because synthetic transactions already include Pad effects
    // The Pad directive generates synthetic transactions that are included in account_transactions
    // Adding source_pads again would double-count the Pad effects

    // Add transactions
    for tx in account_transactions {
        let posting_amount = get_posting_amount_for_account(tx, account_name);
        timeline.push(TimelineItem {
            date: tx.date.clone(),
            time: if tx.has_time() { tx.time.clone() } else { String::new() },
            item_type: TimelineItemType::Transaction,
            posting_amount,
            running_balance: 0.0,
            description: if !tx.payee.is_empty() && !tx.narration.is_empty() {
                format!("{} - {}", tx.payee, tx.narration)
            } else if !tx.payee.is_empty() {
                tx.payee.clone()
            } else {
                tx.narration.clone()
            },
        });
    }

    // Sort by date ascending (oldest first), then by time
    timeline.sort_by(|a, b| {
        match a.date.cmp(&b.date) {
            std::cmp::Ordering::Equal => a.time.cmp(&b.time),
            other => other,
        }
    });

    // Calculate running balance in forward order
    let mut running_balance = 0.0;
    for item in &mut timeline {
        match item.item_type {
            TimelineItemType::Balance => {
                running_balance = item.posting_amount;
            }
            TimelineItemType::Pad | TimelineItemType::Transaction => {
                running_balance += item.posting_amount;
            }
        }
        item.running_balance = running_balance;
    }

    // Determine primary currency (CNY if present, otherwise the first one)
    let primary_currency = if currencies.contains("CNY") {
        "CNY".to_string()
    } else if let Some(c) = currencies.iter().next() {
        c.clone()
    } else {
        default_currency.to_string()
    };

    eprintln!("[DEBUG calculate_correct_balance] account={}, final_balance={}",
        account_name, running_balance);

    (running_balance, primary_currency)
}

/// Render pre-calculated balances (used when balance is already calculated for ALL transactions)
fn render_account_transactions_with_balance(balances: &[(f64, &beanweb_core::Transaction)], account_name: &str) -> String {
    if balances.is_empty() {
        return String::from(r#"<div class="text-center py-12 text-gray-400"><p>暂无交易记录</p></div>"#);
    }

    eprintln!("[DEBUG render_account_transactions_with_balance] account={}, tx_count={}",
        account_name, balances.len());

    render_balances(balances, account_name)
}

/// Common rendering logic for balances
fn render_balances(balances: &[(f64, &beanweb_core::Transaction)], account_name: &str) -> String {
    let mut html = String::new();

    for (running_balance, tx) in balances {
        let flag = tx.flag.as_deref().unwrap_or("");
        let flag_color = match flag { "*" => "#10B981", "!" => "#F59E0B", _ => "#6B7280" };

        // Get amount for this account
        let amount = get_posting_amount_for_account(tx, account_name);

        let (amount_str, amount_class) = if amount < 0.0 {
            (format!("{:.2}", amount.abs()), "text-red-600")
        } else if amount > 0.0 {
            (format!("{:.2}", amount), "text-green-600")
        } else {
            ("0.00".to_string(), "text-gray-400")
        };

        let datetime = if tx.has_time() {
            format!("{} <span class='text-gray-400'>{}</span>", tx.date, tx.time)
        } else {
            tx.date.clone()
        };

        // Show both payee and narration
        let desc = if !tx.payee.is_empty() && !tx.narration.is_empty() {
            format!("{} - {}", tx.payee, tx.narration)
        } else if !tx.payee.is_empty() {
            tx.payee.clone()
        } else {
            tx.narration.clone()
        };

        let detail_id = format!("account-tx-detail-{}", tx.id);

        html.push_str(&format!(r#"<div class="border border-l-4 rounded-r-lg p-3 hover:bg-gray-50 transition mb-2 cursor-pointer" onclick='toggleAccountDetail("{}")'>
            <div class="flex items-center justify-between gap-2">
                <div class="flex items-center gap-3 flex-1 min-w-0">
                    <span class="w-1 h-8 rounded flex-shrink-0" style="background:{}"></span>
                    <div class="flex-1 min-w-0">
                        <div class="text-sm text-gray-500">{}</div>
                        <div class="font-medium truncate">{}</div>
                    </div>
                </div>
                <div class="flex flex-col items-end gap-1 flex-shrink-0">
                    <span class="font-medium {}">{}</span>
                    <span class="text-xs text-gray-400">余额: {:.2}</span>
                </div>
            </div>
        </div>
        <div id="{}" class="account-tx-detail-container" style="display:none"></div>"#,
            detail_id, flag_color, datetime, desc, amount_class, amount_str, running_balance, detail_id));
    }

    html
}

/// Add toggle script for account transaction details - called once per page
pub fn account_tx_detail_script() -> String {
    r#"<script>
    function toggleAccountDetail(id) {
        var el = document.getElementById(id);
        if (el.style.display === 'none') {
            el.style.display = 'block';
            if (el.innerHTML === '') {
                htmx.ajax('GET', '/transactions/' + id.replace('account-tx-detail-', '') + '/detail', {target: el});
            }
        } else {
            el.style.display = 'none';
            el.innerHTML = '';
        }
    }
    </script>"#.to_string()
}

/// Timeline item type
#[derive(Debug, Clone, PartialEq)]
enum TimelineItemType {
    Balance,   // Balance directive
    Pad,       // Pad directive
    Transaction, // Regular transaction
}

/// Timeline item with balance information
#[derive(Debug, Clone)]
struct TimelineItem {
    date: String,
    time: String,
    item_type: TimelineItemType,
    posting_amount: f64,      // The amount of the posting/transaction
    running_balance: f64,      // The running balance after this item
    description: String,       // Display description
}

pub fn render_account_transactions_paginated(
    account_transactions: &[beanweb_core::Transaction],
    balances: &[beanweb_core::BalanceEntry],
    account_name: &str,
    limit: usize,
    offset: usize,
    initial_balance: f64,
) -> String {
    let total_tx = account_transactions.len();
    let total_events = total_tx + balances.len();

    eprintln!("[DEBUG render_account_transactions_paginated] account={}, initial_balance={}, tx_count={}, balance_count={}",
        account_name, initial_balance, total_tx, balances.len());

    // Build timeline items
    let mut timeline: Vec<TimelineItem> = Vec::new();

    // Find all Pad transactions involving this account
    // A Pad transaction looks like: 2022-10-12 pad TARGET from SOURCE
    // The postings are generated automatically by Beancount
    for tx in account_transactions {
        // Check if this is a Pad transaction (by checking if narration contains " from ")
        let is_pad = tx.narration.contains(" from ");

        if is_pad {
            let raw_amount = get_posting_amount_for_account(tx, account_name);

            // For Income/Expenses accounts, display the actual difference value
            // which can be positive or negative based on the balance change
            let posting_amount = raw_amount;

            // Determine if this account is the source or target of the pad
            let narration = &tx.narration;
            let description = if narration.contains(" from ") {
                // Format: "Assets:FinTech:余利宝 from Income:Interest:利息"
                // If account appears before " from ", it's the TARGET
                // If account appears after " from ", it's the SOURCE
                if let Some(from_pos) = narration.find(" from ") {
                    let before_from = &narration[..from_pos];
                    let after_from = &narration[from_pos + 6..]; // 6 = " from ".len()

                    if account_name == before_from {
                        format!("Pad to {}", after_from)
                    } else {
                        format!("Pad from {}", before_from)
                    }
                } else {
                    narration.clone()
                }
            } else {
                narration.clone()
            };

            eprintln!("[DEBUG Pad] account={}, date={}, amount={}, description={}",
                account_name, tx.date, posting_amount, description);

            timeline.push(TimelineItem {
                date: tx.date.clone(),
                time: if tx.has_time() { tx.time.clone() } else { String::new() },
                item_type: TimelineItemType::Pad,
                posting_amount,
                running_balance: 0.0,
                description,
            });
        } else {
            // Regular transaction
            let posting_amount = get_posting_amount_for_account(tx, account_name);
            timeline.push(TimelineItem {
                date: tx.date.clone(),
                time: if tx.has_time() { tx.time.clone() } else { String::new() },
                item_type: TimelineItemType::Transaction,
                posting_amount,
                running_balance: 0.0,  // Will be calculated later
                description: if !tx.payee.is_empty() && !tx.narration.is_empty() {
                    format!("{} - {}", tx.payee, tx.narration)
                } else if !tx.payee.is_empty() {
                    tx.payee.clone()
                } else {
                    tx.narration.clone()
                },
            });
        }
    }

    // Add balance entries
    for balance in balances {
        let amount = parse_amount(&balance.amount);
        eprintln!("[DEBUG Balance] account={}, date={}, amount_str={}, parsed={}",
            account_name, balance.date, balance.amount, amount);
        timeline.push(TimelineItem {
            date: balance.date.clone(),
            time: String::new(),
            item_type: TimelineItemType::Balance,
            posting_amount: amount,  // Use the balance amount
            running_balance: amount,  // Balance sets the running balance directly
            description: format!("Balance 设置余额: {}", balance.amount),
        });
    }

    // Sort by date ascending (oldest first), then by time
    timeline.sort_by(|a, b| {
        match a.date.cmp(&b.date) {
            std::cmp::Ordering::Equal => a.time.cmp(&b.time),
            other => other,
        }
    });

    // Calculate running balance for all items
    // Beancount semantics:
    // - Balance directive at start of day (00:00), BEFORE any transactions that day
    // - Balance sets the baseline balance directly (this value is the result of all previous transactions)
    // - Pad directive: the posting amount is added/subtracted from the running balance
    // - Transactions add/subtract from the running balance

    // Sort timeline by date ascending (oldest first)
    timeline.sort_by(|a, b| {
        match a.date.cmp(&b.date) {
            std::cmp::Ordering::Equal => a.time.cmp(&b.time),
            other => other,
        }
    });

    // Calculate running balance in forward order
    let mut running_balance = 0.0;
    for item in &mut timeline {
        match item.item_type {
            TimelineItemType::Balance => {
                // Balance directive: the balance value IS the result of all previous transactions
                // So we set running_balance directly to this value
                running_balance = item.posting_amount;
            }
            TimelineItemType::Pad | TimelineItemType::Transaction => {
                // Pad and Transaction: add/subtract the posting amount
                running_balance += item.posting_amount;
            }
        }
        item.running_balance = running_balance;
    }

    eprintln!("[DEBUG] final_balance={}", running_balance);

    // Reverse for display (newest first)
    timeline.reverse();

    // Paginate
    let paginated: Vec<_> = timeline.into_iter().skip(offset).take(limit).collect();

    eprintln!("[DEBUG render_account_transactions_paginated] showing {} events", paginated.len());

    let mut html = String::new();
    if paginated.is_empty() {
        html.push_str(r#"<div class="text-center py-12 text-gray-500"><p>暂无交易记录</p></div>"#);
    } else {
        html.push_str(r#"<div class="space-y-2">"#);
        for item in &paginated {
            let datetime = if item.time.is_empty() {
                item.date.clone()
            } else {
                format!("{} <span class='text-gray-400'>{}</span>", item.date, item.time)
            };

            match item.item_type {
                TimelineItemType::Balance => {
                    // Balance entry - gray color
                    html.push_str(&format!(r#"<div class="border border-l-4 rounded-r-lg p-3 hover:bg-gray-50 transition mb-2" style="border-left-color: #6B7280">
                        <div class="flex items-center justify-between gap-2">
                            <div class="flex items-center gap-3 flex-1 min-w-0">
                                <span class="w-1 h-8 rounded flex-shrink-0" style="background:#6B7280"></span>
                                <div class="flex-1 min-w-0">
                                    <div class="text-sm text-gray-500">{}</div>
                                    <div class="font-medium truncate">{}</div>
                                </div>
                            </div>
                            <div class="flex flex-col items-end gap-1 flex-shrink-0">
                                <span class="font-medium text-blue-600">余额重置</span>
                                <span class="text-xs text-gray-400">余额: {:.2}</span>
                            </div>
                        </div>
                    </div>"#, datetime, item.description, item.running_balance));
                }
                TimelineItemType::Pad => {
                    // Pad entry - purple color
                    // For Income/Expenses accounts: positive = expense (red), negative = income (green)
                    // For Assets/Liabilities accounts: positive = increase (green), negative = decrease (red)
                    let is_income_or_expense = account_name.starts_with("Income:") || account_name.starts_with("Expenses:");
                    let (amount_str, amount_class) = if item.posting_amount < 0.0 {
                        // Negative: for Income/Expenses = income (green), for Assets/Liabilities = decrease (red)
                        if is_income_or_expense {
                            (format!("{:.2}", item.posting_amount.abs()), "text-green-600")
                        } else {
                            (format!("{:.2}", item.posting_amount.abs()), "text-red-600")
                        }
                    } else if item.posting_amount > 0.0 {
                        // Positive: for Income/Expenses = expense (red), for Assets/Liabilities = increase (green)
                        if is_income_or_expense {
                            (format!("{:.2}", item.posting_amount.abs()), "text-red-600")
                        } else {
                            (format!("{:.2}", item.posting_amount), "text-green-600")
                        }
                    } else {
                        ("0.00".to_string(), "text-gray-400")
                    };

                    html.push_str(&format!(r#"<div class="border border-l-4 rounded-r-lg p-3 hover:bg-gray-50 transition mb-2" style="border-left-color: #8B5CF6">
                        <div class="flex items-center justify-between gap-2">
                            <div class="flex items-center gap-3 flex-1 min-w-0">
                                <span class="w-1 h-8 rounded flex-shrink-0" style="background:#8B5CF6"></span>
                                <div class="flex-1 min-w-0">
                                    <div class="text-sm text-gray-500">{}</div>
                                    <div class="font-medium truncate">{}</div>
                                </div>
                            </div>
                            <div class="flex flex-col items-end gap-1 flex-shrink-0">
                                <span class="font-medium {}">{}</span>
                                <span class="text-xs text-gray-400">余额: {:.2}</span>
                            </div>
                        </div>
                    </div>"#, datetime, item.description, amount_class, amount_str, item.running_balance));
                }
                TimelineItemType::Transaction => {
                    // Transaction entry - green color
                    // For Income/Expenses accounts: positive = expense (red), negative = income (green)
                    // For Assets/Liabilities accounts: positive = increase (green), negative = decrease (red)
                    let is_income_or_expense = account_name.starts_with("Income:") || account_name.starts_with("Expenses:");
                    let (amount_str, amount_class) = if item.posting_amount < 0.0 {
                        // Negative: for Income/Expenses = income (green), for Assets/Liabilities = decrease (red)
                        if is_income_or_expense {
                            (format!("{:.2}", item.posting_amount.abs()), "text-green-600")
                        } else {
                            (format!("{:.2}", item.posting_amount.abs()), "text-red-600")
                        }
                    } else if item.posting_amount > 0.0 {
                        // Positive: for Income/Expenses = expense (red), for Assets/Liabilities = increase (green)
                        if is_income_or_expense {
                            (format!("{:.2}", item.posting_amount.abs()), "text-red-600")
                        } else {
                            (format!("{:.2}", item.posting_amount), "text-green-600")
                        }
                    } else {
                        ("0.00".to_string(), "text-gray-400")
                    };

                    html.push_str(&format!(r#"<div class="border border-l-4 rounded-r-lg p-3 hover:bg-gray-50 transition mb-2" style="border-left-color: #10B981">
                        <div class="flex items-center justify-between gap-2">
                            <div class="flex items-center gap-3 flex-1 min-w-0">
                                <span class="w-1 h-8 rounded flex-shrink-0" style="background:#10B981"></span>
                                <div class="flex-1 min-w-0">
                                    <div class="text-sm text-gray-500">{}</div>
                                    <div class="font-medium truncate">{}</div>
                                </div>
                            </div>
                            <div class="flex flex-col items-end gap-1 flex-shrink-0">
                                <span class="font-medium {}">{}</span>
                                <span class="text-xs text-gray-400">余额: {:.2}</span>
                            </div>
                        </div>
                    </div>"#, datetime, item.description, amount_class, amount_str, item.running_balance));
                }
            }
        }
        html.push_str("</div>");

        let current_page = if limit == 0 { 1 } else { offset / limit + 1 };
        let total_pages = if limit == 0 { 1 } else { (total_events + limit - 1) / limit };
        let prev_offset = offset.saturating_sub(limit);
        let next_offset = offset + limit;
        let last_offset = (total_pages.saturating_sub(1)) * limit;
        let encoded_name = urlencoding::encode(account_name);

        let target = "#account-tx-list";
        html.push_str(&format!(
            r#"<div class='mt-6 flex items-center justify-between flex-wrap gap-4'>
            <span class='text-sm text-gray-500'>共 {} 条记录，第 {} / {} 页</span>
            <div class='flex items-center gap-2'>
                <button {} onclick='htmx.ajax("GET", "/accounts/{}/transactions/list?limit={}&offset=0", "{}")' class='px-3 py-1 border rounded hover:bg-gray-100'>首页</button>
                <button {} onclick='htmx.ajax("GET", "/accounts/{}/transactions/list?limit={}&offset={}", "{}")' class='px-3 py-1 border rounded hover:bg-gray-100'>上一页</button>
                <span class='text-sm text-gray-600'>第 <input type='number' id='account-page-jump-input' min='1' max='{}' value='{}' class='w-16 text-center border rounded px-2 py-1'> 页</span>
                <button onclick='const p=document.getElementById("account-page-jump-input").value; htmx.ajax("GET", "/accounts/{}/transactions/list?limit={}&offset=" + (p-1)*{} + "", "{}")' class='px-3 py-1 border rounded bg-blue-50 hover:bg-blue-100 text-blue-600'>跳转</button>
                <button {} onclick='htmx.ajax("GET", "/accounts/{}/transactions/list?limit={}&offset={}", "{}")' class='px-3 py-1 border rounded hover:bg-gray-100'>下一页</button>
                <button {} onclick='htmx.ajax("GET", "/accounts/{}/transactions/list?limit={}&offset={}", "{}")' class='px-3 py-1 border rounded hover:bg-gray-100'>末页</button>
            </div>
        </div>"#,
            total_events, current_page, total_pages,
            if current_page == 1 { "disabled" } else { "" },
            encoded_name, limit, target,
            if current_page == 1 { "disabled" } else { "" },
            encoded_name, limit, prev_offset, target,
            total_pages, current_page,
            encoded_name, limit, limit, target,
            if current_page >= total_pages { "disabled" } else { "" },
            encoded_name, limit, next_offset, target,
            if current_page >= total_pages { "disabled" } else { "" },
            encoded_name, limit, last_offset, target
        ));
        html.push_str(r#"<style>.disabled{cursor:not-allowed;opacity:0.5;pointer-events:none}</style>"#);
        // Add toggle script for account transaction details
        html.push_str(&account_tx_detail_script());
    }
    html
}
