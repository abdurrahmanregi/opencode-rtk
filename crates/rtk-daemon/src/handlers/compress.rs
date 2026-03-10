use super::HandlerResult;
use crate::protocol::{INVALID_PARAMS, INTERNAL_ERROR};
use rtk_core::{compress, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

#[derive(Debug, Deserialize)]
struct CompressParams {
    command: String,
    output: String,
    #[serde(default)]
    context: CompressContext,
}

#[derive(Debug, Deserialize, Default)]
struct CompressContext {
    #[serde(default)]
    cwd: String,
    #[serde(default)]
    exit_code: i32,
    #[serde(default = "default_tool")]
    tool: String,
    #[serde(default)]
    session_id: Option<String>,
}

fn default_tool() -> String {
    "bash".to_string()
}

#[derive(Debug, Serialize)]
struct CompressResult {
    compressed: String,
    original_tokens: usize,
    compressed_tokens: usize,
    saved_tokens: usize,
    savings_pct: f64,
    strategy: String,
    module: String,
}

pub async fn handle(params: Value, _config: &rtk_core::config::Config) -> HandlerResult {
    let params: CompressParams = serde_json::from_value(params)
        .map_err(|e| (INVALID_PARAMS, format!("Invalid parameters: {}", e)))?;
    
    let context = Context {
        cwd: params.context.cwd,
        exit_code: params.context.exit_code,
        tool: params.context.tool,
        session_id: params.context.session_id,
        command: Some(params.command.clone()),
    };
    
    let result = compress(&params.command, &params.output, context.clone())
        .map_err(|e| (INTERNAL_ERROR, format!("Compression failed: {}", e)))?;
    
    // Track if enabled
    if let Some(session_id) = &context.session_id {
        if _config.general.enable_tracking {
            if let Err(e) = rtk_core::tracking::track(rtk_core::tracking::TrackRequest {
                session_id,
                command: &params.command,
                tool: &context.tool,
                cwd: &context.cwd,
                exit_code: context.exit_code,
                original: &params.output,
                compressed: &result.compressed,
                strategy: &result.strategy,
                module: &result.module,
                exec_time_ms: 0, // exec_time_ms - would need to measure this
            }) {
                warn!("Failed to track compression: {}", e);
            }
        }
    }
    
    let response = CompressResult {
        compressed: result.compressed,
        original_tokens: result.original_tokens,
        compressed_tokens: result.compressed_tokens,
        saved_tokens: result.saved_tokens,
        savings_pct: result.savings_pct,
        strategy: result.strategy,
        module: result.module,
    };
    
    serde_json::to_value(response)
        .map_err(|e| (INTERNAL_ERROR, format!("Serialization failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtk_core::config::{Config, DaemonConfig, GeneralConfig};

    fn make_config(enable_tracking: bool) -> Config {
        Config {
            general: GeneralConfig {
                enable_tracking,
                database_path: ":memory:".to_string(),
                retention_days: 90,
                default_filter_level: "minimal".to_string(),
                verbosity: 0,
            },
            daemon: DaemonConfig {
                socket_path: "/tmp/test.sock".to_string(),
                max_connections: 10,
                timeout_seconds: 5,
                auto_restart: false,
                tcp_address: None,
            },
        }
    }

    fn make_params(command: &str, output: &str, session_id: Option<&str>) -> Value {
        serde_json::json!({
            "command": command,
            "output": output,
            "context": {
                "cwd": "/tmp",
                "exit_code": 0,
                "tool": "bash",
                "session_id": session_id
            }
        })
    }

    #[tokio::test]
    async fn test_compress_basic() {
        let config = make_config(false);
        let params = make_params("echo hello", "hello\n", None);

        let result = handle(params, &config).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("compressed").is_some());
    }

    #[tokio::test]
    async fn test_compress_with_tracking_enabled() {
        let config = make_config(true);
        let params = make_params("git status", "M file.rs\n", Some("test-session"));

        // Should not fail even if tracking encounters an error
        let result = handle(params, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_compress_tracking_error_does_not_affect_response() {
        let config = make_config(true);
        // Use a session ID that might cause tracking issues
        let params = make_params("git status", "M file.rs\n", Some(""));

        // Compression should still succeed even if tracking fails
        let result = handle(params, &config).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value.get("compressed").is_some());
    }

    #[tokio::test]
    async fn test_compress_without_session_id() {
        let config = make_config(true);
        let params = make_params("echo test", "test\n", None);

        // Should work fine without session ID (no tracking attempted)
        let result = handle(params, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_compress_tracking_disabled() {
        let config = make_config(false);
        let params = make_params("git status", "M file.rs\n", Some("test-session"));

        // Should work fine with tracking disabled
        let result = handle(params, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_compress_invalid_params() {
        let config = make_config(false);
        let params = serde_json::json!({
            "invalid": "params"
        });

        let result = handle(params, &config).await;
        assert!(result.is_err());
        let (code, _) = result.unwrap_err();
        assert_eq!(code, INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_compress_empty_output() {
        let config = make_config(false);
        let params = make_params("echo", "", None);

        let result = handle(params, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_compress_large_output() {
        let config = make_config(false);
        let large_output: String = "line\n".repeat(1000);
        let params = make_params("cat large_file", &large_output, None);

        let result = handle(params, &config).await;
        assert!(result.is_ok());
    }
}
