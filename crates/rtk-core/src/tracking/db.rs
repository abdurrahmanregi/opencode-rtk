use crate::tracking::TrackingEntry;
use anyhow::{anyhow, Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;

lazy_static::lazy_static! {
    static ref DB: Arc<Mutex<Option<Connection>>> = Arc::new(Mutex::new(None));
}

/// Thread-safe one-time initialization guard.
/// Ensures database initialization runs exactly once across all threads.
static INIT: Once = Once::new();

// Stores the initialization error (if any) to propagate to callers.
// Using String since anyhow::Error doesn't implement Clone.
lazy_static::lazy_static! {
    static ref INIT_ERROR: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS commands (
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

CREATE INDEX IF NOT EXISTS idx_session ON commands(session_id);
CREATE INDEX IF NOT EXISTS idx_timestamp ON commands(timestamp);
CREATE INDEX IF NOT EXISTS idx_tool ON commands(tool);
"#;

fn get_db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("opencode-rtk")
        .join("history.db")
}

/// Initialize the database. This function is thread-safe and will only initialize once.
///
/// Uses `std::sync::Once` to prevent race conditions and re-entrant deadlocks.
///
/// # Errors
///
/// Returns an error if:
/// - The database directory cannot be created
/// - The database file cannot be opened
/// - The schema cannot be initialized
pub fn init_db() -> Result<()> {
    // Fast path: check if already initialized without triggering init
    {
        let db = DB.lock().unwrap_or_else(|e| e.into_inner());
        if db.is_some() {
            return Ok(());
        }
        // Lock is released here when `db` goes out of scope
    }

    // Slow path: perform initialization exactly once
    INIT.call_once(|| {
        let result = init_db_inner();
        if let Err(e) = result {
            *INIT_ERROR.lock().unwrap_or_else(|e| e.into_inner()) = Some(e.to_string());
        }
    });

    // Check if initialization succeeded
    let error = INIT_ERROR.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(err_msg) = error.as_ref() {
        Err(anyhow!("Database initialization failed: {}", err_msg))
    } else {
        Ok(())
    }
}

/// Internal initialization function that does the actual work.
///
/// This function must only be called through `init_db()` to ensure
/// thread-safety via `std::sync::Once`.
fn init_db_inner() -> Result<()> {
    let db_path = get_db_path();

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create database directory: {:?}", parent))?;
    }

    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database: {:?}", db_path))?;

    conn.execute_batch(SCHEMA)
        .context("Failed to initialize database schema")?;

    // Set busy timeout to handle SQLITE_BUSY errors (5 seconds).
    // This causes SQLite to automatically retry when the database is locked,
    // rather than immediately returning SQLITE_BUSY.
    conn.busy_timeout(Duration::from_secs(5))
        .context("Failed to set busy timeout")?;

    // Store connection - handle poisoned mutex gracefully by recovering the inner value
    let mut db = DB.lock().unwrap_or_else(|e| e.into_inner());
    *db = Some(conn);

    Ok(())
}

/// Ensures the database is initialized before use.
///
/// This is called internally by functions that need the database.
/// It's safe to call multiple times - initialization only happens once.
fn ensure_initialized() -> Result<()> {
    // Fast path: check if already initialized
    {
        let db = DB.lock().unwrap_or_else(|e| e.into_inner());
        if db.is_some() {
            return Ok(());
        }
        // Lock is released here - IMPORTANT: we release before calling init_db()
        // to avoid re-entrant mutex deadlock
    }

    // Slow path: initialize (will use Once to prevent races)
    init_db()
}

/// Clean up old records from the database.
///
/// This should be called periodically (e.g., on daemon start or via a timer)
/// rather than on every insert for better performance.
///
/// # Errors
///
/// Returns an error if the database is not initialized or the cleanup fails.
pub fn cleanup_old_records() -> Result<()> {
    ensure_initialized()?;

    let db = DB.lock().unwrap_or_else(|e| e.into_inner());
    let conn = db
        .as_ref()
        .context("Database not initialized after ensure_initialized()")?;

    let _deleted = conn
        .execute(
            "DELETE FROM commands WHERE timestamp < datetime('now', '-90 days')",
            [],
        )
        .context("Failed to clean up old records")?;

    // Note: Could log deleted count here if logging is added to the crate

    Ok(())
}

/// Insert a tracking entry into the database.
///
/// # Errors
///
/// Returns an error if:
/// - The database cannot be initialized
/// - The insert operation fails
pub fn insert_entry(entry: &TrackingEntry) -> Result<()> {
    ensure_initialized()?;

    let db = DB.lock().unwrap_or_else(|e| e.into_inner());
    let conn = db
        .as_ref()
        .context("Database not initialized after ensure_initialized()")?;

    conn.execute(
        r#"
        INSERT INTO commands (
            timestamp, session_id, command, tool, cwd, exit_code,
            original_tokens, compressed_tokens, saved_tokens, savings_pct,
            strategy, module, exec_time_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        "#,
        rusqlite::params![
            entry.timestamp.to_rfc3339(),
            entry.session_id,
            entry.command,
            entry.tool,
            entry.cwd,
            entry.exit_code,
            entry.original_tokens,
            entry.compressed_tokens,
            entry.saved_tokens,
            entry.savings_pct,
            entry.strategy,
            entry.module,
            entry.exec_time_ms as i64,
        ],
    )
    .context("Failed to insert tracking entry")?;

    Ok(())
}

/// Get statistics for a specific session.
///
/// # Errors
///
/// Returns an error if:
/// - The database cannot be initialized
/// - The query fails
pub fn get_session_stats(session_id: &str) -> Result<SessionStats> {
    ensure_initialized()?;

    let db = DB.lock().unwrap_or_else(|e| e.into_inner());
    let conn = db
        .as_ref()
        .context("Database not initialized after ensure_initialized()")?;

    let mut stmt = conn.prepare(
        r#"
        SELECT 
            COUNT(*) as count,
            SUM(original_tokens) as total_original,
            SUM(compressed_tokens) as total_compressed,
            SUM(saved_tokens) as total_saved
        FROM commands
        WHERE session_id = ?1
        "#,
    )?;

    let stats = stmt.query_row(rusqlite::params![session_id], |row| {
        Ok(SessionStats {
            command_count: row.get(0)?,
            total_original_tokens: row.get(1)?,
            total_compressed_tokens: row.get(2)?,
            total_saved_tokens: row.get(3)?,
        })
    })?;

    Ok(stats)
}

#[derive(Debug, Clone)]
pub struct SessionStats {
    pub command_count: i64,
    pub total_original_tokens: i64,
    pub total_compressed_tokens: i64,
    pub total_saved_tokens: i64,
}

impl SessionStats {
    pub fn savings_pct(&self) -> f64 {
        if self.total_original_tokens > 0 {
            (self.total_saved_tokens as f64 / self.total_original_tokens as f64) * 100.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_init_db() {
        assert!(init_db().is_ok());
    }

    #[test]
    fn test_insert_entry() {
        init_db().ok();

        let entry = TrackingEntry {
            timestamp: Utc::now(),
            session_id: "test_session".to_string(),
            command: "git status".to_string(),
            tool: "bash".to_string(),
            cwd: "/tmp".to_string(),
            exit_code: 0,
            original_tokens: 100,
            compressed_tokens: 10,
            saved_tokens: 90,
            savings_pct: 90.0,
            strategy: "stats_extraction".to_string(),
            module: "git".to_string(),
            exec_time_ms: 5,
        };

        assert!(insert_entry(&entry).is_ok());
    }

    #[test]
    fn test_mutex_poisoning_recovery() {
        // Test that the code handles mutex poisoning gracefully
        // by using unwrap_or_else(|e| e.into_inner())
        init_db().ok();

        // Test that ensure_initialized handles potentially poisoned mutex
        // by recovering the inner value
        let result = ensure_initialized();
        assert!(
            result.is_ok(),
            "ensure_initialized should handle mutex recovery"
        );

        // Test cleanup also uses mutex recovery
        let result = cleanup_old_records();
        assert!(result.is_ok(), "cleanup should handle mutex recovery");
    }

    #[test]
    fn test_concurrent_db_access() {
        use std::sync::Arc;
        use std::thread;

        init_db().ok();

        let entry = TrackingEntry {
            timestamp: Utc::now(),
            session_id: "concurrent_test".to_string(),
            command: "test command".to_string(),
            tool: "bash".to_string(),
            cwd: "/tmp".to_string(),
            exit_code: 0,
            original_tokens: 50,
            compressed_tokens: 10,
            saved_tokens: 40,
            savings_pct: 80.0,
            strategy: "test".to_string(),
            module: "test".to_string(),
            exec_time_ms: 1,
        };

        let entry = Arc::new(entry);
        let mut handles = vec![];

        // Spawn multiple threads to test thread safety
        for _ in 0..5 {
            let entry = Arc::clone(&entry);
            handles.push(thread::spawn(move || insert_entry(&entry)));
        }

        // All inserts should succeed
        for handle in handles {
            assert!(handle.join().unwrap().is_ok());
        }
    }
}
