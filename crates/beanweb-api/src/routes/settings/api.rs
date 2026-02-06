//! Settings API endpoints - JSON API

use crate::AppState;

pub async fn api_settings(state: axum::extract::State<AppState>) -> String {
    let config = &state.config;
    serde_json::to_string(config).unwrap_or_default()
}

pub async fn api_settings_metadata() -> String {
    serde_json::to_string(&serde_json::json!({
        "server": {
            "host": "string",
            "port": "number",
            "auth": "object"
        },
        "data": {
            "path": "string",
            "main_file": "string"
        }
    })).unwrap_or_default()
}
