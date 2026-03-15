# OpenCode-RTK

OpenCode-RTK is a token-optimization sidecar for OpenCode CLI.

It combines:
- pre-execution command optimization (`optimize`), and
- post-execution output compression (`compress`),

while preserving fail-open behavior (original output remains usable if RTK is down).

## Repository Contents

- `crates/rtk-core`: command detection, optimization/compression strategies, config, tracking, tee
- `crates/rtk-daemon`: JSON-RPC daemon transport and handlers
- `crates/rtk-cli`: direct CLI entrypoint for daemon functionality
- `plugin`: TypeScript OpenCode plugin (`tool.execute.before`, `tool.execute.after`, `session.idle`)

## Runtime Flow

1. `tool.execute.before`
   - plugin calls daemon `optimize`
   - plugin rewrites command if flags are added
2. command executes
3. `tool.execute.after`
   - plugin ensures daemon health (with startup/restart path)
   - plugin calls daemon `compress`
   - plugin replaces output only when savings are positive
   - on compression failure, plugin can `tee_save` raw output
4. `session.idle`
   - plugin calls `stats` and prints session-level savings

## Startup and Reliability

Current plugin startup behavior (`plugin/src/index.ts`, `plugin/src/spawn.ts`, `plugin/src/client.ts`):

- lock-based startup/restart coordination via `startPromise` and `restartPromise`
- throttled health cache with faster recheck after recent failures
- robust daemon binary resolution:
  - `RTK_DAEMON_PATH` override
  - local `target/release` binary if present
  - fallback to PATH (`opencode-rtk` / `opencode-rtk.exe`)
- strict TCP address parsing (IPv4, hostname, bracketed IPv6) via `plugin/src/address.ts`
- TCP/Unix transport classification without broad `includes(":")` heuristics
- NDJSON response correlation and queue-safe request serialization in client
- fast-fail on malformed newline-delimited JSON-RPC response frames

Platform defaults:
- Windows: TCP `127.0.0.1:9876`
- Unix: Unix socket `/tmp/opencode-rtk.sock`

Environment variables:
- `RTK_DAEMON_ADDR`: override daemon endpoint (socket path or TCP address)
- `RTK_DAEMON_PATH`: override daemon binary path
- `RTK_VERBOSE_STARTUP_LOGS=1`: enable verbose startup diagnostics in plugin spawn flow

## JSON-RPC Methods

Defined in `crates/rtk-daemon/src/protocol.rs`:
- `compress`
- `health`
- `stats`
- `shutdown`
- `optimize`
- `tee_save`
- `tee_list`
- `tee_read`
- `tee_clear`

## Supported Command Modules

Core command modules are registered in `crates/rtk-core/src/commands/mod.rs`.

Current module families include:
- VCS and build tooling: `git`, `cargo`, `docker`, `go`, `golangci-lint`
- JS/TS ecosystem: `npm`, `pnpm`, `eslint`, `tsc`, `next`, `playwright`, `prisma`, `vitest`
- Python and test tooling: `pip`, `pytest`, `ruff`
- network/data tools: `curl`, `wget`, `aws`, `psql`
- file/search tools: `grep`, `diff`, `find`, `ls`, `read`

Pre-execution flag mappings live in `crates/rtk-core/src/commands/pre_execution.rs`.

## Build, Test, Lint

From repo root:

```bash
# Rust workspace
cargo build
cargo test
cargo fmt -- --check
cargo clippy

# Plugin
cd plugin
npm install
npm run build
npm run lint
npm test
```

Note: plugin tests run through Bun (`npm test` -> `bun test`).

## Configuration

Primary config file:
- `~/.config/opencode-rtk/config.toml`

Key sections:
- `[general]`: tracking and pre-execution toggles
- `[daemon]`: socket path / TCP address / runtime limits
- `[tee]`: tee save behavior and retention
- `[llm]`: optional LLM fallback configuration (feature-dependent)

Config source: `crates/rtk-core/src/config/mod.rs`.

## Troubleshooting

### Windows: startup or build lock issues

If startup fails repeatedly or `cargo build --release` returns access denied:

```bash
netstat -ano | findstr :9876
taskkill /F /PID <pid>
cargo build --release
cd plugin && npm run build
```

Then restart OpenCode.

### Healthy startup log pattern

A normal cold start may show:
- initial `ECONNREFUSED` while daemon is not running
- daemon spawn + short delay
- `waitForDaemon` retries
- eventual health success (`status: "ok"`)

## Documentation

- `ARCHITECTURE.md`: full component and runtime architecture
- `AGENTS.md`: contributor/agent operating guide
- `CHANGELOG.md`: release history

## License

MIT
