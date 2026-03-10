use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, Strategy},
    Context,
};
use anyhow::Result;
use std::collections::HashMap;

pub struct PytestModule {
    error_strategy: ErrorOnly,
}

impl PytestModule {
    pub fn new() -> Self {
        Self {
            error_strategy: ErrorOnly,
        }
    }

    /// Parse pytest output
    fn parse_pytest_output(&self, output: &str) -> Result<String> {
        let mut passed = 0;
        let mut failed = 0;
        let mut skipped = 0;
        let mut xfailed = 0;
        let mut xpassed = 0;
        let mut warnings = 0;
        let mut errors = 0;

        let mut failures: Vec<String> = Vec::new();
        let mut skip_reasons: Vec<String> = Vec::new();
        let mut in_failure_section = false;
        let mut current_failure: Vec<String> = Vec::new();

        for line in output.lines() {
            let trimmed = line.trim();

            // Detect failure section (pytest uses === markers)
            if trimmed.contains("FAILURES")
                && trimmed.starts_with("===")
                && trimmed.ends_with("===")
            {
                in_failure_section = true;
                continue;
            }

            // End of failure section (new section starting)
            if in_failure_section
                && trimmed.starts_with("===")
                && trimmed.ends_with("===")
                && !trimmed.contains("FAILURES")
            {
                if !current_failure.is_empty() {
                    failures.push(current_failure.join("\n"));
                    current_failure.clear();
                }
                in_failure_section = false;
            }

            // Collect failure content
            if in_failure_section && !trimmed.is_empty() {
                // Check for new failure header (line starting and ending with underscores)
                if trimmed.starts_with('_') && trimmed.ends_with('_') && trimmed.len() > 10 {
                    // Save previous failure if any
                    if !current_failure.is_empty() {
                        failures.push(current_failure.join("\n"));
                    }
                    current_failure.clear();
                    current_failure.push(trimmed.to_string());
                } else if !current_failure.is_empty() {
                    // Only add content if we're in a failure block
                    current_failure.push(trimmed.to_string());
                }
            }

            // Parse individual test results (must have :: to be a test result line)
            if trimmed.contains("::") {
                if trimmed.contains(" PASSED") || trimmed.ends_with(" PASSED") {
                    passed += 1;
                } else if trimmed.contains(" FAILED") || trimmed.ends_with(" FAILED") {
                    failed += 1;
                } else if trimmed.contains(" SKIPPED") || trimmed.ends_with(" SKIPPED") {
                    skipped += 1;
                    if let Some(reason) = self.extract_skip_reason(trimmed) {
                        skip_reasons.push(reason);
                    }
                } else if trimmed.contains(" XFAILED") || trimmed.ends_with(" XFAILED") {
                    xfailed += 1;
                } else if trimmed.contains(" XPASS") || trimmed.contains(" XPASSED") {
                    xpassed += 1;
                } else if (trimmed.contains(" ERROR") || trimmed.ends_with(" ERROR"))
                    && !trimmed.contains("ERRORS")
                {
                    errors += 1;
                }
            }

            // Parse warnings from summary
            if trimmed.contains("warning") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                for (i, part) in parts.iter().enumerate() {
                    if part.contains("warning") && i > 0 {
                        if let Ok(count) = parts[i - 1].parse::<usize>() {
                            warnings = count;
                        }
                    }
                }
            }
        }

        // Add last failure if any
        if !current_failure.is_empty() {
            failures.push(current_failure.join("\n"));
        }

        self.build_output(
            passed,
            failed,
            skipped,
            xfailed,
            xpassed,
            errors,
            warnings,
            &failures,
            &skip_reasons,
        )
    }

    fn extract_skip_reason(&self, line: &str) -> Option<String> {
        // Format: SKIPPED [reason] or :: SKIPPED (reason)
        if let Some(start) = line.rfind('(') {
            if let Some(end) = line.rfind(')') {
                if start < end {
                    let reason = &line[start + 1..end];
                    return Some(reason.to_string());
                }
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn build_output(
        &self,
        passed: usize,
        failed: usize,
        skipped: usize,
        xfailed: usize,
        xpassed: usize,
        errors: usize,
        warnings: usize,
        failures: &[String],
        skip_reasons: &[String],
    ) -> Result<String> {
        let mut parts = Vec::new();

        // Summary
        let mut summary_parts = Vec::new();
        if passed > 0 {
            summary_parts.push(format!("{} passed", passed));
        }
        if failed > 0 {
            summary_parts.push(format!("{} failed", failed));
        }
        if skipped > 0 {
            summary_parts.push(format!("{} skipped", skipped));
        }
        if xfailed > 0 {
            summary_parts.push(format!("{} xfailed", xfailed));
        }
        if xpassed > 0 {
            summary_parts.push(format!("{} xpassed", xpassed));
        }
        if errors > 0 {
            summary_parts.push(format!("{} errors", errors));
        }
        if warnings > 0 {
            summary_parts.push(format!("{} warnings", warnings));
        }

        if summary_parts.is_empty() {
            return Ok("(no tests)".to_string());
        }

        parts.push(summary_parts.join(", "));

        // Add failures (limited)
        if !failures.is_empty() {
            parts.push(String::new());
            parts.push("=== Failures ===".to_string());
            for failure in failures.iter().take(3) {
                parts.push(failure.clone());
                parts.push(String::new());
            }
            if failures.len() > 3 {
                parts.push(format!("... and {} more failures", failures.len() - 3));
            }
        }

        // Add unique skip reasons
        if !skip_reasons.is_empty() {
            let unique_reasons: HashMap<&str, usize> =
                skip_reasons.iter().fold(HashMap::new(), |mut acc, r| {
                    *acc.entry(r.as_str()).or_insert(0) += 1;
                    acc
                });

            let mut reason_vec: Vec<_> = unique_reasons.into_iter().collect();
            reason_vec.sort_by(|a, b| b.1.cmp(&a.1));

            parts.push(String::new());
            parts.push("=== Skip Reasons ===".to_string());
            for (reason, count) in reason_vec.iter().take(5) {
                parts.push(format!("{} ({})", reason, count));
            }
        }

        Ok(parts.join("\n"))
    }

    /// Check if output has pytest-style formatting
    fn is_pytest_output(&self, output: &str) -> bool {
        // Check for pytest test result markers
        (output.contains("::")
            && (output.contains("PASSED")
                || output.contains("FAILED")
                || output.contains("SKIPPED")
                || output.contains("XFAILED")
                || output.contains("XPASS")
                || output.contains("ERROR")))
            || output.contains("test session")
            || output.contains("short test summary")
            || (output.contains("collected") && output.contains("items"))
    }
}

impl Default for PytestModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for PytestModule {
    fn name(&self) -> &str {
        "pytest"
    }

    fn strategy(&self) -> &str {
        "state_machine"
    }

    fn compress(&self, output: &str, _context: &Context) -> Result<String> {
        if output.is_empty() {
            return Ok("(no output)".to_string());
        }

        if self.is_pytest_output(output) {
            self.parse_pytest_output(output)
        } else {
            // Fallback to error-only strategy for non-pytest output
            self.error_strategy.compress(output)
        }
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
    fn test_pytest_all_passed() {
        let module = PytestModule::new();
        let input = r#"============================= test session starts ==============================
collected 5 items

test_one.py::test_one PASSED
test_two.py::test_two PASSED
test_three.py::test_three PASSED
test_four.py::test_four PASSED
test_five.py::test_five PASSED

============================== 5 passed in 0.12s ===============================
"#;
        let result = module.compress(input, &make_context("pytest")).unwrap();

        assert!(result.contains("5 passed"), "Result was: {}", result);
        assert!(!result.contains("failed"));
    }

    #[test]
    fn test_pytest_with_failures() {
        let module = PytestModule::new();
        let input = r#"============================= test session starts ==============================
collected 3 items

test_one.py::test_one PASSED
test_two.py::test_two FAILED
test_three.py::test_three PASSED

=================================== FAILURES ====================================
_________________________________ test_two ____________________________________

    def test_two():
>       assert 1 == 2
E       assert 1 == 2

test_two.py:4: AssertionError
=========================== short test summary info ============================
FAILED test_two.py::test_two - assert 1 == 2
========================= 1 failed, 2 passed in 0.05s ==========================
"#;
        let result = module.compress(input, &make_context("pytest")).unwrap();

        assert!(result.contains("2 passed"), "Result was: {}", result);
        assert!(result.contains("1 failed"));
        assert!(result.contains("Failures"));
    }

    #[test]
    fn test_pytest_with_skips() {
        let module = PytestModule::new();
        let input = r#"collected 4 items

test_one.py::test_one PASSED
test_two.py::test_two SKIPPED (skip if no network)
test_three.py::test_three PASSED
test_four.py::test_four SKIPPED (skip if no network)

========================= 2 passed, 2 skipped in 0.05s =========================
"#;
        let result = module.compress(input, &make_context("pytest")).unwrap();

        assert!(result.contains("2 passed"), "Result was: {}", result);
        assert!(result.contains("2 skipped"));
        assert!(result.contains("Skip Reasons"));
        assert!(result.contains("skip if no network"));
    }

    #[test]
    fn test_pytest_with_xfail() {
        let module = PytestModule::new();
        let input = r#"collected 3 items

test_one.py::test_one PASSED
test_two.py::test_two XFAILED (expected to fail)
test_three.py::test_three XPASS

===================== 1 passed, 1 xfailed, 1 xpassed ===========================
"#;
        let result = module.compress(input, &make_context("pytest")).unwrap();

        assert!(result.contains("1 passed"), "Result was: {}", result);
        assert!(result.contains("1 xfailed"));
        assert!(result.contains("1 xpassed"));
    }

    #[test]
    fn test_pytest_with_errors() {
        let module = PytestModule::new();
        let input = r#"collected 2 items

test_one.py::test_one PASSED
test_two.py::test_two ERROR

==================================== ERRORS =====================================
____________________ ERROR at setup of test_two ________________________________

    @pytest.fixture
    def broken_fixture():
>       raise Exception("broken")
E       Exception: broken

test_two.py:10: Exception
========================= 1 passed, 1 error in 0.05s ===========================
"#;
        let result = module.compress(input, &make_context("pytest")).unwrap();

        assert!(result.contains("1 passed"), "Result was: {}", result);
        assert!(result.contains("1 error"), "Result was: {}", result);
    }

    #[test]
    fn test_pytest_empty() {
        let module = PytestModule::new();
        let result = module.compress("", &make_context("pytest")).unwrap();

        assert_eq!(result, "(no output)");
    }

    #[test]
    fn test_pytest_no_tests() {
        let module = PytestModule::new();
        let input = "collected 0 items\n\n====== no tests ran in 0.01s ======";
        let result = module.compress(input, &make_context("pytest")).unwrap();

        // Since it has "collected", it should be parsed as pytest output
        // but with no test results, it returns "(no tests)"
        assert_eq!(result, "(no tests)");
    }

    #[test]
    fn test_pytest_non_pytest_output() {
        let module = PytestModule::new();
        let input = "Error: something went wrong\nFailed to run tests";
        let result = module.compress(input, &make_context("pytest")).unwrap();

        // Should fall back to error-only strategy
        assert!(result.contains("Error") || result.contains("Failed") || result == "(no errors)");
    }

    #[test]
    fn test_pytest_many_failures_truncated() {
        let module = PytestModule::new();
        let mut input = String::from("collected 10 items\n\n");
        for i in 1..=10 {
            input.push_str(&format!("test_{}.py::test_{} FAILED\n", i, i));
        }
        input.push_str("\n=== FAILURES ===\n");
        for i in 1..=10 {
            input.push_str(&format!(
                "_________________ test_{} _________________\n\n",
                i
            ));
            input.push_str(&format!("    def test_{}():\n", i));
            input.push_str(&format!(">       assert {} == {}\n", i, i + 1));
            input.push_str(&format!("E       AssertionError\n\n"));
        }
        input.push_str("\n10 failed in 0.50s\n");

        let result = module.compress(&input, &make_context("pytest")).unwrap();

        assert!(result.contains("10 failed"), "Result was: {}", result);
        // Should truncate failures
        assert!(
            result.contains("and") && result.contains("more failures"),
            "Result was: {}",
            result
        );
    }
}
