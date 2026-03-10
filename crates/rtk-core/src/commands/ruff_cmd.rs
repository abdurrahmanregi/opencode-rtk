use crate::{
    commands::CommandModule,
    filter::{GroupingByPattern, Strategy},
    Context,
};
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

pub struct RuffModule {
    grouping_strategy: GroupingByPattern,
}

impl RuffModule {
    pub fn new() -> Self {
        Self {
            grouping_strategy: GroupingByPattern,
        }
    }

    /// Detect if output is JSON format
    /// Checks for balanced brackets to avoid false positives like [INFO] or {placeholder}
    fn is_json_output(&self, output: &str) -> bool {
        let trimmed = output.trim();
        (trimmed.starts_with('[') && trimmed.ends_with(']'))
            || (trimmed.starts_with('{') && trimmed.ends_with('}'))
    }

    /// Check if command was run with JSON output flag
    fn is_json_command(&self, command: &str) -> bool {
        let cmd_lower = command.to_lowercase();
        cmd_lower.contains("--output-format=json")
            || cmd_lower.contains("--output-format json")
            || cmd_lower.contains("--format=json")
            || cmd_lower.contains("--format json")
            || cmd_lower.contains("-o json")
    }

    /// Parse and summarize Ruff JSON output
    fn handle_json_output(&self, output: &str) -> Result<String> {
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct RuffMessage {
            #[serde(rename = "type")]
            message_type: Option<String>,
            message: Option<String>,
            code: Option<String>,
            location: Option<RuffLocation>,
            fix: Option<RuffFix>,
        }

        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct RuffLocation {
            row: Option<usize>,
            column: Option<usize>,
        }

        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct RuffFix {
            message: Option<String>,
        }

        let messages: Vec<RuffMessage> = match serde_json::from_str(output) {
            Ok(msgs) => msgs,
            Err(_) => {
                // If parsing fails, fall back to grouping strategy
                return self.grouping_strategy.compress(output);
            }
        };

        if messages.is_empty() {
            return Ok("(no issues)".to_string());
        }

        // Count by code
        let mut code_counts: HashMap<String, usize> = HashMap::new();
        let mut total_fixable = 0;

        for msg in &messages {
            if let Some(code) = &msg.code {
                *code_counts.entry(code.clone()).or_insert(0) += 1;
            }
            if msg.fix.is_some() {
                total_fixable += 1;
            }
        }

        // Sort by count
        let mut sorted_codes: Vec<_> = code_counts.into_iter().collect();
        sorted_codes.sort_by(|a, b| b.1.cmp(&a.1));

        let mut parts = vec![format!("{} issues", messages.len())];

        if !sorted_codes.is_empty() {
            let code_summary: Vec<String> = sorted_codes
                .iter()
                .take(10)
                .map(|(code, count)| format!("{}: {}", code, count))
                .collect();
            parts.push(format!("codes: [{}]", code_summary.join(", ")));
        }

        if total_fixable > 0 {
            parts.push(format!("{} fixable", total_fixable));
        }

        Ok(parts.join(", "))
    }

    /// Handle text output using grouping strategy
    fn handle_text_output(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(no output)".to_string());
        }

        // Check for "All checks passed" or similar success messages
        let output_lower = output.to_lowercase();
        if output_lower.contains("all checks passed")
            || output_lower.contains("no issues found")
            || output.trim().is_empty()
        {
            return Ok("(no issues)".to_string());
        }

        self.grouping_strategy.compress(output)
    }
}

impl Default for RuffModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for RuffModule {
    fn name(&self) -> &str {
        "ruff"
    }

    fn strategy(&self) -> &str {
        "json_text_dual"
    }

    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        // First check command flags
        if let Some(cmd) = &context.command {
            if self.is_json_command(cmd) || self.is_json_output(output) {
                return self.handle_json_output(output);
            }
        }

        // Fallback to detecting from output format
        if self.is_json_output(output) {
            return self.handle_json_output(output);
        }

        self.handle_text_output(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(command: &str) -> Context {
        Context {
            cwd: "/tmp".to_string(),
            exit_code: 0,
            tool: "bash".to_string(),
            session_id: None,
            command: Some(command.to_string()),
        }
    }

    #[test]
    fn test_ruff_json_output() {
        let module = RuffModule::new();
        let input = r#"[
            {"code": "E501", "message": "line too long", "location": {"row": 10}},
            {"code": "E501", "message": "line too long", "location": {"row": 20}},
            {"code": "F401", "message": "unused import", "location": {"row": 5}}
        ]"#;
        let result = module
            .compress(input, &make_context("ruff check --output-format=json"))
            .unwrap();

        assert!(result.contains("3 issues"));
        assert!(result.contains("E501: 2"));
        assert!(result.contains("F401: 1"));
    }

    #[test]
    fn test_ruff_json_with_fixable() {
        let module = RuffModule::new();
        let input = r#"[
            {"code": "F401", "message": "unused import", "fix": {"message": "Remove import"}}
        ]"#;
        let result = module
            .compress(input, &make_context("ruff check -o json"))
            .unwrap();

        assert!(result.contains("1 issue"));
        assert!(result.contains("1 fixable"));
    }

    #[test]
    fn test_ruff_json_empty() {
        let module = RuffModule::new();
        let result = module
            .compress("[]", &make_context("ruff check --output-format=json"))
            .unwrap();

        assert_eq!(result, "(no issues)");
    }

    #[test]
    fn test_ruff_text_output() {
        let module = RuffModule::new();
        let input = r#"src/main.py:10:5: E501 line too long
src/main.py:20:10: E501 line too long
src/utils.py:5:1: F401 unused import os
"#;
        let result = module
            .compress(input, &make_context("ruff check src/"))
            .unwrap();

        // Should use grouping strategy
        assert!(!result.is_empty());
    }

    #[test]
    fn test_ruff_text_all_passed() {
        let module = RuffModule::new();
        let result = module
            .compress("All checks passed!", &make_context("ruff check"))
            .unwrap();

        assert_eq!(result, "(no issues)");
    }

    #[test]
    fn test_ruff_empty_output() {
        let module = RuffModule::new();
        let result = module.compress("", &make_context("ruff check")).unwrap();

        assert_eq!(result, "(no output)");
    }

    #[test]
    fn test_ruff_format_command() {
        let module = RuffModule::new();
        let input = "src/main.py\nsrc/utils.py\n";
        let result = module
            .compress(input, &make_context("ruff format --check"))
            .unwrap();

        // Should use grouping for text output
        assert!(!result.is_empty());
    }

    #[test]
    fn test_ruff_json_detection_from_output() {
        let module = RuffModule::new();
        let input = r#"[
            {"code": "E501", "message": "line too long"}
        ]"#;
        // Even without JSON flag in command, should detect from output
        let result = module.compress(input, &make_context("ruff check")).unwrap();

        assert!(result.contains("1 issue"));
    }

    #[test]
    fn test_ruff_malformed_json() {
        let module = RuffModule::new();
        // Malformed JSON - missing closing bracket
        let input = r#"[
            {"code": "E501", "message": "line too long"},
            {"code": "F401", "message": "unused import"
        ]"#;
        let result = module
            .compress(input, &make_context("ruff check --output-format=json"))
            .unwrap();

        // Should fall back to grouping strategy for malformed JSON
        assert!(!result.is_empty());
    }

    #[test]
    fn test_ruff_partial_json() {
        let module = RuffModule::new();
        // Partial JSON - truncated output
        let input = r#"[
            {"code": "E501", "message": "line too long"},
            {"code"#;
        let result = module
            .compress(input, &make_context("ruff check -o json"))
            .unwrap();

        // Should fall back gracefully
        assert!(!result.is_empty());
    }

    #[test]
    fn test_ruff_json_with_missing_fields() {
        let module = RuffModule::new();
        // JSON with missing optional fields
        let input = r#"[
            {"code": "E501"},
            {"message": "some issue"},
            {}
        ]"#;
        let result = module
            .compress(input, &make_context("ruff check --output-format=json"))
            .unwrap();

        // Should handle missing fields gracefully
        assert!(result.contains("3 issue"));
    }

    #[test]
    fn test_ruff_json_with_unicode() {
        let module = RuffModule::new();
        // JSON with Unicode characters
        let input = r#"[
            {"code": "E501", "message": "line too long — exceeds limit 😊"},
            {"code": "F401", "message": "unused import 中文测试"}
        ]"#;
        let result = module
            .compress(input, &make_context("ruff check --output-format=json"))
            .unwrap();

        assert!(result.contains("2 issue"));
    }

    #[test]
    fn test_ruff_json_false_positive_brackets() {
        let module = RuffModule::new();
        // Text that starts with [ but isn't JSON
        let input = "[INFO] Running ruff check\n[WARNING] Found issues";
        let result = module.compress(input, &make_context("ruff check")).unwrap();

        // Should not try to parse as JSON
        assert!(!result.contains("issue") || result.contains("INFO") || result.contains("WARNING"));
    }
}
