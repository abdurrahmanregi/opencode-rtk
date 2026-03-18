pub mod settings;

use regex::Regex;
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

    /// Enable model-aware automatic policy selection
    #[serde(default)]
    pub model_auto: ModelAutoConfig,

    /// Per-model override rules (first match wins)
    #[serde(default)]
    pub model_overrides: Vec<ModelOverride>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelCategory {
    Reasoning,
    Instruct,
    Compact,
}

impl Default for ModelCategory {
    fn default() -> Self {
        Self::Instruct
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PostExecutionPolicyMode {
    Off,
    MetadataOnly,
    ReplaceOutput,
}

impl Default for PostExecutionPolicyMode {
    fn default() -> Self {
        Self::MetadataOnly
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompressionAggressiveness {
    Low,
    Medium,
    High,
}

impl Default for CompressionAggressiveness {
    fn default() -> Self {
        Self::Medium
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAutoConfig {
    #[serde(default = "default_model_auto_enabled")]
    pub enabled: bool,

    #[serde(default)]
    pub default_category: ModelCategory,

    #[serde(default)]
    pub default_policy_mode: PostExecutionPolicyMode,

    #[serde(default)]
    pub default_compression_aggressiveness: CompressionAggressiveness,

    #[serde(default = "default_strip_reasoning")]
    pub strip_reasoning: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOverride {
    #[serde(rename = "match")]
    pub match_pattern: String,

    #[serde(default)]
    pub category: Option<ModelCategory>,

    #[serde(default)]
    pub policy_mode: Option<PostExecutionPolicyMode>,

    #[serde(default)]
    pub compression_aggressiveness: Option<CompressionAggressiveness>,

    #[serde(default)]
    pub strip_reasoning: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRuntimePolicy {
    pub model_id: String,
    pub category: ModelCategory,
    pub policy_mode: PostExecutionPolicyMode,
    pub compression_aggressiveness: CompressionAggressiveness,
    pub strip_reasoning: bool,
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
fn default_model_auto_enabled() -> bool {
    false
}
fn default_strip_reasoning() -> bool {
    true
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
            model_auto: ModelAutoConfig::default(),
            model_overrides: Vec::new(),
        }
    }
}

impl Default for ModelAutoConfig {
    fn default() -> Self {
        Self {
            enabled: default_model_auto_enabled(),
            default_category: ModelCategory::default(),
            default_policy_mode: PostExecutionPolicyMode::default(),
            default_compression_aggressiveness: CompressionAggressiveness::default(),
            strip_reasoning: default_strip_reasoning(),
        }
    }
}

impl LlmConfig {
    pub fn resolve_model_policy(&self, model_id: &str) -> Option<ModelRuntimePolicy> {
        if !self.model_auto.enabled {
            return None;
        }

        let normalized_model = model_id.trim();
        let mut resolved = ModelRuntimePolicy {
            model_id: normalized_model.to_string(),
            category: self.model_auto.default_category,
            policy_mode: self.model_auto.default_policy_mode,
            compression_aggressiveness: self.model_auto.default_compression_aggressiveness,
            strip_reasoning: self.model_auto.strip_reasoning,
        };

        for override_rule in &self.model_overrides {
            let pattern = override_rule.match_pattern.trim();
            if pattern.is_empty() {
                continue;
            }

            if glob_matches(pattern, normalized_model) {
                if let Some(category) = override_rule.category {
                    resolved.category = category;
                }
                if let Some(policy_mode) = override_rule.policy_mode {
                    resolved.policy_mode = policy_mode;
                }
                if let Some(aggressiveness) = override_rule.compression_aggressiveness {
                    resolved.compression_aggressiveness = aggressiveness;
                }
                if let Some(strip_reasoning) = override_rule.strip_reasoning {
                    resolved.strip_reasoning = strip_reasoning;
                }
                break;
            }
        }

        Some(resolved)
    }
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    let mut regex_pattern = String::with_capacity(pattern.len() + 2);
    regex_pattern.push('^');

    for ch in pattern.chars() {
        match ch {
            '*' => regex_pattern.push_str(".*"),
            '?' => regex_pattern.push('.'),
            _ => regex_pattern.push_str(&regex::escape(&ch.to_string())),
        }
    }

    regex_pattern.push('$');
    let wrapped = format!("(?i:{regex_pattern})");

    match Regex::new(&wrapped) {
        Ok(regex) => regex.is_match(value),
        Err(_) => false,
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
        assert!(!llm.model_auto.enabled);
        assert!(llm.model_overrides.is_empty());
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
            },
            "model_auto": {
                "enabled": true,
                "default_category": "instruct",
                "default_policy_mode": "replace_output",
                "default_compression_aggressiveness": "high",
                "strip_reasoning": true
            },
            "model_overrides": [
                {
                    "match": "openai/gpt-oss*",
                    "category": "reasoning",
                    "policy_mode": "metadata_only",
                    "compression_aggressiveness": "low",
                    "strip_reasoning": false
                }
            ]
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
        assert!(llm.model_auto.enabled);
        assert_eq!(
            llm.model_auto.default_policy_mode,
            PostExecutionPolicyMode::ReplaceOutput
        );
        assert_eq!(
            llm.model_auto.default_compression_aggressiveness,
            CompressionAggressiveness::High
        );
        assert_eq!(llm.model_overrides.len(), 1);
        assert_eq!(llm.model_overrides[0].match_pattern, "openai/gpt-oss*");
        assert_eq!(
            llm.model_overrides[0].category,
            Some(ModelCategory::Reasoning)
        );
    }

    #[test]
    fn test_glob_matches_wildcards_case_insensitive() {
        assert!(glob_matches(
            "openai/gpt-oss*",
            "openai/gpt-oss-safeguard-20b"
        ));
        assert!(glob_matches(
            "META-LLAMA/*",
            "meta-llama/llama-3.1-8b-instruct"
        ));
        assert!(glob_matches(
            "*/llama-3.?-8b-*",
            "meta-llama/llama-3.1-8b-instruct"
        ));
        assert!(!glob_matches(
            "openai/gpt-oss?",
            "openai/gpt-oss-safeguard-20b"
        ));
    }

    #[test]
    fn test_resolve_model_policy_disabled() {
        let mut llm = LlmConfig::default();
        llm.model_auto.enabled = false;
        assert!(llm
            .resolve_model_policy("openai/gpt-oss-safeguard-20b")
            .is_none());
    }

    #[test]
    fn test_resolve_model_policy_default_and_override() {
        let mut llm = LlmConfig::default();
        llm.model_auto = ModelAutoConfig {
            enabled: true,
            default_category: ModelCategory::Instruct,
            default_policy_mode: PostExecutionPolicyMode::MetadataOnly,
            default_compression_aggressiveness: CompressionAggressiveness::Medium,
            strip_reasoning: true,
        };
        llm.model_overrides = vec![ModelOverride {
            match_pattern: "openai/gpt-oss*".to_string(),
            category: Some(ModelCategory::Reasoning),
            policy_mode: Some(PostExecutionPolicyMode::MetadataOnly),
            compression_aggressiveness: Some(CompressionAggressiveness::Low),
            strip_reasoning: Some(true),
        }];

        let overridden = llm
            .resolve_model_policy("openai/gpt-oss-safeguard-20b")
            .unwrap();
        assert_eq!(overridden.category, ModelCategory::Reasoning);
        assert_eq!(
            overridden.compression_aggressiveness,
            CompressionAggressiveness::Low
        );
        assert!(overridden.strip_reasoning);

        let defaulted = llm
            .resolve_model_policy("meta-llama/llama-3.1-8b-instruct")
            .unwrap();
        assert_eq!(defaulted.category, ModelCategory::Instruct);
        assert_eq!(
            defaulted.compression_aggressiveness,
            CompressionAggressiveness::Medium
        );
        assert_eq!(defaulted.policy_mode, PostExecutionPolicyMode::MetadataOnly);
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
