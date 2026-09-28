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
    #[serde(default)]
    pub auth: AuthConfig,
}

/// How Burst establishes the caller's identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthMode {
    /// Identity comes from the `X-Auth-*` headers a gateway sets, and is
    /// accepted only from a peer in `trusted_proxies`.
    #[default]
    TrustedHeaders,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub mode: AuthMode,
    /// Peers whose `X-Auth-*` headers are believed, as single addresses or CIDR
    /// ranges. Required in `trusted-headers` mode: the headers name the caller,
    /// so anything able to reach Burst directly could otherwise claim any
    /// identity, and any role.
    #[serde(default)]
    pub trusted_proxies: Vec<String>,
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
    pub auth: AuthConfig,
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
    /// Storage backend: "local" (default) or "gateway" (S3 via Barbacane).
    #[serde(default = "default_storage_backend")]
    pub backend: String,
    #[serde(default = "default_local_path")]
    pub local_path: String,
    /// Barbacane S3 sidecar URL (e.g. "http://localhost:8081").
    /// Required when backend = "gateway".
    #[serde(default)]
    pub gateway_url: Option<String>,
    /// API key for authenticating to the S3 sidecar (X-Storage-Key header).
    #[serde(default)]
    pub gateway_api_key: Option<String>,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    #[serde(default = "default_max_files_per_message")]
    pub max_files_per_message: usize,
    #[serde(default = "default_blocked_extensions")]
    pub blocked_extensions: Vec<String>,
    /// Interval in seconds between cleanup runs for soft-deleted attachments.
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_secs: u64,
    /// Number of days after soft-delete before attachment files are removed.
    #[serde(default = "default_cleanup_retention")]
    pub cleanup_retention_days: i64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: default_storage_backend(),
            local_path: default_local_path(),
            gateway_url: None,
            gateway_api_key: None,
            max_file_size: default_max_file_size(),
            max_files_per_message: default_max_files_per_message(),
            blocked_extensions: default_blocked_extensions(),
            cleanup_interval_secs: default_cleanup_interval(),
            cleanup_retention_days: default_cleanup_retention(),
        }
    }
}

fn default_cleanup_interval() -> u64 {
    3600 // 1 hour
}

fn default_cleanup_retention() -> i64 {
    30 // 30 days
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
                    auth: AuthConfig::default(),
                },
            }
        };

        apply_env(&mut config)?;
        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.database.url.is_empty() {
            return Err(ConfigError::Missing("database.url (or BURST_DATABASE_URL)"));
        }
        // Fail closed: an empty list in trusted-headers mode would believe the
        // identity headers from every peer.
        if self.auth.mode == AuthMode::TrustedHeaders && self.auth.trusted_proxies.is_empty() {
            return Err(ConfigError::Missing(
                "auth.trusted_proxies (or BURST_AUTH_TRUSTED_PROXIES) is required in \
                 trusted-headers mode, as a comma-separated list of addresses or CIDR ranges",
            ));
        }
        if !matches!(self.broker.backend.as_str(), "in_process" | "pg_notify") {
            return Err(ConfigError::Invalid(format!(
                "broker.backend '{}' is not in_process or pg_notify",
                self.broker.backend
            )));
        }
        for entry in &self.auth.trusted_proxies {
            if !crate::api::trusted_peer::is_valid_entry(entry) {
                return Err(ConfigError::Invalid(format!(
                    "auth.trusted_proxies entry '{entry}' is not an IP address or CIDR range"
                )));
            }
        }
        Ok(())
    }

    pub fn app_config(&self) -> AppConfig {
        AppConfig {
            storage: self.storage.clone(),
            websocket: self.websocket.clone(),
            auth: self.auth.clone(),
        }
    }
}

fn apply_env(config: &mut Config) -> Result<(), ConfigError> {
    apply_overrides(config, |name| std::env::var(name).ok())
}

/// Applies each `BURST_*` override `var` returns. Split from `apply_env` so
/// tests can supply the variables without touching the process environment.
/// A number that does not parse is an error, not a silently kept default.
fn apply_overrides(
    config: &mut Config,
    var: impl Fn(&str) -> Option<String>,
) -> Result<(), ConfigError> {
    fn parsed<T: std::str::FromStr>(
        name: &str,
        value: Option<String>,
    ) -> Result<Option<T>, ConfigError> {
        value
            .map(|v| {
                v.trim().parse().map_err(|_| {
                    ConfigError::Invalid(format!("{name} '{v}' is not a valid number"))
                })
            })
            .transpose()
    }

    if let Some(v) = var("BURST_SERVER_LISTEN") {
        config.server.listen = v;
    }
    if let Some(v) = var("BURST_SERVER_ADMIN_LISTEN") {
        config.server.admin_listen = v;
    }
    if let Some(n) = parsed(
        "BURST_SERVER_SHUTDOWN_TIMEOUT_SECS",
        var("BURST_SERVER_SHUTDOWN_TIMEOUT_SECS"),
    )? {
        config.server.shutdown_timeout_secs = n;
    }
    if let Some(v) = var("BURST_DATABASE_URL") {
        config.database.url = v;
    }
    if let Some(n) = parsed(
        "BURST_DATABASE_MAX_CONNECTIONS",
        var("BURST_DATABASE_MAX_CONNECTIONS"),
    )? {
        config.database.max_connections = n;
    }
    if let Some(v) = var("BURST_AUTH_TRUSTED_PROXIES") {
        config.auth.trusted_proxies = v
            .split(',')
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .map(str::to_string)
            .collect();
    }
    if let Some(v) = var("BURST_BROKER_BACKEND") {
        config.broker.backend = v;
    }
    if let Some(v) = var("BURST_STORAGE_BACKEND") {
        config.storage.backend = v;
    }
    if let Some(v) = var("BURST_STORAGE_LOCAL_PATH") {
        config.storage.local_path = v;
    }
    if let Some(v) = var("BURST_STORAGE_GATEWAY_URL") {
        config.storage.gateway_url = Some(v);
    }
    if let Some(v) = var("BURST_STORAGE_GATEWAY_API_KEY") {
        config.storage.gateway_api_key = Some(v);
    }
    if let Some(n) = parsed(
        "BURST_STORAGE_MAX_FILE_SIZE",
        var("BURST_STORAGE_MAX_FILE_SIZE"),
    )? {
        config.storage.max_file_size = n;
    }
    if let Some(n) = parsed(
        "BURST_STORAGE_MAX_FILES_PER_MESSAGE",
        var("BURST_STORAGE_MAX_FILES_PER_MESSAGE"),
    )? {
        config.storage.max_files_per_message = n;
    }
    if let Some(n) = parsed(
        "BURST_STORAGE_CLEANUP_INTERVAL_SECS",
        var("BURST_STORAGE_CLEANUP_INTERVAL_SECS"),
    )? {
        config.storage.cleanup_interval_secs = n;
    }
    if let Some(n) = parsed(
        "BURST_STORAGE_CLEANUP_RETENTION_DAYS",
        var("BURST_STORAGE_CLEANUP_RETENTION_DAYS"),
    )? {
        config.storage.cleanup_retention_days = n;
    }
    if let Some(n) = parsed(
        "BURST_WEBSOCKET_BROADCAST_CAPACITY",
        var("BURST_WEBSOCKET_BROADCAST_CAPACITY"),
    )? {
        config.websocket.broadcast_capacity = n;
    }
    if let Some(n) = parsed(
        "BURST_WEBSOCKET_EVENT_BUFFER_CAPACITY",
        var("BURST_WEBSOCKET_EVENT_BUFFER_CAPACITY"),
    )? {
        config.websocket.event_buffer_capacity = n;
    }
    if let Some(v) = var("BURST_TELEMETRY_OTLP_ENDPOINT") {
        config.telemetry.otlp_endpoint = Some(v).filter(|e| !e.is_empty());
    }
    if let Some(n) = parsed(
        "BURST_TELEMETRY_TRACE_SAMPLE_RATE",
        var("BURST_TELEMETRY_TRACE_SAMPLE_RATE"),
    )? {
        config.telemetry.trace_sample_rate = n;
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {0}: {1}")]
    ReadFile(String, std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(toml::de::Error),
    #[error("missing required config: {0}")]
    Missing(&'static str),
    #[error("invalid config: {0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn base() -> Config {
        Config {
            server: ServerConfig::default(),
            database: DatabaseConfig {
                url: "postgres://localhost/burst".into(),
                max_connections: default_max_connections(),
            },
            storage: StorageConfig::default(),
            websocket: WebSocketConfig::default(),
            telemetry: TelemetryConfig::default(),
            broker: BrokerConfig::default(),
            auth: AuthConfig {
                trusted_proxies: vec!["127.0.0.1/32".into()],
                ..AuthConfig::default()
            },
        }
    }

    fn with(vars: &[(&str, &str)]) -> Config {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let mut config = base();
        apply_overrides(&mut config, |name| vars.get(name).cloned()).expect("valid overrides");
        config
    }

    #[test]
    fn the_broker_is_chosen_from_the_environment() {
        assert_eq!(with(&[]).broker.backend, "in_process");
        assert_eq!(
            with(&[("BURST_BROKER_BACKEND", "pg_notify")])
                .broker
                .backend,
            "pg_notify"
        );
    }

    #[test]
    fn an_unknown_broker_is_refused_rather_than_run_in_process() {
        let config = with(&[("BURST_BROKER_BACKEND", "pg-notify")]);
        assert!(matches!(config.validate(), Err(ConfigError::Invalid(_))));
        assert!(
            with(&[("BURST_BROKER_BACKEND", "pg_notify")])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn every_documented_setting_can_come_from_the_environment() {
        let config = with(&[
            ("BURST_SERVER_SHUTDOWN_TIMEOUT_SECS", "12"),
            ("BURST_STORAGE_MAX_FILE_SIZE", "1048576"),
            ("BURST_STORAGE_MAX_FILES_PER_MESSAGE", "3"),
            ("BURST_STORAGE_CLEANUP_INTERVAL_SECS", "60"),
            ("BURST_STORAGE_CLEANUP_RETENTION_DAYS", "7"),
            ("BURST_WEBSOCKET_BROADCAST_CAPACITY", "2048"),
            ("BURST_WEBSOCKET_EVENT_BUFFER_CAPACITY", "512"),
            ("BURST_TELEMETRY_OTLP_ENDPOINT", "http://otel:4317"),
            ("BURST_TELEMETRY_TRACE_SAMPLE_RATE", "0.25"),
        ]);
        assert_eq!(config.server.shutdown_timeout_secs, 12);
        assert_eq!(config.storage.max_file_size, 1_048_576);
        assert_eq!(config.storage.max_files_per_message, 3);
        assert_eq!(config.storage.cleanup_interval_secs, 60);
        assert_eq!(config.storage.cleanup_retention_days, 7);
        assert_eq!(config.websocket.broadcast_capacity, 2048);
        assert_eq!(config.websocket.event_buffer_capacity, 512);
        assert_eq!(
            config.telemetry.otlp_endpoint.as_deref(),
            Some("http://otel:4317")
        );
        assert!((config.telemetry.trace_sample_rate - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn a_number_that_does_not_parse_is_an_error_naming_the_variable() {
        let mut config = base();
        let err = apply_overrides(&mut config, |name| {
            (name == "BURST_STORAGE_MAX_FILE_SIZE").then(|| "50MB".to_string())
        })
        .expect_err("50MB is not a number of bytes");
        assert!(
            err.to_string().contains("BURST_STORAGE_MAX_FILE_SIZE"),
            "{err}"
        );
    }

    #[test]
    fn the_other_overrides_still_apply() {
        let config = with(&[
            ("BURST_DATABASE_MAX_CONNECTIONS", "7"),
            ("BURST_AUTH_TRUSTED_PROXIES", "10.0.0.0/8, 192.168.0.0/16"),
            ("BURST_STORAGE_BACKEND", "gateway"),
        ]);
        assert_eq!(config.database.max_connections, 7);
        assert_eq!(
            config.auth.trusted_proxies,
            vec!["10.0.0.0/8", "192.168.0.0/16"]
        );
        assert_eq!(config.storage.backend, "gateway");
    }
}
