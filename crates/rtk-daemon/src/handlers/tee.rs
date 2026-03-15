//! Handlers for tee-related JSON-RPC methods

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

use rtk_core::config::Config;
use rtk_core::tee::TeeManager;

/// Request for tee_save method
#[derive(Debug, Deserialize)]
pub struct TeeSaveParams {
    pub command: String,
    pub output: String,
}

/// Response for tee_save method
#[derive(Debug, Serialize, Deserialize)]
pub struct TeeSaveResult {
    pub path: String,
    pub size: usize,
}

/// Response for tee_list method
#[derive(Debug, Serialize, Deserialize)]
pub struct TeeListResult {
    pub files: Vec<TeeFileInfo>,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeeFileInfo {
    pub path: String,
    pub command: String,
    pub timestamp: String,
    pub size: usize,
}

/// Response for tee_read method
#[derive(Debug, Serialize, Deserialize)]
pub struct TeeReadResult {
    pub content: String,
    pub size: usize,
}

/// Handle tee_save method
pub async fn handle_save(params: Value, config: &Config) -> Result<Value> {
    if !config.tee.enabled {
        return Ok(json!({
            "error": "Tee mode is disabled in config"
        }));
    }

    let params: TeeSaveParams = serde_json::from_value(params)?;

    let manager = TeeManager::new(
        PathBuf::from(&config.tee.directory),
        config.tee.max_files,
        config.tee.retention_days,
    );

    let path = manager.save(&params.command, &params.output)?;
    let size = params.output.len();

    Ok(json!(TeeSaveResult {
        path: path.to_string_lossy().to_string(),
        size,
    }))
}

/// Handle tee_list method
pub async fn handle_list(_params: Value, config: &Config) -> Result<Value> {
    let manager = TeeManager::new(
        PathBuf::from(&config.tee.directory),
        config.tee.max_files,
        config.tee.retention_days,
    );

    let entries = manager.list()?;
    let total = entries.len();

    let files: Vec<TeeFileInfo> = entries
        .into_iter()
        .map(|e| TeeFileInfo {
            path: e.path.to_string_lossy().to_string(),
            command: e.command,
            timestamp: e.timestamp.to_rfc3339(),
            size: e.size,
        })
        .collect();

    Ok(json!(TeeListResult { files, total }))
}

/// Handle tee_read method
#[derive(Debug, Deserialize)]
pub struct TeeReadParams {
    pub path: String,
}

pub async fn handle_read(params: Value, config: &Config) -> Result<Value> {
    let params: TeeReadParams = serde_json::from_value(params)?;

    let tee_dir = PathBuf::from(&config.tee.directory);

    // Create tee directory if it doesn't exist for canonicalization
    if !tee_dir.exists() {
        std::fs::create_dir_all(&tee_dir)
            .with_context(|| format!("Failed to create tee directory: {:?}", tee_dir))?;
    }

    let tee_dir_canonical = tee_dir
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize tee directory: {:?}", tee_dir))?;

    let requested_path = PathBuf::from(&params.path);

    // Only canonicalize if the path exists
    let requested_canonical = if requested_path.exists() {
        requested_path.canonicalize().with_context(|| {
            format!(
                "Failed to canonicalize requested path: {:?}",
                requested_path
            )
        })?
    } else {
        requested_path
    };

    // Security check: ensure path is within tee directory
    if !requested_canonical.starts_with(&tee_dir_canonical) {
        return Err(anyhow::anyhow!(
            "Path traversal attempt detected: path outside tee directory"
        ));
    }

    let manager = TeeManager::new(
        tee_dir_canonical,
        config.tee.max_files,
        config.tee.retention_days,
    );

    let content = manager.read(&requested_canonical)?;
    let size = content.len();

    Ok(json!(TeeReadResult { content, size }))
}

/// Handle tee_clear method
pub async fn handle_clear(_params: Value, config: &Config) -> Result<Value> {
    let manager = TeeManager::new(
        PathBuf::from(&config.tee.directory),
        config.tee.max_files,
        config.tee.retention_days,
    );

    let count = manager.clear()?;

    Ok(json!({ "deleted": count }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtk_core::config::{DaemonConfig, GeneralConfig, TeeConfig};
    use tempfile::tempdir;

    fn test_config_with_dir(dir: &std::path::Path) -> Config {
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
                tee: TeeConfig {
                    enabled: true,
                    mode: "failures".to_string(),
                    max_files: 10,
                    retention_days: 90,
                    directory: dir.to_string_lossy().to_string(),
                },
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
                tee: TeeConfig {
                    enabled: true,
                    mode: "failures".to_string(),
                    max_files: 10,
                    retention_days: 90,
                    directory: dir.to_string_lossy().to_string(),
                },
            }
        }
    }

    #[tokio::test]
    async fn test_handle_save() {
        let dir = tempdir().unwrap();
        let config = test_config_with_dir(dir.path());
        let params = json!({
            "command": "git status",
            "output": "M file.rs\nA file.ts"
        });

        let result = handle_save(params, &config).await.unwrap();
        let result: TeeSaveResult = serde_json::from_value(result).unwrap();

        assert!(result.path.ends_with(".log"));
        assert!(result.size > 0);
    }

    #[tokio::test]
    async fn test_handle_save_disabled() {
        let dir = tempdir().unwrap();
        let mut config = test_config_with_dir(dir.path());
        config.tee.enabled = false;

        let params = json!({
            "command": "git status",
            "output": "output"
        });

        let result = handle_save(params, &config).await.unwrap();
        assert!(result.get("error").is_some());
    }

    #[tokio::test]
    async fn test_handle_list() {
        let dir = tempdir().unwrap();
        let config = test_config_with_dir(dir.path());

        // Save a file first
        let save_params = json!({
            "command": "git status",
            "output": "output"
        });
        handle_save(save_params, &config).await.unwrap();

        let result = handle_list(json!({}), &config).await.unwrap();
        let result: TeeListResult = serde_json::from_value(result).unwrap();

        assert_eq!(result.total, 1);
        assert_eq!(result.files.len(), 1);
    }

    #[tokio::test]
    async fn test_handle_read() {
        let dir = tempdir().unwrap();
        let config = test_config_with_dir(dir.path());

        // Save a file first
        let save_params = json!({
            "command": "git status",
            "output": "test output content"
        });
        let save_result = handle_save(save_params, &config).await.unwrap();
        let save_result: TeeSaveResult = serde_json::from_value(save_result).unwrap();

        // Read it back
        let read_params = json!({ "path": save_result.path });
        let result = handle_read(read_params, &config).await.unwrap();
        let result: TeeReadResult = serde_json::from_value(result).unwrap();

        assert!(result.content.contains("test output content"));
    }

    #[tokio::test]
    async fn test_handle_clear() {
        let dir = tempdir().unwrap();
        let config = test_config_with_dir(dir.path());

        // Save some files
        for i in 0..3 {
            let save_params = json!({
                "command": format!("cmd{}", i),
                "output": format!("output{}", i)
            });
            handle_save(save_params, &config).await.unwrap();
        }

        let result = handle_clear(json!({}), &config).await.unwrap();
        assert_eq!(result.get("deleted").unwrap().as_u64().unwrap(), 3);

        // Verify empty
        let list_result = handle_list(json!({}), &config).await.unwrap();
        let list_result: TeeListResult = serde_json::from_value(list_result).unwrap();
        assert_eq!(list_result.total, 0);
    }
}
