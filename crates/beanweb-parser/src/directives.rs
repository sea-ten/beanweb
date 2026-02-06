//! Beancount directive types

use serde::{Deserialize, Serialize};
use crate::types::{Account, Amount, Cost, Date, Meta, Price, StringValue, SpanInfo};

/// Spanned directive with position info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpannedDirective {
    pub data: Directive,
    pub span: SpanInfo,
    /// Source file path (relative to data directory)
    pub source: Option<String>,
}

/// Main directive enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Directive {
    Transaction(Transaction),
    Open(OpenDirective),
    Close(CloseDirective),
    Balance(BalanceDirective),
    Pad(PadDirective),
    Commodity(CommodityDirective),
    Document(DocumentDirective),
    Price(PriceDirective),
    Event(EventDirective),
    Note(NoteDirective),
    Option(OptionDirective),
    Include(IncludeDirective),
    Custom(CustomDirective),
    Comment(CommentDirective),
}

/// Transaction directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub date: Date,
    pub flag: Option<String>,
    pub payee: Option<String>,
    pub narration: Option<String>,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub postings: Vec<Posting>,
    pub meta: Meta,
}

/// Open directive (account declaration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenDirective {
    pub date: Date,
    pub account: Account,
    pub currencies: Vec<String>,
    pub meta: Meta,
}

/// Close directive (account closure)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseDirective {
    pub date: Date,
    pub account: Account,
}

/// Balance directive (balance assertion)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceDirective {
    pub date: Date,
    pub account: Account,
    pub amount: Amount,
}

/// Pad directive (balance padding)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadDirective {
    pub date: Date,
    pub account: Account,
    pub pad: Account,
}

/// Commodity directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommodityDirective {
    pub date: Date,
    pub name: String,
}

/// Document directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDirective {
    pub date: Date,
    pub account: Account,
    pub filename: String,
}

/// Price directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceDirective {
    pub date: Date,
    pub commodity: String,
    pub amount: Amount,
}

/// Event directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDirective {
    pub date: Date,
    pub event_type: String,
    pub description: String,
}

/// Note directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDirective {
    pub date: Date,
    pub account: Account,
    pub comment: String,
}

/// Option directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionDirective {
    pub key: String,
    pub value: String,
}

/// Include directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncludeDirective {
    pub path: String,
}

/// Custom directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomDirective {
    pub date: Date,
    pub custom_type: String,
    pub values: Vec<String>,
}

/// Comment directive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentDirective {
    pub content: String,
}

/// Posting within a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Posting {
    pub flag: Option<String>,
    pub account: Account,
    pub amount: Option<Amount>,
    pub cost: Option<Cost>,
    pub price: Option<Price>,
}
