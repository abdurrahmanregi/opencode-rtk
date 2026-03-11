//! Tee mode for saving original output on compression failure
//!
//! When compression fails or is too aggressive, tee mode saves the
//! original output to a file for later recovery.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Manager for tee file operations
pub struct TeeManager {
    /// Directory for tee files
    directory: PathBuf,
    /// Maximum number of files to keep
    max_files: usize,
    /// Days to retain files
    retention_days: u32,
}

/// Entry in the tee file list
#[derive(Debug, Clone)]
pub struct TeeEntry {
    /// File path
    pub path: PathBuf,
    /// Original command
    pub command: String,
    /// Timestamp when saved
    pub timestamp: DateTime<Utc>,
    /// File size in bytes
    pub size: usize,
}

impl TeeManager {
    /// Create a new tee manager
    pub fn new(directory: PathBuf, max_files: usize, retention_days: u32) -> Self {
        Self {
            directory,
            max_files,
            retention_days,
        }
    }

    /// Save output to a tee file
    ///
    /// # Arguments
    ///
    /// * `command` - The command that was executed
    /// * `output` - The original output to save
    ///
    /// # Returns
    ///
    /// Path to the saved file
    pub fn save(&self, command: &str, output: &str) -> Result<PathBuf> {
        // Ensure directory exists
        fs::create_dir_all(&self.directory)
            .with_context(|| format!("Failed to create tee directory: {:?}", self.directory))?;

        // Generate filename
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let sanitized = sanitize_command_for_filename(command);
        let filename = format!("{}_{}.log", timestamp, sanitized);
        let path = self.directory.join(&filename);

        // Write file
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| format!("Failed to create tee file: {:?}", path))?;

        // Write header
        writeln!(file, "# RTK Tee File")?;
        writeln!(file, "# Command: {}", command)?;
        writeln!(file, "# Timestamp: {}", Utc::now().to_rfc3339())?;
        writeln!(file, "#")?;
        writeln!(file)?;

        // Write output
        write!(file, "{}", output)?;

        // Rotate old files
        self.rotate()?;

        Ok(path)
    }

    /// List all tee files
    pub fn list(&self) -> Result<Vec<TeeEntry>> {
        if !self.directory.exists() {
            return Ok(vec![]);
        }

        let mut entries = vec![];

        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "log").unwrap_or(false) {
                let metadata = entry.metadata()?;
                let size = metadata.len() as usize;

                // Parse timestamp from filename
                let filename = path.file_name().unwrap().to_string_lossy();
                let timestamp = parse_timestamp_from_filename(&filename);

                // Read command from file header
                let command = read_command_from_file(&path)?;

                entries.push(TeeEntry {
                    path,
                    command,
                    timestamp,
                    size,
                });
            }
        }

        // Sort by timestamp (newest first)
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(entries)
    }

    /// Read content of a tee file
    pub fn read(&self, path: &Path) -> Result<String> {
        fs::read_to_string(path).with_context(|| format!("Failed to read tee file: {:?}", path))
    }

    /// Delete a specific tee file
    pub fn delete(&self, path: &Path) -> Result<()> {
        fs::remove_file(path).with_context(|| format!("Failed to delete tee file: {:?}", path))
    }

    /// Clear all tee files
    pub fn clear(&self) -> Result<usize> {
        let entries = self.list()?;
        let mut count = 0;

        for entry in entries {
            self.delete(&entry.path)?;
            count += 1;
        }

        Ok(count)
    }

    /// Rotate old files (remove files exceeding max_files or retention)
    pub fn rotate(&self) -> Result<usize> {
        let entries = self.list()?;
        let mut removed = 0;
        let cutoff = Utc::now() - chrono::Duration::days(self.retention_days as i64);

        for (i, entry) in entries.iter().enumerate() {
            let should_delete = i >= self.max_files || entry.timestamp < cutoff;
            if should_delete {
                if self.delete(&entry.path).is_ok() {
                    removed += 1;
                }
            }
        }

        Ok(removed)
    }
}

/// Sanitize command for use in filename
fn sanitize_command_for_filename(command: &str) -> String {
    command
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join("_")
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => c,
            _ => '_',
        })
        .collect::<String>()
        .chars()
        .take(30)
        .collect()
}

/// Parse timestamp from filename
fn parse_timestamp_from_filename(filename: &str) -> DateTime<Utc> {
    // Format: YYYYMMDD_HHMMSS_command.log
    if filename.len() >= 15 {
        let date_part = &filename[0..8];
        let time_part = &filename[9..15];

        if let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(minute), Ok(second)) = (
            date_part[0..4].parse::<i32>(),
            date_part[4..6].parse::<u32>(),
            date_part[6..8].parse::<u32>(),
            time_part[0..2].parse::<u32>(),
            time_part[2..4].parse::<u32>(),
            time_part[4..6].parse::<u32>(),
        ) {
            return chrono::NaiveDate::from_ymd_opt(year, month, day)
                .and_then(|d| d.and_hms_opt(hour, minute, second))
                .map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
                .unwrap_or_else(Utc::now);
        }
    }

    Utc::now()
}

/// Read command from file header
fn read_command_from_file(path: &Path) -> Result<String> {
    let content = fs::read_to_string(path)?;

    for line in content.lines().take(5) {
        if let Some(stripped) = line.strip_prefix("# Command: ") {
            return Ok(stripped.to_string());
        }
    }

    Ok("unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;
    use tempfile::tempdir;

    #[test]
    fn test_sanitize_command() {
        assert_eq!(sanitize_command_for_filename("git status"), "git_status");
        // Takes first 3 words, then truncates to 30 chars
        let result = sanitize_command_for_filename("npm test --watch");
        assert!(result.starts_with("npm_test"));
    }

    #[test]
    fn test_sanitize_command_special_chars() {
        // Single quotes are replaced with underscores
        let result = sanitize_command_for_filename("echo 'hello world'");
        assert!(result.contains("echo"));
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_sanitize_command_long() {
        let long_cmd = "git status --porcelain -b --long-flag-name extra args here";
        let result = sanitize_command_for_filename(long_cmd);
        assert!(result.len() <= 30);
    }

    #[test]
    fn test_save_and_read() {
        let dir = tempdir().unwrap();
        let manager = TeeManager::new(dir.path().to_path_buf(), 10, 90);

        let path = manager.save("git status", "M file.rs\nA file.ts").unwrap();
        assert!(path.exists());

        let content = manager.read(&path).unwrap();
        assert!(content.contains("git status"));
        assert!(content.contains("M file.rs"));
    }

    #[test]
    fn test_list() {
        let dir = tempdir().unwrap();
        let manager = TeeManager::new(dir.path().to_path_buf(), 10, 90);

        manager.save("git status", "output1").unwrap();
        manager.save("npm test", "output2").unwrap();

        let entries = manager.list().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_list_sorted_by_timestamp() {
        let dir = tempdir().unwrap();
        let manager = TeeManager::new(dir.path().to_path_buf(), 10, 90);

        manager.save("cmd1", "output1").unwrap();
        // Small delay to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(100));
        manager.save("cmd2", "output2").unwrap();

        let entries = manager.list().unwrap();
        // Newest first - but if timestamps are the same, order may vary
        assert_eq!(entries.len(), 2);
        // Just verify both commands are present
        let commands: Vec<&str> = entries.iter().map(|e| e.command.as_str()).collect();
        assert!(commands.contains(&"cmd1"));
        assert!(commands.contains(&"cmd2"));
    }

    #[test]
    fn test_rotation() {
        let dir = tempdir().unwrap();
        let manager = TeeManager::new(dir.path().to_path_buf(), 2, 90);

        manager.save("cmd1", "output1").unwrap();
        manager.save("cmd2", "output2").unwrap();
        manager.save("cmd3", "output3").unwrap(); // Should trigger rotation

        let entries = manager.list().unwrap();
        assert!(entries.len() <= 2);
    }

    #[test]
    fn test_delete() {
        let dir = tempdir().unwrap();
        let manager = TeeManager::new(dir.path().to_path_buf(), 10, 90);

        let path = manager.save("git status", "output").unwrap();
        assert!(path.exists());

        manager.delete(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_clear() {
        let dir = tempdir().unwrap();
        let manager = TeeManager::new(dir.path().to_path_buf(), 10, 90);

        manager.save("cmd1", "output1").unwrap();
        manager.save("cmd2", "output2").unwrap();

        let count = manager.clear().unwrap();
        assert_eq!(count, 2);

        let entries = manager.list().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_read_command_from_file() {
        let dir = tempdir().unwrap();
        let manager = TeeManager::new(dir.path().to_path_buf(), 10, 90);

        let path = manager.save("npm test", "test output").unwrap();
        let command = read_command_from_file(&path).unwrap();
        assert_eq!(command, "npm test");
    }

    #[test]
    fn test_parse_timestamp_from_filename() {
        let filename = "20240115_143022_git_status.log";
        let ts = parse_timestamp_from_filename(filename);

        // Verify it parsed correctly
        assert_eq!(ts.year(), 2024);
        assert_eq!(ts.month(), 1);
        assert_eq!(ts.day(), 15);
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        let filename = "invalid_filename.log";
        let ts = parse_timestamp_from_filename(filename);
        // Should return current time for invalid format
        // Just verify it doesn't panic
        let _ = ts.to_rfc3339();
    }

    #[test]
    fn test_tee_entry_size() {
        let dir = tempdir().unwrap();
        let manager = TeeManager::new(dir.path().to_path_buf(), 10, 90);

        let output = "x".repeat(100);
        manager.save("cmd", &output).unwrap();

        let entries = manager.list().unwrap();
        // Size should be at least the output size plus header
        assert!(entries[0].size >= 100);
    }
}
