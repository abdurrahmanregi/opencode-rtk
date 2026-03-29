#[cfg(feature = "llm")]
use super::llm::LlmCompressor;
use super::HandlerResult;
use crate::protocol::{INTERNAL_ERROR, INVALID_PARAMS};
use rtk_core::config::{CompressionAggressiveness, ModelCategory, PostExecutionPolicyMode};
use rtk_core::{compress, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, warn};

/// Maximum characters to keep in fallback compression
const FALLBACK_MAX_CHARS: usize = 1000;
const LOW_MIN_SAVINGS_PCT: f64 = 10.0;
const MEDIUM_MIN_SAVINGS_PCT: f64 = 5.0;
const HIGH_MIN_SAVINGS_PCT: f64 = 1.0;

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
    #[serde(default)]
    model_id: Option<String>,
    #[serde(default)]
    model_category: Option<String>,
    #[serde(default)]
    policy_mode: Option<String>,
    #[serde(default)]
    compression_aggressiveness: Option<String>,
    #[serde(default)]
    strip_reasoning: Option<bool>,
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
    replace_recommended: bool,
}

#[derive(Debug, Clone)]
struct EffectiveModelPolicy {
    model_id: String,
    model_category: ModelCategory,
    policy_mode: PostExecutionPolicyMode,
    compression_aggressiveness: CompressionAggressiveness,
    strip_reasoning: bool,
}

fn parse_model_category(raw: Option<&str>) -> Option<ModelCategory> {
    match raw?.trim().to_lowercase().as_str() {
        "reasoning" => Some(ModelCategory::Reasoning),
        "instruct" => Some(ModelCategory::Instruct),
        "compact" => Some(ModelCategory::Compact),
        _ => None,
    }
}

fn parse_policy_mode(raw: Option<&str>) -> Option<PostExecutionPolicyMode> {
    match raw?.trim().to_lowercase().as_str() {
        "off" => Some(PostExecutionPolicyMode::Off),
        "metadata_only" => Some(PostExecutionPolicyMode::MetadataOnly),
        "replace" | "replace_output" => Some(PostExecutionPolicyMode::ReplaceOutput),
        _ => None,
    }
}

fn parse_aggressiveness(raw: Option<&str>) -> Option<CompressionAggressiveness> {
    match raw?.trim().to_lowercase().as_str() {
        "low" => Some(CompressionAggressiveness::Low),
        "medium" => Some(CompressionAggressiveness::Medium),
        "high" => Some(CompressionAggressiveness::High),
        _ => None,
    }
}

fn resolve_effective_policy(
    context: &CompressContext,
    config: &rtk_core::config::Config,
) -> EffectiveModelPolicy {
    #[cfg(feature = "llm")]
    let default_model = config.llm.openrouter.model.clone();
    #[cfg(not(feature = "llm"))]
    let default_model = "unknown".to_string();

    let model_id = context
        .model_id
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(default_model);

    #[cfg(feature = "llm")]
    let config_policy = config.llm.resolve_model_policy(&model_id);
    #[cfg(not(feature = "llm"))]
    let config_policy: Option<rtk_core::config::ModelRuntimePolicy> = None;

    let model_category = parse_model_category(context.model_category.as_deref())
        .or_else(|| config_policy.as_ref().map(|p| p.category))
        .unwrap_or(ModelCategory::Instruct);

    let policy_mode = parse_policy_mode(context.policy_mode.as_deref())
        .or_else(|| config_policy.as_ref().map(|p| p.policy_mode))
        .unwrap_or(PostExecutionPolicyMode::MetadataOnly);

    let compression_aggressiveness =
        parse_aggressiveness(context.compression_aggressiveness.as_deref())
            .or_else(|| config_policy.as_ref().map(|p| p.compression_aggressiveness))
            .unwrap_or_else(|| match model_category {
                ModelCategory::Reasoning => CompressionAggressiveness::Low,
                ModelCategory::Compact => CompressionAggressiveness::Medium,
                ModelCategory::Instruct => CompressionAggressiveness::High,
            });

    let strip_reasoning = context
        .strip_reasoning
        .or_else(|| config_policy.as_ref().map(|p| p.strip_reasoning))
        .unwrap_or(true);

    EffectiveModelPolicy {
        model_id,
        model_category,
        policy_mode,
        compression_aggressiveness,
        strip_reasoning,
    }
}

fn min_savings_pct(aggressiveness: CompressionAggressiveness) -> f64 {
    match aggressiveness {
        CompressionAggressiveness::Low => LOW_MIN_SAVINGS_PCT,
        CompressionAggressiveness::Medium => MEDIUM_MIN_SAVINGS_PCT,
        CompressionAggressiveness::High => HIGH_MIN_SAVINGS_PCT,
    }
}

fn max_output_size_bytes(aggressiveness: CompressionAggressiveness) -> usize {
    match aggressiveness {
        CompressionAggressiveness::Low => 2_000_000,
        CompressionAggressiveness::Medium => 1_250_000,
        CompressionAggressiveness::High => 800_000,
    }
}

fn should_recommend_replacement(
    policy_mode: PostExecutionPolicyMode,
    savings_pct: f64,
    aggressiveness: CompressionAggressiveness,
    compressed_output: &str,
) -> bool {
    if policy_mode != PostExecutionPolicyMode::ReplaceOutput {
        return false;
    }

    // Never replace with an empty string, even if "savings" are 100%
    if compressed_output.trim().is_empty() {
        return false;
    }

    savings_pct >= min_savings_pct(aggressiveness)
}

pub async fn handle(params: Value, _config: &rtk_core::config::Config) -> HandlerResult {
    let params: CompressParams = serde_json::from_value(params)
        .map_err(|e| (INVALID_PARAMS, format!("Invalid parameters: {}", e)))?;

    let effective_policy = resolve_effective_policy(&params.context, _config);
    debug!(
        "Compression policy: model='{}' category={:?} mode={:?} aggressiveness={:?} strip_reasoning={}",
        effective_policy.model_id,
        effective_policy.model_category,
        effective_policy.policy_mode,
        effective_policy.compression_aggressiveness,
        effective_policy.strip_reasoning
    );

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

    let min_savings = min_savings_pct(effective_policy.compression_aggressiveness);
    // If no module matched and LLM is available, try LLM compression
    if result.module == "unknown" {
        debug!("No module matched, checking LLM fallback");

        if params.output.len() > max_output_size_bytes(effective_policy.compression_aggressiveness)
        {
            debug!(
                "Output exceeds model-aware threshold ({} bytes), using fallback",
                params.output.len()
            );
            result = apply_fallback(&params.output, result.original_tokens);
        } else {
            #[cfg(feature = "llm")]
            {
                debug!(
                    "LLM config: enabled={}, backend={}",
                    _config.llm.enabled, _config.llm.backend
                );
                let llm_compressor = LlmCompressor::new(_config.llm.clone());

                match llm_compressor {
                    Ok(compressor) => {
                        debug!(
                            "LLM compressor initialized, available={}",
                            compressor.is_available()
                        );
                        if compressor.is_available() {
                            debug!("LLM compression available, attempting compression");
                            match compressor
                                .compress(
                                    &params.command,
                                    &params.output,
                                    context.exit_code,
                                    effective_policy.strip_reasoning,
                                )
                                .await
                            {
                                Ok(llm_compressed) => {
                                    debug!("LLM returned {} chars", llm_compressed.len());
                                    let llm_tokens = rtk_core::estimate_tokens(&llm_compressed);
                                    let llm_saved =
                                        result.original_tokens.saturating_sub(llm_tokens);
                                    let llm_savings = if result.original_tokens > 0 {
                                        (llm_saved as f64 / result.original_tokens as f64) * 100.0
                                    } else {
                                        0.0
                                    };

                                    if llm_saved > 0 {
                                        debug!(
                                            "LLM compression succeeded: saved {} tokens",
                                            llm_saved
                                        );
                                        result.compressed = llm_compressed;
                                        result.compressed_tokens = llm_tokens;
                                        result.saved_tokens = llm_saved;
                                        result.savings_pct = llm_savings;
                                        result.strategy = "llm".to_string();
                                        result.module = "llm".to_string();
                                    } else {
                                        debug!("LLM compression provided no savings ({} orig, {} compressed), using fallback", result.original_tokens, llm_tokens);
                                        result =
                                            apply_fallback(&params.output, result.original_tokens);
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
    }

    let replace_recommended = should_recommend_replacement(
        effective_policy.policy_mode,
        result.savings_pct,
        effective_policy.compression_aggressiveness,
        &result.compressed,
    );
    if effective_policy.policy_mode == PostExecutionPolicyMode::ReplaceOutput
        && result.saved_tokens > 0
        && !replace_recommended
    {
        debug!(
            "Replacement not recommended: savings {:.2}% below threshold {:.2}%",
            result.savings_pct, min_savings
        );
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
        replace_recommended,
    };

    serde_json::to_value(response)
        .map_err(|e| (INTERNAL_ERROR, format!("Serialization failed: {}", e)))
}

/// Apply fallback compression for unknown commands
fn apply_fallback(output: &str, original_tokens: usize) -> rtk_core::CompressedOutput {
    // Truncate to FALLBACK_MAX_CHARS characters if longer
    let truncated = if output.len() > FALLBACK_MAX_CHARS {
        // Find safe UTF-8 boundary
        let safe_end = output
            .char_indices()
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

    #[test]
    fn test_parse_policy_fields() {
        assert_eq!(
            parse_model_category(Some("Reasoning")),
            Some(ModelCategory::Reasoning)
        );
        assert_eq!(
            parse_policy_mode(Some("replace_output")),
            Some(PostExecutionPolicyMode::ReplaceOutput)
        );
        assert_eq!(
            parse_aggressiveness(Some("HIGH")),
            Some(CompressionAggressiveness::High)
        );
    }

    #[test]
    fn test_should_recommend_replacement_thresholds() {
        assert!(!should_recommend_replacement(
            PostExecutionPolicyMode::MetadataOnly,
            99.0,
            CompressionAggressiveness::High,
            "compressed",
        ));
        assert!(!should_recommend_replacement(
            PostExecutionPolicyMode::ReplaceOutput,
            4.0,
            CompressionAggressiveness::Medium,
            "compressed",
        ));
        assert!(should_recommend_replacement(
            PostExecutionPolicyMode::ReplaceOutput,
            6.0,
            CompressionAggressiveness::Medium,
            "compressed",
        ));
    }
}
