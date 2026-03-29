use anyhow::{Context as AnyhowContext, Result};
use clap::{Parser, Subcommand};
use rtk_core::{compress, Context};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::ExitCode;
use std::time::Duration;

use serde_json::Value;

/// Maximum input size (10 MB)
const MAX_INPUT_SIZE: usize = 10 * 1024 * 1024;

/// Exit code for unimplemented commands
const EXIT_NOT_IMPLEMENTED: u8 = 2;
/// Default daemon address (Windows uses TCP)
const DAEMON_ADDR: &str = "127.0.0.1:9876";
/// Connection timeout in milliseconds
const CONNECT_TIMEOUT_MS: u64 = 2000;
/// Check if the daemon is healthy by connecting and sending a health check request
fn check_daemon_health() -> Result<bool> {
    use std::net::SocketAddr;
    
    let addr: SocketAddr = DAEMON_ADDR.parse()
        .context("Invalid daemon address")?;
    let stream = TcpStream::connect_timeout(
        &addr,
        Duration::from_millis(CONNECT_TIMEOUT_MS),
    ).context("Failed to connect to daemon. Is it running?")?;
    
    let mut reader = BufReader::new(stream.try_clone().context("Failed to clone stream")?);
    let mut writer = stream;
    
    // Send health check request (JSON-RPC 2.0 format)
    // Note: params should be null or omitted for health check (no params needed)
    let request = r#"{"jsonrpc":"2.0","method":"health","id":1,"params":null}"#;
    writer.write_all(format!("{}\n", request).as_bytes())
        .context("Failed to send health request")?;
    writer.flush().context("Failed to flush request")?;
    
    // Read response
    let mut response = String::new();
    let bytes_read = reader.read_line(&mut response)
        .context("Failed to read response from daemon")?;
    
    if bytes_read == 0 {
        return Err(anyhow::anyhow!("Daemon closed connection without sending response"));
    }
    
    // Parse response
    let parsed: Value = serde_json::from_str(&response.trim())
        .context("Failed to parse daemon response")?;
    
    // Check if response indicates healthy
    if let Some(result) = parsed.get("result") {
        // Check for status field (could be "ok" or an object)
        if let Some(status) = result.get("status") {
            let status_str = status.as_str().unwrap_or("unknown");
            if status_str == "ok" {
                let version = result.get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                println!("Daemon is healthy (version: {})", version);
                return Ok(true);
            }
        }
        
        // Status present but not "ok"
        eprintln!("Daemon returned unhealthy status: {}", result);
        return Ok(false);
    }
    
    // Check for error in response
    if let Some(error) = parsed.get("error") {
        let msg = error.get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        eprintln!("Daemon returned error: {}", msg);
        return Ok(false);
    }
    
    eprintln!("Unexpected response from daemon: {}", response);
    Ok(false)
}

#[derive(Parser)]
#[command(name = "rtk-cli")]
#[command(about = "CLI wrapper for RTK daemon", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Compress output via stdin
    Compress {
        /// Original command
        #[arg(short, long)]
        command: String,
        /// Working directory
        #[arg(short = 'd', long, default_value = ".")]
        cwd: String,
    },
    /// Check daemon health
    Health,
    /// Show session statistics
    Stats {
        /// Session ID
        #[arg(short, long)]
        session: Option<String>,
    },
}
#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {}", e);
            // Print error chain for debugging
            let mut source = e.source();
            while let Some(cause) = source {
                eprintln!("Caused by: {}", cause);
                source = cause.source();
            }
            ExitCode::FAILURE
        }
    }
}
async fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compress { command, cwd } => {
            // Read stdin with validation
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .context("Failed to read from stdin. Make sure to pipe input.")?;
            // Validate input
            if input.is_empty() {
                return Err(anyhow::anyhow!(
                    "No input provided on stdin. Usage: some-command | rtk-cli compress -c 'some-command'"
                ));
            }
            if input.len() > MAX_INPUT_SIZE {
                return Err(anyhow::anyhow!(
                    "Input too large ({} bytes). Maximum allowed: {} bytes ({} MB)",
                    input.len(),
                    MAX_INPUT_SIZE,
                    MAX_INPUT_SIZE / (1024 * 1024)
                ));
            }
            let context = Context {
                cwd,
                exit_code: 0,
                tool: "bash".to_string(),
                session_id: None,
                command: Some(command.clone()),
            };
            let result = compress(&command, &input, context)
                .with_context(|| format!("Failed to compress output for command: {}", command))?;
            println!("{}", result.compressed);
            eprintln!(
                "Saved {} tokens ({:.1}%)",
                result.saved_tokens, result.savings_pct
            );
            Ok(ExitCode::SUCCESS)
        }
        Commands::Health => {
            match check_daemon_health() {
                Ok(healthy) => {
                    if healthy {
                        Ok(ExitCode::SUCCESS)
                    } else {
                        Ok(ExitCode::FAILURE)
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    Ok(ExitCode::FAILURE)
                }
            }
        }
        Commands::Stats { session } => {
            get_daemon_stats(session)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Send a stats request to the daemon and print the results
fn get_daemon_stats(session_id: Option<String>) -> Result<()> {
    use std::net::SocketAddr;
    
    let addr: SocketAddr = DAEMON_ADDR.parse()
        .context("Invalid daemon address")?;
    let stream = TcpStream::connect_timeout(
        &addr,
        Duration::from_millis(CONNECT_TIMEOUT_MS),
    ).context("Failed to connect to daemon. Is it running?")?;
    
    let mut reader = BufReader::new(stream.try_clone().context("Failed to clone stream")?);
    let mut writer = stream;
    
    // Send stats request (JSON-RPC 2.0 format)
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "stats",
        "id": 1,
        "params": {
            "session_id": session_id
        }
    });
    
    writer.write_all(format!("{}\n", request).as_bytes())
        .context("Failed to send stats request")?;
    writer.flush().context("Failed to flush request")?;
    
    // Read response
    let mut response = String::new();
    let bytes_read = reader.read_line(&mut response)
        .context("Failed to read response from daemon")?;
    
    if bytes_read == 0 {
        return Err(anyhow::anyhow!("Daemon closed connection without sending response"));
    }
    
    // Parse response
    let parsed: Value = serde_json::from_str(&response.trim())
        .context("Failed to parse daemon response")?;
    
    // Check for error in response
    if let Some(error) = parsed.get("error") {
        let msg = error.get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Err(anyhow::anyhow!("Daemon returned error: {}", msg));
    }
    
    if let Some(result) = parsed.get("result") {
        if let Some(message) = result.get("message") {
            println!("{}", message.as_str().unwrap_or("Unknown message"));
            return Ok(());
        }
        
        println!("\nCompression Statistics:");
        println!("-----------------------");
        println!("Command count:           {}", result.get("command_count").and_then(|v| v.as_i64()).unwrap_or(0));
        println!("Total original tokens:   {}", result.get("total_original_tokens").and_then(|v| v.as_i64()).unwrap_or(0));
        println!("Total compressed tokens: {}", result.get("total_compressed_tokens").and_then(|v| v.as_i64()).unwrap_or(0));
        println!("Total saved tokens:      {}", result.get("total_saved_tokens").and_then(|v| v.as_i64()).unwrap_or(0));
        println!("Overall savings:         {:.1}%", result.get("savings_pct").and_then(|v| v.as_f64()).unwrap_or(0.0));
        println!();
    } else {
        eprintln!("Unexpected response format: {}", response);
    }
    
    Ok(())
}

