pub mod commands;
pub mod config;
pub mod filter;
pub mod tee;
pub mod tracking;
pub mod utils;

pub use commands::pre_execution::{optimize_command, FlagMapping, OptimizedCommand};
pub use tee::{TeeEntry, TeeManager};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Maximum allowed input size (10 MB)
const MAX_INPUT_SIZE: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub cwd: String,
    pub exit_code: i32,
    pub tool: String,
    pub session_id: Option<String>,
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedOutput {
    pub compressed: String,
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub saved_tokens: usize,
    pub savings_pct: f64,
    pub strategy: String,
    pub module: String,
}

pub fn compress(command: &str, output: &str, mut context: Context) -> Result<CompressedOutput> {
    // Input validation
    if command.len() > MAX_INPUT_SIZE {
        return Err(anyhow!(
            "Command exceeds maximum size limit ({} bytes)",
            MAX_INPUT_SIZE
        ));
    }
    if output.len() > MAX_INPUT_SIZE {
        return Err(anyhow!(
            "Output exceeds maximum size limit ({} bytes)",
            MAX_INPUT_SIZE
        ));
    }

    // Validate UTF-8 (str type already guarantees this, but being explicit)
    if !command
        .chars()
        .all(|c| !c.is_control() || c == '\n' || c == '\t')
    {
        return Err(anyhow!("Command contains invalid control characters"));
    }

    let original_tokens = utils::tokens::estimate_tokens(output);

    // Add command to context for modules that need it
    context.command = Some(command.to_string());

    if let Some(module) = commands::detect_command(command) {
        let compressed = module.compress(output, &context)?;
        let compressed_tokens = utils::tokens::estimate_tokens(&compressed);
        let saved_tokens = original_tokens.saturating_sub(compressed_tokens);
        let savings_pct = if original_tokens > 0 {
            (saved_tokens as f64 / original_tokens as f64) * 100.0
        } else {
            0.0
        };

        Ok(CompressedOutput {
            compressed,
            original_tokens,
            compressed_tokens,
            saved_tokens,
            savings_pct,
            strategy: module.strategy().to_string(),
            module: module.name().to_string(),
        })
    } else {
        Ok(CompressedOutput {
            compressed: output.to_string(),
            original_tokens,
            compressed_tokens: original_tokens,
            saved_tokens: 0,
            savings_pct: 0.0,
            strategy: "none".to_string(),
            module: "unknown".to_string(),
        })
    }
}

pub fn estimate_tokens(text: &str) -> usize {
    utils::tokens::estimate_tokens(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(cwd: &str) -> Context {
        Context {
            cwd: cwd.to_string(),
            exit_code: 0,
            tool: "bash".to_string(),
            session_id: None,
            command: None,
        }
    }

    #[test]
    fn test_compress_small_output() {
        let output = "small output";
        let result = compress("echo test", output, make_context("/tmp")).unwrap();

        assert!(result.original_tokens > 0);
        assert!(result.compressed_tokens > 0);
    }

    #[test]
    fn test_compress_empty_output() {
        let result = compress("echo test", "", make_context("/tmp")).unwrap();

        assert_eq!(result.original_tokens, 0);
        assert_eq!(result.compressed_tokens, 0);
        assert_eq!(result.saved_tokens, 0);
    }

    #[test]
    fn test_compress_large_output() {
        // Test with large output to verify it doesn't panic or hang
        let large_output: String = "line\n".repeat(10000);
        let result = compress("cat large_file.txt", &large_output, make_context("/tmp")).unwrap();

        assert!(result.original_tokens > 0);
        // Large output should still be processed
        assert!(
            !result.compressed.is_empty() || result.compressed_tokens == result.original_tokens
        );
    }

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_simple() {
        let tokens = estimate_tokens("hello world");
        assert!(tokens > 0);
        assert!(tokens < 10);
    }

    #[test]
    fn test_compress_unknown_command() {
        let output = "some output";
        let result = compress("unknown_command", output, make_context("/tmp")).unwrap();

        // Unknown commands should pass through unchanged
        assert_eq!(result.compressed, output);
        assert_eq!(result.saved_tokens, 0);
        assert_eq!(result.strategy, "none");
    }
}
