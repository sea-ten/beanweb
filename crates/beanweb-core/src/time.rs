//! Time control system for filtering transactions by date range

use beanweb_config::TimeRange;
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

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

impl TimeFilter for super::Transaction {
    fn filter_by_time(&self, context: &TimeContext) -> bool {
        if let Ok(date) = NaiveDate::parse_from_str(&self.date, "%Y-%m-%d") {
            context.contains(&date)
        } else {
            // If we can't parse the date, include it
            true
        }
    }
}

impl TimeFilter for super::Account {
    fn filter_by_time(&self, _context: &TimeContext) -> bool {
        // Accounts are not time-filtered by default
        true
    }
}
