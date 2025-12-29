use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

// Re-export shared path utilities
pub use shared::{get_config_path, get_data_dir, get_socket_path, get_state_dir};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub daemon: DaemonConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    pub socket_timeout_ms: u64,
    pub output_buffer_kb: usize,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub theme: String,
    pub font_family: String,
    pub font_size: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            daemon: DaemonConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_timeout_ms: 5000,
            output_buffer_kb: 10,
            log_level: "info".to_string(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            font_family: "monospace".to_string(),
            font_size: 14,
        }
    }
}

pub fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;
    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}
