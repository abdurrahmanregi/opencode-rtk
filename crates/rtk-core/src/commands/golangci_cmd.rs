use crate::{
    commands::CommandModule,
    filter::{GroupingByPattern, Strategy},
    Context,
};
use anyhow::Result;
use std::collections::HashMap;

pub struct GolangciModule {
    grouping_strategy: GroupingByPattern,
}

impl GolangciModule {
    pub fn new() -> Self {
        Self {
            grouping_strategy: GroupingByPattern,
        }
    }

    /// Parse golangci-lint output and create structured summary
    fn parse_lint_output(&self, output: &str) -> Result<String> {
        if output.trim().is_empty() {
            return Ok("(no issues)".to_string());
        }

        // Parse golangci-lint output format
        // Format: file:line:column: message (linter)
        let mut issues_by_linter: HashMap<String, Vec<String>> = HashMap::new();
        let mut issues_by_file: HashMap<String, usize> = HashMap::new();
        let mut total_issues = 0;
        let mut errors = 0;
        let mut warnings = 0;

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Check for file reference pattern
            if line.contains(".go:") {
                total_issues += 1;

                // Count by file
                if let Some(file_part) = line.split(':').next() {
                    *issues_by_file.entry(file_part.to_string()).or_insert(0) += 1;
                }

                // Extract linter name (usually in parentheses at end)
                let linter = if line.contains('(') && line.ends_with(')') {
                    if let Some(start) = line.rfind('(') {
                        let end = line.len() - 1;
                        let extracted = &line[start + 1..end];
                        // Validate linter name: should be alphanumeric with possible hyphens/underscores
                        if !extracted.is_empty()
                            && extracted
                                .chars()
                                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                        {
                            extracted.to_string()
                        } else {
                            "other".to_string()
                        }
                    } else {
                        "other".to_string()
                    }
                } else {
                    // Try to detect common linters from message
                    if line.contains("errcheck") {
                        "errcheck".to_string()
                    } else if line.contains("govet") {
                        "govet".to_string()
                    } else if line.contains("staticcheck") {
                        "staticcheck".to_string()
                    } else if line.contains("ineffassign") {
                        "ineffassign".to_string()
                    } else {
                        "other".to_string()
                    }
                };

                // Classify as error or warning based on severity indicators
                let line_lower = line.to_lowercase();
                if line_lower.contains("error") || line_lower.contains("fatal") {
                    errors += 1;
                } else {
                    warnings += 1;
                }

                issues_by_linter
                    .entry(linter)
                    .or_default()
                    .push(line.to_string());
            }
        }

        if total_issues == 0 {
            // Check for summary line
            if output.to_lowercase().contains("0 issues") || output.contains("no issues") {
                return Ok("(no issues)".to_string());
            }
            // Fall back to grouping strategy
            return self.grouping_strategy.compress(output);
        }

        // Build summary
        let mut summary = Vec::new();

        // Overall stats
        summary.push(format!(
            "Total: {} issue(s) ({} errors, {} warnings)",
            total_issues, errors, warnings
        ));

        // Issues by linter
        if !issues_by_linter.is_empty() {
            summary.push(String::new());
            summary.push("By linter:".to_string());

            let mut linters: Vec<_> = issues_by_linter.iter().collect();
            linters.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

            for (linter, issues) in linters.iter().take(10) {
                summary.push(format!("  {}: {} issue(s)", linter, issues.len()));
                // Show first 3 examples
                for issue in issues.iter().take(3) {
                    // Truncate long lines (UTF-8-safe)
                    let truncated = if issue.len() > 100 {
                        let safe_trunc: String = issue.chars().take(97).collect();
                        format!("{}...", safe_trunc)
                    } else {
                        issue.clone()
                    };
                    summary.push(format!("    {}", truncated));
                }
                if issues.len() > 3 {
                    summary.push(format!("    ... and {} more", issues.len() - 3));
                }
            }
        }

        // Issues by file
        if issues_by_file.len() > 1 {
            summary.push(String::new());
            summary.push("By file:".to_string());

            let mut files: Vec<_> = issues_by_file.iter().collect();
            files.sort_by(|a, b| b.1.cmp(a.1));

            for (file, count) in files.iter().take(10) {
                // Show just the filename, not full path
                let filename = file.rsplit('/').next().unwrap_or(file);
                summary.push(format!("  {}: {} issue(s)", filename, count));
            }
            if files.len() > 10 {
                summary.push(format!("  ... and {} more files", files.len() - 10));
            }
        }

        Ok(summary.join("\n"))
    }

    /// Handle golangci-lint run command
    fn handle_run(&self, output: &str, exit_code: i32) -> Result<String> {
        if exit_code == 0 && output.trim().is_empty() {
            return Ok("(no issues)".to_string());
        }

        self.parse_lint_output(output)
    }

    /// Handle golangci-lint linters command
    fn handle_linters(&self, output: &str) -> Result<String> {
        // Just show count of linters
        let linters: Vec<&str> = output
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with(" "))
            .collect();

        if linters.is_empty() {
            return Ok("(no linters configured)".to_string());
        }

        // Extract enabled/disabled counts if present
        let mut result = vec![format!("{} linters available", linters.len())];

        // Show first few linters
        for linter in linters.iter().take(10) {
            result.push(format!("  {}", linter.trim()));
        }
        if linters.len() > 10 {
            result.push(format!("  ... and {} more", linters.len() - 10));
        }

        Ok(result.join("\n"))
    }
}

impl Default for GolangciModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for GolangciModule {
    fn name(&self) -> &str {
        "golangci-lint"
    }

    fn strategy(&self) -> &str {
        "grouping_by_pattern"
    }

    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        // Detect subcommand from context
        let subcommand = context.command.as_ref().and_then(|cmd| {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if parts.len() >= 2 && parts[0] == "golangci-lint" {
                Some(parts[1].to_string())
            } else {
                None
            }
        });

        match subcommand.as_deref() {
            Some("run") => self.handle_run(output, context.exit_code),
            Some("linters") => self.handle_linters(output),
            _ => self.handle_run(output, context.exit_code),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context(command: &str, exit_code: i32) -> Context {
        Context {
            cwd: "/tmp".to_string(),
            exit_code,
            tool: "bash".to_string(),
            session_id: None,
            command: Some(command.to_string()),
        }
    }

    #[test]
    fn test_empty_output() {
        let module = GolangciModule::new();
        let result = module
            .compress("", &make_context("golangci-lint run", 0))
            .unwrap();

        assert_eq!(result, "(no issues)");
    }

    #[test]
    fn test_clean_output() {
        let module = GolangciModule::new();
        let input = "0 issues.";
        let result = module
            .compress(input, &make_context("golangci-lint run", 0))
            .unwrap();

        assert_eq!(result, "(no issues)");
    }

    #[test]
    fn test_single_issue() {
        let module = GolangciModule::new();
        let input = r#"main.go:10:5: undeclared name: foo (typecheck)
"#;
        let result = module
            .compress(input, &make_context("golangci-lint run", 1))
            .unwrap();

        assert!(result.contains("1 issue(s)"));
        assert!(result.contains("typecheck"));
    }

    #[test]
    fn test_multiple_issues_same_linter() {
        let module = GolangciModule::new();
        let input = r#"main.go:10:5: Error return value is not checked (errcheck)
main.go:20:3: Error return value is not checked (errcheck)
main.go:30:7: Error return value is not checked (errcheck)
"#;
        let result = module
            .compress(input, &make_context("golangci-lint run", 1))
            .unwrap();

        assert!(result.contains("3 issue(s)"));
        assert!(result.contains("errcheck: 3 issue(s)"));
    }

    #[test]
    fn test_multiple_linters() {
        let module = GolangciModule::new();
        let input = r#"main.go:10:5: undeclared name: foo (typecheck)
main.go:20:3: ineffectual assignment to x (ineffassign)
main.go:30:7: printf: non-constant format string (govet)
utils.go:15:2: unused variable: temp (staticcheck)
"#;
        let result = module
            .compress(input, &make_context("golangci-lint run", 1))
            .unwrap();

        assert!(result.contains("4 issue(s)"));
        assert!(result.contains("By linter"));
    }

    #[test]
    fn test_issues_by_file() {
        let module = GolangciModule::new();
        let input = r#"main.go:10:5: issue 1 (errcheck)
main.go:20:3: issue 2 (errcheck)
utils.go:15:2: issue 3 (errcheck)
utils.go:25:5: issue 4 (errcheck)
utils.go:35:10: issue 5 (errcheck)
"#;
        let result = module
            .compress(input, &make_context("golangci-lint run", 1))
            .unwrap();

        assert!(result.contains("By file"));
        assert!(result.contains("utils.go: 3 issue(s)"));
        assert!(result.contains("main.go: 2 issue(s)"));
    }

    #[test]
    fn test_error_classification() {
        let module = GolangciModule::new();
        let input = r#"main.go:10:5: error: undeclared name: foo (typecheck)
main.go:20:3: warning: unused variable (staticcheck)
"#;
        let result = module
            .compress(input, &make_context("golangci-lint run", 1))
            .unwrap();

        assert!(result.contains("1 errors"));
        assert!(result.contains("1 warnings"));
    }

    #[test]
    fn test_long_line_truncation() {
        let module = GolangciModule::new();
        let long_message = "a".repeat(150);
        let input = format!("main.go:10:5: {} (errcheck)", long_message);
        let result = module
            .compress(&input, &make_context("golangci-lint run", 1))
            .unwrap();

        // Should truncate long lines
        assert!(result.contains("..."));
    }

    #[test]
    fn test_linters_command() {
        let module = GolangciModule::new();
        let input = r#"Enabled by your configuration linters:
errcheck: Errcheck is a program for checking for unchecked errors in go programs.
govet: Vet examines Go source code and reports suspicious constructs.
staticcheck: It's a set of rules from staticcheck.

Disabled by your configuration linters:
testpackage: An analyzer checks that you have tested package.
"#;
        let result = module
            .compress(input, &make_context("golangci-lint linters", 0))
            .unwrap();

        assert!(result.contains("linters"));
    }

    #[test]
    fn test_many_issues() {
        let module = GolangciModule::new();
        let mut input = String::new();
        for i in 1..=50 {
            input.push_str(&format!("main.go:{}:5: issue {} (errcheck)\n", i * 10, i));
        }
        let result = module
            .compress(&input, &make_context("golangci-lint run", 1))
            .unwrap();

        assert!(result.contains("50 issue(s)"));
    }

    #[test]
    fn test_no_linter_in_parens() {
        let module = GolangciModule::new();
        // Some output might not have linter in parentheses
        let input = r#"main.go:10:5: errcheck: Error return value is not checked
main.go:20:3: govet: possible misuse of unsafe.Pointer
"#;
        let result = module
            .compress(input, &make_context("golangci-lint run", 1))
            .unwrap();

        assert!(result.contains("2 issue(s)"));
    }

    #[test]
    fn test_unknown_command_defaults_to_run() {
        let module = GolangciModule::new();
        let input = "main.go:10:5: issue (errcheck)";
        let result = module
            .compress(input, &make_context("golangci-lint", 1))
            .unwrap();

        assert!(result.contains("1 issue(s)"));
    }

    #[test]
    fn test_utf8_at_truncation_boundary() {
        let module = GolangciModule::new();
        // Create a message that would truncate right in the middle of a UTF-8 character
        // The truncation happens at 97 chars + "..."
        // "é" is 2 bytes in UTF-8, "🚀" is 4 bytes
        let base = "main.go:10:5: ";
        let mut message = String::new();
        // Build message to be right around 100 chars with UTF-8 at boundary
        message.push_str(base);
        // Add ASCII to get close to boundary
        for _ in 0..80 {
            message.push('a');
        }
        // Add UTF-8 characters near the truncation point
        message.push_str("émoji🚀");
        message.push_str(" (errcheck)");

        let result = module
            .compress(&message, &make_context("golangci-lint run", 1))
            .unwrap();

        // Should not panic and should contain valid UTF-8
        assert!(result.contains("issue"));
        // Verify no malformed UTF-8 in output
        assert!(result.chars().all(|c| c != '\u{FFFD}'));
    }

    #[test]
    fn test_unicode_in_linter_messages() {
        let module = GolangciModule::new();
        // Test various Unicode characters in messages
        let input = r#"main.go:10:5: Error: 中文测试 message (typecheck)
main.go:20:3: Warning: émoji 🚀 in code (govet)
main.go:30:7: Issue with 日本語 characters (staticcheck)
"#;
        let result = module
            .compress(input, &make_context("golangci-lint run", 1))
            .unwrap();

        assert!(result.contains("3 issue"));
        // Should handle Unicode without panicking
        assert!(!result.is_empty());
    }

    #[test]
    fn test_truncation_preserves_valid_utf8() {
        let module = GolangciModule::new();
        // Message with multi-byte characters that would truncate mid-character
        let long_message = "main.go:10:5: ".to_string() + &"é".repeat(100) + " (errcheck)";
        let result = module
            .compress(&long_message, &make_context("golangci-lint run", 1))
            .unwrap();

        // Should contain truncation marker
        assert!(result.contains("..."));
        // Output should be valid UTF-8 (no replacement characters)
        assert!(result.chars().all(|c| c != '\u{FFFD}'));
    }
}
