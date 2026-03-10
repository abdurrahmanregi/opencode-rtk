use crate::{commands::CommandModule, Context};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

pub struct FindModule;

impl FindModule {
    pub fn new() -> Self {
        Self
    }

    /// Extract file statistics from find output
    fn extract_file_stats(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(no files found)".to_string());
        }

        let mut extension_counts: HashMap<String, usize> = HashMap::new();
        let mut dir_counts: HashMap<String, usize> = HashMap::new();
        let mut total_files = 0;

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            total_files += 1;

            // Extract extension
            let path = Path::new(trimmed);
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                *extension_counts.entry(ext.to_string()).or_insert(0) += 1;
            } else {
                *extension_counts.entry("(no ext)".to_string()).or_insert(0) += 1;
            }

            // Extract directory
            if let Some(parent) = path.parent().and_then(|p| p.to_str()) {
                if !parent.is_empty() {
                    *dir_counts.entry(parent.to_string()).or_insert(0) += 1;
                }
            }
        }

        if total_files == 0 {
            return Ok("(no files found)".to_string());
        }

        let mut parts = vec![format!("{} files found", total_files)];

        // Sort and show top extensions
        let mut sorted_ext: Vec<_> = extension_counts.into_iter().collect();
        sorted_ext.sort_by(|a, b| b.1.cmp(&a.1));

        if !sorted_ext.is_empty() {
            let ext_summary: Vec<String> = sorted_ext
                .iter()
                .take(10)
                .map(|(ext, count)| format!(".{}: {}", ext, count))
                .collect();
            parts.push(format!("\nBy extension:\n{}", ext_summary.join("\n")));
        }

        // Sort and show top directories if there are multiple
        if dir_counts.len() > 1 {
            let mut sorted_dirs: Vec<_> = dir_counts.into_iter().collect();
            sorted_dirs.sort_by(|a, b| b.1.cmp(&a.1));

            let dir_summary: Vec<String> = sorted_dirs
                .iter()
                .take(5)
                .map(|(dir, count)| format!("{}: {}", dir, count))
                .collect();
            parts.push(format!("\nBy directory:\n{}", dir_summary.join("\n")));
        }

        Ok(parts.join(""))
    }
}

impl Default for FindModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for FindModule {
    fn name(&self) -> &str {
        "find"
    }

    fn strategy(&self) -> &str {
        "grouping_by_pattern"
    }

    fn compress(&self, output: &str, _context: &Context) -> Result<String> {
        if output.is_empty() {
            return Ok("(no files found)".to_string());
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
    fn test_find_empty() {
        let module = FindModule::new();
        let result = module
            .compress("", &make_context("find . -name '*.xyz'", 0))
            .unwrap();
        assert_eq!(result, "(no files found)");
    }

    #[test]
    fn test_find_small_output() {
        let module = FindModule::new();
        let input = "./src/main.rs\n./src/lib.rs\n./README.md";
        let result = module
            .compress(input, &make_context("find . -type f", 0))
            .unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_find_many_files() {
        let module = FindModule::new();
        let mut input = String::new();
        for i in 0..50 {
            input.push_str(&format!("./src/file{}.rs\n", i));
        }
        for i in 0..20 {
            input.push_str(&format!("./lib/module{}.ts\n", i));
        }

        let result = module
            .compress(&input, &make_context("find . -type f", 0))
            .unwrap();
        assert!(result.contains("70 files found"));
        assert!(result.contains(".rs:"));
        assert!(result.contains(".ts:"));
    }

    #[test]
    fn test_find_with_directories() {
        let module = FindModule::new();
        let mut input = String::new();
        // Files in multiple directories
        for i in 0..10 {
            input.push_str(&format!("./src/core/file{}.rs\n", i));
            input.push_str(&format!("./src/utils/file{}.rs\n", i));
            input.push_str(&format!("./tests/file{}.rs\n", i));
        }

        let result = module
            .compress(&input, &make_context("find . -name '*.rs'", 0))
            .unwrap();
        assert!(result.contains("30 files found"));
        assert!(result.contains("By directory"));
    }

    #[test]
    fn test_find_no_extension() {
        let module = FindModule::new();
        let mut input = String::new();
        for i in 0..15 {
            input.push_str(&format!("./bin/script{}\n", i));
            input.push_str(&format!("./data/file{}.txt\n", i));
        }

        let result = module
            .compress(&input, &make_context("find . -type f", 0))
            .unwrap();
        assert!(result.contains("30 files found"));
        assert!(result.contains("(no ext)"));
        assert!(result.contains(".txt"));
    }
}
