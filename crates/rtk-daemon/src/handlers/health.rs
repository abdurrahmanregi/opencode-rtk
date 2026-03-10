use super::HandlerResult;
use serde_json::{json, Value};

pub async fn handle(_params: Value) -> HandlerResult {
    Ok(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
