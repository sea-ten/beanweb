//! Basic types for the core ledger module

use serde::{Deserialize, Serialize};

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
