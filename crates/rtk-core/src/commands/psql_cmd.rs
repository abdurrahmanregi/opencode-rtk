use crate::{
    commands::CommandModule,
    filter::{ErrorOnly, GroupingByPattern, Strategy},
    Context,
};
use anyhow::Result;

pub struct PsqlModule {
    error_strategy: ErrorOnly,
    grouping_strategy: GroupingByPattern,
}

impl PsqlModule {
    pub fn new() -> Self {
        Self {
            error_strategy: ErrorOnly,
            grouping_strategy: GroupingByPattern,
        }
    }

    /// Check if output is a table (ASCII table format)
    fn is_table_output(&self, output: &str) -> bool {
        let lines: Vec<&str> = output.lines().take(5).collect();
        lines.iter().any(|line| {
            line.trim().starts_with('-') && line.trim().ends_with('-') && line.contains('+')
        }) || lines
            .iter()
            .any(|line| line.contains('|') && line.split('|').count() > 2)
    }

    /// Compress table output - extract structure and row count
    fn compress_table_output(&self, output: &str) -> Result<String> {
        let lines: Vec<&str> = output.lines().collect();

        if lines.is_empty() {
            return Ok("(no output)".to_string());
        }

        let mut columns: Vec<String> = Vec::new();
        let mut row_count = 0;
        let mut in_data = false;
        let mut truncated = false;
        let mut sample_rows: Vec<String> = Vec::new();

        for line in &lines {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Skip separator lines (like -+----+-)
            if trimmed.starts_with('-') && trimmed.contains('+') {
                if columns.is_empty() {
                    // This means we haven't found headers yet, next line might be headers
                    // Set in_data anyway to handle tables with empty/malformed headers
                }
                in_data = true;
                continue;
            }

            // Parse columns from header
            if !in_data && trimmed.contains('|') && columns.is_empty() {
                let parts: Vec<&str> = trimmed
                    .split('|')
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                columns = parts.iter().map(|s| s.trim().to_string()).collect();
                continue;
            }

            // Count data rows
            if in_data && trimmed.contains('|') {
                row_count += 1;
                if sample_rows.len() < 3 {
                    sample_rows.push(trimmed.to_string());
                }
                if row_count > 20 {
                    truncated = true;
                }
            }
        }

        // Check for row count indicator at the end (e.g., "(10 rows)")
        for line in lines.iter().rev().take(3) {
            let trimmed = line.trim();
            if trimmed.starts_with('(') && trimmed.ends_with(" rows)") {
                if let Ok(count) = trimmed
                    .trim_start_matches('(')
                    .trim_end_matches(" rows)")
                    .parse::<usize>()
                {
                    row_count = count;
                }
            }
        }

        let mut result = Vec::new();

        if !columns.is_empty() {
            if columns.len() <= 5 {
                result.push(format!("Columns: {}", columns.join(", ")));
            } else {
                result.push(format!(
                    "Columns: {} ... ({} total)",
                    columns[..5].join(", "),
                    columns.len()
                ));
            }
        }

        if row_count > 0 {
            result.push(format!(
                "Rows: {}{}",
                row_count,
                if truncated { " (truncated)" } else { "" }
            ));
        }

        // Add sample of first row if small
        if sample_rows.len() == 1 && row_count == 1 {
            // Single row - show it
            result.push(format!("Data: {}", sample_rows[0]));
        }

        if result.is_empty() {
            return Ok("(no data)".to_string());
        }

        Ok(result.join("\n"))
    }

    /// Extract query result summary
    fn extract_query_summary(&self, output: &str) -> Result<String> {
        // Check for common psql patterns
        for line in output.lines().rev().take(5) {
            let trimmed = line.trim();

            // INSERT/UPDATE/DELETE count
            if trimmed.starts_with("INSERT ") || trimmed.contains("INSERT") {
                // Format: INSERT 0 5
                if let Some(count) = trimmed.split_whitespace().last() {
                    if let Ok(n) = count.parse::<usize>() {
                        return Ok(format!("Inserted: {} row(s)", n));
                    }
                }
                return Ok("INSERT completed".to_string());
            }

            if trimmed.starts_with("UPDATE ") || trimmed.contains("UPDATE") {
                if let Some(count) = trimmed.split_whitespace().last() {
                    if let Ok(n) = count.parse::<usize>() {
                        return Ok(format!("Updated: {} row(s)", n));
                    }
                }
                return Ok("UPDATE completed".to_string());
            }

            if trimmed.starts_with("DELETE ") || trimmed.contains("DELETE") {
                if let Some(count) = trimmed.split_whitespace().last() {
                    if let Ok(n) = count.parse::<usize>() {
                        return Ok(format!("Deleted: {} row(s)", n));
                    }
                }
                return Ok("DELETE completed".to_string());
            }

            // CREATE/DROP statements
            if trimmed.contains("CREATE TABLE") {
                return Ok("Table created".to_string());
            }
            if trimmed.contains("DROP TABLE") {
                return Ok("Table dropped".to_string());
            }
            if trimmed.contains("CREATE INDEX") {
                return Ok("Index created".to_string());
            }
            if trimmed.contains("ALTER TABLE") {
                return Ok("Table altered".to_string());
            }
        }

        // Check for table output
        if self.is_table_output(output) {
            return self.compress_table_output(output);
        }

        // Default: use grouping
        self.grouping_strategy.compress(output)
    }

    /// Extract errors from psql output
    fn extract_errors(&self, output: &str) -> Option<String> {
        let mut errors = Vec::new();
        let mut in_error = false;

        for line in output.lines() {
            let trimmed = line.trim();

            // psql error patterns
            if trimmed.starts_with("ERROR:")
                || trimmed.starts_with("FATAL:")
                || trimmed.starts_with("PANIC:")
                || trimmed.contains("could not connect")
                || trimmed.contains("connection refused")
                || trimmed.contains("password authentication failed")
            {
                in_error = true;
                errors.push(trimmed.to_string());
            } else if in_error && !trimmed.is_empty() && !trimmed.starts_with("LINE ") {
                // Continue capturing error context
                errors.push(trimmed.to_string());
            } else if in_error && trimmed.is_empty() {
                in_error = false;
            }
        }

        if errors.is_empty() {
            None
        } else {
            // Limit error output
            if errors.len() > 5 {
                Some(format!("{}\n... (truncated)", errors[..5].join("\n")))
            } else {
                Some(errors.join("\n"))
            }
        }
    }
}

impl Default for PsqlModule {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandModule for PsqlModule {
    fn name(&self) -> &str {
        "psql"
    }

    fn strategy(&self) -> &str {
        "multi_strategy"
    }

    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        // Check for errors first
        if let Some(errors) = self.extract_errors(output) {
            return Ok(errors);
        }

        // On non-zero exit, use error strategy
        if context.exit_code != 0 {
            return self.error_strategy.compress(output);
        }

        if output.is_empty() {
            return Ok("(no output)".to_string());
        }

        self.extract_query_summary(output)
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
    fn test_psql_select_query() {
        let module = PsqlModule::new();
        let input = r#" id |  name  |         email
----+--------+----------------------
  1 | Alice  | alice@example.com
  2 | Bob    | bob@example.com
  3 | Charlie| charlie@example.com
(3 rows)
"#;
        let result = module
            .compress(input, &make_context("psql -c \"SELECT * FROM users\"", 0))
            .unwrap();

        assert!(result.contains("Columns:"));
        assert!(result.contains("Rows: 3"));
    }

    #[test]
    fn test_psql_error() {
        let module = PsqlModule::new();
        let input = r#"ERROR:  relation "nonexistent" does not exist
LINE 1: SELECT * FROM nonexistent;
                  ^
"#;
        let result = module
            .compress(
                input,
                &make_context("psql -c \"SELECT * FROM nonexistent\"", 1),
            )
            .unwrap();

        assert!(result.contains("ERROR") || result.contains("does not exist"));
    }

    #[test]
    fn test_psql_insert() {
        let module = PsqlModule::new();
        let input = "INSERT 0 5";
        let result = module
            .compress(input, &make_context("psql -c \"INSERT INTO users ...\"", 0))
            .unwrap();

        assert!(result.contains("Inserted: 5"));
    }

    #[test]
    fn test_psql_update() {
        let module = PsqlModule::new();
        let input = "UPDATE 3";
        let result = module
            .compress(input, &make_context("psql -c \"UPDATE users SET ...\"", 0))
            .unwrap();

        assert!(result.contains("Updated: 3"));
    }

    #[test]
    fn test_psql_delete() {
        let module = PsqlModule::new();
        let input = "DELETE 10";
        let result = module
            .compress(input, &make_context("psql -c \"DELETE FROM users ...\"", 0))
            .unwrap();

        assert!(result.contains("Deleted: 10"));
    }

    #[test]
    fn test_psql_empty_output() {
        let module = PsqlModule::new();
        let result = module.compress("", &make_context("psql", 0)).unwrap();

        assert_eq!(result, "(no output)");
    }

    #[test]
    fn test_psql_connection_error() {
        let module = PsqlModule::new();
        let input = "psql: could not connect to server: Connection refused";
        let result = module
            .compress(input, &make_context("psql -h localhost", 2))
            .unwrap();

        assert!(result.contains("Connection refused") || result.contains("could not connect"));
    }

    #[test]
    fn test_psql_create_table() {
        let module = PsqlModule::new();
        let input = "CREATE TABLE";
        let result = module
            .compress(
                input,
                &make_context("psql -c \"CREATE TABLE users (...)\"", 0),
            )
            .unwrap();

        assert!(result.contains("Table created"));
    }

    #[test]
    fn test_psql_wide_table() {
        let module = PsqlModule::new();
        let input = r#" id | col1 | col2 | col3 | col4 | col5 | col6 | col7 | col8
----+------+------+------+------+------+------+------+-----
  1 | a    | b    | c    | d    | e    | f    | g    | h
(1 row)
"#;
        let result = module
            .compress(
                input,
                &make_context("psql -c \"SELECT * FROM wide_table\"", 0),
            )
            .unwrap();

        assert!(result.contains("Columns:"));
        assert!(result.contains("Rows: 1"));
    }

    #[test]
    fn test_psql_many_rows() {
        let module = PsqlModule::new();
        let mut input = String::from(" id | name\n----+------\n");
        for i in 1..=50 {
            input.push_str(&format!(" {:2} | user{}\n", i, i));
        }
        input.push_str("(50 rows)\n");

        let result = module
            .compress(&input, &make_context("psql -c \"SELECT * FROM users\"", 0))
            .unwrap();

        assert!(result.contains("Rows: 50"));
    }

    #[test]
    fn test_psql_syntax_error() {
        let module = PsqlModule::new();
        let input = r#"ERROR:  syntax error at or near "SELCT"
LINE 1: SELCT * FROM users;
        ^
"#;
        let result = module
            .compress(input, &make_context("psql -c \"SELCT * FROM users\"", 1))
            .unwrap();

        assert!(result.contains("syntax error") || result.contains("ERROR"));
    }

    #[test]
    fn test_psql_auth_error() {
        let module = PsqlModule::new();
        let input = "psql: FATAL:  password authentication failed for user \"postgres\"";
        let result = module
            .compress(input, &make_context("psql -U postgres", 2))
            .unwrap();

        assert!(result.contains("password authentication failed") || result.contains("FATAL"));
    }

    #[test]
    fn test_psql_single_row() {
        let module = PsqlModule::new();
        let input = r#" id | name
----+------
  1 | Alice
(1 row)
"#;
        let result = module
            .compress(
                input,
                &make_context("psql -c \"SELECT * FROM users WHERE id=1\"", 0),
            )
            .unwrap();

        assert!(result.contains("Rows: 1"));
        // Should show the single row data
        assert!(result.contains("Data:") || result.contains("Alice"));
    }
}
