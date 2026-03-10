use crate::{commands::CommandModule, Context};
use anyhow::Result;
use std::collections::HashMap;

pub struct LsModule;

impl LsModule {
    pub fn new() -> Self {
        Self
    }

    /// Extract file statistics from ls output
    fn extract_file_stats(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(empty directory)".to_string());
        }

        let mut extension_counts: HashMap<String, usize> = HashMap::new();
        let mut total_files = 0;
        let mut total_dirs = 0;
        let mut hidden_count = 0;

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Skip "total" line from ls -l output
            if trimmed.starts_with("total ") {
                continue;
            }

            // Skip permission/size columns if present (ls -l format)
            let is_long_format =
                trimmed.starts_with('-') || trimmed.starts_with('d') || trimmed.starts_with('l');

            let name = if is_long_format {
                // Long format - last field is the name
                trimmed.split_whitespace().last().unwrap_or(trimmed)
            } else {
                trimmed
            };

            // Check if directory (from long format)
            if trimmed.starts_with('d') {
                // Check if hidden directory
                if name.starts_with('.') {
                    hidden_count += 1;
                }
                total_dirs += 1;
                continue;
            }

            // Check if hidden file (not a directory)
            if name.starts_with('.') {
                hidden_count += 1;
                // Don't count hidden files in total_files
                continue;
            }

            // Skip lines that don't look like file names (for non-long format)
            if !is_long_format && name.contains(' ') && !name.contains('.') {
                // Likely not a file name
                continue;
            }

            total_files += 1;

            // Extract extension
            if let Some(dot_pos) = name.rfind('.') {
                if dot_pos > 0 && dot_pos < name.len() - 1 {
                    let ext = &name[dot_pos + 1..];
                    *extension_counts.entry(ext.to_string()).or_insert(0) += 1;
                } else {
                    *extension_counts.entry("(no ext)".to_string()).or_insert(0) += 1;
                }
            } else {
                *extension_counts.entry("(no ext)".to_string()).or_insert(0) += 1;
            }
        }

        if total_files == 0 && total_dirs == 0 {
            return Ok("(empty directory)".to_string());
        }

        let mut parts = Vec::new();

        if total_dirs > 0 {
            parts.push(format!("{} directories", total_dirs));
        }
        if total_files > 0 {
            parts.push(format!("{} files", total_files));
        }
        if hidden_count > 0 {
            parts.push(format!("{} hidden", hidden_count));
        }

        // Sort and show top extensions
        if !extension_counts.is_empty() {
            let mut sorted_ext: Vec<_> = extension_counts.into_iter().collect();
            sorted_ext.sort_by(|a, b| b.1.cmp(&a.1));

            let ext_summary: Vec<String> = sorted_ext
                .iter()
                .take(8)
                .map(|(ext, count)| format!(".{}: {}", ext, count))
                .collect();
            parts.push(format!("\n{}", ext_summary.join(", ")));
        }

        Ok(parts.join(", "))
    }
}

impl Default for LsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for LsModule {
    fn name(&self) -> &str {
        "ls"
    }

    fn strategy(&self) -> &str {
        "grouping_by_pattern"
    }

    fn compress(&self, output: &str, _context: &Context) -> Result<String> {
        if output.is_empty() {
            return Ok("(empty directory)".to_string());
        }

        // For small output, return as-is
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() <= 10 {
            return Ok(output.to_string());
        }

        // For larger output, extract statistics
        self.extract_file_stats(output)
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
    fn test_ls_empty() {
        let module = LsModule::new();
        let result = module
            .compress("", &make_context("ls empty_dir", 0))
            .unwrap();
        assert_eq!(result, "(empty directory)");
    }

    #[test]
    fn test_ls_small_output() {
        let module = LsModule::new();
        let input = "main.rs\nlib.rs\nREADME.md";
        let result = module.compress(input, &make_context("ls", 0)).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_ls_many_files() {
        let module = LsModule::new();
        let mut input = String::new();
        for i in 0..30 {
            input.push_str(&format!("file{}.rs\n", i));
        }
        for i in 0..20 {
            input.push_str(&format!("module{}.ts\n", i));
        }

        let result = module.compress(&input, &make_context("ls", 0)).unwrap();
        assert!(result.contains("50 files"));
        assert!(result.contains(".rs:"));
        assert!(result.contains(".ts:"));
    }

    #[test]
    fn test_ls_long_format() {
        let module = LsModule::new();
        let mut input = String::new();
        input.push_str("total 100\n");
        for i in 0..15 {
            input.push_str(&format!(
                "-rw-r--r-- 1 user group {} Jan 01 file{}.txt\n",
                i * 100,
                i
            ));
        }
        for i in 0..10 {
            input.push_str(&format!(
                "drwxr-xr-x 2 user group {} Jan 01 dir{}\n",
                i * 4096,
                i
            ));
        }

        let result = module.compress(&input, &make_context("ls -l", 0)).unwrap();
        assert!(result.contains("10 directories"));
        assert!(result.contains("15 files"));
    }

    #[test]
    fn test_ls_with_hidden() {
        let module = LsModule::new();
        let mut input = String::new();
        for i in 0..10 {
            input.push_str(&format!("file{}.rs\n", i));
        }
        input.push_str(".hidden1\n");
        input.push_str(".hidden2\n");

        let result = module.compress(&input, &make_context("ls -la", 0)).unwrap();
        assert!(result.contains("10 files"));
        assert!(result.contains("2 hidden"));
    }

    #[test]
    fn test_ls_no_extension() {
        let module = LsModule::new();
        let mut input = String::new();
        for i in 0..15 {
            input.push_str(&format!("script{}\n", i));
        }

        let result = module.compress(&input, &make_context("ls", 0)).unwrap();
        assert!(result.contains("15 files"));
        assert!(result.contains("(no ext)"));
    }
}
