# OpenCode-RTK

> High-performance token optimization proxy for OpenCode CLI

**60-90% token savings** on common development commands through intelligent output filtering and compression.

## What Does This Do?

When you run commands in OpenCode CLI (like `git status`, `npm test`, `cargo build`), the output can be very long and consume lots of tokens. This tool:

1. **Intercepts** command output before it reaches the LLM
2. **Compresses** the output intelligently (keeps errors, removes noise)
3. **Saves** 60-90% of tokens on most commands

**Example:**
```
Before (1250 tokens):
$ git log --oneline -10
abc123def Fix authentication bug in OAuth flow
def456abc Add user profile validation
... (8 more lines)

After (125 tokens, 90% savings):
$ git log --oneline -10
10 commits, +523/-312
```

## Features

- 🚀 **Rust daemon** - Fast, no garbage collection pauses
- 🔌 **OpenCode plugin** - Automatic integration
- 📊 **SQLite tracking** - See how many tokens you've saved
- 🛡️ **Process isolation** - RTK crash doesn't affect OpenCode
- ⚡ **Low latency** - Unix socket (or TCP on Windows)
- 📦 **26 command modules** - Git, npm, cargo, pytest, go, aws, and more
- 🧪 **362 tests** - Well-tested and reliable

## Installation

### Option 1: Build from Source

```bash
# Clone the repository
git clone https://github.com/yourname/opencode-rtk
cd opencode-rtk

# Build (requires Rust installed)
cargo build --release

# The binary will be at:
# target/release/opencode-rtk (Unix)
# target/release/opencode-rtk.exe (Windows)
```

### Option 2: Install Rust First (if you don't have it)

```bash
# Windows: Download from https://rustup.rs
# Or run in PowerShell:
winget install Rustlang.Rustup

# Unix/macOS:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Quick Start

### Step 1: Build the Daemon and Plugin

```bash
# Build Rust daemon
cargo build --release

# Build TypeScript plugin
cd plugin
bun install
bun run build
```

### Step 2: Configure OpenCode

Add to your project's `opencode.json`:

```json
{
  "plugin": ["C:/Users/abdur/OneDrive/Work/opencode-rtk/plugin/src/index.ts"]
}
```

### Step 3: Use OpenCode Normally

The plugin will **automatically start the daemon** and compress command output!

**Note:** The daemon auto-starts when OpenCode loads. No manual startup needed.

## Supported Commands

| Category | Commands | Typical Savings |
|----------|----------|-----------------|
| **Git** | status, diff, log, add, commit, push, checkout | 85-99% |
| **npm/pnpm** | test, install, list, run | 70-95% |
| **cargo** | test, build, clippy | 75-90% |
| **pytest** | test runs | 90%+ |
| **go** | test, build, vet | 75-90% |
| **ESLint/TSC** | lint, compile | 80-85% |
| **AWS CLI** | various commands | 80% |
| **Docker** | ps, logs, images | 60-80% |

...and more! See [PHASE2_SUMMARY.md](./PHASE2_SUMMARY.md) for the full list.

## How It Works

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   OpenCode  │────▶│   Plugin    │────▶│ RTK Daemon  │
│   (Go CLI)  │     │ (TypeScript)│     │   (Rust)    │
└─────────────┘     └─────────────┘     └─────────────┘
                        │                      │
                        │ Auto-start           │ Unix/TCP
                        └──────▶──────────────▶
                              Spawned process
                                                │
                                                ▼
                                         ┌─────────────┐
                                         │ Compressed  │
                                         │   Output    │
                                         └─────────────┘
```

1. OpenCode loads plugin → Plugin auto-starts daemon if not running
2. You run a command in OpenCode
3. Plugin captures the output
4. Sends to RTK daemon via socket
5. Daemon compresses based on command type
6. Compressed output goes to LLM

## Project Structure

```
opencode-rtk/
├── crates/
│   ├── rtk-core/      # Core library (26 command modules)
│   ├── rtk-daemon/    # Socket daemon server
│   └── rtk-cli/       # Optional CLI tool
├── plugin/            # TypeScript plugin for OpenCode
├── PHASE1_SUMMARY.md  # Phase 1 completion details
├── PHASE2_SUMMARY.md  # Phase 2 completion details
├── CODE_REVIEW_SUMMARY.md  # Code audit results
├── ARCHITECTURE.md    # System design
└── AGENTS.md          # Build/test commands
```

## Development

### Common Commands

```bash
# Build everything
cargo build

# Build optimized release
cargo build --release

# Run all tests
cargo test

# Run tests for one crate
cargo test -p rtk-core

# Check for code issues
cargo clippy

# Format code
cargo fmt
```

### Running Tests

```bash
# All tests
cargo test

# With output visible
cargo test -- --nocapture

# Specific test
cargo test test_git_status
```

### Viewing Stats

```bash
# Using CLI
cargo run --bin rtk-cli -- stats

# Or check the database directly
sqlite3 ~/.local/share/opencode-rtk/history.db "SELECT * FROM commands LIMIT 10"
```

## Configuration

Config file location: `~/.config/opencode-rtk/config.toml`

```toml
[general]
enable_tracking = true    # Save token savings to SQLite

[daemon]
socket_path = "/tmp/opencode-rtk.sock"  # Unix
# tcp_address = "127.0.0.1:9876"        # Windows (optional)
timeout_seconds = 5
max_connections = 100

[tracking]
retention_days = 90       # Delete old records after 90 days
```

## Status

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 1 | Core infrastructure | ✅ Complete |
| Phase 2 | Command modules | ✅ Complete |
| Code Review | Bug fixes | ✅ Complete |
| Phase 3 | Polish & testing | 🔜 Next |

## Documentation

- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - How the system is designed
- **[AGENTS.md](./AGENTS.md)** - Build commands and code style guide
- **[PHASE2_SUMMARY.md](./PHASE2_SUMMARY.md)** - Module details
- **[CODE_REVIEW_SUMMARY.md](./CODE_REVIEW_SUMMARY.md)** - What was audited and fixed

## Troubleshooting

### "Plugin not compressing"

1. Check plugin path in `opencode.json` is correct
2. Rebuild plugin: `cd plugin && bun run build`
3. Check for errors in console - daemon auto-starts on plugin load
4. On Windows, verify port 9876 is not in use: `netstat -an | findstr 9876`

### "Daemon failed to start"

1. Verify `opencode-rtk` binary is in PATH or use full path
2. Check logs for specific error messages
3. On Windows: Run daemon manually to see startup errors
4. On Unix: Check permissions on `/tmp/opencode-rtk.sock`

### "Build fails"

Make sure you have Rust installed:
```bash
rustc --version
cargo --version
```

### "nul or NUL files created in project"

If you see `nul` or `NUL` files appearing in your project directory, this is a Git Bash + Windows issue.

**Cause:** Running `command 2>nul` in Git Bash creates files instead of redirecting to Windows null device.

**Solution:**
```bash
# Delete the files (Windows del may fail)
rm -f ./nul ./NUL

# Use /dev/null instead of nul in bash
# ❌ Bad: command 2>nul
# ✅ Good: command 2>/dev/null
```

## License
```bash
rustc --version
cargo --version
```

## License

MIT

## Acknowledgments

- [rtk](https://github.com/rtk-ai/rtk) - Reference implementation
- [OpenCode](https://opencode.ai) - CLI tool this integrates with
