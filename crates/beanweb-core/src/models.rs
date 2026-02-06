//! Core data models for the ledger

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::types::{AccountStatus, AccountType};

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
    /// Amount as string (e.g., "100.00 CNY")
    pub amount: String,
    /// Currency
    pub currency: String,
    /// Cost basis (if specified)
    pub cost: Option<String>,
    /// Price (for conversion)
    pub price: Option<String>,
    /// Balance assertion (from balance directive)
    pub balance: Option<String>,
}

impl Posting {
    /// Get the numeric amount
    pub fn amount_num(&self) -> Option<f64> {
        self.amount.split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
    }

    /// Check if this is a negative posting (expense/liability)
    pub fn is_negative(&self) -> bool {
        self.amount_num().map_or(false, |n| n < 0.0)
    }
}

/// Commodity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commodity {
    /// Commodity name (e.g., "USD", "AAPL")
    pub name: String,
    /// Date commodity was defined
    pub date: String,
}

/// Balance entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceEntry {
    /// Account name
    pub account: String,
    /// Balance amount
    pub amount: String,
    /// Currency
    pub currency: String,
    /// Date of balance assertion
    pub date: String,
}
