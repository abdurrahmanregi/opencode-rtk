use super::HandlerResult;
use serde_json::json;

pub async fn handle(_params: serde_json::Value) -> HandlerResult {
    // Signal shutdown (in real implementation, would trigger graceful shutdown)
    Ok(json!({
        "status": "shutting_down"
    }))
}
