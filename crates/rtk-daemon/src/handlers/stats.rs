use super::HandlerResult;
use crate::protocol::{INVALID_PARAMS, INTERNAL_ERROR};
use rtk_core::tracking::db::get_session_stats;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
struct StatsParams {
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct StatsResult {
    command_count: i64,
    total_original_tokens: i64,
    total_compressed_tokens: i64,
    total_saved_tokens: i64,
    savings_pct: f64,
}

pub async fn handle(params: Value) -> HandlerResult {
    let params: StatsParams = serde_json::from_value(params)
        .map_err(|e| (INVALID_PARAMS, format!("Invalid parameters: {}", e)))?;
    
    // For now, return empty stats if no session_id
    // In a real implementation, we'd aggregate across all sessions
    let stats = if let Some(session_id) = params.session_id {
        get_session_stats(&session_id)
            .map_err(|e| (INTERNAL_ERROR, format!("Database error: {}", e)))?
    } else {
        return Ok(json!({
            "message": "Session ID required for stats"
        }));
    };
    
    let result = StatsResult {
        command_count: stats.command_count,
        total_original_tokens: stats.total_original_tokens,
        total_compressed_tokens: stats.total_compressed_tokens,
        total_saved_tokens: stats.total_saved_tokens,
        savings_pct: stats.savings_pct(),
    };
    
    serde_json::to_value(result)
        .map_err(|e| (INTERNAL_ERROR, format!("Serialization failed: {}", e)))
}
