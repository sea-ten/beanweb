//! Common types for Beancount parser

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Span information for error reporting
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanInfo {
    pub start: usize,
    pub end: usize,
}

/// Account structure with type and components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub account_type: AccountType,
    pub name: String,
    pub components: Vec<String>,
}

/// Amount with currency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Amount {
    pub amount: rust_decimal::Decimal,
    pub currency: String,
}

/// Cost specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cost {
    pub amount: rust_decimal::Decimal,
    pub currency: String,
    pub date: Option<chrono::NaiveDate>,
}

/// Price specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Price {
    Single(Amount),
    Total(Amount),
}

/// Date type (date or datetime)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Date {
    Date(chrono::NaiveDate),
    DateTime(chrono::NaiveDateTime),
}

/// String value (quoted or unquoted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StringValue {
    Quote(String),
    Unquote(String),
}

impl StringValue {
    pub fn as_str(&self) -> &str {
        match self {
            StringValue::Quote(s) => s.as_str(),
            StringValue::Unquote(s) => s.as_str(),
        }
    }
}

/// Account type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AccountType {
    Assets,
    Liabilities,
    Equity,
    Income,
    Expenses,
}

impl AccountType {
    pub fn as_str(&self) -> &str {
        match self {
            AccountType::Assets => "Assets",
            AccountType::Liabilities => "Liabilities",
            AccountType::Equity => "Equity",
            AccountType::Income => "Income",
            AccountType::Expenses => "Expenses",
        }
    }
}

impl std::str::FromStr for AccountType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Assets" => Ok(AccountType::Assets),
            "Liabilities" => Ok(AccountType::Liabilities),
            "Equity" => Ok(AccountType::Equity),
            "Income" => Ok(AccountType::Income),
            "Expenses" => Ok(AccountType::Expenses),
            _ => Err(()),
        }
    }
}

/// Metadata key-value store
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta(HashMap<String, StringValue>);

impl Meta {
    pub fn get(&self, key: &str) -> Option<&StringValue> {
        self.0.get(key)
    }

    pub fn insert(&mut self, key: String, value: StringValue) {
        self.0.insert(key, value);
    }

    pub fn remove(&mut self, key: &str) -> Option<StringValue> {
        self.0.remove(key)
    }

    /// Get inner HashMap reference for iteration
    pub fn inner(&self) -> &HashMap<String, StringValue> {
        &self.0
    }
}

impl From<Vec<(String, StringValue)>> for Meta {
    fn from(v: Vec<(String, StringValue)>) -> Self {
        Meta(v.into_iter().collect())
    }
}
