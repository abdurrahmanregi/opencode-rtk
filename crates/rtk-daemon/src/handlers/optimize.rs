//! Handler for the `optimize` JSON-RPC method

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use rtk_core::commands::pre_execution::optimize_command;
use rtk_core::config::Config;

/// Request parameters for optimize method
#[derive(Debug, Deserialize)]
pub struct OptimizeParams {
    /// The command string to optimize
    pub command: String,
}

/// Response for optimize method
#[derive(Debug, Serialize, Deserialize)]
pub struct OptimizeResult {
    /// Original command string
    pub original: String,
    /// Optimized command string
    pub optimized: String,
    /// Flags that were added
    pub flags_added: Vec<String>,
    /// Whether optimization was skipped
    pub skipped: bool,
    /// Reason for skipping (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
}

/// Handle the optimize JSON-RPC method
///
/// # Arguments
///
/// * `params` - JSON-RPC request parameters
/// * `config` - Daemon configuration
///
/// # Returns
///
/// JSON result with optimization details
pub async fn handle(params: Value, config: &Config) -> Result<Value> {
    // Parse parameters
    let params: OptimizeParams =
        serde_json::from_value(params).map_err(|e| anyhow::anyhow!("Invalid parameters: {}", e))?;

    // Check if pre-execution is enabled
    if !config.general.enable_pre_execution_flags {
        return Ok(json!(OptimizeResult {
            original: params.command.clone(),
            optimized: params.command,
            flags_added: vec![],
            skipped: true,
            skip_reason: Some("pre-execution flags disabled in config".to_string()),
        }));
    }

    // Optimize command
    let result = optimize_command(&params.command)?;

    // Build response
    Ok(json!(OptimizeResult {
        original: result.original,
        optimized: result.optimized,
        flags_added: result.flags_added,
        skipped: result.skipped,
        skip_reason: result.skip_reason,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtk_core::config::{DaemonConfig, GeneralConfig, TeeConfig};

    fn test_config() -> Config {
        #[cfg(feature = "llm")]
        {
            Config {
                general: GeneralConfig {
                    enable_tracking: true,
                    database_path: ":memory:".to_string(),
                    retention_days: 90,
                    default_filter_level: "minimal".to_string(),
                    verbosity: 0,
                    enable_pre_execution_flags: true,
                    flag_mappings_path: None,
                },
                daemon: DaemonConfig::default(),
                tee: TeeConfig::default(),
                llm: rtk_core::config::LlmConfig::default(),
            }
        }
        #[cfg(not(feature = "llm"))]
        {
            Config {
                general: GeneralConfig {
                    enable_tracking: true,
                    database_path: ":memory:".to_string(),
                    retention_days: 90,
                    default_filter_level: "minimal".to_string(),
                    verbosity: 0,
                    enable_pre_execution_flags: true,
                    flag_mappings_path: None,
                },
                daemon: DaemonConfig::default(),
                tee: TeeConfig::default(),
            }
        }
    }

    #[tokio::test]
    async fn test_handle_optimize_git_status() {
        let config = test_config();
        let params = json!({ "command": "git status" });

        let result = handle(params, &config).await.unwrap();
        let result: OptimizeResult = serde_json::from_value(result).unwrap();

        assert_eq!(result.original, "git status");
        assert_eq!(result.optimized, "git status --porcelain -b");
        assert_eq!(result.flags_added, vec!["--porcelain", "-b"]);
        assert!(!result.skipped);
    }

    #[tokio::test]
    async fn test_handle_optimize_disabled() {
        let mut config = test_config();
        config.general.enable_pre_execution_flags = false;

        let params = json!({ "command": "git status" });
        let result = handle(params, &config).await.unwrap();
        let result: OptimizeResult = serde_json::from_value(result).unwrap();

        assert!(result.skipped);
        assert_eq!(result.original, result.optimized);
    }

    #[tokio::test]
    async fn test_handle_optimize_unknown_command() {
        let config = test_config();
        let params = json!({ "command": "unknown-command" });

        let result = handle(params, &config).await.unwrap();
        let result: OptimizeResult = serde_json::from_value(result).unwrap();

        assert!(result.skipped);
    }

    #[tokio::test]
    async fn test_handle_optimize_npm_test() {
        let config = test_config();
        let params = json!({ "command": "npm test" });

        let result = handle(params, &config).await.unwrap();
        let result: OptimizeResult = serde_json::from_value(result).unwrap();

        assert_eq!(result.optimized, "npm test --silent");
        assert!(!result.skipped);
    }

    #[tokio::test]
    async fn test_handle_optimize_cargo_build() {
        let config = test_config();
        let params = json!({ "command": "cargo build" });

        let result = handle(params, &config).await.unwrap();
        let result: OptimizeResult = serde_json::from_value(result).unwrap();

        assert_eq!(result.optimized, "cargo build --quiet");
        assert!(!result.skipped);
    }

    #[tokio::test]
    async fn test_handle_optimize_empty_command() {
        let config = test_config();
        let params = json!({ "command": "" });

        let result = handle(params, &config).await.unwrap();
        let result: OptimizeResult = serde_json::from_value(result).unwrap();

        assert!(result.skipped);
    }

    #[tokio::test]
    async fn test_handle_optimize_piped_command() {
        let config = test_config();
        let params = json!({ "command": "git status | grep modified" });

        let result = handle(params, &config).await.unwrap();
        let result: OptimizeResult = serde_json::from_value(result).unwrap();

        assert!(result.optimized.contains("--porcelain"));
        assert!(result.optimized.contains("| grep modified"));
    }
}
