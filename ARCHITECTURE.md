# OpenCode-RTK Architecture

> System design and technical implementation details

## Table of Contents

1. [System Overview](#system-overview)
2. [Component Architecture](#component-architecture)
3. [Data Flow](#data-flow)
4. [Module System](#module-system)
5. [Filtering Strategies](#filtering-strategies)
6. [Token Tracking](#token-tracking)
7. [Plugin Integration](#plugin-integration)
8. [Protocol Specification](#protocol-specification)

---

## System Overview

### Proxy Pattern Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    OpenCode-RTK Architecture                             │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│  OpenCode Process (Go)                                                  │
│  ┌───────────────────────────────────────────────────────────────────┐ │
│  │  TypeScript Plugin Layer                                           │ │
│  │  ┌──────────────────────────────────────────────────────────────┐ │ │
│  │  │  Hooks:                                                       │ │ │
│  │  │  • tool.execute.before → detect bash commands                │ │ │
│  │  │  • tool.execute.after → send to RTK daemon                   │ │ │
│  │  │  • session.idle → flush tracking, report savings             │ │ │
│  │  └──────────────────────────────────────────────────────────────┘ │ │
│  └───────────────────────────────────────────────────────────────────┘ │
│                                    │                                    │
│                                    │ Unix Socket                        │
│                                    │ /tmp/opencode-rtk.sock             │
└────────────────────────────────────┼────────────────────────────────────┘
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  opencode-rtk Daemon (Rust)                                             │
│  ┌───────────────────────────────────────────────────────────────────┐ │
│  │  Socket Server (tokio)                                             │ │
│  │  • Accept connections                                              │ │
│  │  • Parse JSON-RPC 2.0                                              │ │
│  │  • Route to handlers                                               │ │
│  └───────────────────────────────────────────────────────────────────┘ │
│                                    │                                    │
│                                    ▼                                    │
│  ┌───────────────────────────────────────────────────────────────────┐ │
│  │  Core Engine (rtk-core)                                            │ │
│  │  ┌─────────────────────┐  ┌──────────────────────────────────┐   │ │
│  │  │  Command Router     │  │  Module Registry (30+ modules)   │   │ │
│  │  │  • detect_command() │→ │  • git.rs, npm_cmd.rs, ...       │   │ │
│  │  └─────────────────────┘  └──────────────────────────────────┘   │ │
│  │                                    │                              │ │
│  │                                    ▼                              │ │
│  │  ┌──────────────────────────────────────────────────────────┐    │ │
│  │  │  Filter Strategies (12 strategies)                        │    │ │
│  │  │  Stats, ErrorOnly, Grouping, Dedup, Code, ...            │    │ │
│  │  └──────────────────────────────────────────────────────────┘    │ │
│  └───────────────────────────────────────────────────────────────────┘ │
│                                    │                                    │
│                                    ▼                                    │
│  ┌───────────────────────────────────────────────────────────────────┐ │
│  │  Token Tracker (SQLite)                                            │ │
│  │  • ~/.local/share/opencode-rtk/history.db                          │ │
│  └───────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Process Isolation** - RTK runs as separate process, crash doesn't affect OpenCode
2. **Zero-Cost Abstractions** - Rust ownership model, no GC pauses
3. **Async I/O** - Tokio runtime for high concurrency
4. **Fail-Safe** - If filtering fails, return original output
5. **Minimal Overhead** - Target <5ms latency per request

---

## Component Architecture

### 1. rtk-core (Library Crate)

Core library containing all compression logic.

**Structure:**
```
crates/rtk-core/
├── src/
│   ├── lib.rs                    # Public API
│   ├── commands/                 # Command modules
│   │   ├── mod.rs                # Module registry
│   │   ├── git.rs                # Git commands
│   │   ├── npm_cmd.rs            # npm/yarn/pnpm
│   │   └── ... (30+ modules)
│   ├── filter/                   # Filtering strategies
│   │   ├── mod.rs
│   │   ├── stats.rs
│   │   └── ... (12 strategies)
│   ├── tracking/                 # Token tracking
│   │   ├── mod.rs
│   │   └── db.rs
│   ├── utils/                    # Utilities
│   │   ├── command.rs            # Command execution
│   │   └── tokens.rs             # Token estimation
│   └── config/                   # Configuration
│       └── settings.rs
```

**Public API:**
```rust
pub fn compress(command: &str, output: &str, context: Context) -> Result<CompressedOutput>;
pub fn decompress(compressed_id: &str) -> Result<String>;
pub fn detect_command(command: &str) -> Option<CommandType>;
pub fn estimate_tokens(text: &str) -> usize;
```

### 2. rtk-daemon (Binary Crate)

Unix socket daemon that listens for requests.

**Structure:**
```
crates/rtk-daemon/
├── src/
│   ├── main.rs                   # Entry point
│   ├── server.rs                 # Unix socket server
│   ├── protocol.rs               # JSON-RPC 2.0
│   ├── handlers/                 # Request handlers
│   │   ├── compress.rs
│   │   ├── health.rs
│   │   └── stats.rs
│   └── lifecycle.rs              # Graceful shutdown
```

**Startup Flow:**
1. Load configuration from `~/.config/opencode-rtk/config.toml`
2. Initialize SQLite database
3. Pre-warm command modules
4. Bind to Unix socket
5. Accept connections (async)

### 3. rtk-cli (Binary Crate)

Optional standalone CLI for debugging.

**Usage:**
```bash
# Compress via stdin
echo "git log output..." | opencode-rtk-cli compress "git log"

# Health check
opencode-rtk-cli health

# View stats
opencode-rtk-cli stats
```

### 4. TypeScript Plugin

OpenCode plugin that integrates with daemon.

**Structure:**
```
plugin/
├── src/
│   ├── index.ts                  # Plugin entry
│   ├── client.ts                 # RTKDaemonClient
│   ├── hooks/
│   │   ├── tool-before.ts
│   │   ├── tool-after.ts
│   │   └── session.ts
│   └── types.ts
```

---

## Data Flow

### Request Lifecycle

```
1. OpenCode calls bash tool
   └─ tool.execute.before hook fires

2. Plugin detects command
   └─ Check if command is supported (git, npm, etc.)
   └─ Store command context

3. Bash executes
   └─ Raw output captured

4. tool.execute.after hook fires
   └─ Plugin sends to RTK daemon via Unix socket
   └─ JSON-RPC request: { method: "compress", params: {...} }

5. RTK daemon processes
   └─ Parse JSON-RPC request
   └─ Route to compress handler
   └─ Detect command type
   └─ Select filtering strategy
   └─ Apply compression
   └─ Track token savings in SQLite
   └─ Return compressed output

6. Plugin receives response
   └─ Replace output with compressed version
   └─ LLM sees compressed output

Total latency: <5ms
```

### Error Handling

```
If RTK daemon is down:
  └─ Plugin catches connection error
  └─ Falls back to original output
  └─ Logs warning
  └─ OpenCode continues normally

If compression fails:
  └─ RTK returns error
  └─ Plugin falls back to original output
  └─ Logs error for debugging

If output is too large (>1MB):
  └─ Plugin skips compression
  └─ Returns original output
  └─ Logs warning
```

---

## Module System

### Command Module Interface

```rust
pub trait CommandModule: Send + Sync {
    /// Command patterns this module handles
    fn patterns(&self) -> &[Regex];
    
    /// Compress output for this command
    fn compress(&self, output: &str, context: &Context) -> Result<String>;
    
    /// Module name for tracking
    fn name(&self) -> &str;
    
    /// Default strategy for this module
    fn default_strategy(&self) -> Strategy;
}
```

### Module Registry

```rust
lazy_static::lazy_static! {
    static ref MODULES: Vec<Box<dyn CommandModule>> = vec![
        Box::new(GitModule::new()),
        Box::new(NpmModule::new()),
        Box::new(CargoModule::new()),
        // ... 30+ modules
    ];
}

pub fn detect_command(command: &str) -> Option<&'static dyn CommandModule> {
    MODULES.iter()
        .find(|m| m.patterns().iter().any(|p| p.is_match(command)))
        .map(|m| m.as_ref())
}
```

### Adding New Module

```rust
// crates/rtk-core/src/commands/mycmd.rs
use crate::{CommandModule, Context, Result, Strategy};
use regex::Regex;

pub struct MyCmdModule {
    patterns: Vec<Regex>,
}

impl MyCmdModule {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                Regex::new(r"^mycmd\s+").unwrap(),
            ],
        }
    }
}

impl CommandModule for MyCmdModule {
    fn patterns(&self) -> &[Regex] {
        &self.patterns
    }
    
    fn compress(&self, output: &str, _context: &Context) -> Result<String> {
        // Apply compression logic
        Ok(compress_mytool_output(output))
    }
    
    fn name(&self) -> &str {
        "mycmd"
    }
    
    fn default_strategy(&self) -> Strategy {
        Strategy::StatsExtraction
    }
}
```

---

## Filtering Strategies

### Strategy Interface

```rust
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    fn compress(&self, input: &str) -> Result<String>;
}
```

### Strategy Catalog

| # | Strategy              | Technique                          | Reduction | Use Cases                    |
|---|-----------------------|------------------------------------|-----------|------------------------------|
| 1 | Stats Extraction      | Count/aggregate, drop details      | 90-99%    | git status, git log          |
| 2 | Error Only            | stderr only, drop stdout           | 60-80%    | test failures, build errors  |
| 3 | Grouping by Pattern   | Group by rule, count/summarize     | 80-90%    | lint, tsc, grep              |
| 4 | Deduplication         | Unique + count occurrences         | 70-85%    | logs, repeated errors        |
| 5 | Structure Only        | Keys + types, strip values         | 80-95%    | JSON responses               |
| 6 | Code Filtering        | Strip comments/bodies by level     | 0-90%     | read, smart                  |
| 7 | Failure Focus         | Failures only, hide passing        | 94-99%    | vitest, playwright           |
| 8 | Tree Compression      | Directory hierarchy with counts    | 50-70%    | ls, find                     |
| 9 | Progress Filtering    | Strip ANSI progress bars           | 85-95%    | wget, pnpm install           |
| 10| JSON/Text Dual Mode   | JSON when available, text fallback | 80%+      | ruff, pip                    |
| 11| State Machine Parsing | Track state, extract outcomes      | 90%+      | pytest                       |
| 12| NDJSON Streaming      | Line-by-line JSON parse            | 90%+      | go test -json                |

### Example: Stats Extraction

```rust
// Input (1250 tokens):
// M src/auth.rs
// M src/user.rs
// A src/new_feature.rs
// D src/deprecated.rs
// ...
// (50 more lines)

// Output (125 tokens):
// 53 files changed: +1,247 insertions, -892 deletions

impl Strategy for StatsExtraction {
    fn name(&self) -> &str { "stats_extraction" }
    
    fn compress(&self, input: &str) -> Result<String> {
        let modified = input.lines().filter(|l| l.starts_with('M')).count();
        let added = input.lines().filter(|l| l.starts_with('A')).count();
        let deleted = input.lines().filter(|l| l.starts_with('D')).count();
        
        Ok(format!("{} files: {} modified, {} added, {} deleted",
            modified + added + deleted, modified, added, deleted))
    }
}
```

---

## Token Tracking

### SQLite Schema

```sql
CREATE TABLE commands (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp     TEXT NOT NULL,
    session_id    TEXT NOT NULL,
    command       TEXT NOT NULL,
    tool          TEXT NOT NULL,
    cwd           TEXT NOT NULL,
    exit_code     INTEGER DEFAULT 0,
    
    original_tokens   INTEGER NOT NULL,
    compressed_tokens INTEGER NOT NULL,
    saved_tokens      INTEGER NOT NULL,
    savings_pct       REAL NOT NULL,
    
    strategy      TEXT,
    module        TEXT,
    exec_time_ms  INTEGER DEFAULT 0,
    
    metadata      TEXT
);

CREATE INDEX idx_session ON commands(session_id);
CREATE INDEX idx_timestamp ON commands(timestamp);
```

### Token Estimation

```rust
pub fn estimate_tokens(text: &str) -> usize {
    // GPT-style tokenization heuristic
    // Average: ~4 characters per token
    (text.len() as f64 / 4.0).ceil() as usize
}
```

### Tracking Flow

```rust
pub fn track(
    session_id: &str,
    command: &str,
    original: &str,
    compressed: &str,
    strategy: &str,
    module: &str,
) -> Result<()> {
    let db = get_db_connection()?;
    
    let original_tokens = estimate_tokens(original);
    let compressed_tokens = estimate_tokens(compressed);
    let saved_tokens = original_tokens.saturating_sub(compressed_tokens);
    let savings_pct = if original_tokens > 0 {
        (saved_tokens as f64 / original_tokens as f64) * 100.0
    } else {
        0.0
    };
    
    db.execute(
        "INSERT INTO commands (...) VALUES (?1, ?2, ...)",
        params![
            timestamp,
            session_id,
            command,
            original_tokens,
            compressed_tokens,
            saved_tokens,
            savings_pct,
            strategy,
            module,
        ]
    )?;
    
    Ok(())
}
```

---

## Plugin Integration

### Plugin Hooks

```typescript
// tool.execute.before - Detect and store command
"tool.execute.before": async (input, output) => {
  if (input.tool === "bash") {
    const command = output.args.command;
    const context = {
      command,
      cwd: process.cwd(),
      timestamp: Date.now(),
    };
    
    // Store for later
    pendingCommands.set(input.callID, context);
  }
}

// tool.execute.after - Compress output
"tool.execute.after": async (input, output) => {
  if (input.tool === "bash") {
    const context = pendingCommands.get(input.callID);
    
    if (context && shouldCompress(context.command)) {
      try {
        const compressed = await rtkClient.compress({
          command: context.command,
          output: output.output,
          context,
        });
        
        output.output = compressed;
      } catch (error) {
        // Fall back to original output
        console.error("RTK compression failed:", error);
      }
    }
    
    pendingCommands.delete(input.callID);
  }
}

// session.idle - Report savings
event: async ({ event }) => {
  if (event.type === "session.idle") {
    const stats = await rtkClient.getStats(event.session_id);
    
    if (stats.total_saved > 0) {
      console.log(`📊 RTK saved ${stats.total_saved} tokens (${stats.savings_pct}% reduction)`);
    }
  }
}
```

### RTKDaemonClient

```typescript
export class RTKDaemonClient {
  private socketPath: string;
  private connection: net.Socket | null = null;
  private requestId = 0;
  
  async compress(request: CompressRequest): Promise<string> {
    const response = await this.call("compress", request);
    return response.compressed;
  }
  
  async health(): Promise<boolean> {
    try {
      const response = await this.call("health", {});
      return response.status === "ok";
    } catch {
      return false;
    }
  }
  
  private async call(method: string, params: any): Promise<any> {
    const socket = await this.connect();
    
    const request = {
      jsonrpc: "2.0",
      id: ++this.requestId,
      method,
      params,
    };
    
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error("Request timeout"));
      }, 5000);
      
      socket.write(JSON.stringify(request));
      
      socket.once("data", (data) => {
        clearTimeout(timeout);
        
        try {
          const response = JSON.parse(data.toString());
          if (response.error) {
            reject(new Error(response.error.message));
          } else {
            resolve(response.result);
          }
        } catch (error) {
          reject(error);
        }
      });
    });
  }
  
  private async connect(): Promise<net.Socket> {
    if (this.connection && !this.connection.destroyed) {
      return this.connection;
    }
    
    return new Promise((resolve, reject) => {
      const socket = net.createConnection(this.socketPath);
      
      socket.once("connect", () => {
        this.connection = socket;
        resolve(socket);
      });
      
      socket.once("error", reject);
    });
  }
}
```

---

## Protocol Specification

### JSON-RPC 2.0

All communication uses JSON-RPC 2.0 over Unix socket.

#### Request

```json
{
  "jsonrpc": "2.0",
  "method": "compress",
  "id": 1,
  "params": {
    "command": "git log --oneline -5",
    "output": "abc123 First commit\ndef456 Second commit\n...",
    "context": {
      "cwd": "/path/to/project",
      "exit_code": 0,
      "tool": "bash",
      "session_id": "sess_abc123"
    }
  }
}
```

#### Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "compressed": "5 commits, +142/-89",
    "original_tokens": 1250,
    "compressed_tokens": 125,
    "saved_tokens": 1125,
    "savings_pct": 90.0,
    "strategy": "stats_extraction",
    "module": "git"
  }
}
```

#### Error Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32600,
    "message": "Invalid request",
    "data": "Missing required field: command"
  }
}
```

### Methods

| Method     | Description              | Parameters                          |
|------------|--------------------------|-------------------------------------|
| compress   | Compress output          | command, output, context            |
| decompress | Restore original         | compressed_id                       |
| health     | Health check             | (none)                              |
| stats      | Get session statistics   | session_id?, since?                 |
| shutdown   | Graceful shutdown        | (none)                              |

---

## Performance Targets

| Metric            | Target      | Measurement                    |
|-------------------|-------------|--------------------------------|
| Daemon latency    | < 5ms       | Request to response            |
| Startup time      | < 100ms     | Daemon initialization          |
| Memory usage      | < 10MB      | Steady state RSS               |
| Binary size       | < 8MB       | Stripped release               |
| Throughput        | > 1000/s    | Requests per second            |
| Compression ratio | 60-90%      | Token reduction                |

---

## Security Considerations

1. **Unix Socket Permissions** - Socket file has 0600 permissions
2. **Input Validation** - All inputs sanitized before processing
3. **Resource Limits** - Max request size 1MB, timeout 5s
4. **No Code Execution** - RTK only processes text, never executes
5. **Crash Isolation** - RTK crash doesn't affect OpenCode

---

## Future Enhancements

1. **LLM-Powered Compression** - Use local LLM for smarter filtering
2. **Auto-Configuration** - Detect project type, suggest optimal settings
3. **Custom Modules** - User-defined command modules via config
4. **Real-time Dashboard** - Web UI for token savings analytics
5. **Multi-Language Support** - Compression for non-English outputs
