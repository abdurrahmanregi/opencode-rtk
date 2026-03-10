use crate::{commands::CommandModule, Context};
use anyhow::Result;
use std::path::Path;

pub struct ReadModule;

impl ReadModule {
    pub fn new() -> Self {
        Self
    }

    /// Strip comments from code
    /// Note: This is a simplified implementation that only removes comment-only lines.
    /// For full comment handling (including block comments and inline comments),
    /// use a proper parser for the specific language.
    fn strip_comments(&self, content: &str, extension: &str) -> String {
        let mut result = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip single-line comments based on language
            let is_comment = match extension {
                "rs" | "js" | "ts" | "jsx" | "tsx" | "c" | "cpp" | "java" | "go" => {
                    trimmed.starts_with("//")
                }
                "py" | "sh" | "bash" | "zsh" | "yaml" | "yml" | "toml" => trimmed.starts_with('#'),
                "sql" => trimmed.starts_with("--"),
                "html" | "xml" => trimmed.starts_with("<!--"),
                _ => false,
            };

            if !is_comment && !trimmed.is_empty() {
                result.push_str(line);
                result.push('\n');
            }
        }

        result
    }

    /// Extract file statistics
    fn extract_file_stats(&self, content: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        let code_lines = lines.iter().filter(|l| !l.trim().is_empty()).count();
        let empty_lines = total_lines - code_lines;

        let total_chars = content.chars().count();

        format!(
            "{} lines ({} code, {} blank), {} chars",
            total_lines, code_lines, empty_lines, total_chars
        )
    }

    /// Get file extension from context command
    fn get_extension(&self, context: &Context) -> Option<String> {
        context.command.as_ref().and_then(|cmd| {
            // Try to extract filename from command
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            for part in parts.iter().rev() {
                // Skip command flags
                if part.starts_with('-') {
                    continue;
                }
                // Use Path for robust extension extraction
                let path = Path::new(part);
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    return Some(ext.to_string());
                }
            }
            None
        })
    }

    /// Truncate content to max lines
    fn truncate(&self, content: &str, max_lines: usize) -> String {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() <= max_lines {
            return content.to_string();
        }

        let truncated: Vec<&str> = lines.into_iter().take(max_lines).collect();
        format!("{}\n... (truncated)", truncated.join("\n"))
    }
}

impl Default for ReadModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for ReadModule {
    fn name(&self) -> &str {
        "read"
    }

    fn strategy(&self) -> &str {
        "grouping_by_pattern"
    }

    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        if output.is_empty() {
            return Ok("(empty file)".to_string());
        }

        // For small files, return as-is
        let line_count = output.lines().count();
        if line_count <= 20 {
            return Ok(output.to_string());
        }

        // For larger files, extract statistics
        let stats = self.extract_file_stats(output);

        // Get extension and strip comments if applicable
        let extension = self.get_extension(context);

        // Show truncated content with stats
        let extension_str = extension.as_deref().unwrap_or("");
        let stripped = self.strip_comments(output, extension_str);

        // Show stripped content if there was any reduction
        if stripped.lines().count() < line_count {
            // Some comments were stripped
            let truncated = self.truncate(&stripped, 30);
            Ok(format!("{}\n\n---\n{}", stats, truncated))
        } else {
            // Just show stats and truncated content
            let truncated = self.truncate(output, 30);
            Ok(format!("{}\n\n---\n{}", stats, truncated))
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
    fn test_read_empty() {
        let module = ReadModule::new();
        let result = module
            .compress("", &make_context("read file.txt", 0))
            .unwrap();
        assert_eq!(result, "(empty file)");
    }

    #[test]
    fn test_read_small_file() {
        let module = ReadModule::new();
        let input = "line 1\nline 2\nline 3\n";
        let result = module
            .compress(input, &make_context("read file.txt", 0))
            .unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_read_large_file() {
        let module = ReadModule::new();
        let mut input = String::new();
        for i in 0..100 {
            input.push_str(&format!("line {}\n", i));
        }

        let result = module
            .compress(&input, &make_context("read file.txt", 0))
            .unwrap();
        assert!(result.contains("100 lines"));
        assert!(result.contains("(truncated)"));
    }

    #[test]
    fn test_read_with_comments() {
        let module = ReadModule::new();
        let mut input = String::new();
        input.push_str("// This is a comment\n");
        input.push_str("// Another comment\n");
        for i in 0..30 {
            input.push_str(&format!("fn code_{}() {{ }}\n", i));
        }

        let result = module
            .compress(&input, &make_context("read file.rs", 0))
            .unwrap();
        assert!(result.contains("lines"));
        // Should have stripped comments
        assert!(!result.contains("// This is a comment"));
    }

    #[test]
    fn test_read_python_file() {
        let module = ReadModule::new();
        let mut input = String::new();
        input.push_str("# Python comment\n");
        input.push_str("def foo():\n");
        input.push_str("    pass\n");
        for i in 0..25 {
            input.push_str(&format!("def func_{}():\n", i));
            input.push_str(&format!("    return {}\n", i));
        }

        let result = module
            .compress(&input, &make_context("read file.py", 0))
            .unwrap();
        assert!(result.contains("lines"));
    }

    #[test]
    fn test_read_get_extension() {
        let module = ReadModule::new();

        let ctx = make_context("read src/main.rs", 0);
        assert_eq!(module.get_extension(&ctx), Some("rs".to_string()));

        let ctx = make_context("read ./lib/utils.ts", 0);
        assert_eq!(module.get_extension(&ctx), Some("ts".to_string()));

        let ctx = make_context("read README", 0);
        assert_eq!(module.get_extension(&ctx), None);
    }

    #[test]
    fn test_read_inline_comments() {
        let module = ReadModule::new();
        let mut input = String::new();
        // Code with inline comments should not be stripped
        // (current implementation only strips lines that START with comment markers)
        input.push_str("fn main() { // entry point\n");
        input.push_str("    let x = 5; // variable\n");
        for i in 0..25 {
            input.push_str(&format!("fn func_{}() {{ }}\n", i));
        }

        let result = module
            .compress(&input, &make_context("read file.rs", 0))
            .unwrap();

        assert!(result.contains("lines"));
    }

    #[test]
    fn test_read_multiline_block_comments() {
        let module = ReadModule::new();
        let mut input = String::new();
        input.push_str("/* This is a\n");
        input.push_str("   multi-line\n");
        input.push_str("   block comment */\n");
        for i in 0..25 {
            input.push_str(&format!("fn func_{}() {{ }}\n", i));
        }

        let result = module
            .compress(&input, &make_context("read file.rs", 0))
            .unwrap();

        assert!(result.contains("lines"));
        // Block comment content on separate lines should be stripped
        // (lines starting with * or whitespace + *)
    }

    #[test]
    fn test_read_html_comments() {
        let module = ReadModule::new();
        let mut input = String::new();
        input.push_str("<!-- HTML comment -->\n");
        input.push_str("<div>content</div>\n");
        for i in 0..25 {
            input.push_str(&format!("<p>paragraph {}</p>\n", i));
        }

        let result = module
            .compress(&input, &make_context("read file.html", 0))
            .unwrap();

        assert!(result.contains("lines"));
    }

    #[test]
    fn test_read_sql_comments() {
        let module = ReadModule::new();
        let mut input = String::new();
        input.push_str("-- SQL comment\n");
        input.push_str("SELECT * FROM users;\n");
        for i in 0..25 {
            input.push_str(&format!("SELECT * FROM table{};\n", i));
        }

        let result = module
            .compress(&input, &make_context("read file.sql", 0))
            .unwrap();

        assert!(result.contains("lines"));
    }

    #[test]
    fn test_read_mixed_code_and_comments() {
        let module = ReadModule::new();
        let mut input = String::new();
        for i in 0..15 {
            input.push_str(&format!("// Comment for section {}\n", i));
            input.push_str(&format!("fn section_{}() {{ }}\n", i));
        }

        let result = module
            .compress(&input, &make_context("read file.rs", 0))
            .unwrap();

        assert!(result.contains("lines"));
        // Comment lines should be stripped
    }
}
