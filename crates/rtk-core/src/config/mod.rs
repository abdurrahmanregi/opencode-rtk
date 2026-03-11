pub mod settings;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub tee: TeeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub enable_tracking: bool,
    pub database_path: String,
    pub retention_days: u32,
    pub default_filter_level: String,
    pub verbosity: u8,
    /// Enable pre-execution flag optimization
    #[serde(default = "default_enable_pre_execution_flags")]
    pub enable_pre_execution_flags: bool,
    /// Custom flag mappings file path (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flag_mappings_path: Option<String>,
}

fn default_enable_pre_execution_flags() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub socket_path: String,
    pub max_connections: u32,
    pub timeout_seconds: u64,
    pub auto_restart: bool,
    /// TCP address for Windows (since Unix sockets aren't available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp_address: Option<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_path: "/tmp/opencode-rtk.sock".to_string(),
            max_connections: 100,
            timeout_seconds: 5,
            auto_restart: true,
            tcp_address: None,
        }
    }
}

/// Tee mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeConfig {
    /// Enable tee mode
    #[serde(default = "default_tee_enabled")]
    pub enabled: bool,

    /// Tee mode: "failures", "always", "never"
    #[serde(default = "default_tee_mode")]
    pub mode: String,

    /// Maximum number of tee files to keep
    #[serde(default = "default_tee_max_files")]
    pub max_files: usize,

    /// Days to retain tee files
    #[serde(default = "default_tee_retention_days")]
    pub retention_days: u32,

    /// Directory for tee files
    #[serde(default = "default_tee_directory")]
    pub directory: String,
}

fn default_tee_enabled() -> bool {
    true
}

fn default_tee_mode() -> String {
    "failures".to_string()
}

fn default_tee_max_files() -> usize {
    20
}

fn default_tee_retention_days() -> u32 {
    90
}

fn default_tee_directory() -> String {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("opencode-rtk");
    base.join("tee").to_string_lossy().to_string()
}

impl Default for TeeConfig {
    fn default() -> Self {
        Self {
            enabled: default_tee_enabled(),
            mode: default_tee_mode(),
            max_files: default_tee_max_files(),
            retention_days: default_tee_retention_days(),
            directory: default_tee_directory(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            general: GeneralConfig {
                enable_tracking: true,
                database_path: dirs::data_local_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("opencode-rtk")
                    .join("history.db")
                    .to_string_lossy()
                    .to_string(),
                retention_days: 90,
                default_filter_level: "minimal".to_string(),
                verbosity: 0,
                enable_pre_execution_flags: true,
                flag_mappings_path: None,
            },
            daemon: DaemonConfig {
                socket_path: "/tmp/opencode-rtk.sock".to_string(),
                max_connections: 100,
                timeout_seconds: 5,
                auto_restart: true,
                tcp_address: None, // Will use default 127.0.0.1:9876 on Windows
            },
            tee: TeeConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.general.enable_tracking);
        assert!(config.general.enable_pre_execution_flags);
        assert!(config.tee.enabled);
        assert_eq!(config.tee.mode, "failures");
    }

    #[test]
    fn test_tee_config_default() {
        let tee = TeeConfig::default();
        assert!(tee.enabled);
        assert_eq!(tee.mode, "failures");
        assert_eq!(tee.max_files, 20);
        assert_eq!(tee.retention_days, 90);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("enable_pre_execution_flags"));
        assert!(json.contains("tee"));
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{
            "general": {
                "enable_tracking": true,
                "database_path": "/tmp/test.db",
                "retention_days": 30,
                "default_filter_level": "minimal",
                "verbosity": 0
            },
            "daemon": {
                "socket_path": "/tmp/test.sock",
                "max_connections": 50,
                "timeout_seconds": 10,
                "auto_restart": false
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert!(config.general.enable_pre_execution_flags); // Should use default
        assert!(config.tee.enabled); // Should use default
    }

    #[test]
    fn test_tee_config_custom() {
        let json = r#"{
            "enabled": false,
            "mode": "always",
            "max_files": 50,
            "retention_days": 30,
            "directory": "/custom/tee"
        }"#;

        let tee: TeeConfig = serde_json::from_str(json).unwrap();
        assert!(!tee.enabled);
        assert_eq!(tee.mode, "always");
        assert_eq!(tee.max_files, 50);
    }
}
