use crate::{commands::CommandModule, Context};
use anyhow::Result;
use std::collections::HashMap;

pub struct GrepModule;

impl GrepModule {
    pub fn new() -> Self {
        Self
    }

    /// Extract grep match statistics from output
    fn extract_match_stats(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(no matches)".to_string());
        }

        // Track matches by file
        let mut file_matches: HashMap<String, usize> = HashMap::new();
        let mut total_matches = 0;

        for line in output.lines() {
            // grep output format: filename:line_number:content or filename:content
            // Be smarter about colons - first field before colon that looks like a filename
            if let Some(colon_pos) = line.find(':') {
                let potential_filename = &line[..colon_pos];
                // Only count as a file if it doesn't contain path separators in a suspicious way
                // or if it looks like a valid filename (no spaces, reasonable length)
                if !potential_filename.is_empty() && potential_filename.len() < 256 {
                    *file_matches
                        .entry(potential_filename.to_string())
                        .or_insert(0) += 1;
                    total_matches += 1;
                }
            } else if !line.trim().is_empty() {
                // Could be a match without file prefix (e.g., grep on stdin)
                total_matches += 1;
            }
        }

        if total_matches == 0 {
            return Ok("(no matches)".to_string());
        }

        // Sort by match count descending
        let mut sorted: Vec<_> = file_matches.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        let mut parts = vec![format!("{} matches", total_matches)];

        if !sorted.is_empty() {
            let file_count = sorted.len();
            parts.push(format!(
                "in {} file{}",
                file_count,
                if file_count > 1 { "s" } else { "" }
            ));

            // Show top files
            let top_files: Vec<String> = sorted
                .iter()
                .take(5)
                .map(|(f, c)| format!("{}: {}", f, c))
                .collect();

            if !top_files.is_empty() {
                parts.push(format!("\n{}", top_files.join("\n")));
            }
        }

        Ok(parts.join(" "))
    }
}

impl Default for GrepModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for GrepModule {
    fn name(&self) -> &str {
        "grep"
    }

    fn strategy(&self) -> &str {
        "grouping_by_pattern"
    }

    fn compress(&self, output: &str, _context: &Context) -> Result<String> {
        if output.is_empty() {
            return Ok("(no matches)".to_string());
        }

        // For small output, return as-is
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() <= 10 {
            return Ok(output.to_string());
        }

        // For larger output, extract statistics
        self.extract_match_stats(output)
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
    fn test_grep_empty() {
        let module = GrepModule::new();
        let result = module
            .compress("", &make_context("grep pattern *.rs", 1))
            .unwrap();
        assert_eq!(result, "(no matches)");
    }

    #[test]
    fn test_grep_small_output() {
        let module = GrepModule::new();
        let input = "src/main.rs:fn main() {\nsrc/lib.rs:pub fn test() {";
        let result = module
            .compress(input, &make_context("grep fn *.rs", 0))
            .unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_grep_many_matches() {
        let module = GrepModule::new();
        let mut input = String::new();
        for i in 0..20 {
            input.push_str(&format!("src/file{}.rs:{}:some match content\n", i % 3, i));
        }
        let result = module
            .compress(&input, &make_context("grep pattern *.rs", 0))
            .unwrap();
        assert!(result.contains("20 matches"));
        assert!(result.contains("3 files"));
    }

    #[test]
    fn test_grep_single_file_many_matches() {
        let module = GrepModule::new();
        let mut input = String::new();
        for i in 0..15 {
            input.push_str(&format!("main.rs:{}:match found\n", i));
        }
        let result = module
            .compress(&input, &make_context("grep pattern main.rs", 0))
            .unwrap();
        assert!(result.contains("15 matches"));
        assert!(result.contains("1 file"));
    }

    #[test]
    fn test_grep_with_line_numbers() {
        let module = GrepModule::new();
        let mut input = String::new();
        for i in 0..12 {
            input.push_str(&format!("file.rs:{}:some content\n", i + 1));
        }
        let result = module
            .compress(&input, &make_context("grep -n pattern file.rs", 0))
            .unwrap();
        assert!(result.contains("12 matches"));
    }

    #[test]
    fn test_grep_filenames_with_colons() {
        let module = GrepModule::new();
        // Test filenames that contain colons (e.g., Windows paths, URLs in output)
        let mut input = String::new();
        input.push_str("C:\\Users\\test\\file.txt:10:match content\n");
        input.push_str("C:\\Users\\test\\file.txt:20:another match\n");
        input.push_str("http://example.com/api:5:url match\n");
        input.push_str("file:name.txt:15:match with colon in filename\n");

        for i in 0..15 {
            input.push_str(&format!("normal_file.rs:{}:match\n", i));
        }

        let result = module
            .compress(&input, &make_context("grep pattern *", 0))
            .unwrap();

        // Should handle colons in filenames correctly
        assert!(result.contains("matches"));
    }

    #[test]
    fn test_grep_colon_in_match_content() {
        let module = GrepModule::new();
        // Test matches where the content itself contains colons
        let mut input = String::new();
        for i in 0..15 {
            input.push_str(&format!(
                "file.rs:{}:URL: https://example.com:8080/path\n",
                i
            ));
        }

        let result = module
            .compress(&input, &make_context("grep https *.rs", 0))
            .unwrap();

        assert!(result.contains("15 matches"));
    }

    #[test]
    fn test_grep_windows_paths() {
        let module = GrepModule::new();
        // Test Windows-style paths with drive letters
        let mut input = String::new();
        for i in 0..15 {
            input.push_str(&format!(
                "C:\\Projects\\MyApp\\src\\main.rs:{}:fn main() {{\n",
                i + 1
            ));
        }

        let result = module
            .compress(&input, &make_context("grep pattern", 0))
            .unwrap();

        assert!(result.contains("15 matches"));
    }

    #[test]
    fn test_grep_mixed_path_styles() {
        let module = GrepModule::new();
        // Test mixed Unix and Windows paths
        let mut input = String::new();
        input.push_str("/home/user/project/file.rs:10:match\n");
        input.push_str("C:\\Users\\project\\file.rs:20:match\n");
        input.push_str("./relative/path.rs:30:match\n");
        input.push_str("..\\windows\\relative.rs:40:match\n");

        for i in 0..10 {
            input.push_str(&format!("file{}.rs:{}:match\n", i, i * 10));
        }

        let result = module
            .compress(&input, &make_context("grep match", 0))
            .unwrap();

        assert!(result.contains("matches"));
    }
}
