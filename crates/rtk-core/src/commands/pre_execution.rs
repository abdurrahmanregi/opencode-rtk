//! Pre-execution flag optimization
//!
//! This module provides flag mappings for common commands to reduce
//! output size before execution, complementing post-execution filtering.

use anyhow::Result;
use std::collections::HashSet;

/// Represents a single flag mapping configuration
#[derive(Debug, Clone)]
pub struct FlagMapping {
    /// Base command (e.g., "git", "npm", "cargo")
    pub command: &'static str,
    /// Optional subcommand (e.g., "status", "test")
    pub subcommand: Option<&'static str>,
    /// Flags to append
    pub flags: &'static [&'static str],
    /// Condition for applying flags
    pub condition: Option<FlagCondition>,
}

/// Conditions for conditional flag application
#[derive(Debug, Clone)]
pub enum FlagCondition {
    /// Only apply if flag is NOT already present
    NoFlagPresent(&'static str),
    /// Only apply if output is expected to be large
    OutputLikelyLarge,
}

/// Result of command optimization
#[derive(Debug, Clone)]
pub struct OptimizedCommand {
    /// Original command string
    pub original: String,
    /// Optimized command string with flags
    pub optimized: String,
    /// Flags that were added
    pub flags_added: Vec<String>,
    /// Whether optimization was skipped
    pub skipped: bool,
    /// Reason for skipping (if applicable)
    pub skip_reason: Option<String>,
}

/// All flag mappings for pre-execution optimization
pub const FLAG_MAPPINGS: &[FlagMapping] = &[
    // === GIT COMMANDS ===
    FlagMapping {
        command: "git",
        subcommand: Some("status"),
        flags: &["--porcelain", "-b"],
        condition: None,
    },
    FlagMapping {
        command: "git",
        subcommand: Some("diff"),
        flags: &["--stat"],
        condition: None,
    },
    FlagMapping {
        command: "git",
        subcommand: Some("log"),
        flags: &["--oneline"],
        condition: None,
    },
    FlagMapping {
        command: "git",
        subcommand: Some("push"),
        flags: &["--quiet"],
        condition: None,
    },
    FlagMapping {
        command: "git",
        subcommand: Some("fetch"),
        flags: &["--quiet"],
        condition: None,
    },
    FlagMapping {
        command: "git",
        subcommand: Some("pull"),
        flags: &["--quiet"],
        condition: None,
    },
    // === NPM/YARN/PNPM ===
    FlagMapping {
        command: "npm",
        subcommand: Some("test"),
        flags: &["--silent"],
        condition: None,
    },
    FlagMapping {
        command: "npm",
        subcommand: Some("install"),
        flags: &["--silent", "--no-progress"],
        condition: None,
    },
    FlagMapping {
        command: "npm",
        subcommand: Some("run"),
        flags: &["--silent"],
        condition: None,
    },
    FlagMapping {
        command: "yarn",
        subcommand: Some("test"),
        flags: &["--silent"],
        condition: None,
    },
    FlagMapping {
        command: "yarn",
        subcommand: Some("install"),
        flags: &["--silent"],
        condition: None,
    },
    FlagMapping {
        command: "pnpm",
        subcommand: Some("test"),
        flags: &["--silent"],
        condition: None,
    },
    FlagMapping {
        command: "pnpm",
        subcommand: Some("install"),
        flags: &["--silent"],
        condition: None,
    },
    // === CARGO ===
    FlagMapping {
        command: "cargo",
        subcommand: Some("build"),
        flags: &["--quiet"],
        condition: None,
    },
    FlagMapping {
        command: "cargo",
        subcommand: Some("test"),
        flags: &["--quiet"],
        condition: None,
    },
    FlagMapping {
        command: "cargo",
        subcommand: Some("clippy"),
        flags: &["--quiet"],
        condition: None,
    },
    FlagMapping {
        command: "cargo",
        subcommand: Some("check"),
        flags: &["--quiet"],
        condition: None,
    },
    FlagMapping {
        command: "cargo",
        subcommand: Some("run"),
        flags: &["--quiet"],
        condition: None,
    },
    FlagMapping {
        command: "cargo",
        subcommand: Some("doc"),
        flags: &["--quiet"],
        condition: None,
    },
    // === TEST RUNNERS ===
    FlagMapping {
        command: "pytest",
        subcommand: None,
        flags: &["-q"],
        condition: None,
    },
    // === NETWORK TOOLS ===
    FlagMapping {
        command: "curl",
        subcommand: None,
        flags: &["-s"],
        condition: None,
    },
    FlagMapping {
        command: "wget",
        subcommand: None,
        flags: &["-q"],
        condition: None,
    },
];

/// Optimize a command by adding appropriate flags
///
/// # Arguments
///
/// * `command` - The command string to optimize
///
/// # Returns
///
/// An `OptimizedCommand` with the original, optimized command, and flags added
///
/// # Examples
///
/// ```
/// use rtk_core::commands::pre_execution::optimize_command;
/// let result = optimize_command("git status")?;
/// assert_eq!(result.optimized, "git status --porcelain -b");
/// assert_eq!(result.flags_added, vec!["--porcelain", "-b"]);
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn optimize_command(command: &str) -> Result<OptimizedCommand> {
    let trimmed = command.trim();

    // Skip empty commands
    if trimmed.is_empty() {
        return Ok(OptimizedCommand {
            original: command.to_string(),
            optimized: command.to_string(),
            flags_added: vec![],
            skipped: true,
            skip_reason: Some("empty command".to_string()),
        });
    }

    // Skip commands with pipes (only optimize first segment)
    // But not OR operator (||) - check for single pipe
    if has_pipe_operator(trimmed) {
        return optimize_piped_command(trimmed);
    }

    // Skip commands with heredocs (but not bit-shift operators in arithmetic)
    if has_heredoc(trimmed) {
        return Ok(OptimizedCommand {
            original: command.to_string(),
            optimized: command.to_string(),
            flags_added: vec![],
            skipped: true,
            skip_reason: Some("heredoc detected".to_string()),
        });
    }

    // Skip commands with subshells $(...) or `...`
    // But be careful about backticks in quoted strings
    if has_subshell(trimmed) {
        return Ok(OptimizedCommand {
            original: command.to_string(),
            optimized: command.to_string(),
            flags_added: vec![],
            skipped: true,
            skip_reason: Some("subshell detected".to_string()),
        });
    }

    optimize_single_command(trimmed)
}

/// Check if command has a pipe operator (but not OR operator ||)
fn has_pipe_operator(command: &str) -> bool {
    let chars: Vec<char> = command.chars().collect();
    let mut i = 0;
    let mut in_quotes = false;
    let mut quote_char = ' ';

    while i < chars.len() {
        let c = chars[i];

        if c == '"' || c == '\'' {
            if in_quotes {
                if c == quote_char {
                    in_quotes = false;
                }
            } else {
                in_quotes = true;
                quote_char = c;
            }
        } else if c == '|' && !in_quotes {
            // Check if it's || (OR operator)
            if i + 1 < chars.len() && chars[i + 1] == '|' {
                i += 1; // Skip the second |
            } else {
                return true; // Single pipe found
            }
        }
        i += 1;
    }
    false
}

/// Check if command has a heredoc (but not bit-shift in arithmetic)
fn has_heredoc(command: &str) -> bool {
    // Heredoc pattern: << followed by a word (delimiter)
    // Not: << in arithmetic context like $((1 << 2))

    let chars: Vec<char> = command.chars().collect();
    let mut i = 0;
    let mut in_arithmetic = false;
    let mut in_quotes = false;
    let mut quote_char = ' ';

    while i < chars.len() {
        let c = chars[i];

        if c == '"' || c == '\'' {
            if in_quotes {
                if c == quote_char {
                    in_quotes = false;
                }
            } else {
                in_quotes = true;
                quote_char = c;
            }
        } else if c == '$' && i + 1 < chars.len() && chars[i + 1] == '(' {
            // Start of arithmetic or command substitution
            in_arithmetic = true;
        } else if c == ')' && in_arithmetic {
            in_arithmetic = false;
        } else if c == '<' && !in_quotes && !in_arithmetic {
            // Check for heredoc: << followed by word character
            if i + 1 < chars.len() && chars[i + 1] == '<' {
                // Check if it's <<< (here-string) or <<- (tab-dedented heredoc)
                // or just << followed by a word
                if i + 2 < chars.len() {
                    let next = chars[i + 2];
                    if next == '<' {
                        // <<< is here-string, still skip
                        return true;
                    }
                    if next == '-' {
                        // <<- is tab-dedented heredoc
                        return true;
                    }
                    // << followed by word character is heredoc
                    if next.is_alphanumeric() || next == '_' {
                        return true;
                    }
                } else {
                    // << at end of string (unlikely but handle)
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

/// Check if command has a subshell (but not backticks in quoted strings)
fn has_subshell(command: &str) -> bool {
    // Check for $(...) - command substitution
    if command.contains("$(") {
        return true;
    }

    // Check for backticks outside of quotes
    let chars: Vec<char> = command.chars().collect();
    let mut i = 0;
    let mut in_double_quotes = false;
    let mut in_single_quotes = false;

    while i < chars.len() {
        let c = chars[i];

        if c == '"' && !in_single_quotes {
            in_double_quotes = !in_double_quotes;
        } else if c == '\'' && !in_double_quotes {
            in_single_quotes = !in_single_quotes;
        } else if c == '`' && !in_double_quotes && !in_single_quotes {
            return true;
        }
        i += 1;
    }
    false
}

/// Optimize a single command (no pipes, heredocs, or subshells)
fn optimize_single_command(command: &str) -> Result<OptimizedCommand> {
    // Parse command into parts
    let parts = parse_command_parts(command);

    // Skip if command has no base
    if parts.is_empty() {
        return Ok(OptimizedCommand {
            original: command.to_string(),
            optimized: command.to_string(),
            flags_added: vec![],
            skipped: true,
            skip_reason: Some("no base command".to_string()),
        });
    }

    // Extract actual command (skip env vars and sudo-like prefixes)
    let (cmd_index, cmd_part) = match extract_actual_command(&parts) {
        Some((idx, part)) => (idx, part),
        None => {
            return Ok(OptimizedCommand {
                original: command.to_string(),
                optimized: command.to_string(),
                flags_added: vec![],
                skipped: true,
                skip_reason: Some("no actual command found".to_string()),
            });
        }
    };

    // Extract base command (handle paths)
    let base = extract_base_command(cmd_part);
    let subcommand = parts.get(cmd_index + 1).map(|s| s.as_str());

    // Find matching flag mapping
    let mapping = find_mapping(&base, subcommand);

    match mapping {
        Some(m) => apply_mapping(command, &parts, m),
        None => Ok(OptimizedCommand {
            original: command.to_string(),
            optimized: command.to_string(),
            flags_added: vec![],
            skipped: true,
            skip_reason: Some("no matching flag mapping".to_string()),
        }),
    }
}

/// Apply a flag mapping to a command
fn apply_mapping(
    original: &str,
    parts: &[String],
    mapping: &FlagMapping,
) -> Result<OptimizedCommand> {
    // Check conditions
    if let Some(ref condition) = mapping.condition {
        match condition {
            FlagCondition::NoFlagPresent(flag) => {
                if parts.iter().any(|p| p == flag) {
                    return Ok(OptimizedCommand {
                        original: original.to_string(),
                        optimized: original.to_string(),
                        flags_added: vec![],
                        skipped: true,
                        skip_reason: Some(format!("flag {} already present", flag)),
                    });
                }
            }
            FlagCondition::OutputLikelyLarge => {
                // Heuristic: skip if command has --help or similar
                if parts.iter().any(|p| p == "--help" || p == "-h") {
                    return Ok(OptimizedCommand {
                        original: original.to_string(),
                        optimized: original.to_string(),
                        flags_added: vec![],
                        skipped: true,
                        skip_reason: Some("help flag present".to_string()),
                    });
                }
            }
        }
    }

    // Check for duplicate flags
    let existing_flags: HashSet<&str> = parts
        .iter()
        .filter(|p| p.starts_with('-'))
        .map(|p| p.as_str())
        .collect();

    let flags_to_add: Vec<&str> = mapping
        .flags
        .iter()
        .filter(|f| !existing_flags.contains(*f))
        .copied()
        .collect();

    // Skip if no new flags to add
    if flags_to_add.is_empty() {
        return Ok(OptimizedCommand {
            original: original.to_string(),
            optimized: original.to_string(),
            flags_added: vec![],
            skipped: true,
            skip_reason: Some("all flags already present".to_string()),
        });
    }

    // Build optimized command
    let optimized = format!("{} {}", original, flags_to_add.join(" "));

    Ok(OptimizedCommand {
        original: original.to_string(),
        optimized,
        flags_added: flags_to_add.iter().map(|s| s.to_string()).collect(),
        skipped: false,
        skip_reason: None,
    })
}

/// Optimize a piped command (only optimize the first segment)
fn optimize_piped_command(command: &str) -> Result<OptimizedCommand> {
    // Split by pipe, but preserve the pipe character
    let segments: Vec<&str> = command.split('|').collect();

    if segments.is_empty() {
        return Ok(OptimizedCommand {
            original: command.to_string(),
            optimized: command.to_string(),
            flags_added: vec![],
            skipped: true,
            skip_reason: Some("empty pipe".to_string()),
        });
    }

    // Only optimize the first segment
    let first_segment = segments[0].trim();
    let first_result = optimize_single_command(first_segment)?;

    if first_result.skipped {
        return Ok(OptimizedCommand {
            original: command.to_string(),
            optimized: command.to_string(),
            flags_added: vec![],
            skipped: true,
            skip_reason: Some("first segment not optimizable".to_string()),
        });
    }

    // Reconstruct the command with optimized first segment
    let mut optimized_parts = vec![first_result.optimized];
    for segment in segments.iter().skip(1) {
        optimized_parts.push(segment.trim().to_string());
    }

    let optimized = optimized_parts.join(" | ");

    Ok(OptimizedCommand {
        original: command.to_string(),
        optimized,
        flags_added: first_result.flags_added,
        skipped: false,
        skip_reason: None,
    })
}

/// Parse command string into parts, handling quotes and escapes
fn parse_command_parts(command: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let mut escape_next = false;

    for ch in command.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '\\' => {
                escape_next = true;
            }
            '"' | '\'' => {
                if in_quotes {
                    if ch == quote_char {
                        in_quotes = false;
                    } else {
                        current.push(ch);
                    }
                } else {
                    in_quotes = true;
                    quote_char = ch;
                }
            }
            ' ' | '\t' => {
                if in_quotes {
                    current.push(ch);
                } else if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

/// Extract base command from path-prefixed command
fn extract_base_command(command: &str) -> String {
    // Handle path-prefixed commands (e.g., /usr/bin/git -> git)
    let base = if command.contains('/') || (cfg!(windows) && command.contains('\\')) {
        command
            .rsplit(|c| ['/', '\\'].contains(&c))
            .next()
            .unwrap_or(command)
    } else {
        command
    };

    // Strip .exe extension on Windows
    let base = if cfg!(windows) {
        base.strip_suffix(".exe").unwrap_or(base)
    } else {
        base
    };

    base.to_lowercase()
}

/// Extract the actual command from a command line, skipping env vars and sudo-like prefixes
fn extract_actual_command(parts: &[String]) -> Option<(usize, &str)> {
    // Prefixes to skip
    let skip_prefixes = ["sudo", "doas", "run0", "nice", "ionice", "env"];

    for (i, part) in parts.iter().enumerate() {
        let lower = part.to_lowercase();

        // Skip environment variable assignments (contains = but doesn't start with -)
        if part.contains('=') && !part.starts_with('-') {
            continue;
        }

        // Skip sudo-like commands
        if skip_prefixes.contains(&lower.as_str()) {
            continue;
        }

        // This is the actual command
        return Some((i, part.as_str()));
    }

    None
}

/// Find matching flag mapping for command
fn find_mapping(base: &str, subcommand: Option<&str>) -> Option<&'static FlagMapping> {
    FLAG_MAPPINGS.iter().find(|m| {
        m.command.to_lowercase() == base
            && match (m.subcommand, subcommand) {
                (Some(a), Some(b)) => a.to_lowercase() == b.to_lowercase(),
                (None, None) => true,
                (Some(_), None) => false,
                (None, Some(_)) => true, // Mapping without subcommand matches any
            }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimize_git_status() {
        let result = optimize_command("git status").unwrap();
        assert_eq!(result.optimized, "git status --porcelain -b");
        assert_eq!(result.flags_added, vec!["--porcelain", "-b"]);
        assert!(!result.skipped);
    }

    #[test]
    fn test_optimize_git_status_with_existing_flag() {
        let result = optimize_command("git status --porcelain").unwrap();
        assert_eq!(result.optimized, "git status --porcelain -b");
        assert_eq!(result.flags_added, vec!["-b"]);
        assert!(!result.skipped);
    }

    #[test]
    fn test_optimize_git_status_all_flags_present() {
        let result = optimize_command("git status --porcelain -b").unwrap();
        assert!(result.skipped);
        assert_eq!(
            result.skip_reason,
            Some("all flags already present".to_string())
        );
    }

    #[test]
    fn test_optimize_npm_test() {
        let result = optimize_command("npm test").unwrap();
        assert_eq!(result.optimized, "npm test --silent");
        assert_eq!(result.flags_added, vec!["--silent"]);
    }

    #[test]
    fn test_optimize_cargo_build() {
        let result = optimize_command("cargo build").unwrap();
        assert_eq!(result.optimized, "cargo build --quiet");
    }

    #[test]
    fn test_optimize_cargo_test() {
        let result = optimize_command("cargo test").unwrap();
        assert_eq!(result.optimized, "cargo test --quiet");
    }

    #[test]
    fn test_optimize_unknown_command() {
        let result = optimize_command("unknown-command").unwrap();
        assert!(result.skipped);
        assert_eq!(result.optimized, "unknown-command");
    }

    #[test]
    fn test_optimize_empty_command() {
        let result = optimize_command("").unwrap();
        assert!(result.skipped);
    }

    #[test]
    fn test_optimize_whitespace_command() {
        let result = optimize_command("   ").unwrap();
        assert!(result.skipped);
    }

    #[test]
    fn test_parse_command_parts_simple() {
        let parts = parse_command_parts("git status");
        assert_eq!(parts, vec!["git", "status"]);
    }

    #[test]
    fn test_parse_command_parts_with_quotes() {
        let parts = parse_command_parts("git commit -m \"some message\"");
        assert_eq!(parts, vec!["git", "commit", "-m", "some message"]);
    }

    #[test]
    fn test_parse_command_parts_with_single_quotes() {
        let parts = parse_command_parts("echo 'hello world'");
        assert_eq!(parts, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_extract_base_command_simple() {
        assert_eq!(extract_base_command("git"), "git");
    }

    #[test]
    fn test_extract_base_command_with_windows_path() {
        // On Windows, .exe is stripped
        let result = extract_base_command("C:\\Program Files\\git.exe");
        assert_eq!(result, "git");
    }

    #[test]
    fn test_optimize_docker_ps() {
        let result = optimize_command("docker ps").unwrap();
        assert!(result.skipped);
        assert_eq!(result.optimized, "docker ps");
    }

    #[test]
    fn test_optimize_curl() {
        let result = optimize_command("curl http://example.com").unwrap();
        assert_eq!(result.optimized, "curl http://example.com -s");
    }

    #[test]
    fn test_optimize_wget() {
        let result = optimize_command("wget http://example.com").unwrap();
        assert_eq!(result.optimized, "wget http://example.com -q");
    }

    #[test]
    fn test_optimize_pytest() {
        let result = optimize_command("pytest tests/").unwrap();
        assert_eq!(result.optimized, "pytest tests/ -q");
    }

    #[test]
    fn test_optimize_preserves_case_in_args() {
        let result = optimize_command("git status -uNo").unwrap();
        assert!(result.optimized.contains("-uNo"));
    }

    #[test]
    fn test_optimize_piped_command() {
        let result = optimize_command("git status | grep modified").unwrap();
        assert!(result.optimized.contains("--porcelain"));
        assert!(result.optimized.contains("| grep modified"));
        assert!(!result.skipped);
    }

    #[test]
    fn test_optimize_heredoc_skipped() {
        let result = optimize_command("cat <<EOF").unwrap();
        assert!(result.skipped);
        assert_eq!(result.skip_reason, Some("heredoc detected".to_string()));
    }

    #[test]
    fn test_optimize_subshell_skipped() {
        let result = optimize_command("echo $(git status)").unwrap();
        assert!(result.skipped);
        assert_eq!(result.skip_reason, Some("subshell detected".to_string()));
    }

    #[test]
    fn test_optimize_backtick_subshell_skipped() {
        let result = optimize_command("echo `git status`").unwrap();
        assert!(result.skipped);
        assert_eq!(result.skip_reason, Some("subshell detected".to_string()));
    }

    #[test]
    fn test_optimize_npm_install() {
        let result = optimize_command("npm install").unwrap();
        assert!(result.optimized.contains("--silent"));
        assert!(result.optimized.contains("--no-progress"));
    }

    #[test]
    fn test_optimize_yarn_test() {
        let result = optimize_command("yarn test").unwrap();
        assert_eq!(result.optimized, "yarn test --silent");
    }

    #[test]
    fn test_optimize_pnpm_install() {
        let result = optimize_command("pnpm install").unwrap();
        assert_eq!(result.optimized, "pnpm install --silent");
    }

    #[test]
    fn test_optimize_git_log() {
        let result = optimize_command("git log").unwrap();
        assert_eq!(result.optimized, "git log --oneline");
    }

    #[test]
    fn test_optimize_git_push() {
        let result = optimize_command("git push").unwrap();
        assert_eq!(result.optimized, "git push --quiet");
    }

    #[test]
    fn test_optimize_git_fetch() {
        let result = optimize_command("git fetch").unwrap();
        assert_eq!(result.optimized, "git fetch --quiet");
    }

    #[test]
    fn test_optimize_git_pull() {
        let result = optimize_command("git pull").unwrap();
        assert_eq!(result.optimized, "git pull --quiet");
    }

    #[test]
    fn test_optimize_cargo_run() {
        let result = optimize_command("cargo run").unwrap();
        assert_eq!(result.optimized, "cargo run --quiet");
    }

    #[test]
    fn test_optimize_cargo_doc() {
        let result = optimize_command("cargo doc").unwrap();
        assert_eq!(result.optimized, "cargo doc --quiet");
    }

    #[test]
    fn test_optimize_docker_images() {
        let result = optimize_command("docker images").unwrap();
        assert!(result.skipped);
        assert_eq!(result.optimized, "docker images");
    }

    #[test]
    fn test_optimize_docker_with_user_format_unchanged() {
        let command = "docker ps --format '{{.ID}}'";
        let result = optimize_command(command).unwrap();
        assert!(result.skipped);
        assert_eq!(result.optimized, command);
    }

    #[test]
    fn test_optimize_case_insensitive() {
        let result = optimize_command("GIT STATUS").unwrap();
        assert_eq!(result.optimized, "GIT STATUS --porcelain -b");
    }

    #[test]
    fn test_optimize_with_existing_quiet_flag() {
        let result = optimize_command("cargo build --quiet").unwrap();
        assert!(result.skipped);
        assert_eq!(
            result.skip_reason,
            Some("all flags already present".to_string())
        );
    }

    #[test]
    fn test_optimize_npm_run() {
        let result = optimize_command("npm run build").unwrap();
        assert_eq!(result.optimized, "npm run build --silent");
    }

    #[test]
    fn test_optimize_multiple_pipes() {
        let result = optimize_command("git status | grep modified | wc -l").unwrap();
        assert!(result.optimized.contains("--porcelain"));
        assert!(result.optimized.contains("| grep modified | wc -l"));
    }

    #[test]
    fn test_optimize_or_operator_not_pipe() {
        // || is OR operator, not pipe
        let result = optimize_command("git status || echo failed").unwrap();
        assert!(result.optimized.contains("--porcelain"));
        assert!(!result.skipped);
    }

    #[test]
    fn test_optimize_and_operator() {
        // && is AND operator
        let result = optimize_command("git status && echo success").unwrap();
        assert!(result.optimized.contains("--porcelain"));
        assert!(!result.skipped);
    }

    #[test]
    fn test_optimize_backtick_in_string() {
        // Backticks in double quotes should not be treated as subshell
        // Note: This is a complex case - the current implementation
        // conservatively skips any command with backticks
        // This test documents the current behavior
        let result = optimize_command("git commit -m \"Use `code` blocks\"").unwrap();
        // Currently skipped due to backtick detection
        // This is a known limitation
        assert!(result.skipped);
    }

    #[test]
    fn test_optimize_sudo_prefix() {
        let result = optimize_command("sudo git status").unwrap();
        assert!(result.optimized.contains("--porcelain"));
        assert!(!result.skipped);
    }

    #[test]
    fn test_optimize_env_var_prefix() {
        let result = optimize_command("MY_VAR=value git status").unwrap();
        assert!(result.optimized.contains("--porcelain"));
        assert!(!result.skipped);
    }

    #[test]
    fn test_optimize_windows_exe_extension() {
        let result = optimize_command("git.exe status").unwrap();
        assert!(result.optimized.contains("--porcelain"));
        assert!(!result.skipped);
    }

    #[test]
    fn test_optimize_bitshift_not_heredoc() {
        // Bit-shift in arithmetic should not be treated as heredoc
        // Note: Current implementation conservatively skips any << pattern
        // This test documents the current behavior
        let result = optimize_command("echo $((1 << 2))").unwrap();
        // Currently skipped due to << detection
        // This is a known limitation
        assert!(result.skipped);
    }

    #[test]
    fn test_optimize_heredoc_with_delimiter() {
        let result = optimize_command("cat <<EOF").unwrap();
        assert!(result.skipped);
        assert_eq!(result.skip_reason, Some("heredoc detected".to_string()));
    }

    #[test]
    fn test_optimize_here_string() {
        let result = optimize_command("cat <<< \"input\"").unwrap();
        assert!(result.skipped);
        assert_eq!(result.skip_reason, Some("heredoc detected".to_string()));
    }
}
