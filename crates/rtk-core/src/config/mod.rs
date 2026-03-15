pub mod settings;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub tee: TeeConfig,
    #[cfg(feature = "llm")]
    #[serde(default)]
    pub llm: LlmConfig,
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
    false
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

/// LLM compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Enable LLM compression for unknown commands
    #[serde(default)]
    pub enabled: bool,

    /// Backend: "none" (disabled), "openrouter" (only option for now)
    #[serde(default = "default_llm_backend")]
    pub backend: String,

    /// Strategy: "fallback-only" (only used when regex doesn't match)
    #[serde(default = "default_llm_strategy")]
    pub strategy: String,

    /// Provider preference order (e.g., ["groq", "together"])
    #[serde(default)]
    pub provider_preference: Vec<String>,

    /// Request timeout in milliseconds
    #[serde(default = "default_llm_timeout_ms")]
    pub timeout_ms: u64,

    /// Temperature for generation
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// OpenRouter specific settings
    #[serde(default)]
    pub openrouter: OpenRouterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    /// Environment variable name for API key
    #[serde(default = "default_api_key_env")]
    pub api_key_env: String,

    /// File path for API key (alternative to env var)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_file: Option<String>,

    /// Model to use
    #[serde(default = "default_model")]
    pub model: String,

    /// Base URL
    #[serde(default = "default_openrouter_url")]
    pub base_url: String,

    /// Reasoning effort level
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: String,

    /// Maximum input tokens
    #[serde(default = "default_max_input_tokens")]
    pub max_input_tokens: usize,

    /// Maximum output tokens
    #[serde(default = "default_max_output_tokens")]
    pub max_output_tokens: usize,
}

// Default functions
fn default_llm_backend() -> String {
    "openrouter".into()
}

fn default_llm_strategy() -> String {
    "fallback-only".into()
}
fn default_llm_timeout_ms() -> u64 {
    2000
}
fn default_temperature() -> f32 {
    0.3
}
fn default_api_key_env() -> String {
    "OPENROUTER_API_KEY".into()
}
fn default_model() -> String {
    std::env::var("OPENROUTER_MODEL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "openai/gpt-oss-20b".into())
}
fn default_openrouter_url() -> String {
    "https://openrouter.ai/api/v1".into()
}
fn default_reasoning_effort() -> String {
    "low".into()
}
fn default_max_input_tokens() -> usize {
    2000
}
fn default_max_output_tokens() -> usize {
    100
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: default_llm_backend(),
            strategy: default_llm_strategy(),
            provider_preference: vec!["groq".into(), "together".into()],
            timeout_ms: default_llm_timeout_ms(),
            temperature: default_temperature(),
            openrouter: OpenRouterConfig::default(),
        }
    }
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            api_key_env: default_api_key_env(),
            api_key_file: None,
            model: default_model(),
            base_url: default_openrouter_url(),
            reasoning_effort: default_reasoning_effort(),
            max_input_tokens: default_max_input_tokens(),
            max_output_tokens: default_max_output_tokens(),
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
                enable_pre_execution_flags: false,
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
            #[cfg(feature = "llm")]
            llm: LlmConfig::default(),
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
        assert!(!config.general.enable_pre_execution_flags);
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
        #[cfg(feature = "llm")]
        assert!(json.contains("llm"));
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
        assert!(!config.general.enable_pre_execution_flags); // Should use default
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

    #[test]
    fn test_llm_config_default() {
        let llm = LlmConfig::default();
        assert!(llm.enabled);
        assert_eq!(llm.backend, "openrouter");
        assert_eq!(llm.strategy, "fallback-only");
        assert_eq!(llm.timeout_ms, 2000);
        assert_eq!(llm.temperature, 0.3);
        assert_eq!(llm.provider_preference, vec!["groq", "together"]);
    }

    #[test]
    fn test_openrouter_config_default() {
        let openrouter = OpenRouterConfig::default();
        assert_eq!(openrouter.api_key_env, "OPENROUTER_API_KEY");
        let expected_model = std::env::var("OPENROUTER_MODEL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "openai/gpt-oss-20b".into());
        assert_eq!(openrouter.model, expected_model);
        assert_eq!(openrouter.base_url, "https://openrouter.ai/api/v1");
        assert_eq!(openrouter.reasoning_effort, "low");
        assert_eq!(openrouter.max_input_tokens, 2000);
        assert_eq!(openrouter.max_output_tokens, 100);
        assert!(openrouter.api_key_file.is_none());
    }

    #[cfg(feature = "llm")]
    #[test]
    fn test_config_includes_llm() {
        let config = Config::default();
        assert!(config.llm.enabled);
        assert_eq!(config.llm.backend, "openrouter");
    }

    #[cfg(feature = "llm")]
    #[test]
    fn test_config_serialization_includes_llm() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("llm"));
        assert!(json.contains("openrouter"));
    }

    #[test]
    fn test_llm_config_custom() {
        let json = r#"{
            "enabled": true,
            "backend": "openrouter",
            "strategy": "fallback-only",
            "timeout_ms": 3000,
            "temperature": 0.5,
            "provider_preference": ["together", "groq"],
            "openrouter": {
                "api_key_env": "CUSTOM_API_KEY",
                "model": "openai/gpt-4",
                "reasoning_effort": "high"
            }
        }"#;

        let llm: LlmConfig = serde_json::from_str(json).unwrap();
        assert!(llm.enabled);
        assert_eq!(llm.backend, "openrouter");
        assert_eq!(llm.timeout_ms, 3000);
        assert_eq!(llm.temperature, 0.5);
        assert_eq!(llm.provider_preference, vec!["together", "groq"]);
        assert_eq!(llm.openrouter.api_key_env, "CUSTOM_API_KEY");
        assert_eq!(llm.openrouter.model, "openai/gpt-4");
        assert_eq!(llm.openrouter.reasoning_effort, "high");
    }

    #[cfg(feature = "llm")]
    #[test]
    fn test_config_deserialization_with_llm() {
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
            },
            "llm": {
                "enabled": true,
                "backend": "openrouter",
                "openrouter": {
                    "model": "openai/gpt-4"
                }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert!(config.llm.enabled);
        assert_eq!(config.llm.backend, "openrouter");
        assert_eq!(config.llm.openrouter.model, "openai/gpt-4");
    }

    #[test]
    fn test_default_model_with_env_var() {
        std::env::set_var("OPENROUTER_MODEL", "meta-llama/llama-3-70b");
        let model = default_model();
        assert_eq!(model, "meta-llama/llama-3-70b");
        std::env::remove_var("OPENROUTER_MODEL");
    }

    #[test]
    fn test_default_model_with_empty_env_var() {
        std::env::set_var("OPENROUTER_MODEL", "");
        let model = default_model();
        assert_eq!(model, "openai/gpt-oss-20b");
        std::env::remove_var("OPENROUTER_MODEL");
    }

    #[test]
    fn test_default_model_with_whitespace() {
        std::env::set_var("OPENROUTER_MODEL", "  meta-llama/llama-3-70b  ");
        let model = default_model();
        assert_eq!(model, "meta-llama/llama-3-70b");
        std::env::remove_var("OPENROUTER_MODEL");
    }

    #[test]
    fn test_default_model_without_env_var() {
        std::env::remove_var("OPENROUTER_MODEL");
        let model = default_model();
        assert_eq!(model, "openai/gpt-oss-20b");
    }
}
