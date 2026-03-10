use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, Strategy},
    Context,
};
use anyhow::Result;
use std::collections::HashSet;

pub struct DiffModule {
    error_strategy: ErrorOnly,
}

impl DiffModule {
    pub fn new() -> Self {
        Self {
            error_strategy: ErrorOnly,
        }
    }

    /// Extract diff statistics from output
    fn extract_diff_stats(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(no changes)".to_string());
        }

        // Extract statistics from diff output
        let mut added = 0;
        let mut deleted = 0;
        let mut files: HashSet<String> = HashSet::new();

        for line in output.lines() {
            if line.starts_with("+++") || line.starts_with("---") {
                // Extract filename from diff header
                if let Some(filename) = line.split_whitespace().nth(1) {
                    if !filename.starts_with("/dev/null") {
                        files.insert(
                            filename
                                .trim_start_matches("a/")
                                .trim_start_matches("b/")
                                .to_string(),
                        );
                    }
                }
            } else if line.starts_with('+') && !line.starts_with("++") {
                added += 1;
            } else if line.starts_with('-') && !line.starts_with("--") {
                deleted += 1;
            }
        }

        if files.is_empty() {
            // Check for unified diff with index lines
            let has_index = output
                .lines()
                .any(|l| l.starts_with("index ") || l.starts_with("diff --git"));
            if !has_index {
                return Ok(output.to_string());
            }
            return Ok("(no changes)".to_string());
        }

        let mut parts = vec![format!(
            "{} file{} changed",
            files.len(),
            if files.len() > 1 { "s" } else { "" }
        )];
        if added > 0 {
            parts.push(format!(
                "{} insertion{}",
                added,
                if added > 1 { "s" } else { "" }
            ));
        }
        if deleted > 0 {
            parts.push(format!(
                "{} deletion{}",
                deleted,
                if deleted > 1 { "s" } else { "" }
            ));
        }

        // List files if not too many
        if files.len() <= 10 {
            let mut sorted_files: Vec<_> = files.into_iter().collect();
            sorted_files.sort();
            parts.push(format!("\n{}", sorted_files.join("\n")));
        }

        Ok(parts.join(", "))
    }
}

impl Default for DiffModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for DiffModule {
    fn name(&self) -> &str {
        "diff"
    }

    fn strategy(&self) -> &str {
        "stats_extraction"
    }

    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        // On error, show errors only
        if context.exit_code != 0 && !output.contains("diff --git") {
            // diff returns 1 when differences found, 2 on error
            if context.exit_code >= 2 {
                return self.error_strategy.compress(output);
            }
        }

        if output.is_empty() {
            return Ok("(no differences)".to_string());
        }

        // For small diffs, return as-is
        let line_count = output.lines().count();
        if line_count <= 20 {
            return Ok(output.to_string());
        }

        // For larger diffs, extract statistics
        self.extract_diff_stats(output)
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
    fn test_diff_empty() {
        let module = DiffModule::new();
        let result = module
            .compress("", &make_context("diff file1.txt file2.txt", 0))
            .unwrap();
        assert_eq!(result, "(no differences)");
    }

    #[test]
    fn test_diff_small_output() {
        let module = DiffModule::new();
        let input = "1c1\n< old line\n---\n> new line\n";
        let result = module
            .compress(input, &make_context("diff file1.txt file2.txt", 1))
            .unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_diff_unified_format() {
        let module = DiffModule::new();
        let mut input = String::new();
        input.push_str("diff --git a/src/main.rs b/src/main.rs\n");
        input.push_str("index 1234567..abcdefg 100644\n");
        input.push_str("--- a/src/main.rs\n");
        input.push_str("+++ b/src/main.rs\n");
        input.push_str("@@ -1,5 +1,6 @@\n");
        for i in 0..25 {
            input.push_str(&format!("+line added {}\n", i));
        }
        for i in 0..10 {
            input.push_str(&format!("-line removed {}\n", i));
        }

        let result = module
            .compress(&input, &make_context("diff -u old new", 1))
            .unwrap();
        assert!(result.contains("1 file changed"));
        assert!(result.contains("25 insertions"));
        assert!(result.contains("10 deletions"));
    }

    #[test]
    fn test_diff_multiple_files() {
        let module = DiffModule::new();
        let mut input = String::new();

        // File 1
        input.push_str("diff --git a/src/main.rs b/src/main.rs\n");
        input.push_str("--- a/src/main.rs\n");
        input.push_str("+++ b/src/main.rs\n");
        for i in 0..10 {
            input.push_str(&format!("+added {}\n", i));
        }

        // File 2
        input.push_str("diff --git a/src/lib.rs b/src/lib.rs\n");
        input.push_str("--- a/src/lib.rs\n");
        input.push_str("+++ b/src/lib.rs\n");
        for i in 0..15 {
            input.push_str(&format!("+added {}\n", i));
        }

        let result = module
            .compress(&input, &make_context("diff -u old new", 1))
            .unwrap();
        assert!(result.contains("2 files changed"));
    }

    #[test]
    fn test_diff_error() {
        let module = DiffModule::new();
        let input = "diff: file1.txt: No such file or directory";
        let result = module
            .compress(input, &make_context("diff nonexistent file2.txt", 2))
            .unwrap();
        assert!(result.contains("No such file") || result.contains("error"));
    }
}
