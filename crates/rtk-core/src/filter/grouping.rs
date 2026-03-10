use super::Strategy;
use anyhow::Result;
use std::collections::HashMap;

pub struct GroupingByPattern;

impl Strategy for GroupingByPattern {
    fn name(&self) -> &str {
        "grouping_by_pattern"
    }

    fn compress(&self, input: &str) -> Result<String> {
        if input.is_empty() {
            return Ok("(empty)".to_string());
        }

        // Group lines by similarity (first N characters)
        let mut groups: HashMap<String, usize> = HashMap::new();

        for line in input.lines() {
            if line.trim().is_empty() {
                continue;
            }

            // Use first 40 chars as grouping key (character-safe for UTF-8)
            let key = if line.chars().count() > 40 {
                line.chars().take(40).collect::<String>()
            } else {
                line.to_string()
            };

            *groups.entry(key).or_insert(0) += 1;
        }

        if groups.is_empty() {
            return Ok("(empty)".to_string());
        }

        // Sort by count (descending)
        let mut sorted: Vec<_> = groups.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        // Format output
        let mut output = Vec::new();
        for (key, count) in sorted.into_iter().take(50) {
            if count > 1 {
                output.push(format!("{} ({} occurrences)", key, count));
            } else {
                output.push(key);
            }
        }

        Ok(output.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let result = GroupingByPattern.compress("").unwrap();
        assert_eq!(result, "(empty)");
    }

    #[test]
    fn test_grouping() {
        let input = r#"error at line 10: foo
error at line 20: bar
error at line 30: baz
error at line 10: foo
error at line 10: foo
"#;
        let result = GroupingByPattern.compress(input).unwrap();
        assert!(result.contains("(3 occurrences)"));
    }

    #[test]
    fn test_utf8_multibyte_chars() {
        // Test with multi-byte UTF-8 characters (emoji, CJK, etc.)
        let input = "🦀 Rust 语言编程入门教程 - 第1章\n🦀 Rust 语言编程入门教程 - 第2章\n🦀 Rust 语言编程入门教程 - 第3章\n";
        let result = GroupingByPattern.compress(input).unwrap();
        // Should not panic and should group properly
        assert!(result.contains("🦀"));
        assert!(!result.is_empty());
    }

    #[test]
    fn test_long_utf8_line() {
        // Test long line with multi-byte chars that exceeds 40 characters
        let input = "这是一段很长的中文测试文本，用于验证当字符数超过四十个字符时的处理逻辑是否正确\n这是另一段很长的中文测试文本，用于验证当字符数超过四十个字符时的处理逻辑是否正确\n";
        let result = GroupingByPattern.compress(input).unwrap();
        // Should handle without panic
        assert!(!result.is_empty());
    }
}
