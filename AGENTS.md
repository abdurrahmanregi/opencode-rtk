# OpenCode-RTK Agent Instructions

This document provides essential information for AI coding agents working on the OpenCode-RTK codebase.

## Project Overview

OpenCode-RTK is a token optimization proxy for OpenCode CLI that reduces LLM token consumption by 60-90% through intelligent command output filtering.

**Tech Stack**: Rust (core library, daemon, CLI), TypeScript (OpenCode plugin), SQLite (tracking)

**Architecture**: 3 Rust crates (rtk-core, rtk-daemon, rtk-cli) + 1 TypeScript plugin

## Build Commands

### Rust

```bash
# Build all crates
cargo build

# Build in release mode (optimized)
cargo build --release

# Build specific crate
cargo build -p rtk-core
cargo build -p rtk-daemon
cargo build -p rtk-cli

# Clean build artifacts
cargo clean
```

### TypeScript

```bash
cd plugin
bun install          # Install dependencies
bun run build        # Compile TypeScript
bun run dev          # Watch mode
```

## Test Commands

### Rust

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p rtk-core

# Run single test
cargo test test_name
cargo test test_git_status -p rtk-core

# Run tests with output
cargo test -- --nocapture

# Run tests in specific module
cargo test --lib rtk_core::commands::git::tests

# Run integration tests only
cargo test --test '*'

# Run unit tests only
cargo test --lib
```

### TypeScript

```bash
cd plugin
bun test              # Run all tests
bun test src/client.test.ts  # Run specific test file
```

## Lint & Format

### Rust

```bash
# Format code
cargo fmt

# Check formatting without applying
cargo fmt -- --check

# Run linter (Clippy)
cargo clippy

# Run clippy with strict warnings
cargo clippy -- -D warnings

# Fix clippy warnings automatically
cargo clippy --fix
```

### TypeScript

```bash
cd plugin
bun run lint           # Run linter (if configured)
bun run format         # Format code (if configured)
```

## Code Style Guidelines

### Rust

#### Imports

```rust
// Standard library first
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// External crates second (alphabetical)
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::net::UnixListener;

// Internal modules last (crate-level first, then submodules)
use crate::commands::CommandModule;
use crate::filter::{Strategy, StatsExtraction};
use crate::tracking::TrackingEntry;
```

#### Formatting

- **Indentation**: 4 spaces (enforced by `cargo fmt`)
- **Line length**: 100 characters max (default)
- **Brace style**: Same line for functions, structs, enums
- **Trailing commas**: Always in multi-line

```rust
// Good
pub struct Config {
    pub enable_tracking: bool,
    pub database_path: String,
    pub retention_days: u32,
}

// Bad
pub struct Config {
    pub enable_tracking: bool,
    pub database_path: String,
    pub retention_days: u32};
```

#### Types

- **Use `Result<T>` from anyhow** for error handling
- **Use `Option<T>` for nullable values**
- **Use `&str` for string parameters, `String` for owned strings**
- **Use `Path`/`PathBuf` for file paths**
- **Prefer `usize` for sizes/counts, `i32` for exit codes**

```rust
// Good
pub fn compress(command: &str, output: &str, context: Context) -> Result<CompressedOutput>

// Bad
pub fn compress(command: String, output: String, context: Context) -> std::result::Result<CompressedOutput, Box<dyn std::error::Error>>
```

#### Naming Conventions

- **Functions/variables**: `snake_case`
- **Types/structs/traits**: `PascalCase`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules**: `snake_case`
- **File names**: `snake_case.rs`

```rust
// Good
pub fn estimate_tokens(text: &str) -> usize { ... }
pub struct TrackingEntry { ... }
const MAX_BUFFER_SIZE: usize = 1024 * 1024;
mod token_tracker;

// Bad
pub fn EstimateTokens(text: &str) -> usize { ... }
pub struct trackingEntry { ... }
const maxBufferSize: usize = 1024 * 1024;
mod TokenTracker;
```

#### Error Handling

```rust
// Use .context() for meaningful error messages
let conn = Connection::open(&db_path)
    .with_context(|| format!("Failed to open database: {:?}", db_path))?;

// Use .map_err() for custom errors
let result = parse_output(output)
    .map_err(|e| anyhow::anyhow!("Parsing failed: {}", e))?;

// Use ? operator for early returns
let config = load_config()?;
let db = init_db()?;

// Provide context in error messages
.context("Failed to compress output")?;
.context(format!("Invalid command: {}", command))?;
```

#### Documentation

```rust
/// Short description (imperative mood)
/// 
/// Detailed description with examples.
/// 
/// # Arguments
/// 
/// * `command` - The command string to detect
/// * `output` - The raw command output to compress
/// 
/// # Returns
/// 
/// The compressed output with token savings
/// 
/// # Errors
/// 
/// Returns an error if compression fails
/// 
/// # Examples
/// 
/// ```
/// let result = compress("git status", "M file.rs", context)?;
/// ```
pub fn compress(command: &str, output: &str, context: Context) -> Result<CompressedOutput> {
    ...
}
```

### TypeScript

#### Imports

```typescript
// Node.js built-ins first
import * as net from "net";
import * as path from "path";

// External packages second (alphabetical)
import type { Plugin } from "@opencode-ai/plugin";

// Internal modules last
import { RTKDaemonClient } from "./client";
import type { CompressRequest } from "./types";
```

#### Formatting

- **Indentation**: 2 spaces
- **Semicolons**: Required
- **Quotes**: Double quotes for strings
- **Trailing commas**: Always

```typescript
// Good
const config: Config = {
  socketPath: "/tmp/opencode-rtk.sock",
  timeout: 5000,
};

// Bad
const config: Config = {
  socketPath: '/tmp/opencode-rtk.sock',
  timeout: 5000
}
```

#### Types

- **Always use TypeScript strict mode**
- **Prefer interfaces over type aliases for objects**
- **Use `type` for unions, intersections, primitives**
- **Avoid `any`, use `unknown` when type is truly unknown**

```typescript
// Good
interface CompressRequest {
  command: string;
  output: string;
  context?: {
    cwd?: string;
    exit_code?: number;
  };
}

type Strategy = "stats_extraction" | "error_only" | "grouping";

// Bad
interface CompressRequest {
  command: any;
  output: any;
  context?: any;
}
```

#### Error Handling

```typescript
// Always handle errors with try-catch
try {
  const result = await client.compress(request);
  return result;
} catch (error) {
  console.error("RTK: Compression failed:", error);
  // Fall back to original output
  return { compressed: request.output, saved_tokens: 0 };
}

// Use error types when possible
if (error instanceof Error) {
  console.error(error.message);
}
```

## OpenCode-RTK Workflow

### Architecture Overview

OpenCode-RTK uses a **hybrid optimization approach** combining pre-execution flag injection and post-execution filtering for maximum token savings.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    HYBRID OPTIMIZATION FLOW                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. PRE-EXECUTION (tool.execute.before):                                       │
│     • Detect command type (git, npm, cargo, etc.)                          │
│     • Apply optimization flags (--json, --quiet, --porcelain)                  │
│     • Store original command in context                                          │
│     • Modify output.args.command for execution                                    │
│                                                                             │
│  2. EXECUTION:                                                                │
│     • Command runs with optimized flags → smaller output                       │
│                                                                             │
│  3. POST-EXECUTION (tool.execute.after):                                        │
│     • Capture optimized output                                                │
│     • Send to RTK daemon for compression                                     │
│     • Replace with compressed version (80%+ reduction)                        │
│     • Track token savings in SQLite                                         │
│                                                                             │
│  4. OPTIONAL: TEE MODE                                                          │
│     • On compression failure, save full output to file                         │
│     • File path: ~/.local/share/opencode-rtk/tee/<timestamp>_<cmd>.log     │
│     • LLM can read original if needed                                        │
│                                                                             │
│  SAVINGS: Pre-execution flags (50%) + Post-execution filter (80%) = 90% total  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Differences from Original RTK

| Aspect                  | Original RTK (Claude Code) | OpenCode-RTK (OpenCode)     |
| ----------------------- | ----------------------------- | ------------------------------ |
| **Integration**           | Claude Code hooks            | OpenCode plugin API              |
| **When to intercept**   | BEFORE execution (rewrite)     | BOTH: before (flags) + after (filter) |
| **Communication**       | Direct CLI spawn             | Persistent daemon via socket       |
| **Process model**       | Per-command process         | Single daemon process            |
| **Platform**            | Unix-focused                | Cross-platform (Unix + Windows)  |
| **Language**            | Rust only                   | Rust daemon + TypeScript plugin  |
| **Command optimization** | None                        | Pre-execution flag injection      |
| **Tee mode**           | Built-in                    | Planned (Phase 3.5)           |

### Pre-Execution Flag Optimizations

**Why this approach:** Original RTK authors chose to prepend `rtk` to commands (e.g., `rtk git status`) because it provides command interception capability via shell hooks. Our approach achieves the same optimization (adding flags) but uses OpenCode's plugin API instead of shell hooks.

**Current flag mappings (hardcoded):**

```rust
// crates/rtk-core/src/commands/pre_execution.rs

pub const FLAG_MAPPINGS: &[(&str, &[&str])] = &[
    // Git
    ("git status", &["--porcelain", "-b"]),
    ("git diff", &["--stat"]),  // fallback to full diff if < 100 lines
    ("git log", &["--oneline"]),
    ("git add", &[]),  // No flags (silent by default)
    ("git commit", &[]),  // No flags (silent by default)
    ("git push", &["--quiet"]),  // Filter progress
    ("git branch", &[]),
    ("git checkout", &[]),
    
    // npm/yarn/pnpm
    ("npm test", &["--silent"]),
    ("npm install", &["--silent", "--no-progress"]),
    ("yarn test", &["--silent"]),
    ("pnpm test", &["--silent"]),
    ("pnpm install", &["--silent"]),
    
    // Cargo
    ("cargo build", &["--quiet"]),
    ("cargo test", &["--quiet"]),
    ("cargo clippy", &["--quiet"]),
    
    // Docker
    ("docker ps", &["--format", "table {{.ID}}\\t{{.Names}}"]),
    ("docker images", &["--format", "table {{.Repository}}\\t{{.Tag}}"]),
    
    // Test runners
    ("pytest", &["-q"]),  // quiet
    ("go test", &[]),  // Use -json only if verbose
    
    // Network tools
    ("curl", &["-s"]),  // silent
    ("wget", &["-q"]),  // quiet
];
```

### DCP (Dynamic Context Pruning) Synergy

**How RTK helps DCP:**
- Smaller compressed messages = DCP can keep more turns in context
- Example: Without RTK, session lasts ~15 turns before hitting 200k tokens
- With RTK, session lasts ~75+ turns (5x improvement)
- DCP can be less aggressive, preserving more context

**DCP-aware optimizations:**
- Use consistent output format across commands
- Group related information (e.g., test results summary)
- Minimize unique identifiers in output
- Avoid redundant timestamps

### Plugin Hooks Implementation (Hybrid)

**tool.execute.before - Expanded:**
```typescript
export const onToolExecuteBefore = async (input, output) => {
  if (input.tool === "bash") {
    const command = output.args.command;
    
    // Detect command type and apply flags
    const optimized = applyPreExecutionFlags(command);
    
    // Store context for post-execution hook
    pendingCommands.set(input.callID, {
      originalCommand: command,
      optimizedCommand: optimized,
      timestamp: Date.now(),
    });
    
    // Modify command to be executed
    output.args.command = optimized;
  }
};
```

**tool.execute.after - Unchanged:**
```typescript
export const onToolExecuteAfter = async (input, output) => {
  if (input.tool === "bash") {
    const context = pendingCommands.get(input.callID);
    
    if (context) {
      try {
        // Send to daemon for compression
        const compressed = await rtkClient.compress({
          command: context.optimizedCommand,
          output: output.output,
          context: { cwd, exit_code, tool },
        });
        
        // Replace output if savings > 0
        if (compressed.saved_tokens > 0) {
          output.output = compressed.compressed;
          // Add metadata for debugging
          console.log(`[RTK] ${compressed.saved_tokens} tokens saved (${compressed.savings_pct}%)`);
        }
      } catch (error) {
        console.error("[RTK] Compression failed, using original output");
        // Fallback to original (already in output.output)
      }
    }
    
    pendingCommands.delete(input.callID);
  }
};
```

### When Working on OpenCode-RTK

1. **Pre-execution flags** → Apply optimization in `tool.execute.before`
2. **Post-execution compression** → Core daemon handles filtering
3. **DCP-aware output** → Format for DCP compatibility
4. **Tee mode** → Save original on failure (when implemented)
5. **Memory efficiency** → Daemon uses <10MB steady state

---

## Testing Patterns

### Rust Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_simple() {
        let tokens = estimate_tokens("hello");
        assert!(tokens > 0);
        assert!(tokens < 10);
    }

    #[test]
    fn test_compress_git_status() {
        let module = GitModule::new();
        let input = "M file1.rs\nA file2.rs";
        let result = module.compress(input, &Context::default()).unwrap();
        
        assert!(result.contains("2 files changed"));
        assert!(result.contains("1 modified"));
    }
}
```

### Integration Tests

Create files in `tests/` directory:

```rust
// tests/integration_test.rs
use rtk_core::{compress, Context};

#[tokio::test]
async fn test_daemon_health() {
    // Start daemon
    // Test health endpoint
    // Assert response
}
```

## Project Structure Conventions

### Rust Modules

- **`mod.rs`** - Module declaration and public API
- **One module per file** - `git.rs`, `npm_cmd.rs`, etc.
- **Submodules in directories** - `commands/mod.rs` exports `commands::git`

### File Organization

**Rust:**
```
crates/rtk-core/src/
├── lib.rs              # Public API exports
├── commands/           # Command-specific modules
│   ├── mod.rs          # Registry and detection
│   ├── git.rs          # Git commands
│   └── ...
├── filter/             # Filtering strategies
│   ├── mod.rs          # Strategy trait
│   ├── stats.rs        # Stats extraction
│   └── ...
├── tracking/           # Token tracking
│   ├── mod.rs          # Public API
│   └── db.rs           # SQLite operations
└── utils/              # Shared utilities
    └── tokens.rs       # Token estimation
```

**TypeScript Plugin:**
```
plugin/src/
├── index.ts                  # Plugin entry with auto-start daemon
├── client.ts                 # RTKDaemonClient (TCP/Unix socket)
├── spawn.ts                 # Daemon spawn and lifecycle management
├── state.ts                 # Plugin state (pending commands, cleanup)
├── hooks/
│   ├── tool-before.ts         # Pre-tool execution hook
│   ├── tool-after.ts         # Post-tool execution hook
│   └── session.ts           # Session idle hook
└── types.ts                 # TypeScript interfaces
```

## Common Patterns

### TypeScript Plugin Auto-Start Pattern

```typescript
// Auto-start daemon with race condition protection
let startPromise: Promise<boolean> | null = null;
let isStarting = false;

export const RTKPlugin: Plugin = async ({ directory, worktree }) => {
  const client = new RTKDaemonClient(RTK_SOCKET_PATH);
  
  // Check if daemon is already running
  let isHealthy = await isDaemonRunning(client);
  
  if (!isHealthy) {
    // Use promise-based lock to prevent concurrent spawns
    if (startPromise || isStarting) {
      console.log("[RTK] Waiting for existing daemon startup...");
      isHealthy = await startPromise!;
    } else {
      isStarting = true;
      startPromise = (async () => {
        try {
          console.log(`[RTK] Daemon not running, starting '${RTK_BINARY}'...`);
          return await autoStartDaemon(RTK_BINARY, client);
        } finally {
          isStarting = false;
        }
      })();
      
      isHealthy = await startPromise;
      startPromise = null;
    }
  }
  
  if (isHealthy) {
    console.log("[RTK] Daemon is running");
  } else {
    console.error("[RTK] Failed to start daemon after multiple attempts");
  }
  
  // Return plugin hooks...
};
```

### Daemon Spawn Pattern

```typescript
// Platform-aware daemon spawning
export function spawnDaemon(binaryPath: string): DaemonSpawnResult {
  const isWindows = os.platform() === "win32";
  
  let child: cp.ChildProcess;
  const spawnOptions: cp.SpawnOptions = {
    detached: true,
    stdio: "ignore",
    windowsHide: isWindows,
  };
  
  try {
    if (isWindows) {
      child = cp.spawn(binaryPath, [], spawnOptions);
    } else {
      child = cp.spawn(binaryPath, [], spawnOptions);
      child.unref(); // Allow parent to exit
    }
    
    // Attach event listeners for async errors
    child.on('error', (error) => {
      console.error(`[RTK] Daemon failed to start: ${error.message}`);
    });
    
    return { process: child, success: true };
  } catch (error) {
    return {
      process: null,
      success: false,
      error: error instanceof Error ? error.message : String(error)
    };
  }
}
```

### Health Check with Exponential Backoff

```typescript
export async function waitForDaemon(
  client: RTKDaemonClient,
  maxAttempts: number = 15,
  initialDelayMs: number = 200
): Promise<boolean> {
  for (let i = 0; i < maxAttempts; i++) {
    const isHealthy = await isDaemonRunning(client);
    if (isHealthy) {
      return true;
    }
    
    // Exponential backoff with jitter (prevents thundering herd)
    const backoff = Math.min(initialDelayMs * Math.pow(1.5, i), 2000);
    const jitter = Math.random() * 50;
    const delay = backoff + jitter;
    
    await new Promise(resolve => setTimeout(resolve, delay));
  }
  
  return false;
}
```

### Command Module Pattern

```rust
pub struct GitModule {
    strategy: StatsExtraction,
}

impl GitModule {
    pub fn new() -> Self {
        Self {
            strategy: StatsExtraction,
        }
    }
}

impl CommandModule for GitModule {
    fn name(&self) -> &str { "git" }
    fn strategy(&self) -> &str { self.strategy.name() }
    fn compress(&self, output: &str, context: &Context) -> Result<String> {
        self.strategy.compress(output)
    }
}
```

### Strategy Pattern

```rust
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    fn compress(&self, input: &str) -> Result<String>;
}

impl Strategy for StatsExtraction {
    fn name(&self) -> &str { "stats_extraction" }
    fn compress(&self, input: &str) -> Result<String> {
        // Implementation
    }
}
```

## Platform-Specific Code

Use conditional compilation for platform differences:

```rust
#[cfg(unix)]
use tokio::net::UnixListener;

#[cfg(windows)]
use tokio::net::TcpListener;

#[cfg(unix)]
async fn run_unix(socket_path: String) -> Result<()> { ... }

#[cfg(windows)]
async fn run_tcp(addr: String) -> Result<()> { ... }
```

## Performance Guidelines

- **Use `&str` over `String`** for function parameters when possible
- **Use `lazy_static!`** for compiled regex patterns
- **Use connection pooling** for SQLite (via `r2d2` or `deadpool`)
- **Avoid cloning large structs** - prefer references
- **Use `Arc<Mutex<T>>`** for shared mutable state

## Debugging

```bash
# Run with debug logging
RUST_LOG=debug cargo run --bin opencode-rtk

# Run with trace logging
RUST_LOG=trace cargo run --bin opencode-rtk

# Run tests with output
cargo test -- --nocapture

# Check specific crate
cargo check -p rtk-core
```

## Windows-Specific Issues

### Git Bash and `nul` File Creation

When running commands in Git Bash on Windows, using `2>nul` for null redirection creates actual files named `nul` or `NUL` instead of redirecting to the Windows null device.

**Problem:**
```bash
# ❌ BAD in Git Bash - creates files named nul and NUL
dir ... 2>nul
```

**Solution:**
```bash
# ✅ GOOD in Git Bash - redirects to /dev/null
dir ... 2>/dev/null
```

**If `nul` or `NUL` files are created:**
```bash
# Delete them using rm (Windows del may fail due to reserved name)
rm -f ./nul ./NUL
```

**Why this happens:** Git Bash on Windows doesn't recognize Windows device names (`nul`, `NUL`, `CON`, etc.) as special, so it treats them as regular filenames. Windows `del` command fails because these are reserved device names, but `rm` in Git Bash works around this limitation.

## Pre-Commit Checklist

Before committing:

- [ ] `cargo fmt` - Format code
- [ ] `cargo clippy` - Fix all warnings
- [ ] `cargo test` - All tests pass
- [ ] `cargo build --release` - Release build succeeds
- [ ] Update documentation if API changed
- [ ] Add tests for new functionality

## Additional Resources

- **Architecture**: See `ARCHITECTURE.md` for system design
- **Roadmap**: See `PLAN.md` for development phases
- **Status**: See `PHASE1_SUMMARY.md` for current progress
- **Next Steps**: See `NEXT_STEPS.md` for immediate tasks
