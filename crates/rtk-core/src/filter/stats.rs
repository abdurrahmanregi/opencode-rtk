use super::Strategy;
use anyhow::Result;

pub struct StatsExtraction;

impl Strategy for StatsExtraction {
    fn name(&self) -> &str {
        "stats_extraction"
    }

    fn compress(&self, input: &str) -> Result<String> {
        if input.is_empty() {
            return Ok("(empty)".to_string());
        }

        let lines: Vec<&str> = input.lines().collect();

        // Try to extract statistics (check for proper git status format)
        let modified = lines
            .iter()
            .filter(|l| {
                let trimmed = l.trim();
                trimmed.starts_with("M ") || trimmed.starts_with(" M ")
            })
            .count();
        let added = lines
            .iter()
            .filter(|l| {
                let trimmed = l.trim();
                trimmed.starts_with("A ") || trimmed.starts_with(" A ")
            })
            .count();
        let deleted = lines
            .iter()
            .filter(|l| {
                let trimmed = l.trim();
                trimmed.starts_with("D ") || trimmed.starts_with(" D ")
            })
            .count();
        let untracked = lines
            .iter()
            .filter(|l| {
                let trimmed = l.trim();
                // Check for exact pattern "?? " (with space) to avoid false positives like "???"
                trimmed.starts_with("?? ")
            })
            .count();

        let total = modified + added + deleted + untracked;

        if total == 0 {
            // No standard status lines, check for other patterns
            if input.contains("nothing to commit") || input.contains("working tree clean") {
                return Ok("(clean)".to_string());
            }
            return Ok(input.to_string());
        }

        let mut parts = vec![format!("{} files changed", total)];

        if modified > 0 {
            parts.push(format!("{} modified", modified));
        }
        if added > 0 {
            parts.push(format!("{} added", added));
        }
        if deleted > 0 {
            parts.push(format!("{} deleted", deleted));
        }
        if untracked > 0 {
            parts.push(format!("{} untracked", untracked));
        }

        Ok(parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let result = StatsExtraction.compress("").unwrap();
        assert_eq!(result, "(empty)");
    }

    #[test]
    fn test_clean() {
        let result = StatsExtraction
            .compress("nothing to commit, working tree clean")
            .unwrap();
        assert_eq!(result, "(clean)");
    }

    #[test]
    fn test_git_status() {
        let input = r#"M src/main.rs
A src/new.rs
D src/old.rs
?? src/test.rs
"#;
        let result = StatsExtraction.compress(input).unwrap();
        assert!(result.contains("4 files changed"));
        assert!(result.contains("1 modified"));
        assert!(result.contains("1 added"));
        assert!(result.contains("1 deleted"));
        assert!(result.contains("1 untracked"));
    }

    #[test]
    fn test_git_status_false_positives() {
        // Should NOT match lines like "Makefile updated" or "Adding feature"
        let input = "Makefile updated with new targets\nAdding feature to handle edge cases\nDeleted old comments from code\n";
        let result = StatsExtraction.compress(input).unwrap();
        // Should return original input since no valid git status lines found
        assert_eq!(result, input);
    }

    #[test]
    fn test_git_status_staged() {
        // Test staged changes (with leading space)
        let input = r#" M src/modified.rs
 A src/added.rs
 D src/deleted.rs
"#;
        let result = StatsExtraction.compress(input).unwrap();
        assert!(result.contains("3 files changed"));
        assert!(result.contains("1 modified"));
        assert!(result.contains("1 added"));
        assert!(result.contains("1 deleted"));
    }

    #[test]
    fn test_git_status_mixed() {
        // Mix of valid status and false positives
        let input = r#"M src/main.rs
Makefile updated
A src/new.rs
Adding feature
D src/old.rs
Deleted old code
?? src/test.rs
"#;
        let result = StatsExtraction.compress(input).unwrap();
        assert!(result.contains("4 files changed"));
        assert!(result.contains("1 modified"));
        assert!(result.contains("1 added"));
        assert!(result.contains("1 deleted"));
        assert!(result.contains("1 untracked"));
    }

    #[test]
    fn test_untracked_false_positive() {
        // Should NOT match "???" as untracked
        let input = "??? question\n?? valid_file.txt\n???? too many";
        let result = StatsExtraction.compress(input).unwrap();
        // Only "?? valid_file.txt" should match (has space after ??)
        assert!(result.contains("1 untracked"));
        assert!(!result.contains("3 untracked"));
    }

    #[test]
    fn test_untracked_with_spaces_in_filename() {
        let input = "?? my file with spaces.txt\n?? another file.rs";
        let result = StatsExtraction.compress(input).unwrap();
        assert!(result.contains("2 untracked"));
    }
}
