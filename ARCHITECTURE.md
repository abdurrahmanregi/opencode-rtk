# OpenCode-RTK Architecture

Technical architecture for the current implementation.

## 1) System Overview

OpenCode-RTK is a sidecar optimization layer for OpenCode CLI.

- OpenCode loads a TypeScript plugin.
- Plugin communicates with a Rust daemon over JSON-RPC 2.0 (NDJSON framing).
- Daemon executes optimization/compression logic from `rtk-core`.
- Token savings and history are tracked in SQLite.

Primary goal: reduce token volume while preserving command usability and fail-open behavior.

## 2) Components

### `crates/rtk-core`

Responsibilities:
- command detection and module routing
- pre-execution flag optimization
- post-execution compression strategies
- configuration model
- token tracking and tee storage

Primary files:
- `crates/rtk-core/src/commands/mod.rs`
- `crates/rtk-core/src/commands/pre_execution.rs`
- `crates/rtk-core/src/config/mod.rs`
- `crates/rtk-core/src/tracking/`
- `crates/rtk-core/src/tee/`

### `crates/rtk-daemon`

Responsibilities:
- platform transport server (Unix socket on Unix, TCP on Windows)
- JSON-RPC parsing and dispatch
- handler execution for optimize/compress/stats/health/shutdown/tee

Primary files:
- `crates/rtk-daemon/src/main.rs`
- `crates/rtk-daemon/src/server.rs`
- `crates/rtk-daemon/src/protocol.rs`
- `crates/rtk-daemon/src/handlers/`

### `crates/rtk-cli`

Responsibilities:
- CLI surface for direct daemon operations and local debugging.

### `plugin`

Responsibilities:
- OpenCode hook integration
- daemon startup/restart orchestration
- command rewrite before execution
- output compression and fallback handling after execution

Primary files:
- `plugin/src/index.ts`
- `plugin/src/client.ts`
- `plugin/src/address.ts`
- `plugin/src/spawn.ts`
- `plugin/src/hooks/tool-before.ts`
- `plugin/src/hooks/tool-after.ts`
- `plugin/src/hooks/session.ts`
- `plugin/src/state.ts`

## 3) Runtime Data Flow

### A) Plugin bootstrap

1. `RTKDaemonClient` is initialized with `RTK_DAEMON_ADDR` or platform default.
2. `isDaemonRunning(...)` probes daemon health.
3. If unhealthy, `autoStartDaemon(...)` runs under startup lock (`startPromise`).

### B) `tool.execute.before`

1. Plugin detects bash tool command.
2. Plugin requests daemon `optimize`.
3. If optimization applies, command args are rewritten.
4. Call context is saved for post-execution handling.

### C) `tool.execute.after`

1. Plugin retrieves pending context.
2. Plugin enforces output guardrails.
3. `ensureDaemonRunning(...)` checks health with throttled caching and restart path.
4. Plugin requests daemon `compress`.
5. If `saved_tokens > 0`, output is replaced with compressed result.
6. On compression failure, plugin attempts `tee_save` while preserving original output.

### D) `session.idle`

1. Plugin requests daemon `stats`.
2. Session savings are reported when available.

## 4) Startup and Recovery Model

Orchestration is split across `plugin/src/index.ts` and `plugin/src/spawn.ts`.

Key controls:
- `startPromise`: dedupe initial startup attempts
- `restartPromise`: dedupe runtime restart attempts
- cached health policy in `shouldUseCachedHealth(...)`
  - shorter cache on recent healthy checks
  - faster re-probe after recent failures
- bounded restart attempts via `MAX_RESTARTS_PER_SESSION`

`autoStartDaemon(...)` path:
- validate daemon binary path policy
- optional TCP port precheck for local bind hosts
- spawn detached child
- short startup delay
- `waitForDaemon(...)` retry/backoff loop
- cleanup/terminate child on startup failure

Verbose startup diagnostics are gated by `RTK_VERBOSE_STARTUP_LOGS=1`.

## 5) Transport and Addressing

### Defaults

- Windows client default: `127.0.0.1:9876`
- Unix client default: `/tmp/opencode-rtk.sock`

### Address parsing and classification

`plugin/src/address.ts` is the transport parsing source of truth:
- supports `host:port` and `[ipv6]:port`
- validates port bounds (1..65535)
- extracts TCP port for precheck logic
- avoids broad `includes(":")` transport detection

### Client request lifecycle

`plugin/src/client.ts` provides:
- serialized request queue with failure recovery
- reconnect attempts and connection timeout
- NDJSON frame processing with JSON-RPC `id` correlation
- fail-fast behavior for malformed newline-terminated response frames
- runtime socket error handling and cleanup

## 6) Protocol

Transport protocol: JSON-RPC 2.0 over newline-delimited JSON.

Router: `crates/rtk-daemon/src/protocol.rs`

Methods:
- `compress`
- `health`
- `stats`
- `shutdown`
- `optimize`
- `tee_save`
- `tee_list`
- `tee_read`
- `tee_clear`

## 7) Command Modules and Strategies

Command registry:
- `crates/rtk-core/src/commands/mod.rs`

Current module families:
- git/npm/cargo/docker/pytest
- eslint/tsc/next/playwright/prisma/vitest
- pnpm/pip/ruff/go/golangci-lint
- wget/curl/aws/psql
- grep/diff/find/ls/read

Pre-execution mappings:
- `crates/rtk-core/src/commands/pre_execution.rs`

Execution model:
- `optimize` before command execution
- `compress` after command execution

## 8) Tracking and Tee

Tracking:
- SQLite-backed token metrics and session aggregates
- surfaced via `stats`

Tee:
- plugin uses `tee_save` when compression fails
- daemon exposes tee list/read/clear operations
- original output availability is preserved for debugging

## 9) Failure Semantics

Design principle: fail open.

- daemon unavailable -> command output still returns
- compression error -> original output preserved
- tee failure -> no hard failure of user-facing tool result

This keeps OpenCode functional even when RTK is degraded.

## 10) Operational Notes

### Build/reload loop

```bash
cargo build --release
cd plugin && npm run build
```

Restart OpenCode after plugin/daemon rebuild.

### Windows lock behavior

`cargo build --release` may fail with access denied when:
- `opencode-rtk.exe` is still running, or
- OneDrive temporarily locks rebuilt binaries.

### Expected cold-start logs

Normal sequence may include:
- initial `ECONNREFUSED`
- daemon spawn
- `waitForDaemon` retries
- health response `status: "ok"`

## 11) Future Improvements

High-value next candidates:
- stricter semantic JSON-RPC response validation in plugin client
- startup telemetry with explicit reason codes
- deeper integration tests for restart/throttle edge cases
- packaging/deployment docs for non-dev installations
