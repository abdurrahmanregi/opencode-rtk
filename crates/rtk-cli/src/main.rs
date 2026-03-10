use anyhow::{Context as AnyhowContext, Result};
use clap::{Parser, Subcommand};
use rtk_core::{compress, Context};
use std::io::{self, Read};
use std::process::ExitCode;

/// Maximum input size (10 MB)
const MAX_INPUT_SIZE: usize = 10 * 1024 * 1024;

/// Exit code for unimplemented commands
const EXIT_NOT_IMPLEMENTED: u8 = 2;

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
            eprintln!("Error: Health check not implemented in CLI mode.");
            eprintln!("Hint: Use the daemon mode (rtk-daemon) for health checks.");
            Ok(ExitCode::from(EXIT_NOT_IMPLEMENTED))
        }

        Commands::Stats { session } => {
            eprintln!("Error: Stats not implemented in CLI mode.");
            eprintln!("Hint: Use the daemon mode (rtk-daemon) for statistics.");
            if let Some(s) = session {
                eprintln!("Requested session: {}", s);
            }
            Ok(ExitCode::from(EXIT_NOT_IMPLEMENTED))
        }
    }
}
