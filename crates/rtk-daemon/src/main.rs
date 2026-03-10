mod server;
mod protocol;
mod handlers;
mod lifecycle;

use anyhow::Result;
use rtk_core::config::settings::load_config;
use rtk_core::tracking::db::init_db;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
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
    
    // Initialize database
    init_db()?;
    tracing::info!("Database initialized");
    
    // Start server
    let socket_path = config.daemon.socket_path.clone();
    tracing::info!("Listening on {}", socket_path);
    
    server::run(socket_path, config).await?;
    
    Ok(())
}
