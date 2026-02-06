//! Files API endpoints - JSON API
//!
//! Features:
//! - File listing with include recursion
//! - File content read/write
//! - Account validation on save

use crate::AppState;
use axum::extract::Path;
use std::path::PathBuf;

/// File info structure
#[derive(Debug, Clone)]
struct FileInfo {
    name: String,
    modified: String,
    size: String,
    referenced: bool,
}

/// Extract include paths from content using regex
fn extract_includes(content: &str) -> Vec<String> {
    let mut includes = Vec::new();
    let re = regex::Regex::new(r#"include\s+"([^"]+)""#).unwrap();

    for cap in re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            includes.push(m.as_str().to_string());
        }
    }
    includes
}

/// Check if a pattern contains glob characters (* or ?)
fn is_glob_pattern(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?')
}

/// Expand glob pattern and return matched file paths relative to base_dir
fn expand_glob_pattern(base_dir: &PathBuf, pattern: &str) -> Vec<String> {
    let mut matched = Vec::new();

    // Handle glob patterns
    let glob_pattern = if pattern.starts_with('/') {
        // Absolute path from data directory
        base_dir.join(&pattern[1..])
    } else {
        // Relative path
        base_dir.join(pattern)
    };

    if let Ok(paths) = glob::glob(&glob_pattern.to_string_lossy()) {
        for entry in paths.flatten() {
            if entry.is_file() {
                if let Ok(relative) = entry.strip_prefix(base_dir) {
                    matched.push(relative.to_string_lossy().into_owned());
                }
            }
        }
    }

    matched
}

/// Parse include directives and return referenced file paths (iterative, no recursion)
fn parse_includes_iterative(base_path: &PathBuf, content: &str) -> Vec<String> {
    let mut referenced: Vec<String> = Vec::new();
    let mut to_process: Vec<String> = Vec::new();
    let mut processed = std::collections::HashSet::new();

    // Initial pass - get includes from main content
    let initial_includes = extract_includes(content);
    for inc in initial_includes {
        if !processed.contains(&inc) {
            processed.insert(inc.clone());
            // Check if it's a glob pattern
            if is_glob_pattern(&inc) {
                // Expand glob pattern
                let expanded = expand_glob_pattern(base_path, &inc);
                for file in expanded {
                    if !referenced.contains(&file) {
                        referenced.push(file.clone());
                        // Add to processing queue if it's a beancount file
                        if file.ends_with(".bean") || file.ends_with(".beancount") || file.ends_with(".bc") {
                            to_process.push(file);
                        }
                    }
                }
            } else {
                referenced.push(inc.clone());
                to_process.push(inc);
            }
        }
    }

    // Process remaining includes
    while let Some(inc) = to_process.pop() {
        let include_full = if inc.starts_with('/') {
            base_path.join(&inc[1..])
        } else {
            base_path.join(&inc)
        };

        if let Ok(include_content) = std::fs::read_to_string(&include_full) {
            let nested_includes = extract_includes(&include_content);
            for nested in nested_includes {
                if !processed.contains(&nested) {
                    processed.insert(nested.clone());
                    // Check if it's a glob pattern
                    if is_glob_pattern(&nested) {
                        // Expand glob pattern
                        let expanded = expand_glob_pattern(base_path, &nested);
                        for file in expanded {
                            if !referenced.contains(&file) {
                                referenced.push(file.clone());
                                // Add to processing queue if it's a beancount file
                                if file.ends_with(".bean") || file.ends_with(".beancount") || file.ends_with(".bc") {
                                    to_process.push(file);
                                }
                            }
                        }
                    } else {
                        referenced.push(nested.clone());
                        to_process.push(nested);
                    }
                }
            }
        }
    }

    referenced
}

/// Recursively scan directory for Beancount files (using iteration)
fn scan_directory_iterative(base_path: &PathBuf) -> Vec<FileInfo> {
    let mut files: Vec<FileInfo> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    dirs.push(base_path.clone());

    while let Some(current_dir) = dirs.pop() {
        if let Ok(entries) = std::fs::read_dir(&current_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let relative_path = path.strip_prefix(base_path).unwrap_or(&path).to_string_lossy().into_owned();

                if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str());
                    if ext == Some("bean") || ext == Some("beancount") || ext == Some("bc") {
                        files.push(FileInfo {
                            name: relative_path,
                            modified: get_file_modified(&path),
                            size: get_file_size(&path),
                            referenced: false,
                        });
                    }
                } else if path.is_dir() {
                    dirs.push(path);
                }
            }
        }
    }

    files
}

fn get_file_modified(path: &PathBuf) -> String {
    match path.metadata() {
        Ok(m) => match m.modified() {
            Ok(t) => chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M").to_string(),
            Err(_) => "未知".to_string(),
        },
        Err(_) => "未知".to_string(),
    }
}

fn get_file_size(path: &PathBuf) -> String {
    match path.metadata() {
        Ok(m) => {
            let size = m.len();
            if size < 1024 { format!("{} B", size) }
            else if size < 1024 * 1024 { format!("{:.1} KB", size as f64 / 1024.0) }
            else { format!("{:.1} MB", size as f64 / (1024.0 * 1024.0)) }
        }
        Err(_) => "未知".to_string(),
    }
}

/// HTMX: Get file list (with optional search filter)
pub async fn api_files_list(state: axum::extract::State<AppState>, query: Option<axum::extract::Query<std::collections::HashMap<String, String>>>) -> String {
    let config = &state.config;
    let base_path = &config.data.path;

    // Collect all files with include recursion
    let mut files = scan_directory_iterative(base_path);

    // Parse include directives to find referenced files and mark them
    let main_file = base_path.join(&config.data.main_file);
    if main_file.exists() {
        if let Ok(main_content) = std::fs::read_to_string(&main_file) {
            let referenced_files = parse_includes_iterative(base_path, &main_content);
            let referenced_set: std::collections::HashSet<String> = referenced_files.into_iter().collect();
            for file in &mut files {
                file.referenced = referenced_set.contains(&file.name);
            }
        }
    }

    // Apply search filter
    let search_term = query.as_ref().and_then(|q| q.0.get("search").map(|s| s.to_lowercase())).unwrap_or_default();
    if !search_term.is_empty() {
        files.retain(|f| f.name.to_lowercase().contains(&search_term));
    }

    files.sort_by(|a, b| a.name.cmp(&b.name));

    // Render HTML
    if files.is_empty() {
        return r#"<div class='text-center py-12 text-gray-500'>没有找到 Beancount 文件</div>"#.to_string();
    }

    let mut html = r#"<table class='w-full'><thead><tr><th class='text-left p-4 bg-gray-50'>文件名</th><th class='text-left p-4 bg-gray-50'>修改时间</th><th class='text-left p-4 bg-gray-50'>大小</th><th class='text-left p-4 bg-gray-50'>操作</th></tr></thead><tbody>"#.to_string();

    for file in &files {
        // NOTE: 引用状态显示已禁用（不准确）
        // let status = if file.referenced {
        //     r#"<span class='text-xs bg-green-100 text-green-800 px-2 py-1 rounded'>已引用</span>"#
        // } else {
        //     r#"<span class='text-xs bg-yellow-100 text-yellow-800 px-2 py-1 rounded'>未引用</span>"#
        // };

        html.push_str(&format!(
            r#"<tr class='border-b hover:bg-gray-50'><td class='p-4'><a href='/files/{}' class='text-indigo-600 hover:text-indigo-800 hover:underline'>{}</a></td><td class='p-4 text-gray-500'>{}</td><td class='p-4 text-gray-500'>{}</td><td class='p-4'><a href='/files/{}' class='px-3 py-1 bg-indigo-100 text-indigo-700 rounded hover:bg-indigo-200'>编辑</a></td></tr>"#,
            urlencoding::encode(&file.name),
            file.name,
            file.modified,
            file.size,
            urlencoding::encode(&file.name)
        ));
    }

    html.push_str("</tbody></table>");
    html
}

pub async fn api_file_content(state: axum::extract::State<AppState>, path: Path<String>) -> String {
    let config = &state.config;
    let file_path = config.data.path.join(&path.0);

    match std::fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(_) => String::new(),
    }
}

/// Validation result with warnings
struct ValidationResult {
    is_valid: bool,
    warnings: Vec<String>,
}

/// Extract account names from specific directives
fn extract_accounts_from_line(line: &str, pattern: &str) -> Vec<String> {
    let mut accounts = Vec::new();
    let re = regex::Regex::new(pattern).unwrap();
    if let Some(caps) = re.captures(line) {
        if let Some(m) = caps.get(1) {
            accounts.push(m.as_str().to_string());
        }
    }
    accounts
}

/// Validate account references in file content
fn validate_accounts(content: &str, ledger: &beanweb_core::Ledger) -> ValidationResult {
    let open_accounts: std::collections::HashSet<String> = ledger.accounts().iter()
        .map(|a| a.name.clone())
        .collect();

    let mut warnings: Vec<String> = Vec::new();
    let mut all_valid = true;

    // Get all accounts opened in this file
    let file_open_accounts: std::collections::HashSet<String> = extract_accounts_from_line(content, r"open\s+(\S+)")
        .into_iter()
        .collect();

    let account_patterns = [
        (r"balance\s+(\S+)", "balance"),
        (r"pad\s+(\S+)", "pad"),
        (r"note\s+(\S+)", "note"),
    ];

    for (line_num, line) in content.lines().enumerate() {
        let line_num = line_num + 1;

        for (pattern, directive_type) in &account_patterns {
            let accounts = extract_accounts_from_line(line, pattern);
            for account in accounts {
                // Check if account is a valid account type
                if account.starts_with("Assets") || account.starts_with("Liabilities") ||
                   account.starts_with("Expenses") || account.starts_with("Income") ||
                   account.starts_with("Equity") {
                    if !open_accounts.contains(&account) && !file_open_accounts.contains(&account) {
                        warnings.push(format!("第 {} 行: {} 指令使用了未声明的账户 '{}'", line_num, directive_type, account));
                        all_valid = false;
                    }
                }
            }
        }
    }

    ValidationResult {
        is_valid: all_valid,
        warnings,
    }
}

pub async fn api_file_save(state: axum::extract::State<AppState>, path: Path<String>, body: String) -> String {
    let config = &state.config;
    let file_path = config.data.path.join(&path.0);

    // Validate accounts before saving
    let ledger = state.ledger.read().await;
    let validation = validate_accounts(&body, &ledger);

    // Show warnings even if valid
    let warning_html = if !validation.warnings.is_empty() {
        let warnings: Vec<String> = validation.warnings.iter().map(|w| format!("<li class='ml-4'>{}</li>", w)).collect();
        format!(r#"<div class='bg-yellow-50 border border-yellow-200 rounded-lg p-4 mt-2'><div class='flex items-center gap-2'><span class='text-yellow-600'>⚠</span><span class='font-medium text-yellow-800'>警告</span></div><ul class='mt-2 text-sm text-yellow-700'>{}</ul></div>"#, warnings.join(""))
    } else {
        String::new()
    };

    // Save file
    match std::fs::write(&file_path, &body) {
        Ok(_) => {
            // Trigger ledger reload directly (not through HTTP)
            let mut ledger = state.ledger.write().await;
            if let Err(e) = ledger.reload().await {
                eprintln!("[ERROR] Failed to reload ledger after file save: {}", e);
            }

            format!(r#"<div class='bg-green-50 border border-green-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-green-600'>✓</span><span class='font-medium text-green-800'>保存成功！账本已重新加载</span></div>{}</div>"#, warning_html)
        }
        Err(e) => format!(r#"<div class='bg-red-50 border border-red-200 rounded-lg p-4'><div class='flex items-center gap-2'><span class='text-red-600'>✗</span><span class='font-medium text-red-800'>保存失败: {}</span></div>{}</div>"#, e, warning_html),
    }
}
