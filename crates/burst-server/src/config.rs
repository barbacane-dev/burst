use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_listen")]
    pub listen: String,
    #[serde(default = "default_admin_listen")]
    pub admin_listen: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: default_listen(),
            admin_listen: default_admin_listen(),
        }
    }
}

fn default_listen() -> String {
    "0.0.0.0:3000".into()
}

fn default_admin_listen() -> String {
    "0.0.0.0:3001".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

fn default_max_connections() -> u32 {
    20
}

/// Subset of config passed into AppState (no secrets like DB URL).
#[derive(Debug, Clone)]
pub struct AppConfig;

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let mut config = if let Some(path) = path {
            let contents = std::fs::read_to_string(path)
                .map_err(|e| ConfigError::ReadFile(path.display().to_string(), e))?;
            toml::from_str(&contents).map_err(ConfigError::Parse)?
        } else {
            // Try default path, fall back to env-only config
            match std::fs::read_to_string("burst.toml") {
                Ok(contents) => toml::from_str(&contents).map_err(ConfigError::Parse)?,
                Err(_) => Config {
                    server: ServerConfig::default(),
                    database: DatabaseConfig {
                        url: String::new(),
                        max_connections: default_max_connections(),
                    },
                },
            }
        };

        apply_env(&mut config);
        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.database.url.is_empty() {
            return Err(ConfigError::Missing("database.url (or BURST_DATABASE_URL)"));
        }
        Ok(())
    }

    pub fn app_config(&self) -> AppConfig {
        AppConfig
    }
}

fn apply_env(config: &mut Config) {
    if let Ok(v) = std::env::var("BURST_SERVER_LISTEN") {
        config.server.listen = v;
    }
    if let Ok(v) = std::env::var("BURST_SERVER_ADMIN_LISTEN") {
        config.server.admin_listen = v;
    }
    if let Ok(v) = std::env::var("BURST_DATABASE_URL") {
        config.database.url = v;
    }
    if let Ok(v) = std::env::var("BURST_DATABASE_MAX_CONNECTIONS")
        && let Ok(n) = v.parse()
    {
        config.database.max_connections = n;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {0}: {1}")]
    ReadFile(String, std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(toml::de::Error),
    #[error("missing required config: {0}")]
    Missing(&'static str),
}
