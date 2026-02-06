//! Configuration management for beanweb
//!
//! This module handles loading, validation, and management of
//! beanweb configuration from YAML files.

pub mod error;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use error::ConfigError;

// ==================== Configuration Types ====================

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    /// Server host address
    #[serde(default = "default_host")]
    pub host: String,
    /// Server port
    #[serde(default = "default_port")]
    pub port: u16,
    /// Basic authentication (optional)
    #[serde(default)]
    pub auth: Option<AuthConfig>,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8081
}

/// Basic authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub username: String,
    pub password: String,
}

/// Data directory configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataConfig {
    /// Path to ledger directory
    #[serde(default = "default_data_path")]
    pub path: PathBuf,
    /// Main Beancount file name
    #[serde(default = "default_main_file")]
    pub main_file: String,
    /// Enable file watching for auto-reload
    #[serde(default = "default_true")]
    pub watch_enable: bool,
    /// Default file for new transactions (relative to data path)
    #[serde(default = "default_new_transaction_file")]
    pub new_transaction_file: String,
}

fn default_data_path() -> PathBuf {
    PathBuf::from("./data")
}

fn default_main_file() -> String {
    "main.bean".to_string()
}

fn default_new_transaction_file() -> String {
    "transactions.bean".to_string()
}

/// Feature toggles
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeaturesConfig {
    /// Enable budget management
    #[serde(default = "default_false")]
    pub budget_enable: bool,
    /// Extract time from transaction metadata
    #[serde(default = "default_true")]
    pub time_extraction: bool,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoggingConfig {
    /// Log level: debug, info, warn, error
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_log_level() -> String {
    "debug".to_string()
}

/// Pagination settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaginationConfig {
    /// Records per page for lists
    #[serde(default = "default_records_per_page")]
    pub records_per_page: usize,
}

fn default_records_per_page() -> usize {
    50
}

/// Time range configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimeRangeConfig {
    /// Default time range (e.g., "month", "quarter", "year")
    #[serde(default)]
    pub default_range: TimeRange,
    /// Show current year to date
    #[serde(default)]
    pub show_current_year_to_date: bool,
    /// Fiscal year start month (1-12)
    #[serde(default = "default_fiscal_start")]
    pub fiscal_year_start: u32,
}

// fn default_time_range() -> String {
//     "month".to_string()
// }

fn default_fiscal_start() -> u32 {
    1
}

/// Time range enumeration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeRange {
    /// Current month
    Month,
    /// Last 3 months
    Quarter,
    /// Current year
    Year,
    /// All time
    All,
    /// Custom range
    Custom,
}

impl Default for TimeRange {
    fn default() -> Self {
        TimeRange::Month
    }
}

impl std::str::FromStr for TimeRange {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "month" => Ok(TimeRange::Month),
            "quarter" => Ok(TimeRange::Quarter),
            "year" => Ok(TimeRange::Year),
            "all" => Ok(TimeRange::All),
            "custom" => Ok(TimeRange::Custom),
            _ => Err(format!("Invalid time range: {}", s)),
        }
    }
}

impl std::fmt::Display for TimeRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeRange::Month => write!(f, "month"),
            TimeRange::Quarter => write!(f, "quarter"),
            TimeRange::Year => write!(f, "year"),
            TimeRange::All => write!(f, "all"),
            TimeRange::Custom => write!(f, "custom"),
        }
    }
}

/// Journal display settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JournalConfig {
    /// Expand transaction details inline
    #[serde(default = "default_true")]
    pub expand_detail: bool,
    /// Default edit mode ("form" or "text")
    #[serde(default = "default_edit_mode")]
    pub edit_mode: EditMode,
    /// Default page: "dashboard" or "journals"
    #[serde(default = "default_default_page")]
    pub default_page: String,
}

fn default_edit_mode() -> EditMode {
    EditMode::Form
}

fn default_default_page() -> String {
    "journals".to_string()
}

/// Edit mode enumeration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditMode {
    /// Form-based editing
    Form,
    /// Raw text editing
    Text,
}

impl Default for EditMode {
    fn default() -> Self {
        EditMode::Form
    }
}

impl std::str::FromStr for EditMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "form" => Ok(EditMode::Form),
            "text" => Ok(EditMode::Text),
            _ => Err(format!("Invalid edit mode: {}", s)),
        }
    }
}

impl std::fmt::Display for EditMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditMode::Form => write!(f, "form"),
            EditMode::Text => write!(f, "text"),
        }
    }
}

/// Chart and visualization settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartConfig {
    /// Default chart type
    #[serde(default = "default_chart_type")]
    pub default_chart_type: ChartType,
    /// Number of top items to show
    #[serde(default = "default_top_items")]
    pub top_items_count: usize,
    /// Show chart legends
    #[serde(default = "default_true")]
    pub show_legend: bool,
    /// Use interactive charts
    #[serde(default = "default_true")]
    pub interactive: bool,
}

impl Default for ChartConfig {
    fn default() -> Self {
        Self {
            default_chart_type: ChartType::Bar,
            top_items_count: 10,
            show_legend: true,
            interactive: true,
        }
    }
}

fn default_chart_type() -> ChartType {
    ChartType::Bar
}

fn default_top_items() -> usize {
    10
}

/// Chart type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartType {
    Bar,
    Line,
    Pie,
    Area,
    StackedBar,
}

impl Default for ChartType {
    fn default() -> Self {
        ChartType::Bar
    }
}

/// Currency and number formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyConfig {
    /// Default currency
    #[serde(default = "default_currency")]
    pub default_currency: String,
    /// Number of decimal places
    #[serde(default = "default_decimal_places")]
    pub decimal_places: u32,
    /// Thousands separator
    #[serde(default = "default_thousands_sep")]
    pub thousands_separator: String,
    /// Decimal separator
    #[serde(default = "default_decimal_sep")]
    pub decimal_separator: String,
    /// Currency symbol position ("before" or "after")
    #[serde(default = "default_symbol_position")]
    pub symbol_position: SymbolPosition,
}

impl Default for CurrencyConfig {
    fn default() -> Self {
        Self {
            default_currency: "CNY".to_string(),
            decimal_places: 2,
            thousands_separator: ",".to_string(),
            decimal_separator: ".".to_string(),
            symbol_position: SymbolPosition::Before,
        }
    }
}

fn default_currency() -> String {
    "CNY".to_string()
}

fn default_decimal_places() -> u32 {
    2
}

fn default_thousands_sep() -> String {
    ",".to_string()
}

fn default_decimal_sep() -> String {
    ".".to_string()
}

fn default_symbol_position() -> SymbolPosition {
    SymbolPosition::Before
}

/// Currency symbol position
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolPosition {
    Before,
    After,
}

impl Default for SymbolPosition {
    fn default() -> Self {
        SymbolPosition::Before
    }
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Server settings
    #[serde(default)]
    pub server: ServerConfig,
    /// Data directory settings
    #[serde(default)]
    pub data: DataConfig,
    /// Feature toggles
    #[serde(default)]
    pub features: FeaturesConfig,
    /// Pagination settings
    #[serde(default)]
    pub pagination: PaginationConfig,
    /// Journal display settings
    #[serde(default)]
    pub journal: JournalConfig,
    /// Time range settings
    #[serde(default)]
    pub time_range: TimeRangeConfig,
    /// Chart settings
    #[serde(default)]
    pub charts: ChartConfig,
    /// Currency settings
    #[serde(default)]
    pub currency: CurrencyConfig,
    /// Logging settings
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Config {
    /// Load configuration from a YAML file
    pub fn load(path: PathBuf) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(&path)
            .map_err(|_| ConfigError::IoError)?;

        // Try to parse the YAML
        let config: Config = serde_yaml::from_str(&content)
            .map_err(|_| ConfigError::InvalidYaml)?;

        // Validate the configuration
        config.validate()?;

        Ok(config)
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate port
        if self.server.port == 0 {
            return Err(ConfigError::InvalidValue {
                field: "server.port".to_string(),
                reason: "Port must be greater than 0".to_string(),
            });
        }

        // Validate fiscal year start
        if self.time_range.fiscal_year_start < 1 || self.time_range.fiscal_year_start > 12 {
            return Err(ConfigError::InvalidValue {
                field: "time_range.fiscal_year_start".to_string(),
                reason: "Fiscal year start must be between 1 and 12".to_string(),
            });
        }

        // Validate decimal places
        if self.currency.decimal_places > 10 {
            return Err(ConfigError::InvalidValue {
                field: "currency.decimal_places".to_string(),
                reason: "Decimal places must be between 0 and 10".to_string(),
            });
        }

        Ok(())
    }

    /// Generate a default configuration file
    pub fn generate_default() -> &'static str {
        include_str!("../templates/default_config.yaml")
    }

    /// Get the full path to the main ledger file
    pub fn ledger_path(&self) -> PathBuf {
        self.data.path.join(&self.data.main_file)
    }

    /// Check if a feature is enabled
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        match feature {
            "budget" => self.features.budget_enable,
            "time_extraction" => self.features.time_extraction,
            _ => false,
        }
    }
}
