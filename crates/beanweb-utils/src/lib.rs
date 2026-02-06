//! Utility functions and helpers

/// Format a number with thousands separators
pub fn format_number<T: ToString>(n: T) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let mut count = 0;
    for c in s.chars().rev() {
        if count == 3 {
            result.push(',');
            count = 0;
        }
        result.push(c);
        count += 1;
    }
    result.chars().rev().collect()
}

/// Sanitize HTML content for HTMX responses
pub fn sanitize_html(content: &str) -> String {
    // Basic HTML sanitization - remove potentially dangerous elements
    content
        .replace("<script", "&lt;script")
        .replace("</script>", "&lt;/script&gt;")
}

/// Generate a unique ID
pub fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("{}", now)
}
