pub mod db;

use crate::utils::tokens::estimate_tokens;
use anyhow::Result;
use chrono::{DateTime, Utc};

pub struct TrackingEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub command: String,
    pub tool: String,
    pub cwd: String,
    pub exit_code: i32,
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub saved_tokens: usize,
    pub savings_pct: f64,
    pub strategy: String,
    pub module: String,
    pub exec_time_ms: u64,
}

/// Request parameters for tracking a command execution
pub struct TrackRequest<'a> {
    pub session_id: &'a str,
    pub command: &'a str,
    pub tool: &'a str,
    pub cwd: &'a str,
    pub exit_code: i32,
    pub original: &'a str,
    pub compressed: &'a str,
    pub strategy: &'a str,
    pub module: &'a str,
    pub exec_time_ms: u64,
}

pub fn track(req: TrackRequest<'_>) -> Result<()> {
    let original_tokens = estimate_tokens(req.original);
    let compressed_tokens = estimate_tokens(req.compressed);
    let saved_tokens = original_tokens.saturating_sub(compressed_tokens);
    let savings_pct = if original_tokens > 0 {
        (saved_tokens as f64 / original_tokens as f64) * 100.0
    } else {
        0.0
    };

    let entry = TrackingEntry {
        timestamp: Utc::now(),
        session_id: req.session_id.to_string(),
        command: req.command.to_string(),
        tool: req.tool.to_string(),
        cwd: req.cwd.to_string(),
        exit_code: req.exit_code,
        original_tokens,
        compressed_tokens,
        saved_tokens,
        savings_pct,
        strategy: req.strategy.to_string(),
        module: req.module.to_string(),
        exec_time_ms: req.exec_time_ms,
    };

    db::insert_entry(&entry)
}
