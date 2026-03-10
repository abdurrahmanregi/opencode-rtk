use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, GroupingByPattern, Strategy},
    Context,
};
use anyhow::Result;

pub struct CurlModule {
    error_strategy: ErrorOnly,
    grouping_strategy: GroupingByPattern,
}

impl CurlModule {
    pub fn new() -> Self {
        Self {
            error_strategy: ErrorOnly,
            grouping_strategy: GroupingByPattern,
        }
    }

    /// Extract response summary from curl output
    fn extract_response_summary(&self, output: &str) -> Result<String> {
        let mut status_code: Option<String> = None;
        let mut content_type: Option<String> = None;
        let mut content_length: Option<String> = None;
        let mut has_response_body = false;
        let mut error_lines: Vec<String> = Vec::new();

        for line in output.lines() {
            let trimmed = line.trim();

            // Remove curl's verbose prefixes (< for incoming, > for outgoing)
            let cleaned = if let Some(stripped) = trimmed.strip_prefix("< ") {
                stripped
            } else {
                trimmed
            };

            // Extract HTTP status code
            if cleaned.starts_with("HTTP/") {
                // Format: HTTP/1.1 200 OK
                let parts: Vec<&str> = cleaned.split_whitespace().collect();
                if parts.len() >= 2 {
                    status_code = Some(format!("{} {}", parts[1], parts.get(2).unwrap_or(&"")));
                }
            }

            // Extract Content-Type header
            if cleaned.to_lowercase().starts_with("content-type:") {
                content_type = Some(
                    cleaned
                        .split(':')
                        .nth(1)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default(),
                );
            }

            // Extract Content-Length header
            if cleaned.to_lowercase().starts_with("content-length:") {
                content_length = Some(
                    cleaned
                        .split(':')
                        .nth(1)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default(),
                );
            }

            // Check for response body (non-header lines after empty line)
            if !trimmed.is_empty() && !trimmed.contains(':') && !trimmed.starts_with("HTTP/") {
                has_response_body = true;
            }

            // Check for curl errors
            if trimmed.starts_with("curl:")
                || trimmed.contains("Connection refused")
                || trimmed.contains("Could not resolve host")
                || trimmed.contains("Failed to connect")
                || trimmed.contains("SSL certificate problem")
            {
                error_lines.push(trimmed.to_string());
            }
        }

        // If there are errors, return them
        if !error_lines.is_empty() {
            return Ok(format!("Error: {}", error_lines.join("; ")));
        }

        // Build summary
        let mut parts = Vec::new();

        if let Some(status) = status_code {
            parts.push(format!("Status: {}", status));
        }

        if let Some(ct) = content_type {
            parts.push(format!("Content-Type: {}", ct));
        }

        if let Some(length) = content_length {
            parts.push(format!("Content-Length: {} bytes", length));
        }

        if has_response_body && parts.is_empty() {
            // Has body but no headers captured - return grouped output
            return self.grouping_strategy.compress(output);
        }

        if parts.is_empty() {
            if output.is_empty() {
                return Ok("(no output)".to_string());
            }
            // Fallback: return the output as-is but truncated if long
            let lines: Vec<&str> = output.lines().take(5).collect();
            if lines.len() < output.lines().count() {
                return Ok(format!("{}\n... (truncated)", lines.join("\n")));
            }
            return Ok(output.to_string());
        }

        Ok(parts.join("\n"))
    }

    /// Check if verbose mode was used (-v or --verbose)
    fn is_verbose_output(&self, command: &str) -> bool {
        // Parse command args properly to avoid false positives
        // e.g., URLs like "http://example.com/view" shouldn't match
        let args: Vec<&str> = command.split_whitespace().collect();
        args.iter().any(|arg| *arg == "-v" || *arg == "--verbose")
    }

    fn extract_curl_errors(&self, output: &str) -> String {
        let mut errors = Vec::new();

        for line in output.lines() {
            let trimmed = line.trim();

            // curl error patterns: "curl: (X) Error message"
            if trimmed.starts_with("curl:")
                || trimmed.contains("Connection refused")
                || trimmed.contains("Could not resolve host")
                || trimmed.contains("Failed to connect")
                || trimmed.contains("SSL certificate problem")
                || trimmed.contains("Operation timed out")
            {
                errors.push(trimmed.to_string());
            }
        }

        errors.join("\n")
    }
}

impl Default for CurlModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for CurlModule {
    fn name(&self) -> &str {
        "curl"
    }

    fn strategy(&self) -> &str {
        "multi_strategy"
    }

    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        // Check for curl-specific errors first (regardless of exit code)
        let curl_errors = self.extract_curl_errors(output);
        if !curl_errors.is_empty() {
            return Ok(curl_errors);
        }

        // On failure, show errors only
        if context.exit_code != 0 {
            return self.error_strategy.compress(output);
        }

        // For verbose output, extract headers summary
        if let Some(cmd) = &context.command {
            if self.is_verbose_output(cmd) {
                return self.extract_response_summary(output);
            }
        }

        // For normal output, check if it's headers or body
        if output.contains("HTTP/") {
            return self.extract_response_summary(output);
        }

        // For simple body output, use grouping for repetitive content
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() > 20 {
            return self.grouping_strategy.compress(output);
        }

        if output.is_empty() {
            return Ok("(no output)".to_string());
        }

        Ok(output.to_string())
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
    fn test_curl_verbose_success() {
        let module = CurlModule::new();
        let input = r#"*   Trying 93.184.216.34:443...
* Connected to example.com (93.184.216.34) port 443
* SSL connection using TLSv1.3
> GET / HTTP/1.1
> Host: example.com
> User-Agent: curl/8.0
>
< HTTP/1.1 200 OK
< Content-Type: text/html; charset=UTF-8
< Content-Length: 1256
< Date: Mon, 01 Jan 2024 12:00:00 GMT
<
<!doctype html>
<html>
</html>
"#;
        let result = module
            .compress(input, &make_context("curl -v https://example.com", 0))
            .unwrap();

        assert!(result.contains("Status: 200"));
        assert!(result.contains("text/html"));
        assert!(result.contains("1256"));
    }

    #[test]
    fn test_curl_error() {
        let module = CurlModule::new();
        let input = "curl: (6) Could not resolve host: nonexistent.example.com";
        let result = module
            .compress(
                input,
                &make_context("curl https://nonexistent.example.com", 6),
            )
            .unwrap();

        assert!(result.contains("Could not resolve host") || result.contains("curl:"));
    }

    #[test]
    fn test_curl_connection_refused() {
        let module = CurlModule::new();
        let input = "curl: (7) Failed to connect to localhost port 9999: Connection refused";
        let result = module
            .compress(input, &make_context("curl http://localhost:9999", 7))
            .unwrap();

        assert!(result.contains("Connection refused") || result.contains("Failed to connect"));
    }

    #[test]
    fn test_curl_headers_only() {
        let module = CurlModule::new();
        let input = r#"HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 42
Date: Mon, 01 Jan 2024 12:00:00 GMT
"#;
        let result = module
            .compress(input, &make_context("curl -I https://example.com", 0))
            .unwrap();

        assert!(result.contains("Status: 200"));
        assert!(result.contains("application/json"));
    }

    #[test]
    fn test_curl_empty_output() {
        let module = CurlModule::new();
        let result = module.compress("", &make_context("curl", 0)).unwrap();

        assert_eq!(result, "(no output)");
    }

    #[test]
    fn test_curl_redirect() {
        let module = CurlModule::new();
        let input = r#"HTTP/1.1 301 Moved Permanently
Location: https://www.example.com/
Content-Type: text/html
Content-Length: 162
"#;
        let result = module
            .compress(input, &make_context("curl -I http://example.com", 0))
            .unwrap();

        assert!(result.contains("Status: 301"));
    }

    #[test]
    fn test_curl_404_error() {
        let module = CurlModule::new();
        let input = r#"HTTP/1.1 404 Not Found
Content-Type: text/html
Content-Length: 162
"#;
        let result = module
            .compress(
                input,
                &make_context("curl -I https://example.com/notfound", 0),
            )
            .unwrap();

        // 404 is still HTTP success (exit code 0), so we show the status
        assert!(result.contains("Status: 404"));
    }

    #[test]
    fn test_curl_json_body() {
        let module = CurlModule::new();
        let input = r#"{"status":"ok","data":[1,2,3]}"#;
        let result = module
            .compress(
                input,
                &make_context("curl https://api.example.com/status", 0),
            )
            .unwrap();

        // Simple body should be returned as-is if short
        assert!(result.contains("status") || result.contains("ok"));
    }

    #[test]
    fn test_curl_verbose_detection() {
        let module = CurlModule::new();

        assert!(module.is_verbose_output("curl -v https://example.com"));
        assert!(module.is_verbose_output("curl --verbose https://example.com"));
        assert!(module.is_verbose_output("curl -v -X POST https://example.com"));
        assert!(!module.is_verbose_output("curl https://example.com"));
        assert!(!module.is_verbose_output("curl -s https://example.com"));
    }

    #[test]
    fn test_curl_verbose_false_positives() {
        let module = CurlModule::new();

        // URLs containing "view" or "verbose" should not trigger verbose mode
        assert!(!module.is_verbose_output("curl https://example.com/view"));
        assert!(!module.is_verbose_output("curl https://example.com/verbose"));
        assert!(!module.is_verbose_output("curl https://example.com/view/page"));

        // Commands with -v in URL should not trigger verbose mode
        assert!(!module.is_verbose_output("curl https://example.com/v1/api"));
        assert!(!module.is_verbose_output("curl https://example.com?version=v2"));

        // Only actual -v or --verbose flags should trigger
        assert!(module.is_verbose_output("curl -v https://example.com/view"));
        assert!(module.is_verbose_output("curl --verbose https://example.com/verbose"));
    }

    #[test]
    fn test_curl_verbose_with_data() {
        let module = CurlModule::new();

        // -v flag should be detected even with other flags
        assert!(module.is_verbose_output("curl -v -X POST -d 'data' https://example.com"));
        assert!(module.is_verbose_output("curl -X POST -v -d 'data' https://example.com"));
        assert!(module.is_verbose_output("curl -d 'data' -v https://example.com"));
    }

    #[test]
    fn test_curl_non_verbose_with_v_in_url() {
        let module = CurlModule::new();
        let input = r#"{"status":"ok","view":"main"}"#;

        let result = module
            .compress(input, &make_context("curl https://api.example.com/view", 0))
            .unwrap();

        // Should not treat as verbose output since no -v flag
        assert!(result.contains("status") || result.contains("ok"));
    }
}
