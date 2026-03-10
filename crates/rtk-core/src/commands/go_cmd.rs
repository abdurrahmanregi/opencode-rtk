use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, Strategy},
    Context,
};
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

/// Go test JSON output event (NDJSON format)
/// See: https://pkg.go.dev/cmd/test2json#hdr-Output_Format
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct GoTestEvent {
    /// Time as RFC3339 timestamp
    #[serde(rename = "Time")]
    time: Option<String>,
    /// Test name (for test-specific events)
    #[serde(rename = "Test")]
    test: Option<String>,
    /// Package name
    #[serde(rename = "Package")]
    package: Option<String>,
    /// Action: start, pass, fail, skip, output
    #[serde(rename = "Action")]
    action: Option<String>,
    /// Output text (for action=output)
    #[serde(rename = "Output")]
    output: Option<String>,
    /// Elapsed time in seconds
    #[serde(rename = "Elapsed")]
    elapsed: Option<f64>,
}

pub struct GoModule {
    error_strategy: ErrorOnly,
}

impl GoModule {
    pub fn new() -> Self {
        Self {
            error_strategy: ErrorOnly,
        }
    }

    /// Detect which go subcommand is being used
    fn detect_subcommand(&self, command: &str) -> Option<String> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.len() >= 2 && parts[0] == "go" {
            Some(parts[1].to_string())
        } else {
            None
        }
    }

    /// Check if command is using -json flag
    fn is_json_output(&self, command: &str) -> bool {
        command.contains("-json")
    }

    /// Handle go test command with NDJSON parsing
    fn handle_test(&self, output: &str, command: &str) -> Result<String> {
        if self.is_json_output(command) {
            self.parse_go_test_json(output)
        } else {
            // For non-JSON output, extract test summary
            self.parse_go_test_text(output)
        }
    }

    /// Parse go test -json output (NDJSON format)
    fn parse_go_test_json(&self, output: &str) -> Result<String> {
        let mut passed = 0;
        let mut failed = 0;
        let mut skipped = 0;
        let mut failed_tests: Vec<String> = Vec::new();
        let mut package_results: HashMap<String, String> = HashMap::new();

        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }

            // Parse each NDJSON line
            if let Ok(event) = serde_json::from_str::<GoTestEvent>(line) {
                if let Some(action) = event.action.as_deref() {
                    match action {
                        "pass" => {
                            if event.test.is_some() {
                                passed += 1;
                            } else if let Some(pkg) = event.package.as_ref() {
                                package_results.insert(pkg.clone(), "PASS".to_string());
                            }
                        }
                        "fail" => {
                            if let Some(test_name) = event.test.as_ref() {
                                failed += 1;
                                failed_tests.push(test_name.clone());
                            } else if let Some(pkg) = event.package.as_ref() {
                                package_results.insert(pkg.clone(), "FAIL".to_string());
                            }
                        }
                        "skip" => {
                            skipped += 1;
                        }
                        "output" => {
                            // Note: Failure output is already captured via failed_tests
                            // Package-level failures are tracked via package_results
                        }
                        _ => {}
                    }
                }
            }
        }

        // Build summary
        let mut summary = Vec::new();

        // Package results
        if !package_results.is_empty() {
            let pass_count = package_results.values().filter(|v| *v == "PASS").count();
            let fail_count = package_results.values().filter(|v| *v == "FAIL").count();
            summary.push(format!(
                "Packages: {} passed, {} failed",
                pass_count, fail_count
            ));
        }

        // Test results
        if passed > 0 || failed > 0 || skipped > 0 {
            summary.push(format!(
                "Tests: {} passed, {} failed, {} skipped",
                passed, failed, skipped
            ));
        }

        // List failed tests (up to 10)
        if !failed_tests.is_empty() {
            summary.push(String::new());
            summary.push("Failed tests:".to_string());
            for test in failed_tests.iter().take(10) {
                summary.push(format!("  - {}", test));
            }
            if failed_tests.len() > 10 {
                summary.push(format!("  ... and {} more", failed_tests.len() - 10));
            }
        }

        if summary.is_empty() {
            Ok("(no test output)".to_string())
        } else {
            Ok(summary.join("\n"))
        }
    }

    /// Parse regular go test output
    fn parse_go_test_text(&self, output: &str) -> Result<String> {
        let mut passed = 0;
        let mut failed = 0;
        let mut skipped = 0;
        let mut failed_tests: Vec<String> = Vec::new();
        let mut package_results: Vec<(String, String)> = Vec::new();

        for line in output.lines() {
            let line = line.trim();

            // Match test result lines: PASS/FAIL/SKIP
            if line.starts_with("--- PASS:") {
                passed += 1;
            } else if line.starts_with("--- FAIL:") {
                failed += 1;
                let test_name = line.trim_start_matches("--- FAIL:").trim();
                failed_tests.push(test_name.to_string());
            } else if line.starts_with("--- SKIP:") {
                skipped += 1;
            }

            // Match package result lines: ok/FAIL package_name
            if line.starts_with("ok ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    package_results.push((parts[1].to_string(), "PASS".to_string()));
                }
            } else if line.starts_with("FAIL\t") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    package_results.push((parts[1].to_string(), "FAIL".to_string()));
                }
            }
        }

        // Build summary
        let mut summary = Vec::new();

        // Package results
        if !package_results.is_empty() {
            let pass_count = package_results.iter().filter(|(_, r)| r == "PASS").count();
            let fail_count = package_results.iter().filter(|(_, r)| r == "FAIL").count();
            summary.push(format!(
                "Packages: {} passed, {} failed",
                pass_count, fail_count
            ));
        }

        // Test results
        if passed > 0 || failed > 0 || skipped > 0 {
            summary.push(format!(
                "Tests: {} passed, {} failed, {} skipped",
                passed, failed, skipped
            ));
        }

        // List failed tests (up to 10)
        if !failed_tests.is_empty() {
            summary.push(String::new());
            summary.push("Failed tests:".to_string());
            for test in failed_tests.iter().take(10) {
                summary.push(format!("  - {}", test));
            }
            if failed_tests.len() > 10 {
                summary.push(format!("  ... and {} more", failed_tests.len() - 10));
            }
        }

        if summary.is_empty() {
            // If no structured output found, return original
            if output.trim().is_empty() {
                Ok("(no test output)".to_string())
            } else {
                Ok(output.to_string())
            }
        } else {
            Ok(summary.join("\n"))
        }
    }

    /// Handle go build command - show only errors/warnings
    fn handle_build(&self, output: &str) -> Result<String> {
        if output.trim().is_empty() {
            return Ok("(build succeeded)".to_string());
        }

        // Extract errors and warnings
        // Go build output format: file:line:col: message
        let mut errors: Vec<&str> = Vec::new();
        let mut warnings: Vec<&str> = Vec::new();

        for line in output.lines() {
            let line_lower = line.to_lowercase();

            // Go compiler errors typically contain "error" or start with file:line:col: syntax error
            // Also check for specific error patterns
            let is_error = line_lower.contains("error:")
                || line_lower.contains("error[")
                || line_lower.contains(": error")
                || (line.contains(".go:") && line_lower.contains(" error"))
                || line_lower.contains("undefined:")
                || line_lower.contains("syntax error")
                || line_lower.contains("invalid syntax")
                || line_lower.contains("cannot find")
                || line_lower.contains("not declared")
                || line_lower.contains("not used");

            let is_warning = line_lower.contains("warning:")
                || line_lower.contains("warn[")
                || line_lower.contains("deprecated");

            if is_error {
                errors.push(line);
            } else if is_warning {
                warnings.push(line);
            }
        }

        if errors.is_empty() && warnings.is_empty() {
            // No explicit errors/warnings, check for build failure indicators
            if output.to_lowercase().contains("build failed")
                || output.to_lowercase().contains("cannot find")
            {
                return self.error_strategy.compress(output);
            }
            return Ok("(build succeeded)".to_string());
        }

        let mut result = Vec::new();

        if !errors.is_empty() {
            result.push(format!("{} error(s):", errors.len()));
            for error in errors.iter().take(20) {
                result.push(format!("  {}", error));
            }
            if errors.len() > 20 {
                result.push(format!("  ... and {} more", errors.len() - 20));
            }
        }

        if !warnings.is_empty() {
            if !result.is_empty() {
                result.push(String::new());
            }
            result.push(format!("{} warning(s):", warnings.len()));
            for warning in warnings.iter().take(10) {
                result.push(format!("  {}", warning));
            }
            if warnings.len() > 10 {
                result.push(format!("  ... and {} more", warnings.len() - 10));
            }
        }

        Ok(result.join("\n"))
    }

    /// Handle go vet command - show issues only
    fn handle_vet(&self, output: &str) -> Result<String> {
        if output.trim().is_empty() {
            return Ok("(no issues)".to_string());
        }

        // Extract vet issues
        let issues: Vec<&str> = output
            .lines()
            .filter(|line| {
                // Vet output format: file:line: message
                line.contains(".go:") && !line.trim().is_empty()
            })
            .collect();

        if issues.is_empty() {
            return Ok("(no issues)".to_string());
        }

        let mut result = vec![format!("{} issue(s) found:", issues.len())];
        for issue in issues.iter().take(20) {
            result.push(format!("  {}", issue));
        }
        if issues.len() > 20 {
            result.push(format!("  ... and {} more", issues.len() - 20));
        }

        Ok(result.join("\n"))
    }

    /// Handle go mod command - show summary
    fn handle_mod(&self, output: &str, exit_code: i32) -> Result<String> {
        if exit_code == 0 {
            if output.trim().is_empty() {
                return Ok("(success)".to_string());
            }
            // Extract key info from go mod output
            let lines: Vec<&str> = output.lines().take(5).collect();
            Ok(lines.join("\n"))
        } else {
            self.error_strategy.compress(output)
        }
    }

    /// Handle go version command
    fn handle_version(&self, output: &str) -> Result<String> {
        // Just return the version line
        let version = output.lines().next().unwrap_or("").trim();
        if version.is_empty() {
            Ok("(no version info)".to_string())
        } else {
            Ok(version.to_string())
        }
    }

    /// Handle go fmt command - silent on success
    fn handle_fmt(&self, output: &str, exit_code: i32) -> Result<String> {
        if exit_code == 0 {
            // go fmt is silent on success, show formatted files if any
            if output.trim().is_empty() {
                Ok(String::new())
            } else {
                let files: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
                if files.is_empty() {
                    Ok(String::new())
                } else {
                    Ok(format!("{} file(s) formatted", files.len()))
                }
            }
        } else {
            self.error_strategy.compress(output)
        }
    }

    /// Handle go run command
    fn handle_run(&self, output: &str, exit_code: i32) -> Result<String> {
        if exit_code == 0 {
            // Show program output, but truncate if too long
            let lines: Vec<&str> = output.lines().collect();
            if lines.is_empty() {
                Ok("(no output)".to_string())
            } else if lines.len() <= 20 {
                Ok(output.to_string())
            } else {
                let shown: Vec<&str> = lines.iter().take(15).copied().collect();
                let mut result = shown.join("\n");
                result.push_str(&format!("\n... ({} more lines)", lines.len() - 15));
                Ok(result)
            }
        } else {
            self.error_strategy.compress(output)
        }
    }
}

impl Default for GoModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for GoModule {
    fn name(&self) -> &str {
        "go"
    }

    fn strategy(&self) -> &str {
        "ndjson_parsing"
    }

    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        let subcommand = context
            .command
            .as_ref()
            .and_then(|cmd| self.detect_subcommand(cmd));

        match subcommand.as_deref() {
            Some("test") => {
                // When command context is available, use it for JSON detection
                // Otherwise detect JSON from output format itself
                let cmd_str = context.command.as_deref();
                self.handle_test(output, cmd_str.unwrap_or(""))
            }
            Some("build") => self.handle_build(output),
            Some("vet") => self.handle_vet(output),
            Some("mod") => self.handle_mod(output, context.exit_code),
            Some("version") => self.handle_version(output),
            Some("fmt") => self.handle_fmt(output, context.exit_code),
            Some("run") => self.handle_run(output, context.exit_code),
            _ => {
                // Default to error-only for unknown commands
                self.error_strategy.compress(output)
            }
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
    fn test_go_test_json_pass() {
        let module = GoModule::new();
        let input = r#"{"Time":"2024-01-01T00:00:00.000000Z","Action":"start","Package":"example.com/pkg"}
{"Time":"2024-01-01T00:00:00.100000Z","Action":"run","Test":"TestAdd","Package":"example.com/pkg"}
{"Time":"2024-01-01T00:00:00.200000Z","Action":"output","Test":"TestAdd","Package":"example.com/pkg","Output":"=== RUN   TestAdd\n"}
{"Time":"2024-01-01T00:00:00.300000Z","Action":"pass","Test":"TestAdd","Package":"example.com/pkg","Elapsed":0.1}
{"Time":"2024-01-01T00:00:00.400000Z","Action":"pass","Package":"example.com/pkg","Elapsed":0.2}
"#;
        let result = module
            .compress(input, &make_context("go test -json ./...", 0))
            .unwrap();

        assert!(result.contains("1 passed"));
        assert!(result.contains("0 failed"));
    }

    #[test]
    fn test_go_test_json_fail() {
        let module = GoModule::new();
        let input = r#"{"Time":"2024-01-01T00:00:00.000000Z","Action":"start","Package":"example.com/pkg"}
{"Time":"2024-01-01T00:00:00.100000Z","Action":"run","Test":"TestFail","Package":"example.com/pkg"}
{"Time":"2024-01-01T00:00:00.200000Z","Action":"fail","Test":"TestFail","Package":"example.com/pkg","Elapsed":0.1}
{"Time":"2024-01-01T00:00:00.300000Z","Action":"fail","Package":"example.com/pkg","Elapsed":0.2}
"#;
        let result = module
            .compress(input, &make_context("go test -json", 1))
            .unwrap();

        assert!(result.contains("0 passed"));
        assert!(result.contains("1 failed"));
        assert!(result.contains("TestFail"));
    }

    #[test]
    fn test_go_test_json_skip() {
        let module = GoModule::new();
        let input = r#"{"Time":"2024-01-01T00:00:00.000000Z","Action":"start","Package":"example.com/pkg"}
{"Time":"2024-01-01T00:00:00.100000Z","Action":"skip","Test":"TestSkip","Package":"example.com/pkg"}
{"Time":"2024-01-01T00:00:00.200000Z","Action":"pass","Package":"example.com/pkg","Elapsed":0.1}
"#;
        let result = module
            .compress(input, &make_context("go test -json", 0))
            .unwrap();

        assert!(result.contains("1 skipped"));
    }

    #[test]
    fn test_go_test_text_pass() {
        let module = GoModule::new();
        let input = r#"=== RUN   TestAdd
--- PASS: TestAdd (0.00s)
=== RUN   TestSubtract
--- PASS: TestSubtract (0.00s)
PASS
ok      example.com/pkg 0.002s
"#;
        let result = module
            .compress(input, &make_context("go test ./...", 0))
            .unwrap();

        assert!(result.contains("2 passed"));
        assert!(result.contains("0 failed"));
        assert!(result.contains("1 passed") || result.contains("Packages:"));
    }

    #[test]
    fn test_go_test_text_fail() {
        let module = GoModule::new();
        let input = r#"=== RUN   TestPass
--- PASS: TestPass (0.00s)
=== RUN   TestFail
--- FAIL: TestFail (0.00s)
    test_test.go:10: unexpected error
FAIL
FAIL    example.com/pkg 0.002s
"#;
        let result = module.compress(input, &make_context("go test", 1)).unwrap();

        assert!(result.contains("1 passed"));
        assert!(result.contains("1 failed"));
        assert!(result.contains("TestFail"));
    }

    #[test]
    fn test_go_build_success() {
        let module = GoModule::new();
        let result = module
            .compress("", &make_context("go build ./...", 0))
            .unwrap();

        assert_eq!(result, "(build succeeded)");
    }

    #[test]
    fn test_go_build_errors() {
        let module = GoModule::new();
        let input = r#"main.go:10:5: undefined: foo
main.go:15:2: syntax error
warning: unused variable x
"#;
        let result = module
            .compress(input, &make_context("go build", 1))
            .unwrap();

        assert!(result.contains("2 error(s)"));
        assert!(result.contains("1 warning(s)"));
    }

    #[test]
    fn test_go_build_warnings_only() {
        let module = GoModule::new();
        let input = r#"main.go:10:5: warning: unused variable x
main.go:20:3: warning: deprecated function
"#;
        let result = module
            .compress(input, &make_context("go build", 0))
            .unwrap();

        assert!(result.contains("2 warning(s)"));
        assert!(!result.contains("error"));
    }

    #[test]
    fn test_go_vet_clean() {
        let module = GoModule::new();
        let result = module
            .compress("", &make_context("go vet ./...", 0))
            .unwrap();

        assert_eq!(result, "(no issues)");
    }

    #[test]
    fn test_go_vet_issues() {
        let module = GoModule::new();
        let input = r#"main.go:10:5: unreachable code
main.go:20:3: printf: invalid format string
main.go:30:1: unused variable
"#;
        let result = module
            .compress(input, &make_context("go vet ./...", 1))
            .unwrap();

        assert!(result.contains("3 issue(s)"));
        assert!(result.contains("unreachable code"));
    }

    #[test]
    fn test_go_version() {
        let module = GoModule::new();
        let input = "go version go1.21.0 linux/amd64";
        let result = module
            .compress(input, &make_context("go version", 0))
            .unwrap();

        assert!(result.contains("go1.21.0"));
    }

    #[test]
    fn test_go_fmt_success() {
        let module = GoModule::new();
        let result = module
            .compress("", &make_context("go fmt ./...", 0))
            .unwrap();

        assert_eq!(result, "");
    }

    #[test]
    fn test_go_fmt_files() {
        let module = GoModule::new();
        let input = "main.go\nutils.go\n";
        let result = module
            .compress(input, &make_context("go fmt ./...", 0))
            .unwrap();

        assert!(result.contains("2 file(s) formatted"));
    }

    #[test]
    fn test_go_mod_success() {
        let module = GoModule::new();
        let result = module
            .compress("", &make_context("go mod tidy", 0))
            .unwrap();

        assert_eq!(result, "(success)");
    }

    #[test]
    fn test_go_run_success() {
        let module = GoModule::new();
        let input = "Hello, World!\n";
        let result = module
            .compress(input, &make_context("go run main.go", 0))
            .unwrap();

        assert!(result.contains("Hello, World!"));
    }

    #[test]
    fn test_go_run_error() {
        let module = GoModule::new();
        let input = "panic: something went wrong\n\ngoroutine 1 [running]:\nmain.main()\n\tmain.go:10 +0x20";
        let result = module
            .compress(input, &make_context("go run main.go", 1))
            .unwrap();

        assert!(result.contains("panic") || result.contains("error"));
    }

    #[test]
    fn test_unknown_go_command() {
        let module = GoModule::new();
        let input = "some output\nerror: something failed";
        let result = module
            .compress(input, &make_context("go unknown", 1))
            .unwrap();

        // Should fall back to error-only
        assert!(result.contains("error"));
    }

    #[test]
    fn test_go_test_json_multiple_packages() {
        let module = GoModule::new();
        let input = r#"{"Time":"2024-01-01T00:00:00.000000Z","Action":"start","Package":"pkg1"}
{"Time":"2024-01-01T00:00:00.100000Z","Action":"pass","Test":"Test1","Package":"pkg1"}
{"Time":"2024-01-01T00:00:00.200000Z","Action":"pass","Package":"pkg1"}
{"Time":"2024-01-01T00:00:00.300000Z","Action":"start","Package":"pkg2"}
{"Time":"2024-01-01T00:00:00.400000Z","Action":"fail","Test":"Test2","Package":"pkg2"}
{"Time":"2024-01-01T00:00:00.500000Z","Action":"fail","Package":"pkg2"}
"#;
        let result = module
            .compress(input, &make_context("go test -json ./...", 1))
            .unwrap();

        assert!(result.contains("1 passed") || result.contains("Packages:"));
        assert!(result.contains("1 failed"));
    }

    #[test]
    fn test_go_test_empty_output() {
        let module = GoModule::new();
        let result = module.compress("", &make_context("go test", 0)).unwrap();

        assert_eq!(result, "(no test output)");
    }

    #[test]
    fn test_go_test_json_malformed_lines() {
        let module = GoModule::new();
        // NDJSON with some malformed lines
        let input = r#"{"Time":"2024-01-01T00:00:00.000000Z","Action":"start","Package":"example.com/pkg"}
this is not valid json
{"Time":"2024-01-01T00:00:00.100000Z","Action":"pass","Test":"TestAdd","Package":"example.com/pkg"}
{broken json here
{"Time":"2024-01-01T00:00:00.200000Z","Action":"pass","Package":"example.com/pkg"}
"#;
        let result = module
            .compress(input, &make_context("go test -json", 0))
            .unwrap();

        // Should skip malformed lines and parse valid ones
        assert!(result.contains("1 passed"));
    }

    #[test]
    fn test_go_test_json_unicode_test_names() {
        let module = GoModule::new();
        // Test with Unicode in test names
        let input = r#"{"Time":"2024-01-01T00:00:00.000000Z","Action":"start","Package":"example.com/pkg"}
{"Time":"2024-01-01T00:00:00.100000Z","Action":"pass","Test":"Test日本語","Package":"example.com/pkg"}
{"Time":"2024-01-01T00:00:00.200000Z","Action":"fail","Test":"TestÉmoji🚀","Package":"example.com/pkg"}
{"Time":"2024-01-01T00:00:00.300000Z","Action":"pass","Package":"example.com/pkg"}
"#;
        let result = module
            .compress(input, &make_context("go test -json", 1))
            .unwrap();

        assert!(result.contains("1 passed"));
        assert!(result.contains("1 failed"));
        // Unicode test name should appear in failed tests
        assert!(result.contains("TestÉmoji"));
    }

    #[test]
    fn test_go_test_json_empty_lines() {
        let module = GoModule::new();
        // NDJSON with empty lines between
        let input = r#"{"Time":"2024-01-01T00:00:00.000000Z","Action":"start","Package":"pkg"}

{"Time":"2024-01-01T00:00:00.100000Z","Action":"pass","Test":"Test1","Package":"pkg"}

{"Time":"2024-01-01T00:00:00.200000Z","Action":"pass","Package":"pkg"}
"#;
        let result = module
            .compress(input, &make_context("go test -json", 0))
            .unwrap();

        // Should handle empty lines gracefully
        assert!(result.contains("1 passed"));
    }

    #[test]
    fn test_go_build_unicode_errors() {
        let module = GoModule::new();
        // Build errors with Unicode
        let input = r#"main.go:10:5: undefined: café
main.go:20:3: 语法错误: invalid syntax
"#;
        let result = module
            .compress(input, &make_context("go build", 1))
            .unwrap();

        // Should handle Unicode in error messages
        assert!(result.contains("2 error"));
    }

    #[test]
    fn test_go_test_text_unicode() {
        let module = GoModule::new();
        // Text output with Unicode test names
        let input = r#"=== RUN   Test日本語
--- PASS: Test日本語 (0.00s)
=== RUN   TestÉmoji🚀
--- FAIL: TestÉmoji🚀 (0.00s)
FAIL
"#;
        let result = module.compress(input, &make_context("go test", 1)).unwrap();

        assert!(result.contains("1 passed"));
        assert!(result.contains("1 failed"));
        assert!(result.contains("TestÉmoji"));
    }
}
