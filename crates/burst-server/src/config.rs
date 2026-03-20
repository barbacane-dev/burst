use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub websocket: WebSocketConfig,
    #[serde(default)]
    pub telemetry: TelemetryConfig,
    #[serde(default)]
    pub broker: BrokerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_listen")]
    pub listen: String,
    #[serde(default = "default_admin_listen")]
    pub admin_listen: String,
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: default_listen(),
            admin_listen: default_admin_listen(),
            shutdown_timeout_secs: default_shutdown_timeout(),
        }
    }
}

fn default_shutdown_timeout() -> u64 {
    30
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
pub struct AppConfig {
    pub storage: StorageConfig,
    pub websocket: WebSocketConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebSocketConfig {
    /// Capacity of the broadcast channel (events in-flight to all connections).
    #[serde(default = "default_broadcast_capacity")]
    pub broadcast_capacity: usize,
    /// Size of the ring buffer used for gap-fill on reconnect.
    #[serde(default = "default_event_buffer_capacity")]
    pub event_buffer_capacity: usize,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            broadcast_capacity: default_broadcast_capacity(),
            event_buffer_capacity: default_event_buffer_capacity(),
        }
    }
}

fn default_broadcast_capacity() -> usize {
    1024
}

fn default_event_buffer_capacity() -> usize {
    500
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelemetryConfig {
    /// OpenTelemetry OTLP endpoint (e.g. "http://otel-collector:4317").
    /// Empty or absent = tracing disabled.
    #[serde(default)]
    pub otlp_endpoint: Option<String>,
    /// Trace sampling rate: 1.0 = all, 0.1 = 10%, 0.0 = disabled.
    #[serde(default = "default_trace_sample_rate")]
    pub trace_sample_rate: f64,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            otlp_endpoint: None,
            trace_sample_rate: default_trace_sample_rate(),
        }
    }
}

fn default_trace_sample_rate() -> f64 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrokerConfig {
    /// Broker backend: "in_process" (default, single-node) or "pg_notify" (multi-node).
    #[serde(default = "default_broker_backend")]
    pub backend: String,
}

impl Default for BrokerConfig {
    fn default() -> Self {
        Self {
            backend: default_broker_backend(),
        }
    }
}

fn default_broker_backend() -> String {
    "in_process".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_storage_backend")]
    pub backend: String,
    #[serde(default = "default_local_path")]
    pub local_path: String,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    #[serde(default = "default_max_files_per_message")]
    pub max_files_per_message: usize,
    #[serde(default = "default_blocked_extensions")]
    pub blocked_extensions: Vec<String>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: default_storage_backend(),
            local_path: default_local_path(),
            max_file_size: default_max_file_size(),
            max_files_per_message: default_max_files_per_message(),
            blocked_extensions: default_blocked_extensions(),
        }
    }
}

fn default_storage_backend() -> String {
    "local".into()
}

fn default_local_path() -> String {
    "./uploads".into()
}

fn default_max_file_size() -> u64 {
    20 * 1024 * 1024 // 20 MB
}

fn default_max_files_per_message() -> usize {
    10
}

fn default_blocked_extensions() -> Vec<String> {
    ["exe", "bat", "sh", "msi", "cmd", "ps1"]
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

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
                    storage: StorageConfig::default(),
                    websocket: WebSocketConfig::default(),
                    telemetry: TelemetryConfig::default(),
                    broker: BrokerConfig::default(),
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
        AppConfig {
            storage: self.storage.clone(),
            websocket: self.websocket.clone(),
        }
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
