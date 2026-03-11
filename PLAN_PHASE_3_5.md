# Phase 3.5 Implementation Plan: Hybrid Optimization

> Meticulous, rigorous implementation guide for pre-execution flag injection, tee mode, and DCP integration

**Status:** Ready to Begin
**Duration:** 6 Weeks (Week 7-12)
**Goal:** Achieve 90-95% token savings through hybrid optimization

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Overview](#architecture-overview)
3. [Week 7: Pre-Execution Flag System (Core Infrastructure)](#week-7-pre-execution-flag-system-core-infrastructure)
4. [Week 8: Command Flag Mappings (Complete Coverage)](#week-8-command-flag-mappings-complete-coverage)
5. [Week 9: Tee Mode Implementation](#week-9-tee-mode-implementation)
6. [Week 10: DCP-Aware Output Formatting](#week-10-dcp-aware-output-formatting)
7. [Week 11-12: Testing & Documentation](#week-11-12-testing--documentation)
8. [Success Criteria](#success-criteria)
9. [Risk Mitigation](#risk-mitigation)
10. [File Change Summary](#file-change-summary)

---

## Executive Summary

### Current State

| Component              | Status | Token Savings |
| ---------------------- | ------ | ------------- |
| Post-execution filtering | ✅ Working | 80%           |
| Pre-execution flags     | ❌ Not implemented | 0%            |
| Tee mode               | ❌ Not implemented | N/A           |
| DCP integration        | ❌ Not tested | N/A           |

### Target State

| Component              | Status | Token Savings |
| ---------------------- | ------ | ------------- |
| Hybrid optimization    | ✅ Target | 90-95%        |
| Tee mode               | ✅ Target | Recovery      |
| DCP synergy            | ✅ Target | 2-5x turns    |

### Expected Impact

| Metric            | Current (Post-Only) | With Hybrid | Improvement |
| ----------------- | ------------------- | ----------- | ----------- |
| Token savings     | 80%                 | 90-95%      | +10-15%     |
| Session length    | ~15 turns           | ~75+ turns  | 5x longer   |
| DCP synergy       | Medium              | High        | 2x more turns |

---

## Architecture Overview

### Hybrid Optimization Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    HYBRID OPTIMIZATION FLOW                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. PRE-EXECUTION (tool.execute.before):                                    │
│     • Detect command type (git, npm, cargo, etc.)                          │
│     • Apply optimization flags (--json, --quiet, --porcelain)               │
│     • Store original command in context                                      │
│     • Modify output.args.command for execution                              │
│                                                                             │
│  2. EXECUTION:                                                               │
│     • Command runs with optimized flags → smaller output                    │
│                                                                             │
│  3. POST-EXECUTION (tool.execute.after):                                    │
│     • Capture optimized output                                              │
│     • Send to RTK daemon for compression                                    │
│     • Replace with compressed version (80%+ reduction)                      │
│     • Track token savings in SQLite                                        │
│                                                                             │
│  4. TEE MODE (on compression failure):                                       │
│     • Save full output to file                                              │
│     • File path: ~/.local/share/opencode-rtk/tee/<timestamp>_<cmd>.log      │
│     • LLM can read original if needed                                       │
│                                                                             │
│  SAVINGS: Pre-execution flags (50%) + Post-execution filter (80%) = 90% total│
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component         | Responsibility                                | Rationale                                      |
| ----------------- | --------------------------------------------- | ---------------------------------------------- |
| **Rust Daemon**   | Flag mapping definitions, `optimize` RPC method | Single source of truth, testable, configurable |
| **TypeScript Plugin** | Call daemon, apply flags, track state     | Low latency via persistent connection, simple logic |

### Key Integration Points

```
plugin/src/hooks/tool-before.ts
    │
    ├── 1. Extract command from output.args.command
    ├── 2. Call daemon: client.optimizeCommand(command)
    ├── 3. Receive: { original, optimized, flags_added }
    ├── 4. Modify: output.args.command = optimized
    └── 5. Store: pendingCommands.set(callID, { original, optimized, ... })
    
plugin/src/hooks/tool-after.ts
    │
    ├── 1. Retrieve context from pendingCommands
    ├── 2. Call daemon: client.compress(command, output)
    ├── 3. Replace output if savings > 0
    └── 4. On failure: client.saveTee(command, output)
```

---

## Week 7: Pre-Execution Flag System (Core Infrastructure)

### 7.1 Rust: Create Pre-Execution Module

**File:** `crates/rtk-core/src/commands/pre_execution.rs`

**Purpose:** Define flag mappings and optimization logic

#### 7.1.1 Create Module File

```bash
# Create the new module file
touch crates/rtk-core/src/commands/pre_execution.rs
```

#### 7.1.2 Module Structure

```rust
//! Pre-execution flag optimization
//!
//! This module provides flag mappings for common commands to reduce
//! output size before execution, complementing post-execution filtering.

use anyhow::{Context, Result};
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
    
    // === DOCKER ===
    FlagMapping {
        command: "docker",
        subcommand: Some("ps"),
        flags: &["--format", "table {{.ID}}\\t{{.Names}}"],
        condition: None,
    },
    FlagMapping {
        command: "docker",
        subcommand: Some("images"),
        flags: &["--format", "table {{.Repository}}\\t{{.Tag}}"],
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
/// let result = optimize_command("git status")?;
/// assert_eq!(result.optimized, "git status --porcelain -b");
/// assert_eq!(result.flags_added, vec!["--porcelain", "-b"]);
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
    
    // Parse command into parts
    let parts = parse_command_parts(trimmed);
    
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
    
    // Extract base command (handle paths)
    let base = extract_base_command(&parts[0]);
    let subcommand = parts.get(1).map(|s| s.as_str());
    
    // Find matching flag mapping
    let mapping = find_mapping(&base, subcommand);
    
    match mapping {
        Some(m) => {
            // Check conditions
            if let Some(ref condition) = m.condition {
                match condition {
                    FlagCondition::NoFlagPresent(flag) => {
                        if parts.iter().any(|p| p == flag) {
                            return Ok(OptimizedCommand {
                                original: command.to_string(),
                                optimized: command.to_string(),
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
                                original: command.to_string(),
                                optimized: command.to_string(),
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
            
            let flags_to_add: Vec<&str> = m.flags
                .iter()
                .filter(|f| !existing_flags.contains(*f))
                .copied()
                .collect();
            
            // Skip if no new flags to add
            if flags_to_add.is_empty() {
                return Ok(OptimizedCommand {
                    original: command.to_string(),
                    optimized: command.to_string(),
                    flags_added: vec![],
                    skipped: true,
                    skip_reason: Some("all flags already present".to_string()),
                });
            }
            
            // Build optimized command
            let optimized = format!("{} {}", trimmed, flags_to_add.join(" "));
            
            Ok(OptimizedCommand {
                original: command.to_string(),
                optimized,
                flags_added: flags_to_add.iter().map(|s| s.to_string()).collect(),
                skipped: false,
                skip_reason: None,
            })
        }
        None => Ok(OptimizedCommand {
            original: command.to_string(),
            optimized: command.to_string(),
            flags_added: vec![],
            skipped: true,
            skip_reason: Some("no matching flag mapping".to_string()),
        }),
    }
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
    if command.contains('/') || (cfg!(windows) && command.contains('\\')) {
        command
            .rsplit(|c| c == '/' || c == '\\')
            .next()
            .unwrap_or(command)
            .to_lowercase()
    } else {
        command.to_lowercase()
    }
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
        assert_eq!(result.skip_reason, Some("all flags already present".to_string()));
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
    fn test_extract_base_command_simple() {
        assert_eq!(extract_base_command("git"), "git");
    }

    #[test]
    fn test_extract_base_command_with_path() {
        assert_eq!(extract_base_command("/usr/bin/git"), "git");
    }

    #[test]
    fn test_optimize_docker_ps() {
        let result = optimize_command("docker ps").unwrap();
        assert!(result.optimized.contains("--format"));
    }

    #[test]
    fn test_optimize_curl() {
        let result = optimize_command("curl http://example.com").unwrap();
        assert_eq!(result.optimized, "curl http://example.com -s");
    }

    #[test]
    fn test_optimize_preserves_case_in_args() {
        let result = optimize_command("git status -uNo").unwrap();
        // Should add flags but preserve existing args
        assert!(result.optimized.contains("-uNo"));
    }
}
```

#### 7.1.3 Export from Module

**File:** `crates/rtk-core/src/commands/mod.rs`

Add at the end of the file:

```rust
pub mod pre_execution;

// Re-export public API
pub use pre_execution::{optimize_command, FlagMapping, OptimizedCommand};
```

#### 7.1.4 Export from Library

**File:** `crates/rtk-core/src/lib.rs`

Add to the public exports section:

```rust
pub use commands::pre_execution::{optimize_command, FlagMapping, OptimizedCommand};
```

#### 7.1.5 Tasks Checklist

- [ ] Create `crates/rtk-core/src/commands/pre_execution.rs`
- [ ] Implement `FlagMapping` struct
- [ ] Implement `OptimizedCommand` struct
- [ ] Implement `FLAG_MAPPINGS` constant with all 23 mappings
- [ ] Implement `optimize_command()` function
- [ ] Implement `parse_command_parts()` helper
- [ ] Implement `extract_base_command()` helper
- [ ] Implement `find_mapping()` helper
- [ ] Add 15+ unit tests
- [ ] Export from `commands/mod.rs`
- [ ] Export from `lib.rs`
- [ ] Run `cargo test -p rtk-core` to verify

---

### 7.2 Rust: Add `optimize` RPC Method

**File:** `crates/rtk-daemon/src/handlers/optimize.rs`

#### 7.2.1 Create Handler File

```rust
//! Handler for the `optimize` JSON-RPC method

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use rtk_core::commands::pre_execution::optimize_command;
use rtk_core::config::Config;

/// Request parameters for optimize method
#[derive(Debug, Deserialize)]
pub struct OptimizeParams {
    /// The command string to optimize
    pub command: String,
}

/// Response for optimize method
#[derive(Debug, Serialize)]
pub struct OptimizeResult {
    /// Original command string
    pub original: String,
    /// Optimized command string
    pub optimized: String,
    /// Flags that were added
    pub flags_added: Vec<String>,
    /// Whether optimization was skipped
    pub skipped: bool,
    /// Reason for skipping (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
}

/// Handle the optimize JSON-RPC method
///
/// # Arguments
///
/// * `params` - JSON-RPC request parameters
/// * `config` - Daemon configuration
///
/// # Returns
///
/// JSON result with optimization details
pub async fn handle(params: Value, config: &Config) -> Result<Value> {
    // Parse parameters
    let params: OptimizeParams = serde_json::from_value(params)
        .map_err(|e| anyhow::anyhow!("Invalid parameters: {}", e))?;
    
    // Check if pre-execution is enabled
    if !config.general.enable_pre_execution_flags {
        return Ok(json!(OptimizeResult {
            original: params.command.clone(),
            optimized: params.command,
            flags_added: vec![],
            skipped: true,
            skip_reason: Some("pre-execution flags disabled in config".to_string()),
        }));
    }
    
    // Optimize command
    let result = optimize_command(&params.command)?;
    
    // Build response
    Ok(json!(OptimizeResult {
        original: result.original,
        optimized: result.optimized,
        flags_added: result.flags_added,
        skipped: result.skipped,
        skip_reason: result.skip_reason,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtk_core::config::{GeneralConfig, DaemonConfig};

    fn test_config() -> Config {
        Config {
            general: GeneralConfig {
                enable_tracking: true,
                database_path: ":memory:".to_string(),
                retention_days: 90,
                default_filter_level: "minimal".to_string(),
                verbosity: 0,
                enable_pre_execution_flags: true,
                flag_mappings_path: None,
            },
            daemon: DaemonConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_handle_optimize_git_status() {
        let config = test_config();
        let params = json!({ "command": "git status" });
        
        let result = handle(params, &config).await.unwrap();
        let result: OptimizeResult = serde_json::from_value(result).unwrap();
        
        assert_eq!(result.original, "git status");
        assert_eq!(result.optimized, "git status --porcelain -b");
        assert_eq!(result.flags_added, vec!["--porcelain", "-b"]);
        assert!(!result.skipped);
    }

    #[tokio::test]
    async fn test_handle_optimize_disabled() {
        let mut config = test_config();
        config.general.enable_pre_execution_flags = false;
        
        let params = json!({ "command": "git status" });
        let result = handle(params, &config).await.unwrap();
        let result: OptimizeResult = serde_json::from_value(result).unwrap();
        
        assert!(result.skipped);
        assert_eq!(result.original, result.optimized);
    }

    #[tokio::test]
    async fn test_handle_optimize_unknown_command() {
        let config = test_config();
        let params = json!({ "command": "unknown-command" });
        
        let result = handle(params, &config).await.unwrap();
        let result: OptimizeResult = serde_json::from_value(result).unwrap();
        
        assert!(result.skipped);
    }
}
```

#### 7.2.2 Register Handler

**File:** `crates/rtk-daemon/src/handlers/mod.rs`

Add:

```rust
pub mod optimize;

pub use optimize::handle as handle_optimize;
```

#### 7.2.3 Add to Protocol Router

**File:** `crates/rtk-daemon/src/protocol.rs`

In the `handle_request` function's match statement, add:

```rust
"optimize" => handlers::handle_optimize(request.params, config).await,
```

#### 7.2.4 Tasks Checklist

- [ ] Create `crates/rtk-daemon/src/handlers/optimize.rs`
- [ ] Implement `OptimizeParams` struct
- [ ] Implement `OptimizeResult` struct
- [ ] Implement `handle()` function
- [ ] Add config check for `enable_pre_execution_flags`
- [ ] Register handler in `handlers/mod.rs`
- [ ] Add method to protocol router in `protocol.rs`
- [ ] Add 3+ handler tests
- [ ] Run `cargo test -p rtk-daemon` to verify

---

### 7.3 Rust: Extend Configuration

**File:** `crates/rtk-core/src/config/mod.rs`

#### 7.3.1 Add New Config Fields

```rust
use serde::{Deserialize, Serialize};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub tee: TeeConfig,
}

/// General configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Enable token tracking
    #[serde(default = "default_enable_tracking")]
    pub enable_tracking: bool,
    
    /// Path to SQLite database
    #[serde(default = "default_database_path")]
    pub database_path: String,
    
    /// Days to retain history
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    
    /// Default filter level
    #[serde(default = "default_filter_level")]
    pub default_filter_level: String,
    
    /// Verbosity level (0-3)
    #[serde(default)]
    pub verbosity: u8,
    
    /// Enable pre-execution flag optimization
    #[serde(default = "default_enable_pre_execution_flags")]
    pub enable_pre_execution_flags: bool,
    
    /// Custom flag mappings file path (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flag_mappings_path: Option<String>,
}

/// Tee mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeConfig {
    /// Enable tee mode
    #[serde(default = "default_tee_enabled")]
    pub enabled: bool,
    
    /// Tee mode: "failures", "always", "never"
    #[serde(default = "default_tee_mode")]
    pub mode: String,
    
    /// Maximum number of tee files to keep
    #[serde(default = "default_tee_max_files")]
    pub max_files: usize,
    
    /// Days to retain tee files
    #[serde(default = "default_tee_retention_days")]
    pub retention_days: u32,
    
    /// Directory for tee files
    #[serde(default = "default_tee_directory")]
    pub directory: String,
}

// Default value functions
fn default_enable_tracking() -> bool { true }
fn default_retention_days() -> u32 { 90 }
fn default_filter_level() -> String { "minimal".to_string() }
fn default_enable_pre_execution_flags() -> bool { true }
fn default_tee_enabled() -> bool { true }
fn default_tee_mode() -> String { "failures".to_string() }
fn default_tee_max_files() -> usize { 20 }
fn default_tee_retention_days() -> u32 { 90 }

fn default_database_path() -> String {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("opencode-rtk");
    base.join("history.db").to_string_lossy().to_string()
}

fn default_tee_directory() -> String {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("opencode-rtk");
    base.join("tee").to_string_lossy().to_string()
}

impl Default for TeeConfig {
    fn default() -> Self {
        Self {
            enabled: default_tee_enabled(),
            mode: default_tee_mode(),
            max_files: default_tee_max_files(),
            retention_days: default_tee_retention_days(),
            directory: default_tee_directory(),
        }
    }
}
```

#### 7.3.2 Update Default Config Generation

**File:** `crates/rtk-core/src/config/settings.rs`

Update the `default_config()` function:

```rust
pub fn default_config() -> Config {
    Config {
        general: GeneralConfig {
            enable_tracking: true,
            database_path: default_database_path(),
            retention_days: 90,
            default_filter_level: "minimal".to_string(),
            verbosity: 0,
            enable_pre_execution_flags: true,
            flag_mappings_path: None,
        },
        daemon: DaemonConfig::default(),
        tee: TeeConfig::default(),
    }
}
```

#### 7.3.3 Tasks Checklist

- [ ] Add `enable_pre_execution_flags` to `GeneralConfig`
- [ ] Add `flag_mappings_path` to `GeneralConfig`
- [ ] Create `TeeConfig` struct
- [ ] Add default value functions
- [ ] Update `default_config()` function
- [ ] Update config loading to handle missing fields
- [ ] Add config validation tests
- [ ] Run `cargo test -p rtk-core` to verify

---

### 7.4 TypeScript: Add `optimizeCommand` Method

**File:** `plugin/src/client.ts`

#### 7.4.1 Add Method to RTKDaemonClient

Add this method to the `RTKDaemonClient` class:

```typescript
/**
 * Optimize a command by adding appropriate flags
 * 
 * @param command - The command string to optimize
 * @returns Optimization result with original, optimized, and flags added
 */
async optimizeCommand(command: string): Promise<OptimizeResponse> {
  const request: OptimizeRequest = { command };
  return await this.sendRequest("optimize", request);
}
```

#### 7.4.2 Add Types

**File:** `plugin/src/types.ts`

Add these interfaces:

```typescript
/**
 * Request for the optimize method
 */
export interface OptimizeRequest {
  command: string;
}

/**
 * Response from the optimize method
 */
export interface OptimizeResponse {
  /** Original command string */
  original: string;
  /** Optimized command string with flags */
  optimized: string;
  /** Flags that were added */
  flags_added: string[];
  /** Whether optimization was skipped */
  skipped: boolean;
  /** Reason for skipping (if applicable) */
  reason?: string;
}
```

#### 7.4.3 Tasks Checklist

- [ ] Add `optimizeCommand()` method to `RTKDaemonClient`
- [ ] Add `OptimizeRequest` interface to `types.ts`
- [ ] Add `OptimizeResponse` interface to `types.ts`
- [ ] Export new types from `types.ts`
- [ ] Run `bun run build` to verify

---

### 7.5 TypeScript: Modify Pre-Execution Hook

**File:** `plugin/src/hooks/tool-before.ts`

#### 7.5.1 Update PendingCommand Interface

**File:** `plugin/src/state.ts`

```typescript
/**
 * Context stored between pre and post execution hooks
 */
export interface PendingCommand {
  /** Original command string (before optimization) */
  originalCommand: string;
  /** Optimized command string (after flags added) */
  optimizedCommand: string;
  /** Flags that were added by optimization */
  flagsAdded: string[];
  /** Working directory where command was executed */
  cwd: string;
  /** Timestamp when command started (for TTL cleanup) */
  timestamp: number;
}
```

#### 7.5.2 Update Pre-Execution Hook

**File:** `plugin/src/hooks/tool-before.ts`

```typescript
import type { ToolExecuteBeforeInput, ToolExecuteBeforeOutput } from "../types";
import { RTKDaemonClient } from "../client";
import { pendingCommands } from "../state";

/**
 * Pre-execution hook for tool.execute
 * 
 * This hook:
 * 1. Detects bash commands
 * 2. Calls daemon to optimize command with flags
 * 3. Modifies the command to be executed
 * 4. Stores context for post-execution hook
 */
export async function onToolExecuteBefore(
  input: ToolExecuteBeforeInput,
  output: ToolExecuteBeforeOutput,
  client: RTKDaemonClient
): Promise<void> {
  // Only process bash commands
  if (input.tool !== "bash") {
    return;
  }

  // Extract command from args
  const originalCommand = (output.args?.command as string) || "";
  
  // Skip empty commands
  if (!originalCommand.trim()) {
    return;
  }

  try {
    // Call daemon to optimize command
    const optimized = await client.optimizeCommand(originalCommand);
    
    // Modify command if optimization was applied
    if (!optimized.skipped && optimized.flags_added.length > 0) {
      output.args = output.args || {};
      output.args.command = optimized.optimized;
      
      // Log optimization for debugging
      console.log(
        `[RTK] Pre-execution: Added flags [${optimized.flags_added.join(", ")}] to "${originalCommand}"`
      );
    }
    
    // Store context for post-execution hook
    pendingCommands.set(input.callID, {
      originalCommand,
      optimizedCommand: optimized.optimized,
      flagsAdded: optimized.flags_added,
      cwd: process.cwd(),
      timestamp: Date.now(),
    });
  } catch (error) {
    // On error, store original command and continue without optimization
    console.error("[RTK] Pre-execution optimization failed:", error);
    
    pendingCommands.set(input.callID, {
      originalCommand,
      optimizedCommand: originalCommand,
      flagsAdded: [],
      cwd: process.cwd(),
      timestamp: Date.now(),
    });
  }
}
```

#### 7.5.3 Update Post-Execution Hook

**File:** `plugin/src/hooks/tool-after.ts`

Update the metadata to include pre-execution info:

```typescript
// After successful compression, add metadata
if (compressed.saved_tokens > 0) {
  output.output = compressed.compressed;
  
  // Add metadata for debugging
  output.metadata = output.metadata || {};
  output.metadata.rtk_compressed = true;
  output.metadata.rtk_strategy = compressed.strategy;
  output.metadata.rtk_module = compressed.module;
  output.metadata.rtk_saved_tokens = compressed.saved_tokens;
  output.metadata.rtk_savings_pct = compressed.savings_pct;
  
  // Add pre-execution metadata if flags were added
  if (context.flagsAdded && context.flagsAdded.length > 0) {
    output.metadata.rtk_pre_execution_flags = context.flagsAdded;
    output.metadata.rtk_original_command = context.originalCommand;
  }
}
```

#### 7.5.4 Tasks Checklist

- [ ] Update `PendingCommand` interface in `state.ts`
- [ ] Rewrite `onToolExecuteBefore()` in `tool-before.ts`
- [ ] Add optimization call with error handling
- [ ] Store both original and optimized commands
- [ ] Modify `output.args.command` when optimization applied
- [ ] Update `onToolExecuteAfter()` in `tool-after.ts` to include pre-execution metadata
- [ ] Run `bun run build` to verify
- [ ] Test with real OpenCode session

---

## Week 8: Command Flag Mappings (Complete Coverage)

### 8.1 Complete Flag Mappings Table

| Category | Command | Subcommand | Flags | Notes |
|----------|---------|------------|-------|-------|
| **Git** | git | status | `--porcelain -b` | Machine-readable + branch |
| **Git** | git | diff | `--stat` | Summary only |
| **Git** | git | log | `--oneline` | Single line per commit |
| **Git** | git | push | `--quiet` | Filter progress |
| **Git** | git | add | (none) | Silent by default |
| **Git** | git | commit | (none) | Silent by default |
| **Git** | git | checkout | (none) | Silent by default |
| **Git** | git | branch | (none) | No flags needed |
| **npm** | npm | test | `--silent` | Suppress npm output |
| **npm** | npm | install | `--silent --no-progress` | Suppress npm + progress |
| **npm** | npm | run | `--silent` | Suppress npm output |
| **yarn** | yarn | test | `--silent` | Suppress yarn output |
| **yarn** | yarn | install | `--silent` | Suppress yarn output |
| **pnpm** | pnpm | test | `--silent` | Suppress pnpm output |
| **pnpm** | pnpm | install | `--silent` | Suppress pnpm output |
| **Cargo** | cargo | build | `--quiet` | Suppress cargo output |
| **Cargo** | cargo | test | `--quiet` | Suppress cargo output |
| **Cargo** | cargo | clippy | `--quiet` | Suppress cargo output |
| **Cargo** | cargo | check | `--quiet` | Suppress cargo output |
| **Docker** | docker | ps | `--format "table {{.ID}}\t{{.Names}}"` | Compact format |
| **Docker** | docker | images | `--format "table {{.Repository}}\t{{.Tag}}"` | Compact format |
| **pytest** | pytest | (none) | `-q` | Quiet mode |
| **curl** | curl | (none) | `-s` | Silent mode |
| **wget** | wget | (none) | `-q` | Quiet mode |

### 8.2 Edge Cases to Handle

#### 8.2.1 Pipes and Redirects

Commands with pipes should NOT have flags added to the entire command:

```bash
# Input: git status | grep modified
# Should NOT become: git status --porcelain -b | grep modified
# Instead: Optimize only the first command in the pipe
```

**Implementation:** Detect pipe character `|` and only optimize the first segment.

#### 8.2.2 Heredocs

Commands with heredocs should be skipped:

```bash
# Input: cat <<EOF
# Should be skipped entirely
```

**Implementation:** Detect `<<` and skip optimization.

#### 8.2.3 Subshells

Commands in subshells should be handled carefully:

```bash
# Input: $(git status)
# Should optimize the inner command
```

#### 8.2.4 Existing Flags

Don't add duplicate flags:

```bash
# Input: git status --porcelain
# Should become: git status --porcelain -b (only add -b)
```

### 8.3 Tasks Checklist

- [ ] Implement all 23 flag mappings in `FLAG_MAPPINGS`
- [ ] Add pipe detection and handling
- [ ] Add heredoc detection and skip
- [ ] Add subshell handling
- [ ] Add duplicate flag detection
- [ ] Add tests for each edge case
- [ ] Run `cargo test -p rtk-core` to verify

---

## Week 9: Tee Mode Implementation

### 9.1 Rust: Create Tee Module

**File:** `crates/rtk-core/src/tee/mod.rs`

#### 9.1.1 Module Structure

```rust
//! Tee mode for saving original output on compression failure
//!
//! When compression fails or is too aggressive, tee mode saves the
//! original output to a file for later recovery.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Manager for tee file operations
pub struct TeeManager {
    /// Directory for tee files
    directory: PathBuf,
    /// Maximum number of files to keep
    max_files: usize,
    /// Days to retain files
    retention_days: u32,
}

/// Entry in the tee file list
#[derive(Debug, Clone)]
pub struct TeeEntry {
    /// File path
    pub path: PathBuf,
    /// Original command
    pub command: String,
    /// Timestamp when saved
    pub timestamp: DateTime<Utc>,
    /// File size in bytes
    pub size: usize,
}

impl TeeManager {
    /// Create a new tee manager
    pub fn new(directory: PathBuf, max_files: usize, retention_days: u32) -> Self {
        Self {
            directory,
            max_files,
            retention_days,
        }
    }
    
    /// Save output to a tee file
    ///
    /// # Arguments
    ///
    /// * `command` - The command that was executed
    /// * `output` - The original output to save
    ///
    /// # Returns
    ///
    /// Path to the saved file
    pub fn save(&self, command: &str, output: &str) -> Result<PathBuf> {
        // Ensure directory exists
        fs::create_dir_all(&self.directory)
            .with_context(|| format!("Failed to create tee directory: {:?}", self.directory))?;
        
        // Generate filename
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let sanitized = sanitize_command_for_filename(command);
        let filename = format!("{}_{}.log", timestamp, sanitized);
        let path = self.directory.join(&filename);
        
        // Write file
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| format!("Failed to create tee file: {:?}", path))?;
        
        // Write header
        writeln!(file, "# RTK Tee File")?;
        writeln!(file, "# Command: {}", command)?;
        writeln!(file, "# Timestamp: {}", Utc::now().to_rfc3339())?;
        writeln!(file, "#")?;
        writeln!(file)?;
        
        // Write output
        write!(file, "{}", output)?;
        
        // Rotate old files
        self.rotate()?;
        
        Ok(path)
    }
    
    /// List all tee files
    pub fn list(&self) -> Result<Vec<TeeEntry>> {
        if !self.directory.exists() {
            return Ok(vec![]);
        }
        
        let mut entries = vec![];
        
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map(|e| e == "log").unwrap_or(false) {
                let metadata = entry.metadata()?;
                let size = metadata.len() as usize;
                
                // Parse timestamp from filename
                let filename = path.file_name().unwrap().to_string_lossy();
                let timestamp = parse_timestamp_from_filename(&filename);
                
                // Read command from file header
                let command = read_command_from_file(&path)?;
                
                entries.push(TeeEntry {
                    path,
                    command,
                    timestamp,
                    size,
                });
            }
        }
        
        // Sort by timestamp (newest first)
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        Ok(entries)
    }
    
    /// Read content of a tee file
    pub fn read(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path)
            .with_context(|| format!("Failed to read tee file: {:?}", path))
    }
    
    /// Delete a specific tee file
    pub fn delete(&self, path: &Path) -> Result<()> {
        fs::remove_file(path)
            .with_context(|| format!("Failed to delete tee file: {:?}", path))
    }
    
    /// Clear all tee files
    pub fn clear(&self) -> Result<usize> {
        let entries = self.list()?;
        let mut count = 0;
        
        for entry in entries {
            self.delete(&entry.path)?;
            count += 1;
        }
        
        Ok(count)
    }
    
    /// Rotate old files (remove files exceeding max_files or retention)
    pub fn rotate(&self) -> Result<usize> {
        let entries = self.list()?;
        let mut removed = 0;
        
        // Remove files exceeding max_files
        if entries.len() >= self.max_files {
            for entry in entries.iter().skip(self.max_files - 1) {
                self.delete(&entry.path)?;
                removed += 1;
            }
        }
        
        // Remove files older than retention_days
        let cutoff = Utc::now() - chrono::Duration::days(self.retention_days as i64);
        for entry in entries.iter() {
            if entry.timestamp < cutoff {
                self.delete(&entry.path)?;
                removed += 1;
            }
        }
        
        Ok(removed)
    }
}

/// Sanitize command for use in filename
fn sanitize_command_for_filename(command: &str) -> String {
    command
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join("_")
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => c,
            _ => '_',
        })
        .collect::<String>()
        .chars()
        .take(30)
        .collect()
}

/// Parse timestamp from filename
fn parse_timestamp_from_filename(filename: &str) -> DateTime<Utc> {
    // Format: YYYYMMDD_HHMMSS_command.log
    if filename.len() >= 15 {
        let date_part = &filename[0..8];
        let time_part = &filename[9..15];
        
        if let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(minute), Ok(second)) = (
            date_part[0..4].parse::<i32>(),
            date_part[4..6].parse::<u32>(),
            date_part[6..8].parse::<u32>(),
            time_part[0..2].parse::<u32>(),
            time_part[2..4].parse::<u32>(),
            time_part[4..6].parse::<u32>(),
        ) {
            return chrono::NaiveDate::from_ymd_opt(year, month, day)
                .and_then(|d| d.and_hms_opt(hour, minute, second))
                .map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
                .unwrap_or_else(Utc::now);
        }
    }
    
    Utc::now()
}

/// Read command from file header
fn read_command_from_file(path: &Path) -> Result<String> {
    let content = fs::read_to_string(path)?;
    
    for line in content.lines().take(5) {
        if line.starts_with("# Command: ") {
            return Ok(line[11..].to_string());
        }
    }
    
    Ok("unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sanitize_command() {
        assert_eq!(sanitize_command_for_filename("git status"), "git_status");
        assert_eq!(sanitize_command_for_filename("npm test --watch"), "npm_test");
        assert_eq!(sanitize_command_for_filename("cargo build --release"), "cargo_build");
    }

    #[test]
    fn test_save_and_read() {
        let dir = tempdir().unwrap();
        let manager = TeeManager::new(dir.path().to_path_buf(), 10, 90);
        
        let path = manager.save("git status", "M file.rs\nA file.ts").unwrap();
        assert!(path.exists());
        
        let content = manager.read(&path).unwrap();
        assert!(content.contains("git status"));
        assert!(content.contains("M file.rs"));
    }

    #[test]
    fn test_list() {
        let dir = tempdir().unwrap();
        let manager = TeeManager::new(dir.path().to_path_buf(), 10, 90);
        
        manager.save("git status", "output1").unwrap();
        manager.save("npm test", "output2").unwrap();
        
        let entries = manager.list().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_rotation() {
        let dir = tempdir().unwrap();
        let manager = TeeManager::new(dir.path().to_path_buf(), 2, 90);
        
        manager.save("cmd1", "output1").unwrap();
        manager.save("cmd2", "output2").unwrap();
        manager.save("cmd3", "output3").unwrap(); // Should trigger rotation
        
        let entries = manager.list().unwrap();
        assert!(entries.len() <= 2);
    }
}
```

#### 9.1.2 Export from Library

**File:** `crates/rtk-core/src/lib.rs`

```rust
pub mod tee;

pub use tee::{TeeManager, TeeEntry};
```

### 9.2 Rust: Add Tee RPC Methods

**File:** `crates/rtk-daemon/src/handlers/tee.rs`

```rust
//! Handlers for tee-related JSON-RPC methods

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

use rtk_core::config::Config;
use rtk_core::tee::{TeeEntry, TeeManager};

/// Request for tee_save method
#[derive(Debug, Deserialize)]
pub struct TeeSaveParams {
    pub command: String,
    pub output: String,
}

/// Response for tee_save method
#[derive(Debug, Serialize)]
pub struct TeeSaveResult {
    pub path: String,
    pub size: usize,
}

/// Response for tee_list method
#[derive(Debug, Serialize)]
pub struct TeeListResult {
    pub files: Vec<TeeFileInfo>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct TeeFileInfo {
    pub path: String,
    pub command: String,
    pub timestamp: String,
    pub size: usize,
}

/// Response for tee_read method
#[derive(Debug, Serialize)]
pub struct TeeReadResult {
    pub content: String,
    pub size: usize,
}

/// Handle tee_save method
pub async fn handle_save(params: Value, config: &Config) -> Result<Value> {
    if !config.tee.enabled {
        return Ok(json!({
            "error": "Tee mode is disabled in config"
        }));
    }
    
    let params: TeeSaveParams = serde_json::from_value(params)?;
    
    let manager = TeeManager::new(
        PathBuf::from(&config.tee.directory),
        config.tee.max_files,
        config.tee.retention_days,
    );
    
    let path = manager.save(&params.command, &params.output)?;
    let size = params.output.len();
    
    Ok(json!(TeeSaveResult {
        path: path.to_string_lossy().to_string(),
        size,
    }))
}

/// Handle tee_list method
pub async fn handle_list(_params: Value, config: &Config) -> Result<Value> {
    let manager = TeeManager::new(
        PathBuf::from(&config.tee.directory),
        config.tee.max_files,
        config.tee.retention_days,
    );
    
    let entries = manager.list()?;
    let total = entries.len();
    
    let files: Vec<TeeFileInfo> = entries
        .into_iter()
        .map(|e| TeeFileInfo {
            path: e.path.to_string_lossy().to_string(),
            command: e.command,
            timestamp: e.timestamp.to_rfc3339(),
            size: e.size,
        })
        .collect();
    
    Ok(json!(TeeListResult { files, total }))
}

/// Handle tee_read method
#[derive(Debug, Deserialize)]
pub struct TeeReadParams {
    pub path: String,
}

pub async fn handle_read(params: Value, config: &Config) -> Result<Value> {
    let params: TeeReadParams = serde_json::from_value(params)?;
    
    let manager = TeeManager::new(
        PathBuf::from(&config.tee.directory),
        config.tee.max_files,
        config.tee.retention_days,
    );
    
    let path = PathBuf::from(&params.path);
    let content = manager.read(&path)?;
    let size = content.len();
    
    Ok(json!(TeeReadResult { content, size }))
}

/// Handle tee_clear method
pub async fn handle_clear(_params: Value, config: &Config) -> Result<Value> {
    let manager = TeeManager::new(
        PathBuf::from(&config.tee.directory),
        config.tee.max_files,
        config.tee.retention_days,
    );
    
    let count = manager.clear()?;
    
    Ok(json!({ "deleted": count }))
}
```

#### 9.2.1 Register Handlers

**File:** `crates/rtk-daemon/src/handlers/mod.rs`

```rust
pub mod tee;

pub use tee::{
    handle_save as handle_tee_save,
    handle_list as handle_tee_list,
    handle_read as handle_tee_read,
    handle_clear as handle_tee_clear,
};
```

#### 9.2.2 Add to Protocol Router

**File:** `crates/rtk-daemon/src/protocol.rs`

```rust
"tee_save" => handlers::handle_tee_save(request.params, config).await,
"tee_list" => handlers::handle_tee_list(request.params, config).await,
"tee_read" => handlers::handle_tee_read(request.params, config).await,
"tee_clear" => handlers::handle_tee_clear(request.params, config).await,
```

### 9.3 TypeScript: Integrate Tee Mode

**File:** `plugin/src/client.ts`

```typescript
/**
 * Save output to tee file
 */
async saveTee(command: string, output: string): Promise<TeeSaveResponse> {
  return await this.sendRequest("tee_save", { command, output });
}

/**
 * List tee files
 */
async listTee(): Promise<TeeListResponse> {
  return await this.sendRequest("tee_list", {});
}

/**
 * Read tee file content
 */
async readTee(path: string): Promise<TeeReadResponse> {
  return await this.sendRequest("tee_read", { path });
}
```

**File:** `plugin/src/types.ts`

```typescript
export interface TeeSaveRequest {
  command: string;
  output: string;
}

export interface TeeSaveResponse {
  path: string;
  size: number;
}

export interface TeeListResponse {
  files: TeeFileInfo[];
  total: number;
}

export interface TeeFileInfo {
  path: string;
  command: string;
  timestamp: string;
  size: number;
}

export interface TeeReadRequest {
  path: string;
}

export interface TeeReadResponse {
  content: string;
  size: number;
}
```

**File:** `plugin/src/hooks/tool-after.ts`

Add tee save on compression failure:

```typescript
// In the catch block or when compression fails:
if (config.tee?.enabled && config.tee?.mode === "failures") {
  try {
    const teeResult = await client.saveTee(context.originalCommand, output.output);
    console.error(`[RTK] Compression failed, saved original output to: ${teeResult.path}`);
    
    // Add tee path to metadata
    output.metadata = output.metadata || {};
    output.metadata.rtk_tee_path = teeResult.path;
  } catch (teeError) {
    console.error("[RTK] Failed to save tee file:", teeError);
  }
}
```

### 9.4 Tasks Checklist

- [ ] Create `crates/rtk-core/src/tee/mod.rs`
- [ ] Implement `TeeManager` struct
- [ ] Implement `save()`, `list()`, `read()`, `delete()`, `clear()`, `rotate()`
- [ ] Add helper functions for filename sanitization
- [ ] Add 5+ unit tests
- [ ] Create `crates/rtk-daemon/src/handlers/tee.rs`
- [ ] Implement `tee_save`, `tee_list`, `tee_read`, `tee_clear` handlers
- [ ] Register handlers in `handlers/mod.rs`
- [ ] Add methods to protocol router
- [ ] Add TypeScript client methods
- [ ] Add TypeScript types
- [ ] Integrate with `tool-after.ts` for failure handling
- [ ] Run all tests

---

## Week 10: DCP-Aware Output Formatting

### 10.1 DCP Compatibility Guidelines

**Goal:** Format compressed output to maximize DCP's ability to keep context

| Guideline | Rationale |
|-----------|-----------|
| Consistent format across commands | DCP can better predict token patterns |
| Group related information | Reduces unique token sequences |
| Minimize unique identifiers | Less token variety = better compression |
| Avoid redundant timestamps | DCP already tracks time |
| Use structured summaries | Easier for DCP to parse |

### 10.2 Output Format Standardization

**Current formats to review:**

| Module | Current Output | DCP-Optimized Output |
|--------|---------------|---------------------|
| git status | `M file.rs\nA file.ts` | `2 files: 1 modified, 1 added` |
| npm test | Full test output | `Tests: 10 passed, 2 failed` |
| cargo build | Full compiler output | `Build: 3 warnings, 0 errors` |

### 10.3 Tasks Checklist

- [ ] Review all 26 command modules for DCP compatibility
- [ ] Standardize output format patterns
- [ ] Add `dcp_optimized` flag to config
- [ ] Create test scenarios with DCP enabled
- [ ] Measure context size improvement
- [ ] Document DCP interaction in README.md

---

## Week 11-12: Testing & Documentation

### 11.1 Test Coverage Requirements

| Category | Tests | Priority |
|----------|-------|----------|
| Pre-execution flag logic | 25 | High |
| Tee mode operations | 15 | High |
| DCP integration | 10 | Medium |
| End-to-end hybrid flow | 10 | High |
| Edge cases (pipes, redirects) | 10 | Medium |
| **Total** | **70** | |

### 11.2 Test Files to Create/Update

| File | Tests |
|------|-------|
| `crates/rtk-core/src/commands/pre_execution.rs` | 25 |
| `crates/rtk-core/src/tee/mod.rs` | 10 |
| `crates/rtk-daemon/src/handlers/optimize.rs` | 5 |
| `crates/rtk-daemon/src/handlers/tee.rs` | 5 |
| `plugin/src/client.test.ts` | 10 |
| `plugin/src/hooks/tool-before.test.ts` | 10 |
| `plugin/src/hooks/tool-after.test.ts` | 5 |

### 11.3 Documentation Updates

| File | Updates |
|------|---------|
| `README.md` | Hybrid mode section, before/after examples, DCP synergy |
| `AGENTS.md` | Pre-execution workflow, flag mapping table, tee mode |
| `ARCHITECTURE.md` | Hybrid flow diagram, tee mode design, DCP integration |
| `PLAN.md` | Mark Phase 3.5 complete, update status |

### 11.4 README.md Section to Add

```markdown
## Hybrid Optimization Mode

OpenCode-RTK uses a hybrid approach for maximum token savings:

### Pre-Execution Flag Injection

Before a command runs, RTK adds optimization flags:

| Command | Flags Added | Savings |
|---------|-------------|---------|
| `git status` | `--porcelain -b` | 70% |
| `npm test` | `--silent` | 60% |
| `cargo build` | `--quiet` | 50% |

### Post-Execution Filtering

After a command runs, RTK compresses the output:

| Command | Strategy | Savings |
|---------|-----------|---------|
| `git status` | Stats extraction | 80% |
| `npm test` | Error only | 90% |
| `cargo build` | Error only | 85% |

### Combined Savings

Pre-execution (50%) + Post-execution (80%) = **90-95% total reduction**

### DCP Synergy

With RTK, DCP can keep 2-5x more conversation turns:

| Metric | Without RTK | With RTK |
|--------|-------------|----------|
| Session length | ~15 turns | ~75+ turns |
| Context per turn | ~10k tokens | ~2k tokens |
```

### 11.5 Tasks Checklist

- [ ] Write 25 pre-execution tests
- [ ] Write 15 tee mode tests
- [ ] Write 10 DCP integration tests
- [ ] Write 10 end-to-end tests
- [ ] Write 10 edge case tests
- [ ] Update README.md with hybrid section
- [ ] Update AGENTS.md with workflow
- [ ] Update ARCHITECTURE.md with diagrams
- [ ] Update PLAN.md status
- [ ] Run full test suite: `cargo test && cd plugin && bun test`
- [ ] Run lint: `cargo clippy && cargo fmt -- --check`
- [ ] Run TypeScript lint: `cd plugin && bun run lint`

---

## Success Criteria

### Technical Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Pre-execution coverage | 20+ commands | Count in FLAG_MAPPINGS |
| Token savings | 90-95% | Benchmark with real sessions |
| Latency overhead | <1ms additional | Benchmark pre vs post hybrid |
| Memory usage | <10MB | Profile daemon |
| Test coverage | 70+ new tests | Count test functions |

### Feature Metrics

| Feature | Target | Verification |
|---------|--------|--------------|
| Flag mappings | 23 commands | All in FLAG_MAPPINGS |
| Tee mode | Working | Test save/list/read |
| DCP synergy | 2x more turns | Test with DCP enabled |
| Documentation | 4 files updated | Review content |

### Quality Gates

| Gate | Requirement |
|------|-------------|
| All tests pass | `cargo test && bun test` |
| No clippy warnings | `cargo clippy -- -D warnings` |
| Formatted code | `cargo fmt -- --check` |
| TypeScript compiles | `bun run build` |

---

## Risk Mitigation

| Risk | Mitigation | Owner |
|------|------------|-------|
| Flag breaks command | Duplicate detection, test all mappings | Implementer |
| Performance regression | Benchmark pre vs post hybrid (<5ms target) | Implementer |
| Tee file overflow | Rotation + max files limit | Implementer |
| Cross-platform issues | Test on Windows/macOS/Linux | Implementer |
| DCP incompatibility | Test with DCP enabled, adjust formats | Implementer |

---

## File Change Summary

### New Files

| File | Lines (est.) |
|------|--------------|
| `crates/rtk-core/src/commands/pre_execution.rs` | 300 |
| `crates/rtk-core/src/tee/mod.rs` | 200 |
| `crates/rtk-daemon/src/handlers/optimize.rs` | 100 |
| `crates/rtk-daemon/src/handlers/tee.rs` | 150 |

### Modified Files

| File | Changes |
|------|---------|
| `crates/rtk-core/src/commands/mod.rs` | Add pre_execution module |
| `crates/rtk-core/src/config/mod.rs` | Add TeeConfig, enable_pre_execution_flags |
| `crates/rtk-core/src/lib.rs` | Export new modules |
| `crates/rtk-daemon/src/handlers/mod.rs` | Register new handlers |
| `crates/rtk-daemon/src/protocol.rs` | Add new RPC methods |
| `plugin/src/client.ts` | Add optimizeCommand, tee methods |
| `plugin/src/hooks/tool-before.ts` | Implement pre-execution logic |
| `plugin/src/hooks/tool-after.ts` | Add tee on failure |
| `plugin/src/state.ts` | Update PendingCommand interface |
| `plugin/src/types.ts` | Add new interfaces |
| `README.md` | Add hybrid section |
| `AGENTS.md` | Update workflow |
| `ARCHITECTURE.md` | Add diagrams |
| `PLAN.md` | Update status |

### Total Estimated Changes

| Category | Count |
|----------|-------|
| New files | 4 |
| Modified files | 13 |
| New lines of code | ~750 |
| New tests | 70+ |

---

## Implementation Order

### Week 7 (Core Infrastructure)

1. Create `pre_execution.rs` module
2. Implement `optimize_command()` function
3. Add `optimize` RPC handler
4. Extend configuration
5. Add TypeScript client method
6. Update pre-execution hook

### Week 8 (Flag Mappings)

1. Complete all 23 flag mappings
2. Handle edge cases (pipes, heredocs)
3. Add comprehensive tests

### Week 9 (Tee Mode)

1. Create `tee/mod.rs` module
2. Implement TeeManager
3. Add tee RPC handlers
4. Integrate with TypeScript

### Week 10 (DCP Integration)

1. Review output formats
2. Standardize for DCP
3. Test with DCP enabled

### Week 11-12 (Testing & Docs)

1. Write all tests
2. Update documentation
3. Final verification

---

## Quick Start for New Session

When starting a new session to implement this plan:

```bash
# 1. Read this plan
cat PLAN_PHASE_3_5.md

# 2. Check current status
cat PLAN.md | grep "Phase 3.5"

# 3. Start with Week 7, Task 7.1
# Create the pre_execution.rs module

# 4. Follow the task checklists in order
# Each section has detailed implementation instructions

# 5. Run tests after each major component
cargo test -p rtk-core
cargo test -p rtk-daemon
cd plugin && bun test

# 6. Update this plan as you complete tasks
# Mark checkboxes as [x] when done
```

---

**End of Phase 3.5 Implementation Plan**
