pub mod settings;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub daemon: DaemonConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub enable_tracking: bool,
    pub database_path: String,
    pub retention_days: u32,
    pub default_filter_level: String,
    pub verbosity: u8,
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
            },
            daemon: DaemonConfig {
                socket_path: "/tmp/opencode-rtk.sock".to_string(),
                max_connections: 100,
                timeout_seconds: 5,
                auto_restart: true,
                tcp_address: None, // Will use default 127.0.0.1:9876 on Windows
            },
        }
    }
}
