#[cfg(feature = "llm")]
use super::llm::LlmCompressor;
use super::HandlerResult;
use crate::protocol::{INTERNAL_ERROR, INVALID_PARAMS};
use rtk_core::{compress, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, warn};

/// Maximum characters to keep in fallback compression
const FALLBACK_MAX_CHARS: usize = 1000;

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

    // Try standard compression first
    let mut result = compress(&params.command, &params.output, context.clone())
        .map_err(|e| (INTERNAL_ERROR, format!("Compression failed: {}", e)))?;

    // If no module matched and LLM is available, try LLM compression
    if result.module == "unknown" {
        debug!("No module matched, checking LLM fallback");

        #[cfg(feature = "llm")]
        {
            debug!("LLM config: enabled={}, backend={}", _config.llm.enabled, _config.llm.backend);
            let llm_compressor = LlmCompressor::new(_config.llm.clone());

            match llm_compressor {
                Ok(compressor) => {
                    debug!("LLM compressor initialized, available={}", compressor.is_available());
                    if compressor.is_available() {
                        debug!("LLM compression available, attempting compression");
                        match compressor
                            .compress(&params.command, &params.output, context.exit_code)
                            .await
                        {
                            Ok(llm_compressed) => {
                                debug!("LLM returned {} chars", llm_compressed.len());
                                let llm_tokens = rtk_core::estimate_tokens(&llm_compressed);
                                let llm_saved = result.original_tokens.saturating_sub(llm_tokens);
                                let llm_savings = if result.original_tokens > 0 {
                                    (llm_saved as f64 / result.original_tokens as f64) * 100.0
                                } else {
                                    0.0
                                };

                                if llm_saved > 0 {
                                    debug!("LLM compression succeeded: saved {} tokens", llm_saved);
                                    result.compressed = llm_compressed;
                                    result.compressed_tokens = llm_tokens;
                                    result.saved_tokens = llm_saved;
                                    result.savings_pct = llm_savings;
                                    result.strategy = "llm".to_string();
                                    result.module = "llm".to_string();
                                } else {
                                    debug!("LLM compression provided no savings ({} orig, {} compressed), using fallback", result.original_tokens, llm_tokens);
                                    result = apply_fallback(&params.output, result.original_tokens);
                                }
                            }
                            Err(e) => {
                                warn!("LLM compression failed: {}, using fallback", e);
                                result = apply_fallback(&params.output, result.original_tokens);
                            }
                        }
                    } else {
                        warn!("LLM compression not available (enabled={}, has_api_key={}), using fallback", 
                              _config.llm.enabled, compressor.has_api_key());
                        result = apply_fallback(&params.output, result.original_tokens);
                    }
                }
                Err(e) => {
                    warn!("Failed to initialize LLM compressor: {}, using fallback", e);
                    result = apply_fallback(&params.output, result.original_tokens);
                }
            }
        }

        #[cfg(not(feature = "llm"))]
        {
            debug!("LLM feature not enabled, using fallback");
            result = apply_fallback(&params.output, result.original_tokens);
        }
    }

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

/// Apply fallback compression for unknown commands
fn apply_fallback(output: &str, original_tokens: usize) -> rtk_core::CompressedOutput {
    // Truncate to FALLBACK_MAX_CHARS characters if longer
    let truncated = if output.len() > FALLBACK_MAX_CHARS {
        // Find safe UTF-8 boundary
        let safe_end = output.char_indices()
            .take_while(|(idx, _)| *idx < FALLBACK_MAX_CHARS)
            .last()
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0);

        // Reserve space for suffix
        let suffix = "...\n[TRUNCATED]";
        let truncate_to = safe_end.saturating_sub(suffix.len()).min(safe_end);

        format!("{}{}", &output[..truncate_to], suffix)
    } else {
        output.to_string()
    };

    // Try to extract errors
    let compressed = if let Some(errors) = extract_errors(&truncated) {
        errors
    } else {
        truncated
    };

    let compressed_tokens = rtk_core::estimate_tokens(&compressed);
    let saved_tokens = original_tokens.saturating_sub(compressed_tokens);
    let savings_pct = if original_tokens > 0 {
        (saved_tokens as f64 / original_tokens as f64) * 100.0
    } else {
        0.0
    };

    rtk_core::CompressedOutput {
        compressed,
        original_tokens,
        compressed_tokens,
        saved_tokens,
        savings_pct,
        strategy: "fallback".to_string(),
        module: "fallback".to_string(),
    }
}

/// Extract errors from output
fn extract_errors(output: &str) -> Option<String> {
    let lines: Vec<&str> = output.lines().collect();
    let errors: Vec<String> = lines
        .into_iter()
        .filter(|line| {
            line.to_lowercase().contains("error")
                || line.to_lowercase().contains("failed")
                || line.to_lowercase().contains("warning")
                || line.to_lowercase().contains("exception")
        })
        .map(|s| s.to_string())
        .collect();

    if errors.is_empty() {
        None
    } else {
        Some(errors.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtk_core::config::Config;

    fn make_config(enable_tracking: bool) -> Config {
        let mut config = Config::default();
        config.general.enable_tracking = enable_tracking;
        config.general.database_path = ":memory:".to_string();
        config.daemon.socket_path = "/tmp/test.sock".to_string();
        config.daemon.max_connections = 10;
        config.daemon.auto_restart = false;
        config
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

    #[tokio::test]
    async fn test_compress_unknown_command_with_fallback() {
        let config = make_config(false);
        let params = make_params(
            "unknown_cmd test",
            "output line 1\noutput line 2\nError: something failed\n",
            None,
        );

        let result = handle(params, &config).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        // Should use fallback for unknown commands
        assert!(value.get("compressed").is_some());
    }

    #[test]
    fn test_extract_errors() {
        let output = "Starting build...\nError: compilation failed\nWarning: unused variable\nBuild complete";
        let errors = extract_errors(output);
        assert!(errors.is_some());
        assert!(errors.unwrap().contains("Error: compilation failed"));
    }

    #[test]
    fn test_extract_errors_none() {
        let output = "Starting build...\nBuild complete";
        let errors = extract_errors(output);
        assert!(errors.is_none());
    }
}
