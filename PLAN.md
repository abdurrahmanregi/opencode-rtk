# OpenCode-RTK Implementation Plan

> Detailed roadmap and milestone tracking

## Project Overview

**Goal:** Build opencode-rtk, an all-Rust daemon providing 60-90% token savings for OpenCode.

**Timeline:** 4-6 weeks

**Approach:** 85-90% code reuse from [rtk](https://github.com/rtk-ai/rtk)

---

## Phase 1: Foundation (Week 1-2)

**Status:** 🚧 In Progress

### Week 1: Core Infrastructure

#### Day 1-2: Project Setup
- [x] Create project directory structure
- [x] Initialize Cargo workspace
- [x] Create README.md, ARCHITECTURE.md, PLAN.md
- [ ] Set up GitHub repository
- [ ] Configure CI/CD (GitHub Actions)
- [ ] Add .gitignore, LICENSE

#### Day 3-4: Port Core Library
- [ ] Create `rtk-core` crate structure
- [ ] Port command modules from rtk:
  - [ ] git.rs (status, diff, log)
  - [ ] npm_cmd.rs (test, install)
  - [ ] cargo_cmd.rs (test, build)
  - [ ] docker.rs (ps, logs)
  - [ ] pytest_cmd.rs
- [ ] Port filter strategies:
  - [ ] stats.rs (stats extraction)
  - [ ] error_only.rs
  - [ ] grouping.rs
- [ ] Port tracking system:
  - [ ] db.rs (SQLite)
  - [ ] schema.sql
- [ ] Port utilities:
  - [ ] command.rs
  - [ ] tokens.rs

#### Day 5: Basic Daemon
- [ ] Create `rtk-daemon` crate
- [ ] Implement Unix socket server
- [ ] JSON-RPC 2.0 protocol handler
- [ ] Basic `compress` endpoint
- [ ] Health check endpoint
- [ ] Graceful shutdown

### Week 2: Plugin Integration

#### Day 1-2: TypeScript Plugin
- [ ] Create plugin directory
- [ ] Initialize package.json
- [ ] Implement RTKDaemonClient:
  - [ ] Unix socket connection
  - [ ] JSON-RPC client
  - [ ] Auto-reconnect logic
  - [ ] Timeout handling

#### Day 3-4: Hook Integration
- [ ] Implement `tool.execute.before` hook
  - [ ] Detect bash commands
  - [ ] Store command context
- [ ] Implement `tool.execute.after` hook
  - [ ] Send to RTK daemon
  - [ ] Replace output
  - [ ] Error handling
- [ ] Implement session tracking
  - [ ] `session.idle` hook
  - [ ] Report savings

#### Day 5: Testing & Polish
- [ ] Test with real OpenCode instance
- [ ] Fix integration issues
- [ ] Add error handling
- [ ] Document setup process
- [ ] Create getting started guide

### Phase 1 Deliverables
- ✅ Working daemon (accepts connections, compresses output)
- ✅ TypeScript plugin (integrates with OpenCode)
- ✅ Basic token tracking (SQLite)
- ✅ 5-10 command modules working
- ✅ Documentation for setup

---

## Phase 2: Feature Parity (Week 3-4)

**Status:** ⏳ Pending

### Week 3: Command Modules

#### Day 1-2: Git Module (Complete)
- [ ] git status (stats extraction)
- [ ] git diff (stats + truncation)
- [ ] git log (stats extraction)
- [ ] git add (silent)
- [ ] git commit (silent)
- [ ] git push (progress filtering)
- [ ] git branch (list)
- [ ] git checkout (silent)
- [ ] Test with real git repositories

#### Day 3-4: JS/TS Tooling
- [ ] lint_cmd.rs (ESLint, grouping)
- [ ] tsc_cmd.rs (TypeScript, grouping)
- [ ] next_cmd.rs (Next.js)
- [ ] playwright_cmd.rs (E2E, failure focus)
- [ ] prisma_cmd.rs (Prisma)
- [ ] vitest_cmd.rs (Vitest, failure focus)
- [ ] pnpm_cmd.rs (pnpm, progress filtering)

#### Day 5: Python Tooling
- [ ] ruff_cmd.rs (Ruff, JSON/Text dual)
- [ ] pytest_cmd.rs (pytest, state machine)
- [ ] pip_cmd.rs (pip list/outdated)

### Week 4: Remaining Modules + Advanced Features

#### Day 1-2: Remaining Modules
- [ ] Go toolchain:
  - [ ] go_cmd.rs (test/build/vet, NDJSON)
  - [ ] golangci_cmd.rs (golangci-lint, grouping)
- [ ] Network tools:
  - [ ] wget_cmd.rs (progress filtering)
  - [ ] curl_cmd.rs (progress filtering)
- [ ] Infrastructure:
  - [ ] aws_cmd.rs (AWS CLI)
  - [ ] psql_cmd.rs (PostgreSQL)
- [ ] Code search:
  - [ ] grep_cmd.rs (grep, grouping)
  - [ ] diff_cmd.rs (diff)
  - [ ] find_cmd.rs (find, tree)
- [ ] File ops:
  - [ ] ls.rs (ls, tree compression)
  - [ ] read.rs (read, code filtering)
- [ ] Execution:
  - [ ] runner.rs (err, test)
  - [ ] summary.rs (smart heuristic)
  - [ ] local_llm.rs (LLM mode, optional)
- [ ] Logs/Data:
  - [ ] log_cmd.rs (log, dedup)
  - [ ] json_cmd.rs (json, structure only)
- [ ] Other:
  - [ ] deps.rs (dependency check)
  - [ ] env_cmd.rs (environment)
  - [ ] container.rs (podman/docker)

#### Day 3: Advanced Filtering
- [ ] Code filtering (strip comments/bodies)
  - [ ] Rust
  - [ ] Python
  - [ ] JavaScript/TypeScript
  - [ ] Go
  - [ ] C/C++
  - [ ] Java
- [ ] State machine parsing (pytest)
- [ ] NDJSON streaming (go test -json)
- [ ] LLM-powered compression (optional, defer if needed)

#### Day 4: Configuration System
- [ ] TOML config file (~/.config/opencode-rtk/config.toml)
- [ ] Configuration structure:
  - [ ] General settings (tracking, retention)
  - [ ] Daemon settings (socket, timeout)
  - [ ] Tool-specific settings
  - [ ] Command-specific overrides
- [ ] Config loading and validation
- [ ] Default configuration

#### Day 5: Testing & Documentation
- [ ] Comprehensive test suite
  - [ ] Unit tests for each module
  - [ ] Integration tests with real commands
  - [ ] Edge case tests
- [ ] Module development guide
- [ ] User documentation
- [ ] API reference

### Phase 2 Deliverables
- ✅ All 30+ command modules ported
- ✅ All 12 filtering strategies implemented
- ✅ Full SQLite tracking with 90-day retention
- ✅ Configuration system
- ✅ Comprehensive test suite
- ✅ Complete documentation

---

## Phase 3: Polish & Optimization (Week 5-6)

**Status:** ⏳ Pending

### Week 5: Performance & Reliability

#### Day 1-2: Performance Optimization
- [ ] Benchmark daemon latency (target: <5ms)
- [ ] Optimize regex patterns
  - [ ] Use lazy_static for compiled regex
  - [ ] Benchmark regex performance
  - [ ] Optimize hot paths
- [ ] SQLite optimization
  - [ ] Connection pooling
  - [ ] Prepared statements
  - [ ] Batch inserts
- [ ] Memory profiling
  - [ ] Target: <10MB steady state
  - [ ] Fix memory leaks
  - [ ] Use arena allocators if needed

#### Day 3: Error Handling
- [ ] Graceful degradation
  - [ ] Fallback to original output on error
  - [ ] Retry logic in plugin
  - [ ] Timeout handling (5s default)
- [ ] Auto-restart daemon on crash
  - [ ] Watchdog process
  - [ ] State recovery
- [ ] Comprehensive error types
- [ ] Error logging and reporting

#### Day 4: Advanced Features
- [ ] Verbosity levels (-v, -vv, -vvv)
- [ ] Progress indicators
- [ ] Colored output (ANSI)
- [ ] Tee mode (preserve original output)
- [ ] Debug mode (show raw + compressed)

#### Day 5: Security Hardening
- [ ] Input validation
- [ ] Path sanitization
- [ ] Rate limiting
- [ ] Resource limits (memory, CPU)
- [ ] Unix socket permissions

### Week 6: Production Readiness

#### Day 1-2: Installation & Distribution
- [ ] Single-binary build (static linking)
- [ ] Cross-platform builds:
  - [ ] Linux (x86_64, aarch64)
  - [ ] macOS (x86_64, aarch64)
  - [ ] Windows (x86_64)
- [ ] Installation script (curl | sh)
- [ ] Auto-update mechanism (optional)
- [ ] Homebrew formula (macOS)
- [ ] Cargo install support

#### Day 3: Documentation & Examples
- [ ] Complete API reference
- [ ] Getting started guide
- [ ] Module development tutorial
- [ ] Plugin configuration examples
- [ ] Troubleshooting guide
- [ ] FAQ

#### Day 4: Testing & QA
- [ ] End-to-end testing
  - [ ] Real OpenCode sessions
  - [ ] Multiple project types
- [ ] Load testing
  - [ ] 1000+ commands
  - [ ] Concurrent sessions
- [ ] Edge case testing
- [ ] Memory leak testing (valgrind/ASAN)
- [ ] Performance regression tests

#### Day 5: Release Preparation
- [ ] Version 1.0.0 release
- [ ] GitHub release notes
- [ ] Announcement blog post
- [ ] Demo video
- [ ] Social media announcement

### Phase 3 Deliverables
- ✅ Optimized daemon (<5ms latency, <10MB memory)
- ✅ Robust error handling and recovery
- ✅ Cross-platform binaries
- ✅ Complete documentation
- ✅ Production-ready release

---

## Success Metrics

### Technical Metrics

| Metric              | Target      | Current | Status |
|---------------------|-------------|---------|--------|
| Daemon latency      | < 5ms       | -       | ⏳     |
| Startup time        | < 100ms     | -       | ⏳     |
| Memory usage        | < 10MB      | -       | ⏳     |
| Binary size         | < 8MB       | -       | ⏳     |
| Throughput          | > 1000/s    | -       | ⏳     |
| Compression ratio   | 60-90%      | -       | ⏳     |
| Test coverage       | > 80%       | -       | ⏳     |

### Feature Metrics

| Feature                | Target | Current | Status |
|------------------------|--------|---------|--------|
| Command modules        | 30+    | 0       | ⏳     |
| Filtering strategies   | 12     | 0       | ⏳     |
| Supported platforms    | 3      | 0       | ⏳     |
| Documentation pages    | 10+    | 3       | ⚠️     |

---

## Risk Register

### High Priority

| Risk                         | Probability | Impact | Mitigation                          | Status |
|------------------------------|-------------|--------|-------------------------------------|--------|
| Rust learning curve          | High        | Medium | Start simple, iterate               | ⚠️     |
| IPC latency too high         | Low         | High   | Benchmark early, optimize           | ⏳     |
| Memory leaks in daemon       | Medium      | Medium | Regular profiling, arena allocators | ⏳     |

### Medium Priority

| Risk                         | Probability | Impact | Mitigation                          | Status |
|------------------------------|-------------|--------|-------------------------------------|--------|
| Scope creep                  | Medium      | Medium | Stick to rtk parity                 | ⏳     |
| Cross-platform issues        | Medium      | Medium | Test early on all platforms         | ⏳     |
| SQLite lock contention       | Low         | Medium | Connection pooling, busy_timeout    | ⏳     |

### Low Priority

| Risk                         | Probability | Impact | Mitigation                          | Status |
|------------------------------|-------------|--------|-------------------------------------|--------|
| rtk repository changes       | Low         | Medium | Fork repo, pin version              | ⏳     |
| OpenCode API changes         | Low         | High   | Pin version, monitor releases       | ⏳     |

---

## Dependencies

### Rust Crates

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
regex = "1"
anyhow = "1"
thiserror = "1"
lazy_static = "1"
toml = "0.8"
chrono = { version = "0.4", features = ["serde"] }
```

### TypeScript Dependencies

```json
{
  "dependencies": {
    "@opencode-ai/plugin": "latest"
  },
  "devDependencies": {
    "typescript": "^5.0",
    "bun-types": "latest"
  }
}
```

---

## Notes

### Lessons Learned

(Will be updated as project progresses)

### Decisions Log

| Date       | Decision                                | Rationale                          |
|------------|-----------------------------------------|------------------------------------|
| 2026-03-09 | All-Rust over Go hybrid                 | Process isolation, code reuse      |
| 2026-03-09 | Unix socket over TCP                    | Lower latency, better security     |
| 2026-03-09 | JSON-RPC 2.0 protocol                   | Standard, simple, debuggable       |
| 2026-03-09 | SQLite for tracking                     | Zero-config, reliable, fast        |

### Questions / Open Items

- [ ] Should we support Windows named pipes?
- [ ] Should we implement LLM-powered compression in Phase 2?
- [ ] Should we add a web dashboard for stats?
- [ ] Should we support custom user modules via config?

---

## Changelog

### 2026-03-09
- Created project structure
- Created README.md, ARCHITECTURE.md, PLAN.md
- Initialized Cargo workspace
- Started Phase 1 implementation
