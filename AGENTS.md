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

## Common Patterns

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
