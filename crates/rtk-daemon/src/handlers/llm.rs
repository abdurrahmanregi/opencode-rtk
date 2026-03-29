use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, warn};

/// System prompt for command output compression
const COMPRESSION_PROMPT: &str = r#"Compress this command output. Output ONLY the compressed result.

PRESERVE: errors, warnings, file paths, exit codes, key metrics, test failures
REMOVE: progress bars, timestamps, debug logs, verbose stack traces, repetitive lines
FORMAT: Single line or brief bullets
MAX: 100 tokens

Command: {command} (exit: {exit_code})
Output:
{output}

Compressed:"#;

#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: usize,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<ProviderConfig>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ProviderConfig {
    order: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Debug, Deserialize)]
struct MessageContent {
    content: Option<Value>,
    #[serde(default)]
    reasoning: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

fn extract_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().filter_map(extract_text).collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        Value::Object(map) => {
            for key in ["text", "content", "output", "reasoning"] {
                if let Some(candidate) = map.get(key).and_then(extract_text) {
                    return Some(candidate);
                }
            }

            let parts: Vec<String> = map.values().filter_map(extract_text).collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        _ => None,
    }
}

/// LLM compressor using OpenRouter API
pub struct LlmCompressor {
    client: Client,
    config: rtk_core::config::LlmConfig,
    api_key: Option<String>,
}

impl LlmCompressor {
    /// Create a new LLM compressor
    pub fn new(config: rtk_core::config::LlmConfig) -> Result<Self> {
        let api_key = Self::load_api_key(&config)?;

        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .context("Failed to create HTTP client")?;

        if api_key.is_some() {
            debug!("LlmCompressor initialized with API key");
        } else {
            warn!("LlmCompressor initialized WITHOUT API key - LLM compression disabled");
            warn!(
                "Set {} env var or configure api_key_file in config",
                config.openrouter.api_key_env
            );
        }

        Ok(Self {
            client,
            config,
            api_key,
        })
    }

    /// Load API key from environment variable or file
    fn load_api_key(config: &rtk_core::config::LlmConfig) -> Result<Option<String>> {
        // Try environment variable first
        if let Ok(key) = std::env::var(&config.openrouter.api_key_env) {
            if !key.is_empty() {
                debug!(
                    "Loaded API key from environment: {}",
                    config.openrouter.api_key_env
                );
                return Ok(Some(key));
            }
        }

        // Try file
        if let Some(path) = &config.openrouter.api_key_file {
            let expanded = shellexpand::tilde(path);
            if let Ok(key) = std::fs::read_to_string(expanded.as_ref()) {
                let key = key.trim().to_string();
                if !key.is_empty() {
                    debug!("Loaded API key from file: {}", path);
                    return Ok(Some(key));
                }
            }
        }

        Ok(None)
    }

    /// Check if LLM compression is available (enabled + has API key)
    pub fn is_available(&self) -> bool {
        self.config.enabled && self.api_key.is_some()
    }

    /// Check if API key is configured (for debugging)
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// Compress output using LLM
    pub async fn compress(
        &self,
        command: &str,
        output: &str,
        exit_code: i32,
        strip_reasoning: bool,
    ) -> Result<String> {
        let api_key = self
            .api_key
            .as_ref()
            .context("OpenRouter API key not configured")?;

        // Build prompt
        let prompt = COMPRESSION_PROMPT
            .replace("{command}", command)
            .replace("{exit_code}", &exit_code.to_string())
            .replace("{output}", output);

        // Truncate prompt if too long (character-based to avoid UTF-8 panic)
        let prompt = if prompt.chars().count() > self.config.openrouter.max_input_tokens * 4 {
            let truncate_chars = self.config.openrouter.max_input_tokens * 4;
            let truncated: String = prompt.chars().take(truncate_chars).collect();
            format!("{}...\n[TRUNCATED]\n\nCompressed output:", truncated)
        } else {
            prompt
        };

        let request = OpenRouterRequest {
            model: self.config.openrouter.model.clone(),
            messages: vec![Message {
                role: "user".into(),
                content: prompt,
            }],
            max_tokens: self.config.openrouter.max_output_tokens,
            temperature: self.config.temperature,
            provider: if !self.config.provider_preference.is_empty() {
                Some(ProviderConfig {
                    order: self.config.provider_preference.clone(),
                })
            } else {
                None
            },
        };

        let url = format!("{}/chat/completions", self.config.openrouter.base_url);

        debug!("Sending request to OpenRouter: {}", url);

        let response: reqwest::Response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("HTTP-Referer", "https://github.com/anomaly/opencode-rtk")
            .header("X-Title", "OpenCode-RTK")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to OpenRouter")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!("OpenRouter API error ({}): {}", status, body);
            anyhow::bail!("OpenRouter API error ({}): {}", status, body);
        }

        // Get raw response text for debugging
        let response_text = response
            .text()
            .await
            .context("Failed to read OpenRouter response body")?;
        debug!(
            "OpenRouter response body: {}",
            &response_text[..response_text.len().min(500)]
        );

        let result: OpenRouterResponse =
            serde_json::from_str(&response_text).with_context(|| {
                format!(
                    "Failed to parse OpenRouter response: {}",
                    &response_text[..response_text.len().min(200)]
                )
            })?;

        let compressed = result
            .choices
            .first()
            .map(|c| {
                debug!(
                    "Content: {:?}, Reasoning: {:?}",
                    c.message.content, c.message.reasoning
                );

                let content_text = c
                    .message
                    .content
                    .as_ref()
                    .and_then(extract_text)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());

                if let Some(content) = content_text {
                    return content;
                }

                if strip_reasoning {
                    String::new()
                } else {
                    c.message
                        .reasoning
                        .as_ref()
                        .and_then(extract_text)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_default()
                }
            })
            .unwrap_or_default();

        debug!("Compressed output: '{}'", compressed);

        // Error if LLM returned empty output
        if compressed.is_empty() {
            warn!("LLM returned empty compressed output - failing compression to allow fallback");
            anyhow::bail!("LLM returned empty compressed output");
        }

        if let Some(usage) = result.usage {
            debug!(
                "LLM compression: {} prompt + {} completion = {} total tokens",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );
        }

        Ok(compressed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_compressor_without_api_key() {
        // Ensure env var is not set (test isolation)
        std::env::remove_var("RTK_TEST_OPENROUTER_API_KEY");
        let mut config = rtk_core::config::LlmConfig::default();
        config.enabled = true;
        config.openrouter.api_key_env = "RTK_TEST_OPENROUTER_API_KEY".to_string();
        let compressor = LlmCompressor::new(config).unwrap();
        assert!(!compressor.is_available());
    }

    #[test]
    fn test_compressor_with_env_key() {
        std::env::set_var("RTK_TEST_OPENROUTER_API_KEY", "test-key");
        let mut config = rtk_core::config::LlmConfig::default();
        config.enabled = true;
        config.openrouter.api_key_env = "RTK_TEST_OPENROUTER_API_KEY".to_string();
        let compressor = LlmCompressor::new(config).unwrap();
        assert!(compressor.is_available());
        std::env::remove_var("RTK_TEST_OPENROUTER_API_KEY");
    }

    #[test]
    fn test_extract_text_from_string_and_array() {
        assert_eq!(
            extract_text(&json!("final output")),
            Some("final output".to_string())
        );
        assert_eq!(
            extract_text(&json!(["one", "two"])),
            Some("one\ntwo".to_string())
        );
    }

    #[test]
    fn test_extract_text_from_object_variants() {
        assert_eq!(
            extract_text(&json!({"text": "done"})),
            Some("done".to_string())
        );
        assert_eq!(
            extract_text(&json!({"content": [{"text": "a"}, {"text": "b"}]})),
            Some("a\nb".to_string())
        );
        assert_eq!(
            extract_text(&json!({"reasoning": "chain", "other": "ignored"})),
            Some("chain".to_string())
        );
    }
}
