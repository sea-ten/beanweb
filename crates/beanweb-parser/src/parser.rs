//! Beancount parser implementation

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

use crate::directives::{
    BalanceDirective, CloseDirective, CommentDirective, CommodityDirective,
    CustomDirective, DocumentDirective, EventDirective, IncludeDirective,
    NoteDirective, OpenDirective, OptionDirective, PadDirective, Posting, PriceDirective,
    SpannedDirective, Transaction, Directive,
};
use crate::types::{Account, AccountType, Amount, Cost, Date, Meta, Price, SpanInfo, StringValue};
use crate::error::ParseError;

/// Simple line-based parser for Beancount files
pub struct SimpleBeancountParser;

impl SimpleBeancountParser {
    /// Parse a Beancount file content
    pub fn parse(content: &str) -> Result<Vec<SpannedDirective>, ParseError> {
        Self::parse_with_source(content, None)
    }

    /// Parse a Beancount file content with source file path
    pub fn parse_with_source(content: &str, source: Option<&str>) -> Result<Vec<SpannedDirective>, ParseError> {
        let mut directives = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        let mut pos = 0usize;

        while i < lines.len() {
            let line = lines[i];
            let line_start = pos;
            let trimmed = line.trim();

            // Skip empty lines and comments (;, #)
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
                pos += line.len() + 1;
                i += 1;
                continue;
            }

            // Handle org-mode style headers (* or ** at start without date)
            // These are comments like "*包含账户信息" or "**支付宝"
            // But "2026-01-01 * ..." is a transaction
            if trimmed.starts_with("*") && !trimmed.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                // Check if it's NOT a date-prefixed line
                static DATE_CHECK: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
                let date_regex = DATE_CHECK.get_or_init(|| regex::Regex::new(r"^\d{4}-\d{2}-\d{2}").unwrap());
                if !date_regex.is_match(trimmed) {
                    pos += line.len() + 1;
                    i += 1;
                    continue;
                }
            }

            // Check if this line starts a directive (has a date at start)
            // Line number is 1-indexed (i starts from 0)
            let line_number = i + 1;
            if let Some((directive, lines_consumed)) = Self::parse_directive_block(&lines, i, line_start, line_number, source) {
                directives.push(directive);
                for j in 0..lines_consumed {
                    if i + j < lines.len() {
                        pos += lines[i + j].len() + 1;
                    }
                }
                i += lines_consumed;
            } else if let Some(directive) = Self::parse_line(trimmed, line_start, line_number, source) {
                directives.push(directive);
                pos += line.len() + 1;
                i += 1;
            } else {
                pos += line.len() + 1;
                i += 1;
            }
        }

        Ok(directives)
    }

    /// Parse a directive that may span multiple lines (transactions, commodities with metadata)
    /// line_number is 1-indexed for display purposes
    fn parse_directive_block(lines: &[&str], start_idx: usize, byte_start: usize, line_number: usize, source: Option<&str>) -> Option<(SpannedDirective, usize)> {
        let first_line = lines[start_idx];
        let trimmed = first_line.trim();

        // Match date pattern: YYYY-MM-DD
        static DATE_PATTERN: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        let date_regex = DATE_PATTERN.get_or_init(|| regex::Regex::new(r"^(\d{4}-\d{2}-\d{2})\s+(.+)$").unwrap());

        let caps = date_regex.captures(trimmed)?;
        let date_str = caps.get(1).unwrap().as_str();
        let rest = caps.get(2).unwrap().as_str();

        // Check if this is a transaction (flag: *, !, txn)
        let is_transaction = rest.starts_with('*') || rest.starts_with('!') || rest.starts_with("txn ");

        if is_transaction {
            // Collect continuation lines (indented lines)
            let mut continuation_lines = Vec::new();
            let mut lines_consumed = 1;
            let mut end_pos = byte_start + first_line.len();

            for i in (start_idx + 1)..lines.len() {
                let line = lines[i];
                // Continuation lines are indented (start with whitespace)
                if !line.is_empty() && (line.starts_with(' ') || line.starts_with('\t')) {
                    continuation_lines.push(line);
                    lines_consumed += 1;
                    end_pos += line.len() + 1;
                } else {
                    break;
                }
            }

            let directive = Self::parse_transaction_full(rest, date_str, &continuation_lines);
            Some((
                SpannedDirective {
                    data: directive,
                    // Use line_number instead of byte_start for transaction ID
                    span: SpanInfo { start: line_number, end: end_pos },
                    source: source.map(|s| s.to_string()),
                },
                lines_consumed,
            ))
        } else if rest.starts_with("commodity ") {
            // Commodities can have metadata on continuation lines
            let mut lines_consumed = 1;
            let mut end_pos = byte_start + first_line.len();

            for i in (start_idx + 1)..lines.len() {
                let line = lines[i];
                if !line.is_empty() && (line.starts_with(' ') || line.starts_with('\t')) {
                    lines_consumed += 1;
                    end_pos += line.len() + 1;
                } else {
                    break;
                }
            }

            let directive = Self::parse_commodity(rest, date_str);
            Some((
                SpannedDirective {
                    data: directive,
                    span: SpanInfo { start: line_number, end: end_pos },
                    source: source.map(|s| s.to_string()),
                },
                lines_consumed,
            ))
        } else {
            // Other directives - parse as single line
            None
        }
    }

    /// Parse a complete transaction with postings
    fn parse_transaction_full(rest: &str, date_str: &str, continuation_lines: &[&str]) -> Directive {
        // Parse transaction header: FLAG "payee" "narration" #tags ^links
        static TXN_HEADER: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        let header_regex = TXN_HEADER.get_or_init(|| {
            regex::Regex::new(r#"^([*!]|txn)\s*(?:"([^"]*)")?\s*(?:"([^"]*)")?\s*(.*)$"#).unwrap()
        });

        let mut flag = None;
        let mut payee = None;
        let mut narration = None;
        let mut tags = Vec::new();
        let mut links = Vec::new();

        if let Some(caps) = header_regex.captures(rest) {
            flag = caps.get(1).map(|m| m.as_str().to_string());
            payee = caps.get(2).map(|m| m.as_str().to_string());
            narration = caps.get(3).map(|m| m.as_str().to_string());

            // Parse tags and links from remaining text
            if let Some(remaining) = caps.get(4) {
                let text = remaining.as_str();
                for part in text.split_whitespace() {
                    if part.starts_with('#') {
                        tags.push(part[1..].to_string());
                    } else if part.starts_with('^') {
                        links.push(part[1..].to_string());
                    }
                }
            }
        }

        // Parse continuation lines into metadata and postings
        let mut meta = Meta::default();
        let mut postings = Vec::new();

        for line in continuation_lines {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') {
                continue;
            }

            // Check if it's metadata (key: "value" or key: value)
            if let Some(colon_pos) = trimmed.find(':') {
                let before_colon = &trimmed[..colon_pos];
                // Metadata keys don't contain spaces and aren't account names
                if !before_colon.contains(' ') && !Self::is_account_name(before_colon) {
                    let key = before_colon.trim();
                    let value = trimmed[colon_pos + 1..].trim().trim_matches('"');
                    meta.insert(key.to_string(), StringValue::Quote(value.to_string()));
                    continue;
                }
            }

            // Try to parse as posting
            if let Some(posting) = Self::parse_posting(trimmed) {
                postings.push(posting);
            }
        }

        Directive::Transaction(Transaction {
            date: Self::parse_date(date_str),
            flag,
            payee,
            narration,
            tags,
            links,
            postings,
            meta,
        })
    }

    /// Check if a string looks like an account name (starts with known prefix)
    fn is_account_name(s: &str) -> bool {
        s.starts_with("Assets") || s.starts_with("Liabilities") ||
        s.starts_with("Equity") || s.starts_with("Income") || s.starts_with("Expenses")
    }

    /// Parse a single posting line
    fn parse_posting(line: &str) -> Option<Posting> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Posting format: [FLAG] ACCOUNT [AMOUNT CURRENCY] [{COST}] [@ PRICE]
        static POSTING_PATTERN: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        let posting_regex = POSTING_PATTERN.get_or_init(|| {
            regex::Regex::new(r#"^([!*])?\s*((?:Assets|Liabilities|Equity|Income|Expenses):[^\s]+)\s*(-?[\d,]+(?:\.\d+)?)?\s*([A-Z][A-Z0-9]*)?(?:\s*\{([^}]*)\})?(?:\s*@\s*(-?[\d,]+(?:\.\d+)?)\s*([A-Z][A-Z0-9]*))?(?:\s*;.*)?$"#).unwrap()
        });

        if let Some(caps) = posting_regex.captures(trimmed) {
            let flag = caps.get(1).map(|m| m.as_str().to_string());
            let account_name = caps.get(2)?.as_str();
            let (account_type, components) = Self::parse_account_name(account_name);

            let amount = if let (Some(amt), Some(curr)) = (caps.get(3), caps.get(4)) {
                let amount_str = amt.as_str().replace(',', "");
                let amount: rust_decimal::Decimal = amount_str.parse().ok()?;
                let currency = curr.as_str().to_string();
                Some(Amount { amount, currency })
            } else {
                None
            };

            // Parse cost if present
            let cost = caps.get(5).and_then(|m| {
                let cost_str = m.as_str().trim();
                // Simple cost parsing: NUMBER CURRENCY
                let parts: Vec<&str> = cost_str.split_whitespace().collect();
                if parts.len() >= 2 {
                    let amount: rust_decimal::Decimal = parts[0].replace(',', "").parse().ok()?;
                    Some(Cost {
                        amount,
                        currency: parts[1].to_string(),
                        date: None,
                    })
                } else {
                    None
                }
            });

            // Parse price if present
            let price = if let (Some(price_amt), Some(price_curr)) = (caps.get(6), caps.get(7)) {
                let amount: rust_decimal::Decimal = price_amt.as_str().replace(',', "").parse().ok()?;
                Some(Price::Single(Amount {
                    amount,
                    currency: price_curr.as_str().to_string(),
                }))
            } else {
                None
            };

            Some(Posting {
                flag,
                account: Account {
                    account_type,
                    name: account_name.to_string(),
                    components,
                },
                amount,
                cost,
                price,
            })
        } else {
            None
        }
    }

    fn parse_line(line: &str, start: usize, line_number: usize, source: Option<&str>) -> Option<SpannedDirective> {
        // Match date pattern: YYYY-MM-DD
        static DATE_PATTERN: once_cell::sync::OnceCell<regex::Regex> =
            once_cell::sync::OnceCell::new();
        let date_regex = DATE_PATTERN.get_or_init(|| {
            regex::Regex::new(r"^(\d{4}-\d{2}-\d{2})\s+(.+)$").unwrap()
        });

        let source_str = source.map(|s| s.to_string());

        // First check if line starts with date
        if let Some(caps) = date_regex.captures(line) {
            let date_str = caps.get(1).unwrap().as_str();
            let rest = caps.get(2).unwrap().as_str();

            let directive = if rest.starts_with("open ") {
                Self::parse_open(rest, date_str)
            } else if rest.starts_with("close ") {
                Self::parse_close(rest, date_str)
            } else if rest.starts_with("balance ") {
                Self::parse_balance(rest, date_str)
            } else if rest.starts_with("commodity ") {
                Self::parse_commodity(rest, date_str)
            } else if rest.starts_with("pad ") {
                Self::parse_pad(rest, date_str)
            } else if rest.starts_with("document ") {
                Self::parse_document(rest, date_str)
            } else if rest.starts_with("price ") {
                Self::parse_price(rest, date_str)
            } else if rest.starts_with("note ") {
                Self::parse_note(rest, date_str)
            } else if rest.starts_with("event ") {
                Self::parse_event(rest, date_str)
            } else if rest.starts_with("option ") {
                Self::parse_option(rest)
            } else if rest.starts_with("include ") {
                Self::parse_include(rest)
            } else if rest.starts_with("custom ") {
                Self::parse_custom(rest, date_str)
            } else {
                // Transactions are handled by parse_directive_block
                Directive::Comment(CommentDirective {
                    content: line.to_string(),
                })
            };

            Some(SpannedDirective {
                data: directive,
                span: SpanInfo {
                    start: line_number,
                    end: start + line.len(),
                },
                source: source_str,
            })
        } else {
            // Handle directives without dates (include, option, etc.)
            let trimmed = line.trim();
            if trimmed.starts_with("option ") {
                Some(SpannedDirective {
                    data: Self::parse_option(trimmed),
                    span: SpanInfo {
                        start: line_number,
                        end: start + line.len(),
                    },
                    source: source_str.clone(),
                })
            } else if trimmed.starts_with("include ") {
                Some(SpannedDirective {
                    data: Self::parse_include(trimmed),
                    span: SpanInfo {
                        start: line_number,
                        end: start + line.len(),
                    },
                    source: source_str.clone(),
                })
            } else if trimmed.starts_with("pushtag ") {
                Some(SpannedDirective {
                    data: Directive::Comment(CommentDirective {
                        content: line.to_string(),
                    }),
                    span: SpanInfo {
                        start: line_number,
                        end: start + line.len(),
                    },
                    source: source_str.clone(),
                })
            } else if trimmed.starts_with("pophtag ") {
                Some(SpannedDirective {
                    data: Directive::Comment(CommentDirective {
                        content: line.to_string(),
                    }),
                    span: SpanInfo {
                        start: line_number,
                        end: start + line.len(),
                    },
                    source: source_str.clone(),
                })
            } else {
                None
            }
        }
    }

    fn parse_open(rest: &str, date_str: &str) -> Directive {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 2 {
            let account_name = parts[1];
            let (account_type, components) = Self::parse_account_name(account_name);
            let currencies = parts[2..].iter().map(|s| s.to_string()).collect();

            Directive::Open(OpenDirective {
                date: Self::parse_date(date_str),
                account: Account {
                    account_type,
                    name: account_name.to_string(),
                    components,
                },
                currencies,
                meta: Meta::default(),
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_close(rest: &str, date_str: &str) -> Directive {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 2 {
            let account_name = parts[1];
            let (account_type, components) = Self::parse_account_name(account_name);

            Directive::Close(CloseDirective {
                date: Self::parse_date(date_str),
                account: Account {
                    account_type,
                    name: account_name.to_string(),
                    components,
                },
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_balance(rest: &str, date_str: &str) -> Directive {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 4 {
            let account_name = parts[1];
            let (account_type, components) = Self::parse_account_name(account_name);
            let amount: rust_decimal::Decimal = parts[2].replace(',', "").parse().unwrap_or_default();
            let currency = parts[3].to_string();

            Directive::Balance(BalanceDirective {
                date: Self::parse_date(date_str),
                account: Account {
                    account_type,
                    name: account_name.to_string(),
                    components,
                },
                amount: Amount { amount, currency },
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_commodity(rest: &str, date_str: &str) -> Directive {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 2 {
            Directive::Commodity(CommodityDirective {
                date: Self::parse_date(date_str),
                name: parts[1].to_string(),
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_pad(rest: &str, date_str: &str) -> Directive {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 3 {
            let account_name = parts[1];
            let pad_name = parts[2];
            let (account_type, components) = Self::parse_account_name(account_name);
            let (pad_type, pad_components) = Self::parse_account_name(pad_name);

            Directive::Pad(PadDirective {
                date: Self::parse_date(date_str),
                account: Account {
                    account_type,
                    name: account_name.to_string(),
                    components,
                },
                pad: Account {
                    account_type: pad_type,
                    name: pad_name.to_string(),
                    components: pad_components,
                },
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_document(rest: &str, date_str: &str) -> Directive {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 3 {
            let account_name = parts[1];
            let (account_type, components) = Self::parse_account_name(account_name);
            let filename = parts[2..].join(" ");

            Directive::Document(DocumentDirective {
                date: Self::parse_date(date_str),
                account: Account {
                    account_type,
                    name: account_name.to_string(),
                    components,
                },
                filename,
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_price(rest: &str, date_str: &str) -> Directive {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 4 {
            let amount: rust_decimal::Decimal = parts[2].replace(',', "").parse().unwrap_or_default();

            Directive::Price(PriceDirective {
                date: Self::parse_date(date_str),
                commodity: parts[1].to_string(),
                amount: Amount {
                    amount,
                    currency: parts[3].to_string(),
                },
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_note(rest: &str, date_str: &str) -> Directive {
        let parts: Vec<&str> = rest.splitn(3, ' ').collect();
        if parts.len() >= 3 {
            let account_name = parts[1];
            let (account_type, components) = Self::parse_account_name(account_name);
            let comment = parts[2..].join(" ");

            Directive::Note(NoteDirective {
                date: Self::parse_date(date_str),
                account: Account {
                    account_type,
                    name: account_name.to_string(),
                    components,
                },
                comment,
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_event(rest: &str, date_str: &str) -> Directive {
        let parts: Vec<&str> = rest.splitn(3, ' ').collect();
        if parts.len() >= 3 {
            Directive::Event(EventDirective {
                date: Self::parse_date(date_str),
                event_type: parts[1].to_string(),
                description: parts[2..].join(" "),
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_option(rest: &str) -> Directive {
        let parts: Vec<&str> = rest.splitn(3, ' ').collect();
        if parts.len() >= 3 {
            Directive::Option(OptionDirective {
                key: parts[1].trim_matches('"').to_string(),
                value: parts[2].trim_matches('"').to_string(),
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_include(rest: &str) -> Directive {
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        if parts.len() >= 2 {
            Directive::Include(IncludeDirective {
                path: parts[1].trim_matches('"').to_string(),
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_custom(rest: &str, date_str: &str) -> Directive {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 3 {
            Directive::Custom(CustomDirective {
                date: Self::parse_date(date_str),
                custom_type: parts[1].to_string(),
                values: parts[2..].iter().map(|s| s.to_string()).collect(),
            })
        } else {
            Directive::Comment(CommentDirective {
                content: rest.to_string(),
            })
        }
    }

    fn parse_date(date_str: &str) -> Date {
        use chrono::NaiveDate;
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map(Date::Date)
            .unwrap_or_else(|_| Date::Date(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()))
    }

    fn parse_account_name(name: &str) -> (AccountType, Vec<String>) {
        let parts: Vec<&str> = name.split(':').collect();
        if parts.is_empty() {
            return (AccountType::Assets, vec![]);
        }

        let account_type = match parts[0] {
            "Assets" => AccountType::Assets,
            "Liabilities" => AccountType::Liabilities,
            "Equity" => AccountType::Equity,
            "Income" => AccountType::Income,
            "Expenses" => AccountType::Expenses,
            _ => AccountType::Assets,
        };

        let components = parts[1..].iter().map(|s| s.to_string()).collect();
        (account_type, components)
    }
}

/// Extract time from metadata (supports various formats)
pub fn extract_time_from_meta(meta: &mut crate::types::Meta) -> Option<chrono::NaiveTime> {
    let time_keys = ["time", "trade_time", "tgbot_time", "payTime", "created_at"];

    for key in time_keys {
        if let Some(value) = meta.remove(key) {
            let time_str = value.as_str();
            if let Ok(time) = chrono::NaiveTime::parse_from_str(time_str, "%Y-%m-%d %H:%M:%S") {
                return Some(time);
            }
            if let Ok(time) = chrono::NaiveTime::parse_from_str(time_str, "%H:%M:%S") {
                return Some(time);
            }
        }
    }
    None
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_transaction() {
        let input = r#"2023-01-15 "Coffee Shop" "Morning coffee"
  Assets:Cash -5.00 CNY
  Expenses:Food 5.00 CNY"#;
        let result = SimpleBeancountParser::parse(input);
        assert!(result.is_ok());
        let directives = result.unwrap();
        assert!(!directives.is_empty());
    }

    #[test]
    fn test_parse_open_account() {
        let input = "2023-01-01 open Assets:Cash CNY";
        let result = SimpleBeancountParser::parse(input);
        assert!(result.is_ok());
        let directives = result.unwrap();
        assert_eq!(directives.len(), 1);
    }

    #[test]
    fn test_parse_balance() {
        let input = "2023-01-15 balance Assets:Cash 100.00 CNY";
        let result = SimpleBeancountParser::parse(input);
        assert!(result.is_ok());
        let directives = result.unwrap();
        assert_eq!(directives.len(), 1);
    }

    #[test]
    fn test_parse_multiple_directives() {
        let input = r#"2023-01-01 open Assets:Cash CNY
2023-01-01 open Expenses:Food CNY
2023-01-15 "Coffee Shop" "Morning coffee"
  Assets:Cash -5.00 CNY
  Expenses:Food 5.00 CNY
2023-01-15 balance Assets:Cash 100.00 CNY"#;
        let result = SimpleBeancountParser::parse(input);
        assert!(result.is_ok());
        let directives = result.unwrap();
        assert_eq!(directives.len(), 4);
    }

    #[test]
    fn test_parse_account_types() {
        let (atype, comps) = SimpleBeancountParser::parse_account_name("Assets:Checking");
        assert_eq!(atype, AccountType::Assets);
        assert_eq!(comps, vec!["Checking"]);

        let (atype, comps) = SimpleBeancountParser::parse_account_name("Expenses:Food:Dining");
        assert_eq!(atype, AccountType::Expenses);
        assert_eq!(comps, vec!["Food", "Dining"]);
    }

    #[test]
    fn test_parse_transaction_with_postings() {
        let input = r#"2026-01-01 * "燕君" "饺子皮等"
  tgbot_uuid: "ff24e685-7cea-4da1-a7b6-3f6ab59f96ca"
  Assets:DebitCard:中国银行:6295           -45.00 CNY
  Expenses:Food:买菜                      45.00 CNY
"#;
        let result = SimpleBeancountParser::parse(input);
        assert!(result.is_ok());
        let directives = result.unwrap();
        assert_eq!(directives.len(), 1, "Should have exactly 1 directive");

        if let Directive::Transaction(txn) = &directives[0].data {
            println!("Transaction: payee={:?} narration={:?}", txn.payee, txn.narration);
            println!("Meta: {:?}", txn.meta);
            println!("Postings count: {}", txn.postings.len());
            for (i, posting) in txn.postings.iter().enumerate() {
                println!("  Posting {}: {} {:?}", i, posting.account.name, posting.amount);
            }
            assert!(txn.postings.len() >= 2, "Should have at least 2 postings");
        } else {
            panic!("Expected Transaction directive, got {:?}", directives[0].data);
        }
    }
}
