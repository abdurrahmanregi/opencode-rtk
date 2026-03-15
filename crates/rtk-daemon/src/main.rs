mod handlers;
mod lifecycle;
mod protocol;
mod server;

use anyhow::Result;
use rtk_core::config::settings::load_config;
use rtk_core::tracking::db::init_db;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env from current directory or parent directories (optional)
    #[cfg(feature = "llm")]
    {
        dotenvy::dotenv().ok();
    }

    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting opencode-rtk daemon...");

    // Load configuration
    let config = load_config()?;
    tracing::info!("Configuration loaded");

    // Log LLM status
    #[cfg(feature = "llm")]
    if config.llm.enabled {
        tracing::info!("LLM compression enabled (backend: {})", config.llm.backend);
    }

    // Initialize database
    init_db()?;
    tracing::info!("Database initialized");

    // Start server
    let socket_path = config.daemon.socket_path.clone();
    #[cfg(windows)]
    tracing::info!(
        "Starting daemon on {}",
        config
            .daemon
            .tcp_address
            .as_deref()
            .unwrap_or("127.0.0.1:9876")
    );

    #[cfg(unix)]
    tracing::info!("Starting daemon on {}", socket_path);

    server::run(socket_path, config).await?;

    Ok(())
}
