use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, GroupingByPattern, StatsExtraction, Strategy},
    Context,
};
use anyhow::Result;

pub struct GitModule {
    stats_strategy: StatsExtraction,
    error_strategy: ErrorOnly,
    grouping_strategy: GroupingByPattern,
}

impl GitModule {
    pub fn new() -> Self {
        Self {
            stats_strategy: StatsExtraction,
            error_strategy: ErrorOnly,
            grouping_strategy: GroupingByPattern,
        }
    }

    /// Detect which git subcommand is being used
    /// Handles edge cases like: git -C /path status, GIT STATUS (case-insensitive)
    fn detect_subcommand(&self, command: &str) -> Option<String> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() || parts[0].to_lowercase() != "git" {
            return None;
        }

        // Skip git flags to find the subcommand
        // Common flags: -C <path>, -c <name>=<value>, --exec-path, etc.
        let mut i = 1;
        while i < parts.len() {
            let part = parts[i];

            // Skip flags (start with -)
            if part.starts_with('-') {
                // These flags take an argument
                if part == "-C" || part == "-c" {
                    i += 2; // Skip flag and its argument
                } else if part.starts_with("--")
                    && !part.contains('=')
                    && i + 1 < parts.len()
                    && !parts[i + 1].starts_with('-')
                {
                    // Long flag without =, next part might be argument
                    // But if next part looks like a subcommand, don't skip it
                    i += 1;
                } else {
                    i += 1;
                }
                continue;
            }

            // Found subcommand
            return Some(part.to_string());
        }

        None
    }

    /// Handle git status command
    fn handle_status(&self, output: &str) -> Result<String> {
        self.stats_strategy.compress(output)
    }

    /// Handle git diff command - stats extraction with truncation
    fn handle_diff(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(no changes)".to_string());
        }

        // Extract statistics from diff output
        let mut added = 0;
        let mut deleted = 0;
        let mut files = std::collections::HashSet::new();

        for line in output.lines() {
            if let Some(rest) = line
                .strip_prefix("+++ ")
                .or_else(|| line.strip_prefix("--- "))
            {
                // Extract filename from diff header (handles filenames with spaces)
                // Format: +++ b/src/my file.rs or +++ /dev/null
                if !rest.starts_with("/dev/null") {
                    files.insert(rest.trim_start_matches("a/").trim_start_matches("b/"));
                }
            } else if line.starts_with('+') && !line.starts_with("++") {
                added += 1;
            } else if line.starts_with('-') && !line.starts_with("--") {
                deleted += 1;
            }
        }

        if files.is_empty() {
            return Ok("(no changes)".to_string());
        }

        let mut parts = vec![format!("{} files changed", files.len())];
        if added > 0 {
            parts.push(format!("{} insertions(+)", added));
        }
        if deleted > 0 {
            parts.push(format!("{} deletions(-)", deleted));
        }

        Ok(parts.join(", "))
    }

    /// Handle git log command - extract summary statistics
    fn handle_log(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(no commits)".to_string());
        }

        // Count commits and extract basic info
        let commit_count = output.lines().filter(|l| l.starts_with("commit ")).count();

        if commit_count == 0 {
            // Try oneline format
            let oneline_count = output.lines().filter(|l| !l.trim().is_empty()).count();
            if oneline_count > 0 {
                return Ok(format!("{} commits", oneline_count));
            }
            return Ok("(no commits)".to_string());
        }

        // Extract authors with proper error handling
        let authors: Vec<&str> = output
            .lines()
            .filter_map(|l| {
                // Use strip_prefix for safe extraction
                let after_prefix = l.strip_prefix("Author: ")?;
                // Split on '<' to get name part (before email)
                let name_part = after_prefix.split('<').next()?;
                let trimmed = name_part.trim();
                // Filter out empty names
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
            .collect();

        let unique_authors: std::collections::HashSet<&str> = authors.into_iter().collect();

        let mut summary = vec![format!("{} commits", commit_count)];
        if !unique_authors.is_empty() {
            summary.push(format!(
                "by {} author{}",
                unique_authors.len(),
                if unique_authors.len() > 1 { "s" } else { "" }
            ));
        }

        Ok(summary.join(", "))
    }

    /// Handle git add command - silent on success
    fn handle_add(&self, output: &str, exit_code: i32) -> Result<String> {
        if exit_code == 0 {
            Ok(String::new())
        } else {
            self.error_strategy.compress(output)
        }
    }

    /// Handle git commit command - silent on success
    fn handle_commit(&self, output: &str, exit_code: i32) -> Result<String> {
        if exit_code == 0 {
            Ok(String::new())
        } else {
            self.error_strategy.compress(output)
        }
    }

    /// Handle git push command - filter progress output
    fn handle_push(&self, output: &str, exit_code: i32) -> Result<String> {
        if exit_code == 0 {
            // Pre-define patterns to filter out (lowercase for case-insensitive matching)
            const FILTER_PATTERNS: &[&str] =
                &["writing objects", "counting objects", "compressing objects"];

            // Filter out progress lines, keep important info
            let important_lines: Vec<&str> = output
                .lines()
                .filter(|line| {
                    let line_lower = line.to_lowercase();
                    // Use pre-computed patterns for efficiency
                    !FILTER_PATTERNS.iter().any(|p| line_lower.contains(p))
                        && !line_lower.starts_with("remote: processing")
                        && !line.trim().is_empty()
                })
                .collect();

            if important_lines.is_empty() {
                return Ok("(pushed successfully)".to_string());
            }

            Ok(important_lines.join("\n"))
        } else {
            self.error_strategy.compress(output)
        }
    }

    /// Handle git branch command - compress branch list
    fn handle_branch(&self, output: &str) -> Result<String> {
        if output.is_empty() {
            return Ok("(no branches)".to_string());
        }

        // Use grouping strategy for branch lists
        self.grouping_strategy.compress(output)
    }

    /// Handle git checkout command - silent on success
    fn handle_checkout(&self, output: &str, exit_code: i32) -> Result<String> {
        if exit_code == 0 {
            Ok(String::new())
        } else {
            self.error_strategy.compress(output)
        }
    }
}

impl Default for GitModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for GitModule {
    fn name(&self) -> &str {
        "git"
    }

    fn strategy(&self) -> &str {
        "multi_strategy"
    }

    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        // Try to detect the subcommand from the context
        let subcommand = context
            .command
            .as_ref()
            .and_then(|cmd| self.detect_subcommand(cmd));

        match subcommand.as_deref() {
            Some("status") => self.handle_status(output),
            Some("diff") => self.handle_diff(output),
            Some("log") => self.handle_log(output),
            Some("add") => self.handle_add(output, context.exit_code),
            Some("commit") => self.handle_commit(output, context.exit_code),
            Some("push") => self.handle_push(output, context.exit_code),
            Some("branch") => self.handle_branch(output),
            Some("checkout") => self.handle_checkout(output, context.exit_code),
            _ => {
                // Default to stats extraction for unknown git commands
                self.stats_strategy.compress(output)
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
    fn test_git_status() {
        let module = GitModule::new();
        let input = "M src/main.rs\nA src/new.rs\n?? test.txt";
        let result = module
            .compress(input, &make_context("git status", 0))
            .unwrap();

        assert!(result.contains("3 files changed"));
    }

    #[test]
    fn test_git_diff() {
        let module = GitModule::new();
        let input = r#"diff --git a/src/main.rs b/src/main.rs
index 1234567..abcdefg 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,6 @@
 fn main() {
-    println!("old");
+    println!("new");
+    println!("added");
 }
"#;
        let result = module
            .compress(input, &make_context("git diff", 0))
            .unwrap();

        assert!(result.contains("1 file"));
        assert!(result.contains("2 insertions"));
        assert!(result.contains("1 deletion"));
    }

    #[test]
    fn test_git_diff_empty() {
        let module = GitModule::new();
        let result = module.compress("", &make_context("git diff", 0)).unwrap();

        assert_eq!(result, "(no changes)");
    }

    #[test]
    fn test_git_log() {
        let module = GitModule::new();
        let input = r#"commit abc123
Author: John Doe <john@example.com>
Date:   Mon Jan 1 00:00:00 2024 +0000

    First commit

commit def456
Author: Jane Smith <jane@example.com>
Date:   Tue Jan 2 00:00:00 2024 +0000

    Second commit
"#;
        let result = module.compress(input, &make_context("git log", 0)).unwrap();

        assert!(result.contains("2 commits"));
        assert!(result.contains("2 authors"));
    }

    #[test]
    fn test_git_log_oneline() {
        let module = GitModule::new();
        let input = "abc123 First commit\ndef456 Second commit\nghi789 Third commit";
        let result = module
            .compress(input, &make_context("git log --oneline", 0))
            .unwrap();

        assert!(result.contains("3 commits"));
    }

    #[test]
    fn test_git_add_success() {
        let module = GitModule::new();
        let result = module.compress("", &make_context("git add .", 0)).unwrap();

        assert_eq!(result, "");
    }

    #[test]
    fn test_git_add_error() {
        let module = GitModule::new();
        let input = "error: pathspec 'nonexistent' did not match any files";
        let result = module
            .compress(input, &make_context("git add nonexistent", 1))
            .unwrap();

        assert!(result.contains("error"));
    }

    #[test]
    fn test_git_commit_success() {
        let module = GitModule::new();
        let result = module
            .compress("", &make_context("git commit -m 'test'", 0))
            .unwrap();

        assert_eq!(result, "");
    }

    #[test]
    fn test_git_commit_error() {
        let module = GitModule::new();
        let input = "error: nothing to commit";
        let result = module
            .compress(input, &make_context("git commit", 1))
            .unwrap();

        assert!(result.contains("error"));
    }

    #[test]
    fn test_git_push_success() {
        let module = GitModule::new();
        let input = r#"Counting objects: 10, done.
Writing objects: 100% (10/10), done.
To github.com:user/repo.git
   abc123..def456  main -> main
"#;
        let result = module
            .compress(input, &make_context("git push", 0))
            .unwrap();

        // Should filter out progress lines
        assert!(!result.contains("Counting objects"));
        assert!(!result.contains("Writing objects"));
        assert!(result.contains("main -> main") || result == "(pushed successfully)");
    }

    #[test]
    fn test_git_push_error() {
        let module = GitModule::new();
        let input = "error: failed to push some refs";
        let result = module
            .compress(input, &make_context("git push", 1))
            .unwrap();

        assert!(result.contains("error"));
    }

    #[test]
    fn test_git_branch() {
        let module = GitModule::new();
        let input = r#"  feature-1
  feature-2
* main
  develop
"#;
        let result = module
            .compress(input, &make_context("git branch", 0))
            .unwrap();

        // Should use grouping strategy
        assert!(!result.is_empty());
    }

    #[test]
    fn test_git_checkout_success() {
        let module = GitModule::new();
        let result = module
            .compress("", &make_context("git checkout main", 0))
            .unwrap();

        assert_eq!(result, "");
    }

    #[test]
    fn test_git_checkout_error() {
        let module = GitModule::new();
        let input = "error: pathspec 'nonexistent' did not match any file(s) known to git";
        let result = module
            .compress(input, &make_context("git checkout nonexistent", 1))
            .unwrap();

        assert!(result.contains("error"));
    }

    #[test]
    fn test_unknown_git_command() {
        let module = GitModule::new();
        let input = "M src/main.rs\nA src/new.rs";
        let result = module
            .compress(input, &make_context("git unknown-command", 0))
            .unwrap();

        // Should fall back to stats extraction
        assert!(result.contains("2 files changed"));
    }

    #[test]
    fn test_no_command_in_context() {
        let module = GitModule::new();
        let input = "M src/main.rs\nA src/new.rs";
        let context = Context {
            cwd: "/tmp".to_string(),
            exit_code: 0,
            tool: "bash".to_string(),
            session_id: None,
            command: None,
        };
        let result = module.compress(input, &context).unwrap();

        // Should fall back to stats extraction
        assert!(result.contains("2 files changed"));
    }

    #[test]
    fn test_git_diff_filename_with_spaces() {
        let module = GitModule::new();
        let input = r#"diff --git a/src/my file.rs b/src/my file.rs
index 1234567..abcdefg 100644
--- a/src/my file.rs
+++ b/src/my file.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("new");
 }
"#;
        let result = module
            .compress(input, &make_context("git diff", 0))
            .unwrap();

        // Should correctly extract filename with spaces
        assert!(result.contains("1 file"));
        assert!(result.contains("1 insertion"));
    }

    #[test]
    fn test_git_subcommand_with_flags() {
        let module = GitModule::new();

        // Test -C flag (common in scripts)
        let input = "M src/main.rs";
        let result = module
            .compress(input, &make_context("git -C /path/to/repo status", 0))
            .unwrap();
        assert!(result.contains("1 file"));

        // Test -c flag
        let result2 = module
            .compress(input, &make_context("git -c core.autocrlf=false status", 0))
            .unwrap();
        assert!(result2.contains("1 file"));
    }

    #[test]
    fn test_git_command_case_insensitive() {
        let module = GitModule::new();
        let input = "M src/main.rs";

        // Test uppercase
        let result = module
            .compress(input, &make_context("GIT STATUS", 0))
            .unwrap();
        assert!(result.contains("1 file"));

        // Test mixed case
        let result2 = module
            .compress(input, &make_context("Git Status", 0))
            .unwrap();
        assert!(result2.contains("1 file"));
    }

    #[test]
    fn test_git_log_malformed_author() {
        let module = GitModule::new();
        let input = r#"commit abc123
Author: John Doe <john@example.com>
Date:   Mon Jan 1 00:00:00 2024 +0000

    First commit

commit def456
Author: 
Date:   Tue Jan 2 00:00:00 2024 +0000

    Second commit

commit ghi789
Author: Jane Smith
Date:   Wed Jan 3 00:00:00 2024 +0000

    Third commit
"#;
        let result = module.compress(input, &make_context("git log", 0)).unwrap();

        // Should handle malformed author lines gracefully
        assert!(result.contains("3 commits"));
        // Should only count valid authors
        assert!(result.contains("1 author") || result.contains("2 author"));
    }

    #[test]
    fn test_git_push_filters_progress() {
        let module = GitModule::new();
        let input = r#"Counting objects: 100% (50/50), done.
Writing objects: 100% (25/25), done.
Compressing objects: 100% (10/10), done.
remote: Processing changes: done.
To github.com:user/repo.git
   abc123..def456  main -> main
"#;
        let result = module
            .compress(input, &make_context("git push", 0))
            .unwrap();

        // Should filter out all progress lines
        assert!(!result.contains("Counting objects"));
        assert!(!result.contains("Writing objects"));
        assert!(!result.contains("Compressing objects"));
        assert!(!result.contains("Processing changes"));
        // Should keep important info
        assert!(result.contains("main -> main") || result == "(pushed successfully)");
    }

    #[test]
    fn test_git_concurrent_calls() {
        use std::sync::Arc;
        use std::thread;

        let module = Arc::new(GitModule::new());
        let mut handles = vec![];

        // Test thread safety with concurrent calls
        for i in 0..5 {
            let module = Arc::clone(&module);
            handles.push(thread::spawn(move || {
                let input = format!("M file{}.rs\nA file{}.rs", i, i + 10);
                module.compress(&input, &make_context("git status", 0))
            }));
        }

        // All calls should succeed
        for handle in handles {
            let result = handle.join().unwrap().unwrap();
            assert!(result.contains("files changed"));
        }
    }
}
