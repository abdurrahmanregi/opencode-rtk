# OpenCode-RTK Agent Guide

This guide is for contributors and coding agents working in this repository.

## Repository Snapshot

Workspace crates:
- `crates/rtk-core`
- `crates/rtk-daemon`
- `crates/rtk-cli`

Plugin:
- `plugin/` (TypeScript OpenCode plugin)

Primary runtime pipeline:
- `tool.execute.before` -> default pass-through (optional rewrite mode -> daemon `optimize`)
- `tool.execute.after` -> default metadata-only compression via daemon `compress`
- `session.idle` -> daemon `stats`

## Source of Truth Files

Core (`rtk-core`):
- `crates/rtk-core/src/commands/mod.rs`
- `crates/rtk-core/src/commands/pre_execution.rs`
- `crates/rtk-core/src/config/mod.rs`
- `crates/rtk-core/src/tracking/`
- `crates/rtk-core/src/tee/`

Daemon (`rtk-daemon`):
- `crates/rtk-daemon/src/main.rs`
- `crates/rtk-daemon/src/server.rs`
- `crates/rtk-daemon/src/protocol.rs`
- `crates/rtk-daemon/src/handlers/`

Plugin:
- `plugin/src/index.ts`
- `plugin/src/client.ts`
- `plugin/src/address.ts`
- `plugin/src/spawn.ts`
- `plugin/src/hooks/tool-before.ts`
- `plugin/src/hooks/tool-after.ts`
- `plugin/src/hooks/session.ts`
- `plugin/src/state.ts`

## Build Commands

From repository root:

```bash
# Rust workspace
cargo build
cargo build --release

# Per crate
cargo build -p rtk-core
cargo build -p rtk-daemon
cargo build -p rtk-cli
```

Plugin:

```bash
cd plugin
npm install
npm run build
```

## Test and Lint Commands

Rust:

```bash
cargo test
cargo test -p rtk-core
cargo test -p rtk-daemon
cargo fmt -- --check
cargo clippy
```

Plugin:

```bash
cd plugin
npm run build
npm run lint
npm test
```

Note: plugin tests are `bun test` via npm script. If Bun is absent, `npm test` fails even when TypeScript is valid.

## Coding Conventions

Rust:
- use `anyhow::Result` with context at fallible boundaries
- keep handlers/modules focused by command responsibility
- keep startup/bind logs accurate (avoid pre-bind "listening" semantics)

TypeScript:
- strict typing, avoid `any`
- keep socket lifecycle explicit (`connect`, timeout, teardown)
- preserve request ordering via `RTKDaemonClient` queue semantics
- keep logs useful and avoid high-noise defaults

## Startup and Recovery Design (Current)

Plugin startup (`plugin/src/index.ts`):
- lock with `startPromise` for initial startup dedupe
- lock with `restartPromise` for runtime restart dedupe
- health cache policy via `shouldUseCachedHealth(...)`
  - shorter cache for healthy checks
  - faster re-probe window after failed checks
- endpoint override via `RTK_DAEMON_ADDR`
- binary resolution fallback chain via `RTK_DAEMON_PATH` -> local release binary -> PATH
- plugin behavior toggles:
  - `RTK_ENABLE_PRE_EXECUTION_FLAGS=1` enables pre-exec rewrite mode (default off)
  - `RTK_POST_EXECUTION_MODE=off|metadata_only|replace_output` (default `metadata_only`)

Spawn orchestration (`plugin/src/spawn.ts`):
- validate binary path (absolute paths validated; relative paths PATH-resolved)
- precheck local TCP port when applicable
- spawn detached daemon
- wait for health with bounded retries/backoff (`waitForDaemon`)
- kill and cleanup process on startup failure
- verbose startup diagnostics gated by `RTK_VERBOSE_STARTUP_LOGS=1`

Client transport (`plugin/src/client.ts` + `plugin/src/address.ts`):
- strict TCP parsing supports `host:port` and `[ipv6]:port`
- robust TCP-vs-Unix classification without naive `includes(":")`
- serialized JSON-RPC calls with queue recovery after errors
- NDJSON `id` correlation; malformed frames fail fast

Platform defaults:
- Windows: TCP `127.0.0.1:9876`
- Unix: `/tmp/opencode-rtk.sock`

## JSON-RPC Methods

Implemented in `crates/rtk-daemon/src/protocol.rs`:
- `compress`
- `health`
- `stats`
- `shutdown`
- `optimize`
- `tee_save`
- `tee_list`
- `tee_read`
- `tee_clear`

## Safe Change Checklist

When touching startup/client/server behavior:
1. run plugin build/lint/tests (`plugin`)
2. run targeted Rust tests for touched crate(s)
3. verify Windows/Unix transport branches remain coherent
4. keep `README.md` and `ARCHITECTURE.md` aligned with code
5. remove stale comments and keep diagnostics meaningful

## Common Pitfalls

- `cargo build --release` can fail on Windows if `opencode-rtk.exe` is still running
- OneDrive may temporarily lock rebuilt binaries
- initial `ECONNREFUSED` during cold startup can be expected before spawn succeeds
- do not assume Bun exists on all Windows setups

## Practical Debug Sequence (Windows)

```bash
netstat -ano | findstr :9876
taskkill /F /PID <pid>
cargo build --release
cd plugin && npm run build
```

Then fully restart OpenCode and inspect RTK startup logs.
