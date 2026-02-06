//! Accounts API endpoints - JSON API and HTMX partial responses

use crate::AppState;
use axum::extract::{Query, Path};
use std::collections::HashMap;

/// Account amount structure with calculated total and currency detail
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccountAmount {
    /// The calculated total amount (converted to operating currency)
    pub calculated: CalculatedAmount,
    /// Detailed amounts per currency
    pub detail: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CalculatedAmount {
    pub number: String,
    pub currency: String,
}

/// Account list item for API response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccountListItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(rename = "status")]
    pub account_status: String,
    pub amount: AccountAmount,
}

/// Account with full tree information for frontend
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccountTreeNode {
    pub name: String,
    pub short_name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(rename = "status")]
    pub account_status: String,
    pub amount: AccountAmount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<AccountTreeNode>>,
    pub has_children: bool,
    pub is_leaf: bool,
    pub is_real: bool,  // true if this node exists in accounts table (can click to view detail)
    pub depth: usize,
}

pub async fn api_accounts(state: axum::extract::State<AppState>) -> String {
    let ledger = state.ledger.read().await;
    let accounts = ledger.accounts();
    let account_balances = ledger.calculate_account_balances();

    // Build account list items with proper amount structure
    let mut items: Vec<AccountListItem> = accounts.iter()
        .map(|acc| {
            let balance = account_balances.get(&acc.name).copied().unwrap_or(0.0);
            let currency = acc.currency.clone().unwrap_or_else(|| "CNY".to_string());

            AccountListItem {
                name: acc.name.clone(),
                alias: acc.alias.clone(),
                account_status: format!("{}", acc.status),
                amount: AccountAmount {
                    calculated: CalculatedAmount {
                        number: format_balance_number(balance),
                        currency: currency.clone(),
                    },
                    detail: {
                        let mut detail = HashMap::new();
                        detail.insert(currency, format_balance_number(balance));
                        detail
                    },
                },
            }
        })
        .collect();

    serde_json::to_string(&items).unwrap_or_default()
}

/// Format balance number for display
fn format_balance_number(value: f64) -> String {
    if value == 0.0 {
        "0.00".to_string()
    } else {
        format!("{:.2}", value)
    }
}

/// Calculate account balances with currency detail
pub(crate) fn calculate_balances_with_detail(
    accounts: &[beanweb_core::Account],
    transactions: &[beanweb_core::Transaction],
) -> HashMap<String, AccountAmount> {
    let mut balances: HashMap<String, HashMap<String, f64>> = HashMap::new();
    let mut currencies: HashMap<String, String> = HashMap::new();

    // Initialize with account currencies
    for account in accounts {
        currencies.insert(account.name.clone(), account.currency.clone().unwrap_or_else(|| "CNY".to_string()));
    }

    // Calculate balances from transactions
    for transaction in transactions {
        for posting in &transaction.postings {
            let account_name = &posting.account;
            let currency = if posting.currency.is_empty() {
                currencies.get(account_name).cloned().unwrap_or_else(|| "CNY".to_string())
            } else {
                posting.currency.clone()
            };

            let amount = posting.amount_value().unwrap_or(0.0);
            let account_balances = balances.entry(account_name.clone()).or_default();
            *account_balances.entry(currency.clone()).or_insert(0.0) += amount;
        }
    }

    // Convert to AccountAmount structure
    let mut result: HashMap<String, AccountAmount> = HashMap::new();
    for (account_name, currency_balances) in balances {
        let operating_currency = currencies.get(&account_name).cloned().unwrap_or_else(|| "CNY".to_string());

        // Calculate total in operating currency (simplified - just use the currency if matches)
        let mut total = 0.0;
        let mut detail = HashMap::new();

        for (currency, balance) in currency_balances {
            let formatted = format_balance_number(balance);
            detail.insert(currency.clone(), formatted);
            if currency == operating_currency {
                total += balance;
            }
        }

        result.insert(account_name, AccountAmount {
            calculated: CalculatedAmount {
                number: format_balance_number(total),
                currency: operating_currency,
            },
            detail,
        });
    }

    result
}

/// Build account tree with hierarchy
pub(crate) fn build_account_tree(accounts: &[beanweb_core::Account], balances: &HashMap<String, AccountAmount>) -> Vec<AccountTreeNode> {
    // Collect all account names
    let account_names: Vec<String> = accounts.iter().map(|a| a.name.clone()).collect();
    let account_set: std::collections::HashSet<String> = account_names.iter().cloned().collect();

    // Build children map with deduplication - use HashSet to avoid duplicates
    let mut children_map: HashMap<String, std::collections::HashSet<String>> = HashMap::new();

    for name in &account_names {
        // Add all intermediate paths as potential parents
        let mut parts: Vec<&str> = name.split(':').collect();
        if parts.len() > 1 {
            let mut current_path = parts[0].to_string();
            for i in 1..parts.len() {
                let part = parts[i];
                let next_path = format!("{}:{}", current_path, part);
                children_map.entry(current_path.clone()).or_insert_with(std::collections::HashSet::new).insert(next_path.clone());
                current_path = next_path;
            }
        }
    }

    // Build tree recursively - integrates amount calculation with tree building
    fn build_node(
        name: &str,
        accounts: &[beanweb_core::Account],
        balances: &HashMap<String, AccountAmount>,
        children_map: &HashMap<String, std::collections::HashSet<String>>,
        account_set: &std::collections::HashSet<String>,
        visited: &mut std::collections::HashSet<String>,
    ) -> Option<AccountTreeNode> {
        if visited.contains(name) {
            return None;
        }
        visited.insert(name.to_string());

        let account = accounts.iter().find(|a| a.name == name);

        let short_name = if let Some(pos) = name.rfind(':') {
            &name[pos + 1..]
        } else {
            name
        };

        let has_children = children_map.contains_key(name);
        let is_leaf = !has_children;

        // Build children first
        let children = if has_children {
            let mut child_nodes = Vec::new();
            if let Some(children) = children_map.get(name) {
                let mut sorted_children: Vec<&String> = children.iter().collect();
                sorted_children.sort();
                for child in sorted_children {
                    if let Some(node) = build_node(child, accounts, balances, children_map, account_set, visited) {
                        child_nodes.push(node);
                    }
                }
            }
            if !child_nodes.is_empty() {
                Some(child_nodes)
            } else {
                None
            }
        } else {
            None
        };

        // Calculate amount: own balance + children's total
        let combined_amount = if let Some(child_list) = &children {
            // Start with own balance
            let mut detail: HashMap<String, String> = HashMap::new();
            let operating_currency = balances.get(name)
                .map(|a| a.calculated.currency.clone())
                .unwrap_or_else(|| "CNY".to_string());

            // Add own balance first
            if let Some(own_balance) = balances.get(name) {
                for (currency, amount) in &own_balance.detail {
                    detail.insert(currency.clone(), amount.clone());
                }
            }

            // Add children's amounts
            for child in child_list {
                for (currency, amount) in &child.amount.detail {
                    let child_num = amount.parse::<f64>().unwrap_or(0.0);
                    if child_num != 0.0 {
                        detail.entry(currency.clone()).and_modify(|current| {
                            let curr_num = current.parse::<f64>().unwrap_or(0.0);
                            *current = format_balance_number(curr_num + child_num);
                        }).or_insert_with(|| amount.clone());
                    }
                }
            }

            // Get the total for operating currency
            let total = detail.get(&operating_currency)
                .map(|s| s.clone())
                .unwrap_or_else(|| "0.00".to_string());

            AccountAmount {
                calculated: CalculatedAmount {
                    number: total,
                    currency: operating_currency,
                },
                detail,
            }
        } else {
            // Leaf node - use own balance
            balances.get(name).cloned().unwrap_or_else(|| AccountAmount {
                calculated: CalculatedAmount { number: "0.00".to_string(), currency: "CNY".to_string() },
                detail: HashMap::new(),
            })
        };

        Some(AccountTreeNode {
            name: name.to_string(),
            short_name: short_name.to_string(),
            path: name.to_string(),
            alias: account.and_then(|a| a.alias.clone()),
            account_status: account.map(|a| format!("{}", a.status)).unwrap_or_else(|| "Open".to_string()),
            amount: combined_amount,
            children,
            has_children,
            is_leaf,
            is_real: account.is_some(),  // true if this node exists in accounts table
            depth: name.chars().filter(|&c| c == ':').count(),
        })
    }

    // Build root-level nodes - find all top-level accounts (accounts without : in name)
    let mut roots = Vec::new();
    let mut visited = std::collections::HashSet::new();

    // Find all top-level account names (accounts without : separator)
    let top_level_accounts: Vec<&String> = account_names.iter()
        .filter(|name| !name.contains(':'))
        .collect();

    // Also collect all possible root prefixes from hierarchical accounts
    let mut all_roots: std::collections::HashSet<String> = top_level_accounts.iter()
        .map(|s| s.as_str())
        .map(String::from)
        .collect();

    // Add root prefixes from hierarchical account names
    for name in &account_names {
        if let Some(pos) = name.find(':') {
            all_roots.insert(name[..pos].to_string());
        }
    }

    // Build tree for each root
    for root_name in all_roots {
        if !visited.contains(&root_name) {
            if let Some(node) = build_node(&root_name, accounts, balances, &children_map, &account_set, &mut visited) {
                roots.push(node);
            }
        }
    }

    // Sort roots by name for consistent display
    roots.sort_by(|a, b| a.name.cmp(&b.name));

    roots
}

pub async fn htmx_accounts_list(
    state: axum::extract::State<AppState>,
    query: Option<Query<HashMap<String, String>>>,
) -> axum::response::Response<String> {
    let ledger = state.ledger.read().await;
    let accounts = ledger.accounts();
    let transactions = ledger.all_transactions();
    let account_balances = calculate_balances_with_detail(&accounts, &transactions);

    let search_term = query
        .as_ref()
        .and_then(|q| q.0.get("search"))
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let hide_closed = query
        .as_ref()
        .and_then(|q| q.0.get("hide_closed"))
        .map(|s| s == "true")
        .unwrap_or(false);

    let tree = build_account_tree(&accounts, &account_balances);

    let body = super::page::render_accounts_tree(&tree, search_term, hide_closed);

    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(body)
        .unwrap()
}

pub async fn htmx_account_suggest(
    state: axum::extract::State<AppState>,
    query: Query<HashMap<String, String>>,
) -> String {
    let ledger = state.ledger.read().await;
    let accounts = ledger.accounts();
    let q = query.get("search").map(|s| s.to_lowercase()).unwrap_or_default();
    let target = query.get("target").map(|s| s.as_str()).unwrap_or("account-suggest");

    if q.is_empty() {
        return format!(r#"<div id='{}' class='absolute z-10 w-full bg-white border rounded-lg shadow-lg mt-1 max-h-40 overflow-auto hidden'></div>"#, target);
    }

    let filtered: Vec<&beanweb_core::Account> = accounts.iter()
        .filter(|a| a.name.to_lowercase().contains(&q))
        .take(10)
        .collect();

    if filtered.is_empty() {
        return format!(r#"<div id='{}' class='absolute z-10 w-full bg-white border rounded-lg shadow-lg mt-1 max-h-40 overflow-auto hidden'></div>"#, target);
    }

    let options: Vec<String> = filtered.iter().map(|a| {
        format!(r#"<div class='px-3 py-2 hover:bg-indigo-50 cursor-pointer text-sm border-b last:border-0' data-account='{}' onclick="selectAccount(this, '{}')"><div class='font-medium'>{}</div></div>"#,
            a.name, target, a.name)
    }).collect();

    format!(
        r#"<div id='{}' class='absolute z-10 w-full bg-white border rounded-lg shadow-lg mt-1 max-h-40 overflow-auto'>{}</div>"#,
        target,
        options.join("")
    )
}

pub async fn htmx_account_transactions_list(
    state: axum::extract::State<AppState>,
    path: Path<String>,
    params: Query<HashMap<String, String>>,
) -> String {
    let ledger = state.ledger.read().await;
    let account_name = path.0;
    let limit = params.get("limit").and_then(|s| s.parse().ok()).unwrap_or(50);
    let offset = params.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0);
    let query = params.get("q").map(|s| s.to_lowercase()).unwrap_or_default();

    // Get all records for balance calculation
    let transactions = ledger.transactions_by_account(&account_name);
    let balances = ledger.balances_by_account(&account_name);
    let _pads = ledger.pads_by_account(&account_name);

    // Initial balance is always 0 for new accounts
    // Balance directives only affect transactions AFTER them (at start of day)
    let initial_balance = 0.0;

    eprintln!("[DEBUG htmx_account_transactions_list] account={}, initial_balance={}, tx_count={}, balance_count={}",
        account_name, initial_balance, transactions.len(), balances.len());

    // Apply search filter if query provided
    let mut filtered_transactions = transactions;
    if !query.is_empty() {
        filtered_transactions.retain(|tx| {
            let payee_match = tx.payee.to_lowercase().contains(&query);
            let narration_match = tx.narration.to_lowercase().contains(&query);
            let tags_match = tx.tags.iter().any(|tag| tag.to_lowercase().contains(&query));
            let links_match = tx.links.iter().any(|link| link.to_lowercase().contains(&query));
            payee_match || narration_match || tags_match || links_match
        });
    }

    super::page::render_account_transactions_paginated(&filtered_transactions, &balances, &account_name, limit, offset, initial_balance)
}
