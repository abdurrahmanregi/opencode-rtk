use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, GroupingByPattern, Strategy},
    Context,
};
use anyhow::Result;

pub struct WgetModule {
    error_strategy: ErrorOnly,
    grouping_strategy: GroupingByPattern,
}

impl WgetModule {
    pub fn new() -> Self {
        Self {
            error_strategy: ErrorOnly,
            grouping_strategy: GroupingByPattern,
        }
    }

    /// Extract download summary from wget output
    fn extract_download_summary(&self, output: &str) -> Result<String> {
        let mut saved = false;
        let mut file_path: Option<String> = None;
        let mut file_size: Option<String> = None;
        let mut speed: Option<String> = None;
        let mut time: Option<String> = None;
        let mut has_error = false;

        for line in output.lines() {
            let trimmed = line.trim();

            // Check for final saved message
            if trimmed.starts_with("Saving to:")
                || trimmed.contains("saved")
                || trimmed.starts_with("'") && trimmed.contains("' saved [")
            {
                saved = true;
                // Extract file path and size from "filename' saved [12345]"
                if let Some(start) = trimmed.find("'") {
                    if let Some(end) = trimmed.rfind("' saved [") {
                        // Bounds check to prevent panic
                        if start + 1 < end {
                            file_path = Some(trimmed[start + 1..end].to_string());
                        }
                    }
                }
                // Extract size from "[12345/12345]"
                if let Some(size_start) = trimmed.find("' saved [") {
                    let rest = &trimmed[size_start + 9..];
                    if let Some(size_end) = rest.find(']') {
                        file_size = Some(rest[..size_end].to_string());
                    }
                }
            }

            // Extract download speed and time
            if trimmed.starts_with("Downloaded:")
                || (trimmed.contains("in") && trimmed.contains("/s"))
            {
                // Format: "Downloaded X in Ys (Z/s)"
                if let Some(pos) = trimmed.find(" in ") {
                    time = Some(
                        trimmed[pos + 4..]
                            .split('(')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string(),
                    );
                }
                if let Some(pos) = trimmed.rfind('(') {
                    if let Some(end) = trimmed.rfind(')') {
                        speed = Some(trimmed[pos + 1..end].to_string());
                    }
                }
            }

            // Check for HTTP status in final line
            if trimmed.contains("HTTP/") && trimmed.contains("200") {
                // Successfully connected
            }

            // Check for errors
            if trimmed.contains("ERROR")
                || trimmed.contains("failed:")
                || trimmed.contains("Unable to")
                || trimmed.contains("Connection refused")
                || trimmed.contains("No such file")
            {
                has_error = true;
            }
        }

        // Build summary
        if has_error {
            return self.error_strategy.compress(output);
        }

        if saved || file_path.is_some() {
            let mut parts = Vec::new();

            if let Some(path) = file_path {
                parts.push(format!("Downloaded: {}", path));
            }

            if let Some(size) = file_size {
                parts.push(format!("Size: {} bytes", size));
            }

            if let Some(t) = time {
                parts.push(format!("Time: {}", t));
            }

            if let Some(s) = speed {
                parts.push(format!("Speed: {}", s));
            }

            if parts.is_empty() {
                Ok("(download completed)".to_string())
            } else {
                Ok(parts.join("\n"))
            }
        } else if output.is_empty() {
            Ok("(no output)".to_string())
        } else {
            // Use grouping for repetitive progress lines
            self.grouping_strategy.compress(output)
        }
    }
}

impl Default for WgetModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for WgetModule {
    fn name(&self) -> &str {
        "wget"
    }

    fn strategy(&self) -> &str {
        "multi_strategy"
    }

    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        // On failure, show errors only
        if context.exit_code != 0 {
            return self.error_strategy.compress(output);
        }

        self.extract_download_summary(output)
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
    fn test_wget_successful_download() {
        let module = WgetModule::new();
        let input = r#"--2024-01-01 12:00:00--  https://example.com/file.zip
Resolving example.com... 93.184.216.34
Connecting to example.com|93.184.216.34|:443... connected.
HTTP request sent, awaiting response... 200 OK
Length: 1234567 (1.2M) [application/zip]
Saving to: 'file.zip'

     0K .......... .......... .......... .......... ..........  4% 1.23M 0:00:59
    50K .......... .......... .......... .......... ..........  8% 2.45M 0:00:52
   100K .......... .......... .......... .......... .......... 12% 3.67M 0:00:45

2024-01-01 12:01:00 (1.45 MB/s) - 'file.zip' saved [1234567/1234567]
"#;
        let result = module
            .compress(input, &make_context("wget https://example.com/file.zip", 0))
            .unwrap();

        assert!(result.contains("Downloaded: file.zip"));
        assert!(result.contains("1234567"));
        assert!(!result.contains("..........")); // Progress bars filtered
    }

    #[test]
    fn test_wget_error() {
        let module = WgetModule::new();
        let input = r#"--2024-01-01 12:00:00--  https://example.com/notfound.zip
Resolving example.com... 93.184.216.34
Connecting to example.com|93.184.216.34|:443... connected.
HTTP request sent, awaiting response... 404 Not Found
2024-01-01 12:00:01 ERROR 404: Not Found.
"#;
        let result = module
            .compress(
                input,
                &make_context("wget https://example.com/notfound.zip", 8),
            )
            .unwrap();

        assert!(result.contains("ERROR") || result.contains("404"));
    }

    #[test]
    fn test_wget_connection_refused() {
        let module = WgetModule::new();
        let input = "Connecting to localhost:9999... failed: Connection refused.";
        let result = module
            .compress(input, &make_context("wget http://localhost:9999/file", 4))
            .unwrap();

        assert!(result.contains("Connection refused") || result.contains("failed"));
    }

    #[test]
    fn test_wget_empty_output() {
        let module = WgetModule::new();
        let result = module.compress("", &make_context("wget", 0)).unwrap();

        assert_eq!(result, "(no output)");
    }

    #[test]
    fn test_wget_progress_filtering() {
        let module = WgetModule::new();
        // Test with repetitive identical progress lines (realistic scenario)
        let input = r#"Resolving example.com... 93.184.216.34
Resolving example.com... 93.184.216.34
Resolving example.com... 93.184.216.34
Resolving example.com... 93.184.216.34
Resolving example.com... 93.184.216.34
"#;
        let result = module
            .compress(input, &make_context("wget https://example.com/file", 0))
            .unwrap();

        // Progress lines should be grouped (identical lines)
        assert!(result.contains("occurrences") || result.lines().count() < 5);
    }

    #[test]
    fn test_wget_quiet_mode() {
        let module = WgetModule::new();
        let input = ""; // wget -q produces no output on success
        let result = module
            .compress(input, &make_context("wget -q https://example.com/file", 0))
            .unwrap();

        assert_eq!(result, "(no output)");
    }
}
