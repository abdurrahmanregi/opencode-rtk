use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    pub id: Option<Value>,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct Error {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub fn success_response(id: Option<Value>, result: Value) -> Vec<u8> {
    let response = Response {
        jsonrpc: "2.0".to_string(),
        result: Some(result),
        error: None,
        id,
    };
    serde_json::to_vec(&response).unwrap_or_else(|e| {
        // Fallback: return a minimal error response if serialization fails
        tracing::error!("Failed to serialize success response: {}", e);
        r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error"},"id":null}"#
            .to_string()
            .into_bytes()
    })
}

pub fn error_response(id: Option<Value>, code: i32, message: &str) -> Vec<u8> {
    let response = Response {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(Error {
            code,
            message: message.to_string(),
            data: None,
        }),
        id,
    };
    serde_json::to_vec(&response).unwrap_or_else(|e| {
        // Fallback: return a minimal error response if serialization fails
        tracing::error!("Failed to serialize error response: {}", e);
        r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error"},"id":null}"#
            .to_string()
            .into_bytes()
    })
}

pub async fn handle_request(request: Request, config: &rtk_core::config::Config) -> Vec<u8> {
    use crate::handlers;
    
    // Validate JSON-RPC version (must be "2.0")
    if request.jsonrpc != "2.0" {
        return error_response(request.id, INVALID_REQUEST, "Invalid JSON-RPC version (must be \"2.0\")");
    }
    
    let result = match request.method.as_str() {
        "compress" => handlers::compress::handle(request.params, config).await,
        "health" => handlers::health::handle(request.params).await,
        "stats" => handlers::stats::handle(request.params).await,
        "shutdown" => handlers::shutdown::handle(request.params).await,
        _ => Err((METHOD_NOT_FOUND, "Method not found".to_string())),
    };
    
    match result {
        Ok(value) => success_response(request.id, value),
        Err((code, message)) => error_response(request.id, code, &message),
    }
}

// JSON-RPC error codes
#[allow(dead_code)]
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;
