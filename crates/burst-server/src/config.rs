use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_listen")]
    pub listen: String,
    #[serde(default = "default_admin_listen")]
    pub admin_listen: String,
    /// Trust X-Auth-Consumer / X-Auth-Consumer-Groups headers set by the
    /// Barbacane gateway. Must be false unless Burst is deployed behind
    /// Barbacane — with it true, any client that reaches the port can
    /// impersonate any user by forging that header.
    #[serde(default)]
    pub trust_auth_headers: bool,
    /// Mark refresh-token cookies as Secure (HTTPS only). Disable only for
    /// local development over plain HTTP.
    #[serde(default = "default_cookie_secure")]
    pub cookie_secure: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: default_listen(),
            admin_listen: default_admin_listen(),
            trust_auth_headers: false,
            cookie_secure: default_cookie_secure(),
        }
    }
}

fn default_cookie_secure() -> bool {
    true
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

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub jwt_secret: String,
    #[serde(default = "default_jwt_expiry")]
    pub jwt_expiry: String,
    #[serde(default = "default_refresh_expiry")]
    pub refresh_expiry: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: String::new(),
            jwt_expiry: default_jwt_expiry(),
            refresh_expiry: default_refresh_expiry(),
        }
    }
}

fn default_jwt_expiry() -> String {
    "15m".into()
}

fn default_refresh_expiry() -> String {
    "7d".into()
}

/// Subset of config passed into AppState (no secrets like DB URL).
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub jwt_secret: String,
    pub jwt_expiry_seconds: i64,
    pub refresh_expiry_seconds: i64,
    pub trust_auth_headers: bool,
    pub cookie_secure: bool,
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
                    auth: AuthConfig::default(),
                },
            }
        };

        // Apply env var overrides: BURST_<SECTION>_<KEY>
        apply_env(&mut config);

        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.database.url.is_empty() {
            return Err(ConfigError::Missing("database.url (or BURST_DATABASE_URL)"));
        }
        if self.auth.jwt_secret.is_empty() {
            return Err(ConfigError::Missing(
                "auth.jwt_secret (or BURST_AUTH_JWT_SECRET)",
            ));
        }
        Ok(())
    }

    pub fn app_config(&self) -> AppConfig {
        AppConfig {
            jwt_secret: self.auth.jwt_secret.clone(),
            jwt_expiry_seconds: parse_duration(&self.auth.jwt_expiry).unwrap_or(900),
            refresh_expiry_seconds: parse_duration(&self.auth.refresh_expiry)
                .unwrap_or(7 * 24 * 3600),
            trust_auth_headers: self.server.trust_auth_headers,
            cookie_secure: self.server.cookie_secure,
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
    if let Ok(v) = std::env::var("BURST_AUTH_JWT_SECRET") {
        config.auth.jwt_secret = v;
    }
    if let Ok(v) = std::env::var("BURST_AUTH_JWT_EXPIRY") {
        config.auth.jwt_expiry = v;
    }
    if let Ok(v) = std::env::var("BURST_AUTH_REFRESH_EXPIRY") {
        config.auth.refresh_expiry = v;
    }
    if let Ok(v) = std::env::var("BURST_SERVER_TRUST_AUTH_HEADERS") {
        config.server.trust_auth_headers =
            matches!(v.to_lowercase().as_str(), "true" | "1" | "yes");
    }
    if let Ok(v) = std::env::var("BURST_SERVER_COOKIE_SECURE") {
        config.server.cookie_secure = !matches!(v.to_lowercase().as_str(), "false" | "0" | "no");
    }
}

fn parse_duration(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Some(mins) = s.strip_suffix('m') {
        return mins.parse::<i64>().ok().map(|m| m * 60);
    }
    if let Some(hours) = s.strip_suffix('h') {
        return hours.parse::<i64>().ok().map(|h| h * 3600);
    }
    if let Some(days) = s.strip_suffix('d') {
        return days.parse::<i64>().ok().map(|d| d * 86400);
    }
    if let Some(secs) = s.strip_suffix('s') {
        return secs.parse::<i64>().ok();
    }
    s.parse::<i64>().ok()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_values() {
        assert_eq!(parse_duration("15m"), Some(900));
        assert_eq!(parse_duration("1h"), Some(3600));
        assert_eq!(parse_duration("7d"), Some(604800));
        assert_eq!(parse_duration("30s"), Some(30));
        assert_eq!(parse_duration("300"), Some(300));
    }
}
