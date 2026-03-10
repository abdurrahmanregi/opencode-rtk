pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    // GPT-style tokenization heuristic
    // Average: ~4 characters per token for English text
    // Adjust for code (more tokens) vs prose (fewer tokens)
    let char_count = text.chars().count(); // Use character count, not byte count
    let base_estimate = (char_count as f64 / 4.0).ceil() as usize;

    // Adjust for whitespace (tokens are often split on whitespace)
    let whitespace_count = text.chars().filter(|c| c.is_whitespace()).count();
    let adjustment = (whitespace_count as f64 * 0.1).ceil() as usize;

    base_estimate.saturating_add(adjustment).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_string() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_short_string() {
        assert_eq!(estimate_tokens("hello"), 2);
    }

    #[test]
    fn test_medium_string() {
        let text = "This is a test string with multiple words";
        let tokens = estimate_tokens(text);
        assert!(tokens > 5 && tokens < 20);
    }

    #[test]
    fn test_code() {
        let code = r#"
fn main() {
    println!("Hello, world!");
}
"#;
        let tokens = estimate_tokens(code);
        assert!(tokens > 10);
    }

    #[test]
    fn test_utf8_text() {
        // UTF-8 characters should be counted correctly, not as 3x bytes
        let text = "你好世界"; // 4 Chinese characters
        let tokens = estimate_tokens(text);
        // Should be ~1 token per char (4/4 = 1), not ~3 tokens per char (12/4 = 3)
        assert!(tokens >= 1 && tokens <= 2);
    }

    #[test]
    fn test_mixed_utf8() {
        let text = "Hello 世界! 🦀"; // Mixed ASCII, CJK, and emoji
        let tokens = estimate_tokens(text);
        // Should count characters, not bytes
        assert!(tokens > 0 && tokens < 10);
    }

    #[test]
    fn test_emoji() {
        let text = "🦀🦀🦀🦀🦀"; // 5 emoji
        let tokens = estimate_tokens(text);
        // Each emoji is 1 character, not 4 bytes
        assert!(tokens >= 1 && tokens <= 3);
    }
}
