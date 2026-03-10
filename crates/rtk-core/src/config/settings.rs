use crate::config::Config;
use anyhow::{Context, Result};
use std::path::PathBuf;

pub fn load_config() -> Result<Config> {
    let config_path = get_config_path();

    if !config_path.exists() {
        return Ok(Config::default());
    }

    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

    let config: Config = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {:?}", config_path))?;

    Ok(config)
}

fn get_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("opencode-rtk")
        .join("config.toml")
}

pub fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path();

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
    }

    let content = toml::to_string_pretty(config).context("Failed to serialize config")?;

    std::fs::write(&config_path, content)
        .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

    Ok(())
}
