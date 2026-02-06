//! Report structures for API responses

use serde::{Deserialize, Serialize};

use super::types::AccountType;

/// Time period summary for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimePeriodSummary {
    pub start_date: String,
    pub end_date: String,
    pub total_income: String,
    pub total_expenses: String,
    pub net_change: String,
    pub transaction_count: usize,
}

/// Account tree node for hierarchical display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTreeNode {
    pub name: String,
    pub short_name: String,
    pub account_type: String,
    pub balance: String,
    pub currency: String,
    pub children: Vec<AccountTreeNode>,
    pub is_leaf: bool,
}

/// Account balance summary for reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalanceSummary {
    pub account: String,
    pub balance: String,
    pub change: String,
    pub currency: String,
}

/// Accounts list response for API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountsResponse {
    pub accounts: Vec<Account>,
    pub total_count: usize,
}

/// Transaction statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionStats {
    pub total_count: usize,
    pub total_income: f64,
    pub total_expenses: f64,
    pub net_change: f64,
    pub average_amount: f64,
    pub largest_transaction: Option<Transaction>,
}

/// Transactions list response for API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionsResponse {
    pub transactions: Vec<Transaction>,
    pub total_count: usize,
    pub page: usize,
    pub per_page: usize,
}

/// Transaction detail response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDetailResponse {
    pub transaction: Transaction,
    pub postings_detail: Vec<PostingDetail>,
    pub metadata: serde_json::Value,
}

/// Detailed posting information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostingDetail {
    pub account: String,
    pub amount: String,
    pub currency: String,
    pub cost: Option<String>,
    pub price: Option<String>,
    pub balance: Option<String>,
    pub is_negative: bool,
}

/// Journal entry for account view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub date: String,
    pub payee: String,
    pub narration: String,
    pub amount: String,
    pub currency: String,
    pub running_balance: String,
}

/// Journal response for account history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalResponse {
    pub account_name: String,
    pub entries: Vec<JournalEntry>,
    pub total_count: usize,
}

// ==================== Report Structures ====================

/// Account balance report entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceReportEntry {
    pub account: String,
    pub account_type: AccountType,
    pub balance: String,
    pub currency: String,
    pub percentage: f64,
}

/// Balance report for all accounts
#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceReport {
    pub entries: Vec<BalanceReportEntry>,
    pub total_assets: String,
    pub total_liabilities: String,
    pub total_equity: String,
    pub net_worth: String,
    pub currency: String,
    pub as_of_date: String,
}

/// Income vs Expenses report
#[derive(Debug, Serialize, Deserialize)]
pub struct IncomeExpenseReport {
    pub income_entries: Vec<IncomeExpenseEntry>,
    pub expense_entries: Vec<IncomeExpenseEntry>,
    pub total_income: String,
    pub total_expenses: String,
    pub net_income: String,
    pub currency: String,
    pub period_start: String,
    pub period_end: String,
}

/// Income/Expense report entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeExpenseEntry {
    pub account: String,
    pub amount: String,
    pub percentage: f64,
}

/// Net worth over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetWorthPoint {
    pub date: String,
    pub assets: String,
    pub liabilities: String,
    pub net_worth: String,
}

/// Net worth history report
#[derive(Debug, Serialize, Deserialize)]
pub struct NetWorthReport {
    pub points: Vec<NetWorthPoint>,
    pub start_net_worth: String,
    pub end_net_worth: String,
    pub change: String,
    pub currency: String,
}

/// Monthly summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlySummary {
    pub month: String,
    pub income: String,
    pub expenses: String,
    pub net_income: String,
    pub transaction_count: usize,
}

/// Monthly summary report
#[derive(Debug, Serialize, Deserialize)]
pub struct MonthlySummaryReport {
    pub summaries: Vec<MonthlySummary>,
    pub year: i32,
    pub total_income: String,
    pub total_expenses: String,
    pub net_income: String,
}

/// Category breakdown for charts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryBreakdown {
    pub category: String,
    pub amount: f64,
    pub percentage: f64,
    pub count: usize,
}

/// Category report
#[derive(Debug, Serialize, Deserialize)]
pub struct CategoryReport {
    pub category_type: String, // "income" or "expense"
    pub entries: Vec<CategoryBreakdown>,
    pub total: String,
    pub currency: String,
}

/// Chart data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataPoint {
    pub label: String,
    pub value: f64,
}

/// Chart data for visualization
#[derive(Debug, Serialize, Deserialize)]
pub struct ChartData {
    pub chart_type: String,
    pub title: String,
    pub labels: Vec<String>,
    pub datasets: Vec<ChartDataset>,
    pub options: serde_json::Value,
}

/// Chart dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataset {
    pub label: String,
    pub data: Vec<f64>,
    pub background_color: Option<String>,
    pub border_color: Option<String>,
}

/// Report period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPeriod {
    pub start_date: String,
    pub end_date: String,
    pub time_range: String,
}
