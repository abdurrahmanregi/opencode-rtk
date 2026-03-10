use super::Strategy;
use anyhow::Result;

pub struct ErrorOnly;

impl Strategy for ErrorOnly {
    fn name(&self) -> &str {
        "error_only"
    }

    fn compress(&self, input: &str) -> Result<String> {
        if input.is_empty() {
            return Ok("(empty)".to_string());
        }

        let error_lines: Vec<&str> = input
            .lines()
            .filter(|line| {
                let line_lower = line.to_lowercase();
                let trimmed = line_lower.trim();

                // Check for error patterns with better specificity
                // Patterns that indicate real errors:
                // 1. Lines starting with "error" or "fatal"
                // 2. "error:" or "error]" (with delimiter)
                // 3. "error" followed by a number (like "ERROR 404")
                // 4. Other error keywords
                let has_error = trimmed.starts_with("error")
                    || trimmed.starts_with("fatal")
                    || line_lower.contains("error:")
                    || line_lower.contains("error]")
                    || line_lower.contains("\"error\"") // JSON format
                    || line_lower.contains("failed:")
                    || line_lower.contains("exception:")
                    || line_lower.contains("panic")
                    // Match "ERROR 404" style (error followed by space and number/code)
                    || line_lower.split_whitespace().any(|word| {
                        word == "error" || word.starts_with("error:") || word == "failed"
                    });

                // Exclude false positives
                let is_false_positive = line_lower.contains("no errors")
                    || line_lower.contains("0 errors")
                    || line_lower.contains("no error")
                    || line_lower.contains("without error");

                has_error && !is_false_positive
            })
            .collect();

        if error_lines.is_empty() {
            return Ok("(no errors)".to_string());
        }

        // Limit to first 50 errors to avoid huge output
        let limited: Vec<&str> = error_lines.into_iter().take(50).collect();
        Ok(limited.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let result = ErrorOnly.compress("").unwrap();
        assert_eq!(result, "(empty)");
    }

    #[test]
    fn test_no_errors() {
        let input = "All tests passed\nSuccess\nOK";
        let result = ErrorOnly.compress(input).unwrap();
        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_with_errors() {
        let input = r#"Test 1 passed
Test 2 passed
Error: Test 3 failed
Test 4 passed
FAILED: test_4
Test 5 passed
"#;
        let result = ErrorOnly.compress(input).unwrap();
        assert!(result.contains("Error: Test 3 failed"));
        assert!(result.contains("FAILED: test_4"));
        assert!(!result.contains("Test 1 passed"));
    }

    #[test]
    fn test_false_positive_no_errors() {
        // Should NOT match "No errors found" or "0 errors"
        let input = "All tests passed\nNo errors found\n0 errors in codebase";
        let result = ErrorOnly.compress(input).unwrap();
        assert_eq!(result, "(no errors)");
    }

    #[test]
    fn test_error_with_colon() {
        let input = "error: something went wrong\nerror] another error\npanic: runtime error";
        let result = ErrorOnly.compress(input).unwrap();
        assert!(result.contains("error: something went wrong"));
        assert!(result.contains("error] another error"));
        assert!(result.contains("panic: runtime error"));
    }

    #[test]
    fn test_fatal_and_exception() {
        let input = "fatal: not a git repository\nException: NullPointerException";
        let result = ErrorOnly.compress(input).unwrap();
        assert!(result.contains("fatal: not a git repository"));
        assert!(result.contains("Exception: NullPointerException"));
    }
}
