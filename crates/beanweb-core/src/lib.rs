//! Core ledger processing and business logic

pub mod error;

use async_trait::async_trait;
use beanweb_config::{Config, TimeRange};
use beanweb_parser::{BeancountParserTrait, Directive, SpannedDirective, Transaction as ParserTransaction};
use chrono::{Datelike, DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;

pub use error::CoreError;
pub use error::ErrorSeverity;

/// Parser reference type
pub type ParserRef = Arc<dyn BeancountParserTrait>;

// ==================== Time Control System ====================

/// Global time context for filtering
#[derive(Debug, Clone, PartialEq)]
pub struct TimeContext {
    /// Current time range
    pub range: TimeRange,
    /// Custom start date (when range is Custom)
    pub custom_start: Option<NaiveDate>,
    /// Custom end date (when range is Custom)
    pub custom_end: Option<NaiveDate>,
}

impl Default for TimeContext {
    fn default() -> Self {
        Self {
            range: TimeRange::Month,
            custom_start: None,
            custom_end: None,
        }
    }
}

impl TimeContext {
    /// Create a new time context
    pub fn new(range: TimeRange) -> Self {
        Self {
            range,
            custom_start: None,
            custom_end: None,
        }
    }

    /// Create with custom date range
    pub fn custom(start: NaiveDate, end: NaiveDate) -> Self {
        Self {
            range: TimeRange::Custom,
            custom_start: Some(start),
            custom_end: Some(end),
        }
    }

    /// Get the effective start date based on range
    pub fn start_date(&self) -> Option<NaiveDate> {
        let today = Utc::now().date_naive();
        match self.range {
            TimeRange::Month => Some(today.with_day(1).unwrap_or(today)),
            TimeRange::Quarter => {
                let quarter_start = ((today.month0() / 3) * 3) as u32 + 1;
                NaiveDate::from_ymd_opt(today.year(), quarter_start, 1)
            }
            TimeRange::Year => NaiveDate::from_ymd_opt(today.year(), 1, 1),
            TimeRange::All => None,
            TimeRange::Custom => self.custom_start,
        }
    }

    /// Get the effective end date based on range
    pub fn end_date(&self) -> Option<NaiveDate> {
        let today = Utc::now().date_naive();
        match self.range {
            TimeRange::Month => {
                let last_day = NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
                    .and_then(|d| d.pred_opt())
                    .unwrap_or(today);
                Some(last_day)
            }
            TimeRange::Quarter => {
                let quarter_end = ((today.month0() / 3) + 1) * 3;
                NaiveDate::from_ymd_opt(today.year(), quarter_end + 1, 1)
                    .and_then(|d| d.pred_opt())
                    .or(Some(today))
            }
            TimeRange::Year => NaiveDate::from_ymd_opt(today.year(), 12, 31),
            TimeRange::All => None,
            TimeRange::Custom => self.custom_end,
        }
    }

    /// Check if a date is within the current time context
    pub fn contains(&self, date: &NaiveDate) -> bool {
        let start = self.start_date();
        let end = self.end_date();

        match (start, end) {
            (None, None) => true,
            (Some(s), None) => *date >= s,
            (None, Some(e)) => *date <= e,
            (Some(s), Some(e)) => *date >= s && *date <= e,
        }
    }

    /// Get a human-readable description of the time range
    pub fn description(&self) -> String {
        match self.range {
            TimeRange::Month => "Current Month".to_string(),
            TimeRange::Quarter => "Current Quarter".to_string(),
            TimeRange::Year => "Current Year".to_string(),
            TimeRange::All => "All Time".to_string(),
            TimeRange::Custom => {
                if let (Some(start), Some(end)) = (self.custom_start, self.custom_end) {
                    format!("{} to {}", start, end)
                } else {
                    "Custom Range".to_string()
                }
            }
        }
    }
}

/// Time filtering trait
pub trait TimeFilter {
    /// Filter items by the current time context
    fn filter_by_time(&self, context: &TimeContext) -> bool;
}

impl TimeFilter for Transaction {
    fn filter_by_time(&self, context: &TimeContext) -> bool {
        if let Ok(date) = NaiveDate::parse_from_str(&self.date, "%Y-%m-%d") {
            context.contains(&date)
        } else {
            // If we can't parse the date, include it
            true
        }
    }
}

impl TimeFilter for Account {
    fn filter_by_time(&self, _context: &TimeContext) -> bool {
        // Accounts are not time-filtered by default
        true
    }
}

/// Main ledger structure
pub struct Ledger {
    config: Config,
    parser: ParserRef,
    data: RwLock<LedgerData>,
    directives: RwLock<Vec<SpannedDirective>>,
    entry: (PathBuf, String),
    time_context: RwLock<TimeContext>,
}

/// In-memory ledger data
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LedgerData {
    pub accounts: Vec<Account>,
    pub transactions: Vec<Transaction>,
    pub commodities: Vec<Commodity>,
    pub balances: Vec<BalanceEntry>,
    pub pads: Vec<PadEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadEntry {
    pub account: String,
    pub source_account: String,
    pub date: String,
}

/// Account type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    /// Asset accounts (cash, bank, investments)
    Assets,
    /// Liability accounts (credit cards, loans)
    Liabilities,
    /// Equity accounts (owner's equity)
    Equity,
    /// Income accounts (salary, dividends)
    Income,
    /// Expense accounts (food, transport)
    Expenses,
}

impl Default for AccountType {
    fn default() -> Self {
        AccountType::Assets
    }
}

impl std::str::FromStr for AccountType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "assets" | "asset" => Ok(AccountType::Assets),
            "liabilities" | "liability" => Ok(AccountType::Liabilities),
            "equity" => Ok(AccountType::Equity),
            "income" => Ok(AccountType::Income),
            "expenses" | "expense" => Ok(AccountType::Expenses),
            _ => Err(format!("Invalid account type: {}", s)),
        }
    }
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountType::Assets => write!(f, "assets"),
            AccountType::Liabilities => write!(f, "liabilities"),
            AccountType::Equity => write!(f, "equity"),
            AccountType::Income => write!(f, "income"),
            AccountType::Expenses => write!(f, "expenses"),
        }
    }
}

/// Account status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountStatus {
    /// Account is open and active
    Open,
    /// Account is closed
    Closed,
    /// Account is temporarily paused
    Paused,
}

impl Default for AccountStatus {
    fn default() -> Self {
        AccountStatus::Open
    }
}

impl std::str::FromStr for AccountStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(AccountStatus::Open),
            "closed" => Ok(AccountStatus::Closed),
            "paused" => Ok(AccountStatus::Paused),
            _ => Err(format!("Invalid account status: {}", s)),
        }
    }
}

impl std::fmt::Display for AccountStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountStatus::Open => write!(f, "open"),
            AccountStatus::Closed => write!(f, "closed"),
            AccountStatus::Paused => write!(f, "paused"),
        }
    }
}

/// Account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Full account name (e.g., "Assets:Checking:Chase")
    pub name: String,
    /// Account type (assets, liabilities, equity, income, expenses)
    pub account_type: AccountType,
    /// Account status (open, closed, paused)
    pub status: AccountStatus,
    /// Current balance as JSON value
    pub balance: serde_json::Value,
    /// Currency of the account (if single-currency)
    pub currency: Option<String>,
    /// Account opening date
    pub open_date: Option<String>,
    /// Account closing date (if closed)
    pub close_date: Option<String>,
    /// Account alias (optional human-readable name)
    pub alias: Option<String>,
    /// Note or description
    pub note: Option<String>,
    /// List of account tags
    pub tags: Vec<String>,
}

impl Account {
    /// Get the account name without the type prefix
    pub fn short_name(&self) -> String {
        if let Some(pos) = self.name.find(':') {
            self.name[pos + 1..].to_string()
        } else {
            self.name.clone()
        }
    }

    /// Check if account is a leaf node (has no children)
    pub fn is_leaf(&self) -> bool {
        !self.name.contains(':')
    }

    /// Get the parent account name
    pub fn parent_name(&self) -> Option<String> {
        if let Some(pos) = self.name.rfind(':') {
            Some(self.name[..pos].to_string())
        } else {
            None
        }
    }

    /// Check if this is a root account
    pub fn is_root(&self) -> bool {
        !self.name.contains(':')
    }

    /// Get the depth level (0 = root)
    pub fn depth(&self) -> usize {
        self.name.chars().filter(|&c| c == ':').count()
    }
}

/// Transaction information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Unique transaction identifier
    pub id: String,
    /// Transaction date (YYYY-MM-DD format)
    pub date: String,
    /// Transaction time (HH:MM:SS format, extracted from metadata)
    pub time: String,
    /// Payee name
    pub payee: String,
    /// Transaction narration/description
    pub narration: String,
    /// List of postings
    pub postings: Vec<Posting>,
    /// Transaction flag (e.g., "*" for cleared, "!" for pending)
    pub flag: Option<String>,
    /// Transaction tags
    pub tags: Vec<String>,
    /// Transaction links
    pub links: Vec<String>,
    /// Metadata key-value pairs
    pub metadata: serde_json::Value,
    /// Source file location
    pub source: Option<String>,
    /// Line number in source file
    pub line: Option<u32>,
}

impl Transaction {
    /// Get the transaction date as NaiveDate
    pub fn date_naive(&self) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(&self.date, "%Y-%m-%d").ok()
    }

    /// Get formatted datetime string (date + time)
    pub fn datetime(&self) -> String {
        if self.time.is_empty() || self.time == "00:00:00" {
            self.date.clone()
        } else {
            format!("{} {}", self.date, self.time)
        }
    }

    /// Check if transaction has time information
    pub fn has_time(&self) -> bool {
        !self.time.is_empty() && self.time != "00:00:00"
    }

    /// Check if transaction involves a specific account
    pub fn involves_account(&self, account_name: &str) -> bool {
        self.postings.iter().any(|p| p.account == account_name)
    }

    /// Get all accounts involved in this transaction
    pub fn accounts(&self) -> Vec<&str> {
        self.postings.iter().map(|p| p.account.as_str()).collect()
    }

    /// Check if transaction is balanced
    pub fn is_balanced(&self) -> bool {
        // Simple check - in production, would check actual amounts
        !self.postings.is_empty()
    }

    /// Get posting count
    pub fn posting_count(&self) -> usize {
        self.postings.len()
    }

    /// Get a summary string
    pub fn summary(&self) -> String {
        let payee = if self.payee.is_empty() {
            &self.narration
        } else {
            &self.payee
        };
        format!("{} - {}", self.date, payee)
    }
}

/// Posting in a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Posting {
    /// Account name
    pub account: String,
    /// Amount (can be negative for expenses/liabilities)
    pub amount: String,
    /// Currency code
    pub currency: String,
    /// Cost per unit (for investments)
    pub cost: Option<String>,
    /// Price (for currency conversion)
    pub price: Option<String>,
    /// Balance assertion
    pub balance: Option<String>,
    /// Posting metadata
    pub metadata: serde_json::Value,
}

impl Posting {
    /// Get amount as f64 (if parseable)
    /// Handles @ price syntax: e.g., "800 PI @ 1 CNY" returns 800.0 (units)
    /// For transactions with price, the caller should calculate total: units * price
    pub fn amount_value(&self) -> Option<f64> {
        // Extract the first number from the amount string
        // Handle formats: "800 CNY", "-800.00 CNY", "800 PI @ 1 CNY"
        let chars: Vec<char> = self.amount.chars().collect();
        let mut i = 0;
        let mut has_decimal = false;
        let mut has_negative = false;

        // Handle negative sign
        if !chars.is_empty() && chars[0] == '-' {
            has_negative = true;
            i = 1;
        }

        // Skip whitespace
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
        }

        // Parse the number (digits and optional decimal point)
        let mut num_str = String::new();
        while i < chars.len() {
            let c = chars[i];
            if c.is_ascii_digit() {
                num_str.push(c);
            } else if c == '.' && !has_decimal {
                num_str.push(c);
                has_decimal = true;
            } else if c == ' ' || c == '\t' {
                break;  // End of number
            } else {
                break;  // Non-numeric character (currency, @, etc.)
            }
            i += 1;
        }

        if num_str.is_empty() {
            return None;
        }

        let value: f64 = num_str.parse().ok()?;
        Some(if has_negative { -value } else { value })
    }

    /// Get price per unit if specified (e.g., "@ 1 CNY")
    /// Returns Some((price_value, price_currency)) or None
    pub fn price_info(&self) -> Option<(f64, String)> {
        // Look for @ in the amount string (single @ for price, @@ for total)
        let at_pos = self.amount.find('@')?;
        let after_at = &self.amount[at_pos + 1..].trim();

        // Parse the price value and currency
        let chars: Vec<char> = after_at.chars().collect();
        let mut i = 0;
        let mut num_str = String::new();

        // Parse number
        while i < chars.len() {
            let c = chars[i];
            if c.is_ascii_digit() || c == '.' {
                num_str.push(c);
            } else if c == ' ' {
                break;
            }
            i += 1;
        }

        // Parse currency
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
        }

        let currency = chars[i..].iter().collect::<String>().trim().to_string();
        let price_value: f64 = num_str.parse().ok()?;

        Some((price_value, currency))
    }

    /// Calculate total value in operating currency using price if available
    /// Returns (value, currency) - currency will be empty if no amount
    pub fn total_value(&self, operating_currency: &str) -> (f64, String) {
        let amount = match self.amount_value() {
            Some(v) => v,
            None => return (0.0, String::new()),
        };

        // If price is available and different from amount currency, calculate total
        if let Some((price, price_currency)) = self.price_info() {
            // Check if we need to convert
            let amount_currency = &self.currency;
            if *amount_currency != operating_currency || *amount_currency != price_currency {
                // Calculate total: units * price
                return (amount.abs() * price, price_currency);
            }
        }

        // Default: return amount as-is with its currency
        (amount, self.currency.clone())
    }

    /// Check if this is a credit (negative amount)
    pub fn is_credit(&self) -> bool {
        self.amount_value()
            .map(|v| v < 0.0)
            .unwrap_or(false)
    }

    /// Check if this is a debit (positive amount)
    pub fn is_debit(&self) -> bool {
        self.amount_value()
            .map(|v| v > 0.0)
            .unwrap_or(false)
    }
}

/// Commodity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commodity {
    pub name: String,
    pub precision: u32,
}

/// Balance entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceEntry {
    pub account: String,
    pub amount: String,
    pub currency: String,
    pub date: String,
}

impl Ledger {
    /// Create a new ledger with config and parser
    pub fn new(config: Config, parser: ParserRef) -> Self {
        // Initialize with All range - time filtering is per-page, not global
        Self {
            config,
            parser,
            data: RwLock::new(LedgerData::default()),
            directives: RwLock::new(Vec::new()),
            entry: (PathBuf::new(), String::new()),
            time_context: RwLock::new(TimeContext::new(TimeRange::All)),
        }
    }

    /// Load ledger from entry point
    pub async fn load(&mut self, entry: PathBuf) -> Result<(), CoreError> {
        let directives = self.parser.parse_file(entry.clone()).await
            .map_err(|e| CoreError::ParseError { message: e.to_string() })?;

        // Store parsed directives
        {
            let mut dir_guard = self.directives.write().unwrap();
            *dir_guard = directives;
        }

        self.entry = (entry.clone(), entry.to_string_lossy().to_string());

        // Process directives and populate data
        self.process_result().await;

        Ok(())
    }

    /// Reload the ledger
    pub async fn reload(&mut self) -> Result<(), CoreError> {
        if self.entry.0.exists() {
            self.load(self.entry.0.clone()).await
        } else {
            Err(CoreError::NotLoaded)
        }
    }

    // ==================== Helper Functions for Directive Processing ====================

    /// Format date from parser type to string
    fn format_date(date: &beanweb_parser::Date) -> String {
        match date {
            beanweb_parser::Date::Date(d) => d.format("%Y-%m-%d").to_string(),
            beanweb_parser::Date::DateTime(dt) => dt.format("%Y-%m-%d").to_string(),
        }
    }

    /// Map parser account type to core account type
    fn map_account_type(parser_type: &beanweb_parser::AccountType) -> AccountType {
        match parser_type {
            beanweb_parser::AccountType::Assets => AccountType::Assets,
            beanweb_parser::AccountType::Liabilities => AccountType::Liabilities,
            beanweb_parser::AccountType::Equity => AccountType::Equity,
            beanweb_parser::AccountType::Income => AccountType::Income,
            beanweb_parser::AccountType::Expenses => AccountType::Expenses,
        }
    }

    /// Extract time string from parser metadata
    /// Supports formats: "HH:MM:SS", "YYYY-MM-DD HH:MM:SS", "HH:MM:SS.microseconds"
    fn extract_time_from_meta(meta: &beanweb_parser::Meta) -> String {
        let time_keys = ["time", "trade_time", "tgbot_time", "payTime", "created_at"];

        for key in time_keys {
            if let Some(value) = meta.get(key) {
                let time_str = value.as_str();

                // Try parsing various time formats
                // Format: "HH:MM:SS" or "HH:MM:SS.microseconds"
                if let Ok(time) = chrono::NaiveTime::parse_from_str(time_str, "%H:%M:%S") {
                    return time.format("%H:%M:%S").to_string();
                }
                if let Ok(time) = chrono::NaiveTime::parse_from_str(time_str, "%H:%M:%S.%f") {
                    return time.format("%H:%M:%S").to_string();
                }
                // Format: "YYYY-MM-DD HH:MM:SS"
                if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(time_str, "%Y-%m-%d %H:%M:%S") {
                    return dt.time().format("%H:%M:%S").to_string();
                }
                // Format: "YYYY-MM-DD HH:MM:%S.%f"
                if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(time_str, "%Y-%m-%d %H:%M:%S.%f") {
                    return dt.time().format("%H:%M:%S").to_string();
                }
                // Format: "YYYY-MM-DD HH:MM:SS +0800 CST" (with timezone)
                // Extract date-time part by taking first 2 parts after split
                let parts: Vec<&str> = time_str.split_whitespace().collect();
                if parts.len() >= 2 {
                    let dt_part = format!("{} {}", parts[0], parts[1]);
                    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&dt_part, "%Y-%m-%d %H:%M:%S") {
                        return dt.time().format("%H:%M:%S").to_string();
                    }
                }
            }
        }
        String::new()
    }

    /// Convert parser metadata to serde_json::Value
    fn convert_metadata(meta: &beanweb_parser::Meta) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (key, value) in meta.inner().iter() {
            let json_value = serde_json::Value::String(value.as_str().to_string());
            map.insert(key.clone(), json_value);
        }
        serde_json::Value::Object(map)
    }

    /// Convert parser transaction to core transaction
    fn convert_transaction(txn: &ParserTransaction, line: usize, source: Option<&str>) -> Transaction {
        let date_str = match &txn.date {
            beanweb_parser::Date::Date(d) => d.format("%Y-%m-%d").to_string(),
            beanweb_parser::Date::DateTime(dt) => dt.format("%Y-%m-%d").to_string(),
        };

        // Extract time from metadata
        let time_str = Self::extract_time_from_meta(&txn.meta);

        // Convert postings - preserve cost and price from parser
        let postings: Vec<Posting> = txn.postings.iter().map(|p| {
            let amount_str = p.amount.as_ref().map(|a| format!("{} {}", a.amount, a.currency));
            let currency = p.amount.as_ref().map(|a| a.currency.clone()).unwrap_or_default();

            // Format cost if present (e.g., {800 PI @ 1 CNY})
            let cost_str = p.cost.as_ref().map(|c| {
                format!("{{{} {}}}", c.amount, c.currency)
            });

            // Format price if present (e.g., @ 1 CNY)
            let price_str = p.price.as_ref().map(|pr| {
                match pr {
                    beanweb_parser::Price::Single(amount) => {
                        format!("@ {} {}", amount.amount, amount.currency)
                    }
                    beanweb_parser::Price::Total(total) => {
                        format!("@@ {} {}", total.amount, total.currency)
                    }
                }
            });

            // Build full amount string with cost and price
            let full_amount = match (&amount_str, &cost_str, &price_str) {
                (Some(amt), Some(cost), Some(price)) => format!("{} {} {}", amt, cost, price),
                (Some(amt), Some(cost), None) => format!("{} {}", amt, cost),
                (Some(amt), None, Some(price)) => format!("{} {}", amt, price),
                (Some(amt), None, None) => amt.clone(),
                _ => String::new(),
            };

            Posting {
                account: p.account.name.clone(),
                amount: full_amount,
                currency,
                cost: cost_str,
                price: price_str,
                balance: None,
                metadata: serde_json::Value::Object(serde_json::Map::new()),
            }
        }).collect();

        // Build transaction content for ID generation
        let txn_content = Self::build_transaction_content(txn, &date_str);

        // Generate unique ID from source, line, and content hash
        let id = beanweb_parser::generate_txn_id(source, line, &txn_content);

        // Convert metadata
        let metadata = Self::convert_metadata(&txn.meta);

        Transaction {
            id,
            date: date_str,
            time: time_str,
            payee: txn.payee.clone().unwrap_or_default(),
            narration: txn.narration.clone().unwrap_or_default(),
            postings,
            flag: txn.flag.clone(),
            tags: txn.tags.clone(),
            links: txn.links.clone(),
            metadata,
            source: source.map(|s| s.to_string()),
            line: Some(line as u32),
        }
    }

    /// Build transaction content string for ID generation
    fn build_transaction_content(txn: &ParserTransaction, date_str: &str) -> String {
        let mut content = format!("{} {} \"{}\" \"{}\"",
            date_str,
            txn.flag.clone().unwrap_or_default(),
            txn.payee.clone().unwrap_or_default(),
            txn.narration.clone().unwrap_or_default()
        );

        for tag in &txn.tags {
            content.push_str(&format!(" #{}", tag));
        }
        for link in &txn.links {
            content.push_str(&format!(" ^{}", link));
        }

        for p in &txn.postings {
            content.push_str(&format!("\n    {}", p.account.name));
            if let Some(amt) = &p.amount {
                content.push_str(&format!(" {} {}", amt.amount, amt.currency));
            }
        }

        content
    }

    /// Process parse result into ledger data
    async fn process_result(&mut self) {
        let mut data = self.data.write().unwrap();
        let directives = self.directives.read().unwrap();

        // eprintln!("[DEBUG] Processing {} directives", directives.len());

        // Count directive types for debugging
        let mut open_count = 0;
        let mut txn_count = 0;
        let mut balance_count = 0;
        let mut include_count = 0;
        let mut other_count = 0;

        for d in directives.iter() {
            match &d.data {
                Directive::Open(_) => open_count += 1,
                Directive::Transaction(_) => txn_count += 1,
                Directive::Balance(_) => balance_count += 1,
                Directive::Include(_) => include_count += 1,
                _ => other_count += 1,
            }
        }
        // eprintln!("[DEBUG] Directive counts - Open: {}, Transaction: {}, Balance: {}, Include: {}, Other: {}",
        //     open_count, txn_count, balance_count, include_count, other_count);

        // Clear existing data
        data.accounts.clear();
        data.transactions.clear();
        data.commodities.clear();
        data.balances.clear();
        data.pads.clear();

        // Track seen accounts to avoid duplicates
        let mut seen_accounts: std::collections::HashSet<String> = std::collections::HashSet::new();

        // First pass: collect all Pad directives (we'll process them after Balance directives)
        let mut pad_directives: Vec<Directive> = Vec::new();

        for directive in directives.iter() {
            if let Directive::Pad(pad) = &directive.data {
                pad_directives.push(directive.data.clone());
                eprintln!("[DEBUG Pad collected] first_account={}, second_account={}, date={}",
                    pad.account.name, pad.pad.name, Self::format_date(&pad.date));
            }
        }

        eprintln!("[DEBUG] Total Pad directives collected: {}", pad_directives.len());

        // Second pass: process all directives
        for directive in directives.iter() {
            match &directive.data {
                Directive::Open(open) => {
                    let name = open.account.name.clone();
                    // eprintln!("[DEBUG] Found account: {}", name);
                    if !seen_accounts.contains(&name) {
                        seen_accounts.insert(name.clone());
                        let account = Account {
                            name: name.clone(),
                            account_type: Self::map_account_type(&open.account.account_type),
                            status: AccountStatus::Open,
                            balance: serde_json::Value::Null,
                            currency: open.currencies.first().cloned(),
                            open_date: Some(Self::format_date(&open.date)),
                            close_date: None,
                            alias: None,
                            note: None,
                            tags: Vec::new(),
                        };
                        data.accounts.push(account);
                    }
                },
                Directive::Close(close) => {
                    // Mark account as closed
                    if let Some(acc) = data.accounts.iter_mut().find(|a| a.name == close.account.name) {
                        acc.status = AccountStatus::Closed;
                        acc.close_date = Some(Self::format_date(&close.date));
                    }
                },
                Directive::Transaction(txn) => {
                    let source = directive.source.as_deref();
                    let transaction = Self::convert_transaction(txn, directive.span.start, source);
                    data.transactions.push(transaction);
                },
                Directive::Balance(balance) => {
                    let balance_date = balance.date.clone();
                    let balance_amount = balance.amount.amount.to_string();
                    let balance_currency = balance.amount.currency.clone();
                    let balance_date_str = Self::format_date(&balance_date);

                    // Only update account balance if this balance is more recent
                    if let Some(acc) = data.accounts.iter_mut().find(|a| a.name == balance.account.name) {
                        // Parse existing balance amount to check if we need to update
                        let _existing_balance = Self::parse_balance_value(&acc.balance);
                        // Compare dates - only update if new balance is more recent
                        if let Some(existing_obj) = acc.balance.as_object() {
                            if let Some(existing_date_str) = existing_obj.get("date").and_then(|v| v.as_str()) {
                                if let Ok(existing_date) = chrono::NaiveDate::parse_from_str(existing_date_str, "%Y-%m-%d") {
                                    if let Ok(new_date) = chrono::NaiveDate::parse_from_str(&balance_date_str, "%Y-%m-%d") {
                                        if new_date > existing_date {
                                            acc.balance = serde_json::json!({
                                                "amount": balance_amount,
                                                "currency": balance_currency,
                                                "date": balance_date_str
                                            });
                                        }
                                    }
                                } else {
                                    // Can't parse existing date, update anyway
                                    acc.balance = serde_json::json!({
                                        "amount": balance_amount,
                                        "currency": balance_currency,
                                        "date": balance_date_str
                                    });
                                }
                            } else {
                                // No existing date, update
                                acc.balance = serde_json::json!({
                                    "amount": balance_amount,
                                    "currency": balance_currency,
                                    "date": balance_date_str
                                });
                            }
                        } else {
                            // First time setting balance
                            acc.balance = serde_json::json!({
                                "amount": balance_amount,
                                "currency": balance_currency,
                                "date": balance_date_str
                            });
                        }
                    }
                    // Also add to balances list for historical records
                    let entry = BalanceEntry {
                        account: balance.account.name.clone(),
                        amount: balance.amount.amount.to_string(),
                        currency: balance.amount.currency.clone(),
                        date: Self::format_date(&balance.date),
                    };
                    eprintln!("[DEBUG Balance stored] account={}, date={}, amount={}",
                        entry.account, entry.date, entry.amount);
                    data.balances.push(entry);
                },
                // Skip Pad here - we'll process them after all Balance directives
                Directive::Pad(_) => {
                    // Already collected in first pass
                },
                _ => {
                    // Other directive types not yet processed
                }
            }
        }

        // eprintln!("[DEBUG] Processed {} accounts, {} transactions, {} balances",
        //     data.accounts.len(), data.transactions.len(), data.balances.len());

        // Third pass: process Pad directives after all Balance directives have been processed
        // This ensures we can find the correct balance amount for each Pad
        eprintln!("[DEBUG] Available balances before Pad processing:");
        for balance in &data.balances {
            eprintln!("[DEBUG]   Balance: account={}, date={}, amount={}", balance.account, balance.date, balance.amount);
        }

        for pad_directive in pad_directives {
            if let Directive::Pad(pad) = pad_directive {
                // Store pad directive for timeline calculation
                let entry = PadEntry {
                    account: pad.account.name.clone(),
                    source_account: pad.pad.name.clone(),
                    date: Self::format_date(&pad.date),
                };
                data.pads.push(entry);

                // Find the balance amount for the pad
                // Beancount semantics: pad SOURCE TARGET
                // - SOURCE: Assets/Liabilities account (has Balance directive)
                // - TARGET: any account to be padded
                // For Income/Expenses accounts (no Balance), we infer from SOURCE's balance change
                let pad_date = NaiveDate::parse_from_str(&Self::format_date(&pad.date), "%Y-%m-%d")
                    .unwrap_or_else(|_| chrono::Utc::now().date_naive());
                let mut target_amount = String::new();
                let mut target_currency = String::new();

                // Determine if this is an Income/Expenses account (TARGET account)
                // Income/Expenses accounts need the "difference" calculation
                let is_income_or_expense = pad.pad.name.starts_with("Income:")
                    || pad.pad.name.starts_with("Expenses:");

                eprintln!("[DEBUG Pad calc] pad_date={}, source={}, target={}, is_income_expense={}",
                    pad_date, pad.account.name, pad.pad.name, is_income_or_expense);

                // For Income/Expenses accounts, we need to calculate the difference
                // difference = Balance_change - Other_transactions_on_source
                // This is because the Balance reflects ALL transactions, not just the Pad
                if is_income_or_expense {
                    eprintln!("[DEBUG Pad calc] Computing difference for Income/Expenses account");

                    // Collect all balances for the source account (pad.account)
                    let source_account = pad.account.name.clone();
                    let mut source_balances: Vec<(NaiveDate, String, String)> = Vec::new();

                    for balance in &data.balances {
                        if balance.account == source_account {
                            if let Ok(balance_date) = NaiveDate::parse_from_str(&balance.date, "%Y-%m-%d") {
                                source_balances.push((balance_date, balance.amount.clone(), balance.currency.clone()));
                            }
                        }
                    }

                    // Sort by date (ascending)
                    source_balances.sort_by(|a, b| a.0.cmp(&b.0));

                    eprintln!("[DEBUG Pad calc] Found {} balances for {}", source_balances.len(), source_account);

                    // Find the balance on or after the pad date
                    let mut balance_on_pad_date = None;
                    let mut prev_balance = None;

                    for (idx, (balance_date, balance_amount, currency)) in source_balances.iter().enumerate() {
                        if *balance_date >= pad_date {
                            eprintln!("[DEBUG Pad calc] Found balance at {}: amount={}", balance_date, balance_amount);
                            balance_on_pad_date = Some((balance_amount.clone(), currency.clone()));

                            if idx > 0 {
                                prev_balance = Some((source_balances[idx - 1].1.clone(), source_balances[idx - 1].2.clone()));
                                eprintln!("[DEBUG Pad calc] Previous balance at {}: amount={}",
                                    source_balances[idx - 1].0, source_balances[idx - 1].1);
                            }
                            break;
                        }
                    }

                    if let Some((curr_amt, curr_curr)) = balance_on_pad_date {
                        let curr_amount: f64 = curr_amt.parse().unwrap_or(0.0);
                        target_currency = curr_curr;

                        if let Some((prev_amt, _)) = prev_balance {
                            let prev_amount: f64 = prev_amt.parse().unwrap_or(0.0);
                            let balance_change = curr_amount - prev_amount;

                            eprintln!("[DEBUG Pad calc] Balance change: {} - {} = {}",
                                curr_amount, prev_amount, balance_change);

                            // Get the actual dates for prev and current balance
                            let prev_balance_date = source_balances.iter()
                                .find(|(d, _, _)| *d < pad_date)
                                .map(|(d, _, _)| *d);

                            let curr_balance_date = source_balances.iter()
                                .find(|(d, _, _)| *d >= pad_date)
                                .map(|(d, _, _)| *d);

                            eprintln!("[DEBUG Pad calc] prev_balance_date={:?}, curr_balance_date={:?}, pad_date={:?}",
                                prev_balance_date, curr_balance_date, pad_date);

                            // Calculate sum of all non-Pad transactions for the source account
                            // We need to look at transactions BETWEEN the prev and curr balance dates
                            // (exclusive of curr_balance_date since Balance shows balance at START of day)
                            let mut other_tx_sum = 0.0f64;
                            for tx in &data.transactions {
                                let tx_date = match NaiveDate::parse_from_str(&tx.date, "%Y-%m-%d") {
                                    Ok(d) => d,
                                    Err(_) => continue,
                                };

                                // Skip transactions before or at prev_balance_date
                                // (Balance shows balance at START of day, so transactions on that day count)
                                if let Some(pbd) = prev_balance_date {
                                    if tx_date <= pbd {
                                        continue;
                                    }
                                }

                                // Skip transactions on or after curr_balance_date
                                // (Balance shows balance at START of day, so transactions on that day are after)
                                if let Some(cbd) = curr_balance_date {
                                    if tx_date >= cbd {
                                        continue;
                                    }
                                }

                                // Check if this is the Pad transaction we're processing
                                // by looking for a posting to the target account (pad.pad.name = Income account)
                                let is_this_pad = tx.postings.iter().any(|p| {
                                    p.account == pad.pad.name
                                });
                                if is_this_pad {
                                    continue;
                                }

                                // Sum up postings to the source account (excluding the Pad)
                                for posting in &tx.postings {
                                    if posting.account == source_account {
                                        if let Some(amount_str) = posting.amount.split_whitespace().next() {
                                            if let Ok(amount) = amount_str.parse::<f64>() {
                                                other_tx_sum += amount;
                                                eprintln!("[DEBUG Pad calc] Found tx {} on {}: amount={}, running_sum={}",
                                                    tx.date, source_account, amount, other_tx_sum);
                                            }
                                        }
                                    }
                                }
                            }

                            eprintln!("[DEBUG Pad calc] Other transactions sum: {}", other_tx_sum);

                            // The Pad difference = Balance change - Other transactions
                            // This is the amount that goes to/from Income
                            let pad_difference = balance_change - other_tx_sum;

                            eprintln!("[DEBUG Pad calc] Pad difference: {} - {} = {}",
                                balance_change, other_tx_sum, pad_difference);

                            // Display as negative for Income received (credit)
                            let display_amount = -pad_difference;
                            target_amount = format!("{:.2}", display_amount);

                            eprintln!("[DEBUG Pad calc] Final target_amount for {}: {}",
                                pad.pad.name, target_amount);
                        }
                    }
                } else {
                    // For Assets/Liabilities accounts, use the balance amount directly
                    let mut found_balance = false;
                    for balance in &data.balances {
                        if balance.account == pad.account.name {
                            let balance_date = NaiveDate::parse_from_str(&balance.date, "%Y-%m-%d")
                                .unwrap_or_else(|_| chrono::Utc::now().date_naive());
                            if balance_date >= pad_date {
                                target_amount = balance.amount.clone();
                                target_currency = if balance.currency.is_empty() {
                                    "CNY".to_string()
                                } else {
                                    balance.currency.clone()
                                };
                                eprintln!("[DEBUG Pad] account={}, pad_date={}, balance_date={}, amount={}",
                                    pad.account.name, pad_date, balance_date, target_amount);
                                found_balance = true;
                                break;
                            }
                        }
                    }
                }

                // If still empty, try account balance
                if target_amount.is_empty() {
                    if let Some(acc) = data.accounts.iter().find(|a| a.name == pad.account.name) {
                        if let Some(balance_obj) = acc.balance.as_object() {
                            if let Some(amount_str) = balance_obj.get("amount").and_then(|v| v.as_str()) {
                                target_amount = amount_str.to_string();
                            }
                            if let Some(currency_str) = balance_obj.get("currency").and_then(|v| v.as_str()) {
                                target_currency = currency_str.to_string();
                            } else {
                                target_currency = "CNY".to_string();
                            }
                            eprintln!("[DEBUG Pad fallback] account={}, amount={}", pad.account.name, target_amount);
                        }
                    }
                }

                if target_amount.is_empty() {
                    target_amount = "0.00".to_string();
                    target_currency = "CNY".to_string();
                }

                // Calculate amounts for the double-entry transaction
                // target_amount is the "difference" for the TARGET account
                // For Income:Income = negative (credit), For Assets = positive (debit)
                let target_num: f64 = target_amount.parse().unwrap_or(0.0);
                let source_num = -target_num;  // Opposite sign for double-entry

                let target_posting_amount = format!("{:.2}", target_num);
                let source_posting_amount = format!("{:.2}", source_num);

                eprintln!("[DEBUG Generate Pad tx] target_amount={}, source_amount={}",
                    target_posting_amount, source_posting_amount);

                // Generate a synthetic transaction for the Pad directive
                // Format: "TARGET from SOURCE" so page can correctly display description
                let narration = format!("{} from {}", pad.account.name, pad.pad.name);
                eprintln!("[DEBUG Generate Pad tx] narration={}", narration);

                // For Assets account (pad.account.name): the difference is a decrease (negative)
                // For Income account (pad.pad.name): the difference is income received (negative)
                // Wait, let me reconsider:
                // - Assets: 余利宝 balance increased from 966.05 to 10619.01
                // - This means money came IN to 余利宝 from Income
                // - So 余利宝 posting should be POSITIVE (money in)
                // - Income posting should be NEGATIVE (money out / income received)

                let tx = Transaction {
                    id: format!("pad-{}", Self::format_date(&pad.date)),
                    date: Self::format_date(&pad.date),
                    time: String::new(),
                    flag: None,
                    payee: String::new(),
                    narration: narration.clone(),
                    tags: vec!["pad".to_string()],
                    links: Vec::new(),
                    postings: vec![
                        Posting {
                            account: pad.account.name.clone(),  // SOURCE = Assets/Liabilities
                            amount: format!("{:.2}", -target_num),  // Negative for Assets decrease (if target was positive)
                            currency: target_currency.clone(),
                            cost: None,
                            price: None,
                            balance: None,
                            metadata: serde_json::Value::Object(serde_json::Map::new()),
                        },
                        Posting {
                            account: pad.pad.name.clone(),  // TARGET = Income/Expenses
                            amount: target_posting_amount.clone(),  // The difference amount
                            currency: target_currency.clone(),
                            cost: None,
                            price: None,
                            balance: None,
                            metadata: serde_json::Value::Object(serde_json::Map::new()),
                        },
                    ],
                    metadata: serde_json::json!({
                        "pad_source": pad.pad.name.clone(),
                        "pad_date": Self::format_date(&pad.date),
                    }),
                    source: None,
                    line: None,
                };
                data.transactions.push(tx);
            }
        }

        drop(data);
    }

    /// Get all accounts
    pub fn accounts(&self) -> Vec<Account> {
        self.data.read().unwrap().accounts.clone()
    }

    /// Get transactions with pagination
    pub fn transactions(&self, limit: usize, offset: usize) -> Vec<Transaction> {
        let data = self.data.read().unwrap();
        data.transactions
            .iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get total transaction count
    pub fn transactions_count(&self) -> usize {
        self.data.read().unwrap().transactions.len()
    }

    /// Get filtered transaction count by time context
    pub fn filtered_transactions_count(&self) -> usize {
        let data = self.data.read().unwrap();
        let context = self.time_context.read().unwrap().clone();
        data.transactions.iter()
            .filter(|t| t.filter_by_time(&context))
            .count()
    }

    /// Get account by name
    pub fn account(&self, name: &str) -> Option<Account> {
        let data = self.data.read().unwrap();
        data.accounts.iter().find(|a| &a.name == name).cloned()
    }

    // ==================== Account Management Methods ====================

    /// Get all accounts
    pub fn all_accounts(&self) -> Vec<Account> {
        self.data.read().unwrap().accounts.clone()
    }

    /// Get accounts by type
    pub fn accounts_by_type(&self, account_type: AccountType) -> Vec<Account> {
        let data = self.data.read().unwrap();
        data.accounts
            .iter()
            .filter(|a| a.account_type == account_type)
            .cloned()
            .collect()
    }

    /// Get accounts by status
    pub fn accounts_by_status(&self, status: AccountStatus) -> Vec<Account> {
        let data = self.data.read().unwrap();
        data.accounts
            .iter()
            .filter(|a| a.status == status)
            .cloned()
            .collect()
    }

    /// Get root accounts (accounts without parent)
    pub fn root_accounts(&self) -> Vec<Account> {
        let data = self.data.read().unwrap();
        data.accounts
            .iter()
            .filter(|a| a.is_root())
            .cloned()
            .collect()
    }

    /// Get child accounts of a parent account
    pub fn child_accounts(&self, parent_name: &str) -> Vec<Account> {
        let data = self.data.read().unwrap();
        data.accounts
            .iter()
            .filter(|a| a.parent_name().as_deref() == Some(parent_name))
            .cloned()
            .collect()
    }

    /// Get all descendant accounts (children and grandchildren)
    pub fn descendant_accounts(&self, parent_name: &str) -> Vec<Account> {
        let data = self.data.read().unwrap();
        let prefix = format!("{}:", parent_name);
        data.accounts
            .iter()
            .filter(|a| a.name.starts_with(&prefix))
            .cloned()
            .collect()
    }

    /// Get account tree structure
    pub fn account_tree(&self) -> Vec<AccountTreeNode> {
        let root_accounts = self.root_accounts();
        root_accounts
            .into_iter()
            .map(|acc| self.build_tree(&acc))
            .collect()
    }

    /// Build tree node recursively
    fn build_tree(&self, account: &Account) -> AccountTreeNode {
        let children = self.child_accounts(&account.name);
        AccountTreeNode {
            account: account.clone(),
            children: children.into_iter().map(|c| self.build_tree(&c)).collect(),
        }
    }

    /// Search accounts by name pattern
    pub fn search_accounts(&self, query: &str) -> Vec<Account> {
        let data = self.data.read().unwrap();
        let query_lower = query.to_lowercase();
        data.accounts
            .iter()
            .filter(|a| a.name.to_lowercase().contains(&query_lower))
            .cloned()
            .collect()
    }

    /// Get account balance summary
    pub fn account_balance_summary(&self) -> AccountBalanceSummary {
        let data = self.data.read().unwrap();

        let assets: Vec<Account> = data.accounts
            .iter()
            .filter(|a| a.account_type == AccountType::Assets)
            .cloned()
            .collect();

        let liabilities: Vec<Account> = data.accounts
            .iter()
            .filter(|a| a.account_type == AccountType::Liabilities)
            .cloned()
            .collect();

        let income: Vec<Account> = data.accounts
            .iter()
            .filter(|a| a.account_type == AccountType::Income)
            .cloned()
            .collect();

        let expenses: Vec<Account> = data.accounts
            .iter()
            .filter(|a| a.account_type == AccountType::Expenses)
            .cloned()
            .collect();

        AccountBalanceSummary {
            total_assets: assets.len(),
            total_liabilities: liabilities.len(),
            total_income: income.len(),
            total_expenses: expenses.len(),
            asset_accounts: assets,
            liability_accounts: liabilities,
            income_accounts: income,
            expense_accounts: expenses,
        }
    }

    /// Get account count by type
    pub fn account_count_by_type(&self) -> serde_json::Value {
        let data = self.data.read().unwrap();
        serde_json::json!({
            "assets": data.accounts.iter().filter(|a| a.account_type == AccountType::Assets).count(),
            "liabilities": data.accounts.iter().filter(|a| a.account_type == AccountType::Liabilities).count(),
            "equity": data.accounts.iter().filter(|a| a.account_type == AccountType::Equity).count(),
            "income": data.accounts.iter().filter(|a| a.account_type == AccountType::Income).count(),
            "expenses": data.accounts.iter().filter(|a| a.account_type == AccountType::Expenses).count(),
        })
    }

    /// Calculate account balance from all transactions
    /// Returns a HashMap of account name to balance
    /// This method correctly calculates balances by:
    /// 1. Starting with initial balances from the latest Balance directive (stored in Account.balance)
    /// 2. Adding only transactions that occur AFTER the latest Balance directive
    pub fn calculate_account_balances(&self) -> std::collections::HashMap<String, f64> {
        let data = self.data.read().unwrap();
        let mut balances: std::collections::HashMap<String, f64> = std::collections::HashMap::new();

        // Build a map of account -> (balance_date, balance_amount)
        // Account.balance stores the latest Balance directive value
        let mut account_balance_dates: std::collections::HashMap<String, (chrono::NaiveDate, f64)> = std::collections::HashMap::new();

        for account in &data.accounts {
            if let Some(balance) = self.parse_balance(&account.balance) {
                // Get the date from the balance JSON
                if let Some(obj) = account.balance.as_object() {
                    if let Some(date_val) = obj.get("date") {
                        if let Some(date_str) = date_val.as_str() {
                            if let Some(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok() {
                                account_balance_dates.insert(account.name.clone(), (date, balance));
                            }
                        }
                    }
                }
                // Always add initial balance for all accounts with Balance directives
                balances.insert(account.name.clone(), balance);
            }
        }

        // Add transaction postings that occur after the balance date
        for transaction in &data.transactions {
            // Parse transaction date
            if let Some(txn_date) = transaction.date_naive() {
                for posting in &transaction.postings {
                    let account_name = &posting.account;

                    // Check if this account has a balance date
                    if let Some((balance_date, _)) = account_balance_dates.get(account_name) {
                        // Add transactions on or after the balance date
                        // Balance directive sets balance at START of that day
                        if txn_date >= *balance_date {
                            // Use calculate_posting_amount to handle empty amounts (inferred from other postings)
                            let amount = Self::calculate_posting_amount(transaction, account_name);
                            let entry = balances.entry(account_name.clone()).or_insert(0.0);
                            *entry += amount;
                        }
                        // If transaction is before balance date, skip it (already accounted for in balance)
                    } else {
                        // No balance directive for this account, add all transactions
                        // Use calculate_posting_amount to handle empty amounts (inferred from other postings)
                        let amount = Self::calculate_posting_amount(transaction, account_name);
                        let entry = balances.entry(account_name.clone()).or_insert(0.0);
                        *entry += amount;
                    }
                }
            } else {
                // Can't parse date, include the transaction
                for posting in &transaction.postings {
                    let account_name = &posting.account;
                    let amount = Self::calculate_posting_amount(transaction, account_name);
                    let entry = balances.entry(account_name.clone()).or_insert(0.0);
                    *entry += amount;
                }
            }
        }

        balances
    }

    /// Parse amount string to f64, handling currency, signs, and commas
    /// Handles formats like "12,306.11 CNY", "-100.00 CNY", "100.00"
    fn parse_amount(amount_str: &str) -> f64 {
        if amount_str.is_empty() {
            return 0.0;
        }
        // Remove commas and extract the first number
        let cleaned: String = amount_str.chars().filter(|&c| c != ',').collect();
        let chars: Vec<char> = cleaned.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];
            if c.is_ascii_digit() || c == '.' {
                // Extract the number starting from here
                let mut num_str = String::new();
                let mut j = i;
                // Check if the previous character is a minus sign
                if j > 0 && chars[j-1] == '-' {
                    num_str.push('-');
                }
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
            }
            i += 1;
        }
        0.0
    }

    /// Calculate the posting amount for a specific account in a transaction
    /// Handles the case where posting amount is empty (inferred from other postings in the same transaction)
    fn calculate_posting_amount(tx: &Transaction, account_name: &str) -> f64 {
        // First, check if this posting has an explicit amount
        if let Some(posting) = tx.postings.iter().find(|p| p.account == account_name) {
            if !posting.amount.is_empty() {
                return Self::parse_amount(&posting.amount);
            }
        }

        // If no explicit amount, calculate from other postings (Beancount double-entry)
        let mut known_total: f64 = 0.0;
        let mut has_known_amount = false;

        for p in &tx.postings {
            if !p.amount.is_empty() {
                let amount = Self::parse_amount(&p.amount);
                if amount != 0.0 {
                    known_total += amount;
                    has_known_amount = true;
                }
            }
        }

        // For empty amount posting, the amount is the negative of known total
        if has_known_amount {
            -known_total
        } else {
            0.0
        }
    }

    // ==================== Transaction Management Methods ====================

    /// Get all transactions
    pub fn all_transactions(&self) -> Vec<Transaction> {
        self.data.read().unwrap().transactions.clone()
    }

    /// Get transaction by ID
    pub fn transaction(&self, id: &str) -> Option<Transaction> {
        let data = self.data.read().unwrap();
        data.transactions.iter().find(|t| &t.id == id).cloned()
    }

    /// Get transactions involving a specific account
    pub fn transactions_by_account(&self, account_name: &str) -> Vec<Transaction> {
        let data = self.data.read().unwrap();
        data.transactions
            .iter()
            .filter(|t| t.involves_account(account_name))
            .cloned()
            .collect()
    }

    /// Get balances for a specific account
    pub fn balances_by_account(&self, account_name: &str) -> Vec<BalanceEntry> {
        let data = self.data.read().unwrap();
        data.balances
            .iter()
            .filter(|b| b.account == account_name)
            .cloned()
            .collect()
    }

    /// Get all balances
    pub fn all_balances(&self) -> Vec<BalanceEntry> {
        self.data.read().unwrap().balances.clone()
    }

    /// Get pads for a specific account (pads where this account is the TARGET)
    pub fn pads_by_account(&self, account_name: &str) -> Vec<PadEntry> {
        let data = self.data.read().unwrap();
        data.pads
            .iter()
            .filter(|p| p.account == account_name)
            .cloned()
            .collect()
    }

    /// Get pads where this account is the SOURCE (for calculating source account balances)
    pub fn pads_by_source_account(&self, account_name: &str) -> Vec<PadEntry> {
        let data = self.data.read().unwrap();
        data.pads
            .iter()
            .filter(|p| p.source_account == account_name)
            .cloned()
            .collect()
    }

    /// Get all pads
    pub fn all_pads(&self) -> Vec<PadEntry> {
        self.data.read().unwrap().pads.clone()
    }

    /// Get transactions within a date range
    pub fn transactions_by_date_range(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Vec<Transaction> {
        let data = self.data.read().unwrap();
        data.transactions
            .iter()
            .filter(|t| {
                if let Some(date) = t.date_naive() {
                    date >= start_date && date <= end_date
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }

    /// Search transactions by payee, narration, tags, links, or account names
    pub fn search_transactions(&self, query: &str) -> Vec<Transaction> {
        let data = self.data.read().unwrap();
        let query_lower = query.to_lowercase();
        data.transactions
            .iter()
            .filter(|t| {
                t.payee.to_lowercase().contains(&query_lower)
                    || t.narration.to_lowercase().contains(&query_lower)
                    || t.tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower))
                    || t.links.iter().any(|link| link.to_lowercase().contains(&query_lower))
                    || t.postings.iter().any(|p| p.account.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect()
    }

    /// Get transactions with pagination and optional filtering
    pub fn transaction_query(
        &self,
        limit: usize,
        offset: usize,
        account_filter: Option<&str>,
        date_filter: Option<TimeContext>,
    ) -> Vec<Transaction> {
        let data = self.data.read().unwrap();
        let mut transactions = data.transactions.iter().cloned().collect::<Vec<_>>();

        // Apply account filter
        if let Some(account) = account_filter {
            transactions.retain(|t| t.involves_account(account));
        }

        // Apply date filter
        if let Some(context) = date_filter {
            transactions.retain(|t| {
                t.date_naive()
                    .map(|d| context.contains(&d))
                    .unwrap_or(true)
            });
        }

        // Sort by date descending
        transactions.sort_by(|a, b| b.date.cmp(&a.date));

        // Apply pagination
        transactions.into_iter().skip(offset).take(limit).collect()
    }

    /// Get transaction count
    pub fn transaction_count(&self) -> usize {
        self.data.read().unwrap().transactions.len()
    }

    /// Get transaction count for an account
    pub fn transaction_count_by_account(&self, account_name: &str) -> usize {
        let data = self.data.read().unwrap();
        data.transactions
            .iter()
            .filter(|t| t.involves_account(account_name))
            .count()
    }

    /// Get transaction statistics
    pub fn transaction_stats(&self) -> TransactionStats {
        let data = self.data.read().unwrap();
        let transactions = &data.transactions;

        let total_count = transactions.len();
        let total_postings: usize = transactions.iter().map(|t| t.posting_count()).sum();

        // Calculate date range - correctly find min and max dates
        let date_range = transactions.iter().filter_map(|t| t.date_naive()).fold(
            (None, None),
            |(min, max), date| {
                (Some(min.unwrap_or(date).min(date)), Some(max.unwrap_or(date).max(date)))
            },
        );

        TransactionStats {
            total_transactions: total_count,
            total_postings,
            date_range_start: date_range.0.map(|d| d.to_string()),
            date_range_end: date_range.1.map(|d| d.to_string()),
        }
    }

    /// Get recent transactions
    pub fn recent_transactions(&self, count: usize) -> Vec<Transaction> {
        let mut transactions = self.all_transactions();
        transactions.sort_by(|a, b| b.date.cmp(&a.date));
        transactions.into_iter().take(count).collect()
    }

    // ==================== Time Control Methods ====================

    /// Get current time context
    pub fn time_context(&self) -> TimeContext {
        self.time_context.read().unwrap().clone()
    }

    /// Set time range
    pub fn set_time_range(&self, range: TimeRange) {
        let mut ctx = self.time_context.write().unwrap();
        ctx.range = range;
        ctx.custom_start = None;
        ctx.custom_end = None;
    }

    /// Set custom date range
    pub fn set_custom_range(&self, start: NaiveDate, end: NaiveDate) {
        let mut ctx = self.time_context.write().unwrap();
        ctx.range = TimeRange::Custom;
        ctx.custom_start = Some(start);
        ctx.custom_end = Some(end);
    }

    /// Get filtered transactions by current time context
    pub fn filtered_transactions(&self, limit: usize, offset: usize) -> Vec<Transaction> {
        let data = self.data.read().unwrap();
        let context = self.time_context.read().unwrap().clone();

        data.transactions
            .iter()
            .filter(|t| t.filter_by_time(&context))
            .skip(offset)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get count of filtered transactions
    pub fn filtered_transaction_count(&self) -> usize {
        let data = self.data.read().unwrap();
        let context = self.time_context.read().unwrap().clone();

        data.transactions
            .iter()
            .filter(|t| t.filter_by_time(&context))
            .count()
    }

    /// Get time period summary
    pub fn time_period_summary(&self) -> TimePeriodSummary {
        let context = self.time_context.read().unwrap().clone();
        TimePeriodSummary {
            range_description: context.description(),
            start_date: context.start_date().map(|d: NaiveDate| d.to_string()),
            end_date: context.end_date().map(|d: NaiveDate| d.to_string()),
            transaction_count: self.filtered_transaction_count(),
        }
    }

    // ==================== Report Generation Methods ====================

    /// Generate balance report
    pub fn balance_report(&self) -> BalanceReport {
        let data = self.data.read().unwrap();
        let context = self.time_context.read().unwrap().clone();

        // Filter accounts that have balances
        let filtered_accounts: Vec<&Account> = data.accounts
            .iter()
            .filter(|a| a.status == AccountStatus::Open)
            .collect();

        // Calculate totals
        let total_assets: f64 = filtered_accounts
            .iter()
            .filter(|a| a.account_type == AccountType::Assets)
            .filter_map(|a| self.parse_balance(&a.balance))
            .sum();

        let total_liabilities: f64 = filtered_accounts
            .iter()
            .filter(|a| a.account_type == AccountType::Liabilities)
            .filter_map(|a| self.parse_balance(&a.balance))
            .sum();

        let total_equity: f64 = filtered_accounts
            .iter()
            .filter(|a| a.account_type == AccountType::Equity)
            .filter_map(|a| self.parse_balance(&a.balance))
            .sum();

        let net_worth = total_assets - total_liabilities;

        // Create entries
        let entries: Vec<BalanceReportEntry> = filtered_accounts
            .iter()
            .map(|a| {
                let balance = self.parse_balance(&a.balance).unwrap_or(0.0);
                let percentage = if total_assets > 0.0 {
                    (balance / total_assets) * 100.0
                } else {
                    0.0
                };
                BalanceReportEntry {
                    account: a.name.clone(),
                    account_type: a.account_type,
                    balance: balance.to_string(),
                    currency: a.currency.clone().unwrap_or_else(|| "USD".to_string()),
                    percentage,
                }
            })
            .collect();

        BalanceReport {
            entries,
            total_assets: total_assets.to_string(),
            total_liabilities: total_liabilities.to_string(),
            total_equity: total_equity.to_string(),
            net_worth: net_worth.to_string(),
            currency: "USD".to_string(),
            as_of_date: Utc::now().date_naive().to_string(),
        }
    }

    /// Generate income vs expenses report
    pub fn income_expense_report(&self) -> IncomeExpenseReport {
        let data = self.data.read().unwrap();
        let context = self.time_context.read().unwrap().clone();

        // Get filtered transactions
        let filtered_txs: Vec<&Transaction> = data.transactions
            .iter()
            .filter(|t| t.filter_by_time(&context))
            .collect();

        // Calculate income and expenses
        let mut income_by_account: HashMap<String, f64> = HashMap::new();
        let mut expense_by_account: HashMap<String, f64> = HashMap::new();

        for tx in &filtered_txs {
            for posting in &tx.postings {
                let amount = posting.amount_value().unwrap_or(0.0);
                if posting.account.starts_with("Income:") {
                    *income_by_account.entry(posting.account.clone()).or_insert(0.0) += amount.abs();
                } else if posting.account.starts_with("Expenses:") {
                    *expense_by_account.entry(posting.account.clone()).or_insert(0.0) += amount.abs();
                }
            }
        }

        let total_income: f64 = income_by_account.values().sum();
        let total_expenses: f64 = expense_by_account.values().sum();
        let net_income = total_income - total_expenses;

        // Create income entries
        let income_entries: Vec<IncomeExpenseEntry> = income_by_account
            .into_iter()
            .map(|(account, amount)| {
                let percentage = if total_income > 0.0 { (amount / total_income) * 100.0 } else { 0.0 };
                let category = account.split(':').nth(1).unwrap_or(&account).to_string();
                IncomeExpenseEntry { account, amount: amount.to_string(), percentage, category }
            })
            .collect();

        // Create expense entries
        let expense_entries: Vec<IncomeExpenseEntry> = expense_by_account
            .into_iter()
            .map(|(account, amount)| {
                let percentage = if total_expenses > 0.0 { (amount / total_expenses) * 100.0 } else { 0.0 };
                let category = account.split(':').nth(1).unwrap_or(&account).to_string();
                IncomeExpenseEntry { account, amount: amount.to_string(), percentage, category }
            })
            .collect();

        let start_date = context.start_date().map(|d| d.to_string()).unwrap_or_default();
        let end_date = context.end_date().map(|d| d.to_string()).unwrap_or_default();

        IncomeExpenseReport {
            income_entries,
            expense_entries,
            total_income: total_income.to_string(),
            total_expenses: total_expenses.to_string(),
            net_income: net_income.to_string(),
            currency: "USD".to_string(),
            period_start: start_date,
            period_end: end_date,
        }
    }

    /// Generate category report for expenses
    pub fn expense_category_report(&self) -> CategoryReport {
        let report = self.income_expense_report();
        let total: f64 = report.total_expenses.parse().unwrap_or(0.0);

        let breakdowns: Vec<CategoryBreakdown> = report.expense_entries
            .into_iter()
            .map(|entry| {
                let amount: f64 = entry.amount.parse().unwrap_or(0.0);
                let percentage = if total > 0.0 { (amount / total) * 100.0 } else { 0.0 };
                CategoryBreakdown {
                    category: entry.category,
                    amount: entry.amount,
                    count: 1,
                    percentage,
                }
            })
            .collect();

        CategoryReport {
            category_type: "expenses".to_string(),
            breakdowns,
            total: total.to_string(),
            currency: report.currency,
        }
    }

    /// Generate chart data for expenses by category
    pub fn expense_chart_data(&self) -> ChartData {
        let report = self.income_expense_report();
        let total: f64 = report.total_expenses.parse().unwrap_or(0.0);

        let colors = vec![
            "#FF6384", "#36A2EB", "#FFCE56", "#4BC0C0", "#9966FF",
            "#FF9F40", "#FF6384", "#C9CBCF", "#7BC225", "#E7E9ED",
        ];

        let data_points: Vec<ChartDataPoint> = report.expense_entries
            .iter()
            .take(10)
            .enumerate()
            .map(|(i, entry)| {
                let amount: f64 = entry.amount.parse().unwrap_or(0.0);
                let percentage = if total > 0.0 { (amount / total) * 100.0 } else { 0.0 };
                ChartDataPoint {
                    label: entry.category.clone(),
                    value: amount,
                    color: Some(colors[i % colors.len()].to_string()),
                }
            })
            .collect();

        let labels: Vec<String> = data_points.iter().map(|dp| dp.label.clone()).collect();
        let values: Vec<f64> = data_points.iter().map(|dp| dp.value).collect();

        let dataset = ChartDataset {
            label: "Expenses".to_string(),
            data: values,
            background_color: Some("#FF6384".to_string()),
            border_color: Some("#FF6384".to_string()),
        };

        ChartData {
            chart_type: "pie".to_string(),
            title: "Expenses by Category".to_string(),
            data_points,
            labels,
            datasets: vec![dataset],
            currency: report.currency,
        }
    }

    /// Generate net worth report
    pub fn net_worth_report(&self) -> NetWorthReport {
        let data = self.data.read().unwrap();
        let context = self.time_context.read().unwrap().clone();

        // Get all transactions sorted by date
        let mut transactions: Vec<&Transaction> = data.transactions
            .iter()
            .filter(|t| t.filter_by_time(&context))
            .collect();
        transactions.sort_by(|a, b| a.date.cmp(&b.date));

        // Calculate running net worth
        let mut running_assets = 0.0;
        let mut running_liabilities = 0.0;
        let mut points = Vec::new();

        for tx in &transactions {
            for posting in &tx.postings {
                let amount = posting.amount_value().unwrap_or(0.0);
                if posting.account.starts_with("Assets:") {
                    running_assets += amount;
                } else if posting.account.starts_with("Liabilities:") {
                    running_liabilities += amount;
                }
            }

            points.push(NetWorthPoint {
                date: tx.date.clone(),
                assets: running_assets.to_string(),
                liabilities: running_liabilities.to_string(),
                net_worth: (running_assets - running_liabilities).to_string(),
            });
        }

        let start_net_worth = points.first()
            .map(|p| p.net_worth.clone())
            .unwrap_or_else(|| "0".to_string());
        let end_net_worth = points.last()
            .map(|p| p.net_worth.clone())
            .unwrap_or_else(|| "0".to_string());

        let start_value: f64 = start_net_worth.parse().unwrap_or(0.0);
        let end_value: f64 = end_net_worth.parse().unwrap_or(0.0);
        let change = end_value - start_value;
        let change_percentage = if start_value != 0.0 {
            (change / start_value.abs()) * 100.0
        } else {
            0.0
        };

        NetWorthReport {
            points,
            start_net_worth,
            end_net_worth,
            change: change.to_string(),
            change_percentage,
            currency: "USD".to_string(),
        }
    }

    /// Generate net worth chart data
    pub fn net_worth_chart_data(&self) -> ChartData {
        let report = self.net_worth_report();

        let labels: Vec<String> = report.points
            .iter()
            .map(|p| p.date.clone())
            .collect();

        let assets_data: Vec<f64> = report.points
            .iter()
            .filter_map(|p| p.assets.parse().ok())
            .collect();

        let liabilities_data: Vec<f64> = report.points
            .iter()
            .filter_map(|p| p.liabilities.parse().ok())
            .collect();

        let net_worth_data: Vec<f64> = report.points
            .iter()
            .filter_map(|p| p.net_worth.parse().ok())
            .collect();

        ChartData {
            chart_type: "line".to_string(),
            title: "Net Worth Over Time".to_string(),
            data_points: Vec::new(),
            labels,
            datasets: vec![
                ChartDataset {
                    label: "Assets".to_string(),
                    data: assets_data,
                    background_color: Some("rgba(75, 192, 192, 0.2)".to_string()),
                    border_color: Some("rgba(75, 192, 192, 1)".to_string()),
                },
                ChartDataset {
                    label: "Liabilities".to_string(),
                    data: liabilities_data,
                    background_color: Some("rgba(255, 99, 132, 0.2)".to_string()),
                    border_color: Some("rgba(255, 99, 132, 1)".to_string()),
                },
                ChartDataset {
                    label: "Net Worth".to_string(),
                    data: net_worth_data,
                    background_color: Some("rgba(54, 162, 235, 0.2)".to_string()),
                    border_color: Some("rgba(54, 162, 235, 1)".to_string()),
                },
            ],
            currency: report.currency,
        }
    }

    /// Helper to parse balance from JSON value
    fn parse_balance(&self, balance: &serde_json::Value) -> Option<f64> {
        if balance.is_number() {
            balance.as_f64()
        } else if let Some(s) = balance.as_str() {
            // Handle strings like "-6307.77 CNY"
            Some(Self::parse_amount(s))
        } else if let Some(obj) = balance.as_object() {
            // Try to get the "amount" field specifically
            if let Some(amount_val) = obj.get("amount") {
                if let Some(s) = amount_val.as_str() {
                    Some(Self::parse_amount(s))
                } else if let Some(n) = amount_val.as_f64() {
                    Some(n)
                } else {
                    Some(0.0)
                }
            } else {
                // Fallback to first value
                obj.values().next().and_then(|v| v.as_f64())
            }
        } else {
            Some(0.0)
        }
    }

    /// Static helper to parse balance from JSON value (for use in static context)
    fn parse_balance_value(balance: &serde_json::Value) -> Option<f64> {
        Self::_parse_balance_static(balance)
    }

    // ==================== Static Helper Functions ====================

    /// Static version of parse_amount - call from parse_balance_static
    fn _parse_amount_static(amount_str: &str) -> f64 {
        if amount_str.is_empty() {
            return 0.0;
        }
        let cleaned: String = amount_str.chars().filter(|&c| c != ',').collect();
        let chars: Vec<char> = cleaned.chars().collect();
        let mut i = 0;
        let chars_len = chars.len();

        while i < chars_len {
            let c = chars[i];
            if c == '+' || c == '-' || c == '.' || c.is_ascii_digit() {
                let mut num_str = String::new();
                let mut has_digit = false;
                let mut has_dot = false;
                let mut j = i;
                while j < chars_len {
                    let nc = chars[j];
                    if nc == '+' || nc == '-' {
                        if !num_str.is_empty() {
                            break;
                        }
                        num_str.push(nc);
                        has_digit = true;
                    } else if nc == '.' {
                        if !has_dot {
                            num_str.push(nc);
                            has_dot = true;
                        } else {
                            break;
                        }
                    } else if nc.is_ascii_digit() {
                        num_str.push(nc);
                        has_digit = true;
                    } else {
                        break;
                    }
                    j += 1;
                }
                if !num_str.is_empty() {
                    return num_str.parse::<f64>().unwrap_or(0.0);
                }
            }
            i += 1;
        }
        0.0
    }

    /// Static helper to parse balance from JSON value
    fn _parse_balance_static(balance: &serde_json::Value) -> Option<f64> {
        if balance.is_number() {
            balance.as_f64()
        } else if let Some(s) = balance.as_str() {
            // Handle strings like "-6307.77 CNY"
            Some(Self::_parse_amount_static(s))
        } else if let Some(obj) = balance.as_object() {
            // Try to get the "amount" field specifically
            if let Some(amount_val) = obj.get("amount") {
                if let Some(s) = amount_val.as_str() {
                    Some(Self::_parse_amount_static(s))
                } else if let Some(n) = amount_val.as_f64() {
                    Some(n)
                } else {
                    Some(0.0)
                }
            } else {
                // Fallback to first value
                obj.values().next().and_then(|v| v.as_f64())
            }
        } else {
            Some(0.0)
        }
    }

    // ==================== Document Management Methods ====================

    /// Get document info for a file
    pub fn document_info(&self, path: &str) -> Option<DocumentInfo> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return None;
        }

        let metadata = std::fs::metadata(&path).ok()?;
        let modified = metadata.modified().ok()?;

        Some(DocumentInfo {
            path: path.to_string_lossy().to_string(),
            file_name: path.file_name()?.to_string_lossy().to_string(),
            size: metadata.len(),
            modified: DateTime::<Utc>::from(modified).to_rfc3339(),
            is_readonly: !path.exists() || metadata.permissions().readonly(),
        })
    }

    /// Read document content
    pub fn read_document(&self, path: &str) -> Result<String, CoreError> {
        std::fs::read_to_string(path)
            .map_err(|e| CoreError::IoError)
    }

    /// Write document content (with backup)
    pub fn write_document(&self, path: &str, content: &str) -> Result<(), CoreError> {
        // Create backup
        let backup_path = format!("{}.bak", path);
        if PathBuf::from(path).exists() {
            std::fs::copy(path, &backup_path)
                .map_err(|_| CoreError::IoError)?;
        }

        // Write new content
        std::fs::write(path, content)
            .map_err(|_| CoreError::IoError)?;

        Ok(())
    }

    /// Get all documents in the data directory (with glob support)
    pub fn list_documents(&self) -> Vec<DocumentInfo> {
        let data_path = &self.config.data.path;
        let mut documents = Vec::new();

        // Search for all .bean files
        if let Ok(entries) = std::fs::read_dir(data_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "bean" || ext == "beancount" || ext == "bc" {
                        if let Some(info) = self.document_info(&path.to_string_lossy()) {
                            documents.push(info);
                        }
                    }
                }
            }
        }

        // Also check subdirectories
        if let Ok(entries) = std::fs::read_dir(data_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    self.collect_documents_recursive(&path, &mut documents);
                }
            }
        }

        documents.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        documents
    }

    /// Recursively collect documents from subdirectories
    fn collect_documents_recursive(&self, dir: &PathBuf, documents: &mut Vec<DocumentInfo>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    self.collect_documents_recursive(&path, documents);
                } else if let Some(ext) = path.extension() {
                    if ext == "bean" || ext == "beancount" || ext == "bc" {
                        if let Some(info) = self.document_info(&path.to_string_lossy()) {
                            documents.push(info);
                        }
                    }
                }
            }
        }
    }

    /// Validate document syntax
    pub fn validate_document(&self, path: &str) -> DocumentValidation {
        let content = match self.read_document(path) {
            Ok(c) => c,
            Err(e) => {
                return DocumentValidation {
                    is_valid: false,
                    errors: vec![ValidationError {
                        line: 1,
                        column: 1,
                        message: format!("Failed to read file: {}", e),
                        severity: ErrorSeverity::Error,
                    }],
                    warnings: vec![],
                    transaction_count: 0,
                    account_count: 0,
                };
            }
        };

        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut transaction_count = 0;
        let mut account_count = 0;

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with(';') || line.is_empty() {
                continue;
            }

            // Check for transaction syntax
            if line.starts_with(|c: char| c.is_ascii_digit()) {
                if line.contains("txn") || self.has_postings(line) {
                    transaction_count += 1;
                }
            }

            // Check for account definition
            if line.starts_with("open ") || line.starts_with("Open ") {
                account_count += 1;
            }

            // Basic validation for posting lines
            if self.is_posting_line(line) {
                if !self.validate_posting_syntax(line) {
                    errors.push(ValidationError {
                        line: line_num as u32 + 1,
                        column: 1,
                        message: "Invalid posting syntax".to_string(),
                        severity: ErrorSeverity::Error,
                    });
                }
            }
        }

        // Check for unbalanced transactions (simplified)
        if let Some(unbalanced) = self.find_unbalanced_transactions(&content) {
            for line_num in unbalanced {
                warnings.push(ValidationError {
                    line: line_num,
                    column: 1,
                    message: "Possible unbalanced transaction".to_string(),
                    severity: ErrorSeverity::Warning,
                });
            }
        }

        DocumentValidation {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            transaction_count,
            account_count,
        }
    }

    /// Check if line contains postings
    fn has_postings(&self, line: &str) -> bool {
        line.contains(':') && (line.contains(' ') || line.is_empty())
    }

    /// Check if line is a posting
    fn is_posting_line(&self, line: &str) -> bool {
        line.starts_with(' ') || line.starts_with('\t')
    }

    /// Validate posting line syntax
    fn validate_posting_syntax(&self, line: &str) -> bool {
        // Posting should have account and optionally amount
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            return false;
        }

        // First part should be an account name
        let account = parts[0];
        if !account.contains(':') && !["Assets", "Liabilities", "Equity", "Income", "Expenses"].iter().any(|a| account.starts_with(a)) {
            return false;
        }

        true
    }

    /// Find potentially unbalanced transactions
    fn find_unbalanced_transactions(&self, content: &str) -> Option<Vec<u32>> {
        let mut unbalanced = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Check if this is a transaction header
            if self.is_transaction_header(line) {
                let tx_start = i;
                let mut posting_count = 0;

                // Count postings
                i += 1;
                while i < lines.len() {
                    let next_line = lines[i].trim();
                    if next_line.is_empty() || !next_line.starts_with(' ') {
                        break;
                    }
                    if self.is_posting_line(next_line) {
                        posting_count += 1;
                    }
                    i += 1;
                }

                // Check if transaction is balanced (simplified check)
                if posting_count == 1 {
                    unbalanced.push(tx_start as u32 + 1);
                }
            } else {
                i += 1;
            }
        }

        if unbalanced.is_empty() {
            None
        } else {
            Some(unbalanced)
        }
    }

    /// Check if line is a transaction header
    fn is_transaction_header(&self, line: &str) -> bool {
        let trimmed = line.trim();
        // Date format: YYYY-MM-DD
        if trimmed.len() >= 10 && trimmed.chars().next().unwrap().is_ascii_digit() {
            if let Ok(_) = NaiveDate::parse_from_str(&trimmed[..10], "%Y-%m-%d") {
                return true;
            }
        }
        false
    }

    /// Search in documents
    pub fn search_documents(&self, query: &str) -> Vec<SearchResult> {
        let documents = self.list_documents();
        let query = query.to_lowercase();
        let mut results = Vec::new();

        for doc in documents {
            if let Ok(content) = self.read_document(&doc.path) {
                let lines: Vec<&str> = content.lines().collect();
                for (line_num, line) in lines.iter().enumerate() {
                    if line.to_lowercase().contains(&query) {
                        // Get context (surrounding lines)
                        let start = if line_num > 2 { line_num - 2 } else { 0 };
                        let end = std::cmp::min(line_num + 3, lines.len());
                        let context = lines[start..end].join("\n");

                        results.push(SearchResult {
                            path: doc.path.clone(),
                            file_name: doc.file_name.clone(),
                            line_number: line_num as u32 + 1,
                            line_content: line.to_string(),
                            context: context,
                        });
                    }
                }
            }
        }

        results
    }

    /// Get document tree for navigation
    pub fn document_tree(&self) -> Vec<DocumentNode> {
        let data_path = &self.config.data.path;
        self.build_document_tree(data_path)
    }

    /// Build document tree recursively
    fn build_document_tree(&self, dir: &PathBuf) -> Vec<DocumentNode> {
        let mut nodes = Vec::new();

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let children = self.build_document_tree(&path);
                    if !children.is_empty() {
                        nodes.push(DocumentNode {
                            name: path.file_name().unwrap().to_string_lossy().to_string(),
                            path: path.to_string_lossy().to_string(),
                            is_directory: true,
                            children: Some(children),
                            is_file: false,
                        });
                    }
                } else if let Some(ext) = path.extension() {
                    if ext == "bean" || ext == "beancount" || ext == "bc" {
                        if let Some(info) = self.document_info(&path.to_string_lossy()) {
                            nodes.push(DocumentNode {
                                name: info.file_name,
                                path: info.path,
                                is_directory: false,
                                children: None,
                                is_file: true,
                            });
                        }
                    }
                }
            }
        }

        nodes
    }

    // ==================== Settings Management Methods ====================

    /// Get all settings as a structured response
    pub fn get_all_settings(&self) -> FullSettingsResponse {
        FullSettingsResponse {
            server: serde_json::to_value(&self.config.server).unwrap_or_default(),
            data: serde_json::to_value(&self.config.data).unwrap_or_default(),
            features: serde_json::to_value(&self.config.features).unwrap_or_default(),
            journal: serde_json::to_value(&self.config.journal).unwrap_or_default(),
            time_range: serde_json::to_value(&self.config.time_range).unwrap_or_default(),
            charts: serde_json::to_value(&self.config.charts).unwrap_or_default(),
            currency: serde_json::to_value(&self.config.currency).unwrap_or_default(),
            pagination: serde_json::to_value(&self.config.pagination).unwrap_or_default(),
        }
    }

    /// Get settings for a specific category
    pub fn get_settings(&self, category: SettingsCategory) -> SettingsResponse {
        let settings = match category {
            SettingsCategory::Server => serde_json::to_value(&self.config.server),
            SettingsCategory::Data => serde_json::to_value(&self.config.data),
            SettingsCategory::Features => serde_json::to_value(&self.config.features),
            SettingsCategory::Journal => serde_json::to_value(&self.config.journal),
            SettingsCategory::TimeRange => serde_json::to_value(&self.config.time_range),
            SettingsCategory::Charts => serde_json::to_value(&self.config.charts),
            SettingsCategory::Currency => serde_json::to_value(&self.config.currency),
            SettingsCategory::Pagination => serde_json::to_value(&self.config.pagination),
        };

        SettingsResponse {
            category,
            settings: settings.unwrap_or_default(),
        }
    }

    /// Validate a settings change before applying
    pub fn validate_setting(&self, category: SettingsCategory, key: &str, value: &serde_json::Value) -> SettingsValidation {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Validate based on category and key
        match category {
            SettingsCategory::Server => {
                if key == "port" {
                    if let Some(port) = value.as_u64().or(value.as_i64().map(|i| i as u64)) {
                        if port == 0 || port > 65535 {
                            errors.push(SettingsValidationError {
                                field: format!("{}.{}", "server", key),
                                message: "Port must be between 1 and 65535".to_string(),
                                severity: ErrorSeverity::Error,
                            });
                        }
                    } else {
                        errors.push(SettingsValidationError {
                            field: format!("{}.{}", "server", key),
                            message: "Port must be a number".to_string(),
                            severity: ErrorSeverity::Error,
                        });
                    }
                }
                if key == "host" {
                    if let Some(host) = value.as_str() {
                        if host.is_empty() {
                            errors.push(SettingsValidationError {
                                field: format!("{}.{}", "server", key),
                                message: "Host cannot be empty".to_string(),
                                severity: ErrorSeverity::Error,
                            });
                        }
                    }
                }
            }
            SettingsCategory::Data => {
                if key == "watch_enable" {
                    if !value.is_boolean() {
                        errors.push(SettingsValidationError {
                            field: format!("{}.{}", "data", key),
                            message: "watch_enable must be a boolean".to_string(),
                            severity: ErrorSeverity::Error,
                        });
                    }
                }
            }
            SettingsCategory::Features => {
                let valid_keys = ["budget_enable", "document_enable", "time_extraction", "balance_check_enable", "plugin_enable", "sql_enable"];
                if !valid_keys.contains(&key) {
                    errors.push(SettingsValidationError {
                        field: format!("{}.{}", "features", key),
                        message: format!("Invalid feature key: {}", key),
                        severity: ErrorSeverity::Error,
                    });
                } else if !value.is_boolean() {
                    errors.push(SettingsValidationError {
                        field: format!("{}.{}", "features", key),
                        message: format!("{} must be a boolean", key),
                        severity: ErrorSeverity::Error,
                    });
                }
            }
            SettingsCategory::TimeRange => {
                if key == "fiscal_year_start" {
                    if let Some(month) = value.as_u64().or(value.as_i64().map(|i| i as u64)) {
                        if month == 0 || month > 12 {
                            errors.push(SettingsValidationError {
                                field: format!("{}.{}", "time_range", key),
                                message: "Fiscal year start must be between 1 and 12".to_string(),
                                severity: ErrorSeverity::Error,
                            });
                        }
                    }
                }
            }
            SettingsCategory::Currency => {
                if key == "decimal_places" {
                    if let Some(decimals) = value.as_u64().or(value.as_i64().map(|i| i as u64)) {
                        if decimals > 10 {
                            warnings.push("Decimal places greater than 10 may cause precision issues".to_string());
                        }
                    }
                }
                if key == "symbol_position" {
                    if let Some(pos) = value.as_str() {
                        if pos != "before" && pos != "after" {
                            errors.push(SettingsValidationError {
                                field: format!("{}.{}", "currency", key),
                                message: "symbol_position must be 'before' or 'after'".to_string(),
                                severity: ErrorSeverity::Error,
                            });
                        }
                    }
                }
            }
            SettingsCategory::Charts => {
                if key == "default_chart_type" {
                    if let Some(chart_type) = value.as_str() {
                        let valid_types = ["bar", "line", "pie", "area", "stacked_bar"];
                        if !valid_types.contains(&chart_type) {
                            errors.push(SettingsValidationError {
                                field: format!("{}.{}", "charts", key),
                                message: format!("Invalid chart type: {}. Valid types: {:?}", chart_type, valid_types),
                                severity: ErrorSeverity::Error,
                            });
                        }
                    }
                }
                if key == "top_items_count" {
                    if let Some(count) = value.as_u64().or(value.as_i64().map(|i| i as u64)) {
                        if count == 0 || count > 100 {
                            warnings.push("top_items_count outside range 1-100 may affect performance".to_string());
                        }
                    }
                }
            }
            SettingsCategory::Journal => {
                if key == "edit_mode" {
                    if let Some(mode) = value.as_str() {
                        if mode != "form" && mode != "text" {
                            errors.push(SettingsValidationError {
                                field: format!("{}.{}", "journal", key),
                                message: "edit_mode must be 'form' or 'text'".to_string(),
                                severity: ErrorSeverity::Error,
                            });
                        }
                    }
                }
            }
            SettingsCategory::Pagination => {
                if key == "records_per_page" {
                    if let Some(count) = value.as_u64().or(value.as_i64().map(|i| i as u64)) {
                        if count == 0 || count > 500 {
                            warnings.push("records_per_page outside range 1-500 may affect performance".to_string());
                        }
                    }
                }
            }
        }

        SettingsValidation {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// Apply a settings change
    pub fn update_setting(&self, category: SettingsCategory, key: &str, value: serde_json::Value) -> SettingsChangeResponse {
        // First validate
        let validation = self.validate_setting(category, key, &value);
        if !validation.is_valid {
            let error_msg = validation.errors.iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join("; ");
            return SettingsChangeResponse {
                success: false,
                category,
                key: key.to_string(),
                error: Some(error_msg),
            };
        }

        // Note: In a real implementation, this would modify the config
        // For now, we just return success as the config is immutable during runtime
        SettingsChangeResponse {
            success: true,
            category,
            key: key.to_string(),
            error: None,
        }
    }

    /// Get settings metadata for UI
    pub fn get_settings_metadata(&self) -> serde_json::Value {
        serde_json::json!({
            "server": {
                "host": { "type": "string", "default": "0.0.0.0", "description": "Server bind address" },
                "port": { "type": "number", "default": 8081, "min": 1, "max": 65535, "description": "Server port" }
            },
            "data": {
                "path": { "type": "string", "default": "./data", "description": "Path to ledger directory" },
                "main_file": { "type": "string", "default": "main.bean", "description": "Main ledger file name" },
                "watch_enable": { "type": "boolean", "default": true, "description": "Enable file watching" }
            },
            "features": {
                "budget_enable": { "type": "boolean", "default": true, "description": "Enable budget management" },
                "document_enable": { "type": "boolean", "default": false, "description": "Enable document management" },
                "time_extraction": { "type": "boolean", "default": true, "description": "Extract time from metadata" },
                "balance_check_enable": { "type": "boolean", "default": true, "description": "Enable balance checks" },
                "plugin_enable": { "type": "boolean", "default": false, "description": "Enable plugin system" },
                "sql_enable": { "type": "boolean", "default": true, "description": "Enable SQL interface" }
            },
            "journal": {
                "expand_detail": { "type": "boolean", "default": true, "description": "Expand transaction details" },
                "edit_mode": { "type": "string", "enum": ["form", "text"], "default": "form", "description": "Default edit mode" },
                "full_account_names": { "type": "boolean", "default": true, "description": "Show full account names" }
            },
            "time_range": {
                "default_range": { "type": "string", "enum": ["month", "quarter", "year", "all", "custom"], "default": "month", "description": "Default time range" },
                "fiscal_year_start": { "type": "number", "min": 1, "max": 12, "default": 1, "description": "Fiscal year start month" }
            },
            "charts": {
                "default_chart_type": { "type": "string", "enum": ["bar", "line", "pie", "area", "stacked_bar"], "default": "bar", "description": "Default chart type" },
                "top_items_count": { "type": "number", "min": 1, "max": 100, "default": 10, "description": "Number of top items to show" },
                "show_legend": { "type": "boolean", "default": true, "description": "Show chart legends" },
                "interactive": { "type": "boolean", "default": true, "description": "Enable interactive charts" }
            },
            "currency": {
                "default_currency": { "type": "string", "default": "CNY", "description": "Default currency" },
                "decimal_places": { "type": "number", "min": 0, "max": 10, "default": 2, "description": "Decimal places" },
                "thousands_separator": { "type": "string", "default": ",", "description": "Thousands separator" },
                "decimal_separator": { "type": "string", "default": ".", "description": "Decimal separator" },
                "symbol_position": { "type": "string", "enum": ["before", "after"], "default": "before", "description": "Currency symbol position" }
            },
            "pagination": {
                "records_per_page": { "type": "number", "min": 1, "max": 500, "default": 50, "description": "Records per page" }
            }
        })
    }
}

/// Time period summary for API responses
#[derive(Debug, Serialize, Deserialize)]
pub struct TimePeriodSummary {
    pub range_description: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub transaction_count: usize,
}

/// Account tree node for hierarchical display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTreeNode {
    pub account: Account,
    pub children: Vec<AccountTreeNode>,
}

/// Account balance summary for reports
#[derive(Debug, Serialize, Deserialize)]
pub struct AccountBalanceSummary {
    pub total_assets: usize,
    pub total_liabilities: usize,
    pub total_income: usize,
    pub total_expenses: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub asset_accounts: Vec<Account>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub liability_accounts: Vec<Account>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub income_accounts: Vec<Account>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub expense_accounts: Vec<Account>,
}

/// Accounts list response for API
#[derive(Debug, Serialize, Deserialize)]
pub struct AccountsResponse {
    pub accounts: Vec<Account>,
    pub total_count: usize,
    pub by_type: serde_json::Value,
}

/// Transaction statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionStats {
    pub total_transactions: usize,
    pub total_postings: usize,
    pub date_range_start: Option<String>,
    pub date_range_end: Option<String>,
}

/// Transactions list response for API
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionsResponse {
    pub transactions: Vec<Transaction>,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
}

/// Transaction detail response
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionDetailResponse {
    pub transaction: Transaction,
    pub related_transactions: Vec<Transaction>,
}

/// Journal entry for account view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub transaction: Transaction,
    pub posting_index: usize,
    pub account: String,
    pub amount: String,
    pub balance: Option<String>,
}

/// Journal response for account history
#[derive(Debug, Serialize, Deserialize)]
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
    pub category: String,
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
    pub change_percentage: f64,
    pub currency: String,
}

/// Monthly summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlySummary {
    pub month: String,
    pub income: String,
    pub expenses: String,
    pub net: String,
}

/// Monthly summary report
#[derive(Debug, Serialize, Deserialize)]
pub struct MonthlySummaryReport {
    pub summaries: Vec<MonthlySummary>,
    pub total_income: String,
    pub total_expenses: String,
    pub total_net: String,
    pub currency: String,
}

/// Category breakdown for charts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryBreakdown {
    pub category: String,
    pub amount: String,
    pub count: usize,
    pub percentage: f64,
}

/// Category report
#[derive(Debug, Serialize, Deserialize)]
pub struct CategoryReport {
    pub category_type: String,
    pub breakdowns: Vec<CategoryBreakdown>,
    pub total: String,
    pub currency: String,
}

/// Chart data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataPoint {
    pub label: String,
    pub value: f64,
    pub color: Option<String>,
}

/// Chart data for visualization
#[derive(Debug, Serialize, Deserialize)]
pub struct ChartData {
    pub chart_type: String,
    pub title: String,
    pub data_points: Vec<ChartDataPoint>,
    pub labels: Vec<String>,
    pub datasets: Vec<ChartDataset>,
    pub currency: String,
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
    pub time_range: TimeRange,
}

// ==================== Document Management Structures ====================

/// Document information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentInfo {
    pub path: String,
    pub file_name: String,
    pub size: u64,
    pub modified: String,
    pub is_readonly: bool,
}

/// Document node for tree navigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentNode {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<DocumentNode>>,
    pub is_file: bool,
}

/// Document validation result
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentValidation {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
    pub transaction_count: usize,
    pub account_count: usize,
}

/// Validation error/warning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub severity: ErrorSeverity,
}

/// Document search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub file_name: String,
    pub line_number: u32,
    pub line_content: String,
    pub context: String,
}

/// Document list response
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentListResponse {
    pub documents: Vec<DocumentInfo>,
    pub total_count: usize,
    pub tree: Vec<DocumentNode>,
}

/// File edit request
#[derive(Debug, Serialize, Deserialize)]
pub struct FileEditRequest {
    pub path: String,
    pub content: String,
    pub create_backup: bool,
}

/// File edit response
#[derive(Debug, Serialize, Deserialize)]
pub struct FileEditResponse {
    pub success: bool,
    pub path: String,
    pub backup_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ==================== Settings Management Structures ====================

/// Settings category enumeration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingsCategory {
    /// Server settings
    Server,
    /// Data settings
    Data,
    /// Feature toggles
    Features,
    /// Journal display settings
    Journal,
    /// Time range settings
    TimeRange,
    /// Chart settings
    Charts,
    /// Currency settings
    Currency,
    /// Pagination settings
    Pagination,
}

/// Settings change request
#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsUpdateRequest {
    pub category: SettingsCategory,
    pub key: String,
    pub value: serde_json::Value,
}

/// Settings validation result
#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsValidation {
    pub is_valid: bool,
    pub errors: Vec<SettingsValidationError>,
    pub warnings: Vec<String>,
}

/// Settings validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsValidationError {
    pub field: String,
    pub message: String,
    pub severity: ErrorSeverity,
}

/// Settings response
#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub category: SettingsCategory,
    pub settings: serde_json::Value,
}

/// Full settings response
#[derive(Debug, Serialize, Deserialize)]
pub struct FullSettingsResponse {
    pub server: serde_json::Value,
    pub data: serde_json::Value,
    pub features: serde_json::Value,
    pub journal: serde_json::Value,
    pub time_range: serde_json::Value,
    pub charts: serde_json::Value,
    pub currency: serde_json::Value,
    pub pagination: serde_json::Value,
}

/// Settings change response
#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsChangeResponse {
    pub success: bool,
    pub category: SettingsCategory,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Trait for ledger operations
pub trait LedgerOperations {
    /// Get ledger summary
    fn summary(&self) -> LedgerSummary;

    /// Get accounts by type
    fn accounts_by_type(&self, account_type: AccountType) -> Vec<Account>;
}

/// Ledger summary
#[derive(Debug, Serialize, Deserialize)]
pub struct LedgerSummary {
    pub total_accounts: usize,
    pub total_transactions: usize,
    pub total_commodities: usize,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
}

impl LedgerOperations for Ledger {
    fn summary(&self) -> LedgerSummary {
        let data = self.data.read().unwrap();
        LedgerSummary {
            total_accounts: data.accounts.len(),
            total_transactions: data.transactions.len(),
            total_commodities: data.commodities.len(),
            period_start: None,
            period_end: None,
        }
    }

    fn accounts_by_type(&self, account_type: AccountType) -> Vec<Account> {
        let data = self.data.read().unwrap();
        data.accounts
            .iter()
            .filter(|a| a.account_type == account_type)
            .cloned()
            .collect()
    }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_context_month() {
        let ctx = TimeContext::new(TimeRange::Month);
        assert_eq!(ctx.range, TimeRange::Month);
        assert!(ctx.start_date().is_some());
        assert!(ctx.end_date().is_some());
        assert!(ctx.start_date().unwrap() <= ctx.end_date().unwrap());
    }

    #[test]
    fn test_time_context_quarter() {
        let ctx = TimeContext::new(TimeRange::Quarter);
        assert_eq!(ctx.range, TimeRange::Quarter);
        assert!(ctx.start_date().is_some());
        assert!(ctx.end_date().is_some());
    }

    #[test]
    fn test_time_context_year() {
        let ctx = TimeContext::new(TimeRange::Year);
        assert_eq!(ctx.range, TimeRange::Year);
        assert!(ctx.start_date().is_some());
        assert_eq!(ctx.end_date().unwrap().month(), 12);
        assert_eq!(ctx.end_date().unwrap().day(), 31);
    }

    #[test]
    fn test_time_context_all() {
        let ctx = TimeContext::new(TimeRange::All);
        assert_eq!(ctx.range, TimeRange::All);
        assert!(ctx.start_date().is_none());
        assert!(ctx.end_date().is_none());
    }

    #[test]
    fn test_time_context_custom() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let ctx = TimeContext::custom(start, end);

        assert_eq!(ctx.range, TimeRange::Custom);
        assert_eq!(ctx.start_date(), Some(start));
        assert_eq!(ctx.end_date(), Some(end));
    }

    #[test]
    fn test_time_context_contains() {
        let ctx = TimeContext::custom(
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );

        assert!(ctx.contains(&NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()));
        assert!(!ctx.contains(&NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()));
        assert!(!ctx.contains(&NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()));
    }

    #[test]
    fn test_time_context_description() {
        let ctx_month = TimeContext::new(TimeRange::Month);
        assert_eq!(ctx_month.description(), "Current Month");

        let ctx_quarter = TimeContext::new(TimeRange::Quarter);
        assert_eq!(ctx_quarter.description(), "Current Quarter");

        let ctx_year = TimeContext::new(TimeRange::Year);
        assert_eq!(ctx_year.description(), "Current Year");

        let ctx_all = TimeContext::new(TimeRange::All);
        assert_eq!(ctx_all.description(), "All Time");

        let ctx_custom = TimeContext::custom(
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        assert!(ctx_custom.description().contains("2024-01-01"));
        assert!(ctx_custom.description().contains("2024-12-31"));
    }

    #[test]
    fn test_transaction_time_filter() {
        let tx = Transaction {
            id: "test".to_string(),
            date: "2024-06-15".to_string(),
            time: "".to_string(),
            payee: "Test Payee".to_string(),
            narration: "Test narration".to_string(),
            postings: vec![],
            flag: None,
            tags: vec![],
            links: vec![],
            metadata: serde_json::json!({}),
            source: None,
            line: None,
        };

        let ctx = TimeContext::custom(
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );

        assert!(tx.filter_by_time(&ctx));
    }

    #[test]
    fn test_transaction_methods() {
        let tx = Transaction {
            id: "test-123".to_string(),
            date: "2024-06-15".to_string(),
            time: "10:30:00".to_string(),
            payee: "Coffee Shop".to_string(),
            narration: "Morning coffee".to_string(),
            postings: vec![
                Posting {
                    account: "Expenses:Coffee".to_string(),
                    amount: "-5.00".to_string(),
                    currency: "USD".to_string(),
                    cost: None,
                    price: None,
                    balance: None,
                    metadata: serde_json::json!({}),
                },
                Posting {
                    account: "Assets:Cash".to_string(),
                    amount: "5.00".to_string(),
                    currency: "USD".to_string(),
                    cost: None,
                    price: None,
                    balance: None,
                    metadata: serde_json::json!({}),
                },
            ],
            flag: Some("*".to_string()),
            tags: vec!["coffee".to_string()],
            links: vec![],
            metadata: serde_json::json!({}),
            source: Some("main.bean".to_string()),
            line: Some(100),
        };

        assert!(tx.date_naive().is_some());
        assert!(tx.involves_account("Expenses:Coffee"));
        assert!(!tx.involves_account("Income:Salary"));
        assert_eq!(tx.accounts().len(), 2);
        assert!(tx.is_balanced());
        assert_eq!(tx.posting_count(), 2);
        assert!(tx.summary().contains("2024-06-15"));
    }

    #[test]
    fn test_posting_methods() {
        let posting = Posting {
            account: "Expenses:Food".to_string(),
            amount: "-25.50".to_string(),
            currency: "USD".to_string(),
            cost: None,
            price: None,
            balance: None,
            metadata: serde_json::json!({}),
        };

        assert_eq!(posting.amount_value(), Some(-25.50));
        assert!(posting.is_credit());
        assert!(!posting.is_debit());
    }

    #[test]
    fn test_account_time_filter() {
        let account = Account {
            name: "Assets:Checking".to_string(),
            account_type: AccountType::Assets,
            status: AccountStatus::Open,
            balance: serde_json::json!({}),
            currency: Some("USD".to_string()),
            open_date: Some("2024-01-01".to_string()),
            close_date: None,
            alias: None,
            note: None,
            tags: vec![],
        };

        let ctx = TimeContext::new(TimeRange::Month);
        assert!(account.filter_by_time(&ctx));
    }

    #[test]
    fn test_account_short_name() {
        let account = Account {
            name: "Assets:Checking:Chase".to_string(),
            account_type: AccountType::Assets,
            status: AccountStatus::Open,
            balance: serde_json::json!({}),
            currency: None,
            open_date: None,
            close_date: None,
            alias: None,
            note: None,
            tags: vec![],
        };

        assert_eq!(account.short_name(), "Checking:Chase");
        assert!(!account.is_root());
        assert!(!account.is_leaf());
        assert_eq!(account.depth(), 2);
    }

    #[test]
    fn test_account_root_detection() {
        let root_account = Account {
            name: "Assets".to_string(),
            account_type: AccountType::Assets,
            status: AccountStatus::Open,
            balance: serde_json::json!({}),
            currency: None,
            open_date: None,
            close_date: None,
            alias: None,
            note: None,
            tags: vec![],
        };

        assert!(root_account.is_root());
        assert!(root_account.is_leaf());
        assert_eq!(root_account.depth(), 0);
    }

    #[test]
    fn test_account_parent_name() {
        let account = Account {
            name: "Assets:Checking:Chase".to_string(),
            account_type: AccountType::Assets,
            status: AccountStatus::Open,
            balance: serde_json::json!({}),
            currency: None,
            open_date: None,
            close_date: None,
            alias: None,
            note: None,
            tags: vec![],
        };

        assert_eq!(account.parent_name(), Some("Assets:Checking".to_string()));
    }

    #[test]
    fn test_account_type_from_str() {
        assert_eq!("assets".parse::<AccountType>().unwrap(), AccountType::Assets);
        assert_eq!("liabilities".parse::<AccountType>().unwrap(), AccountType::Liabilities);
        assert_eq!("equity".parse::<AccountType>().unwrap(), AccountType::Equity);
        assert_eq!("income".parse::<AccountType>().unwrap(), AccountType::Income);
        assert_eq!("expenses".parse::<AccountType>().unwrap(), AccountType::Expenses);
    }

    #[test]
    fn test_account_status_from_str() {
        assert_eq!("open".parse::<AccountStatus>().unwrap(), AccountStatus::Open);
        assert_eq!("closed".parse::<AccountStatus>().unwrap(), AccountStatus::Closed);
        assert_eq!("paused".parse::<AccountStatus>().unwrap(), AccountStatus::Paused);
    }
}
