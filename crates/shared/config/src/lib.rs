//! Typed configuration loading for every service.
//!
//! Precedence (highest wins): process environment variables prefixed
//! `CBTC__<SERVICE>__...` > `.env` file in the working directory >
//! `config/default.toml` if present > struct defaults. Every service
//! calls `AppConfig::load(service_name)` once at startup.

use serde::Deserialize;

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
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RabbitMqConfig {
    pub url: String,
    #[serde(default = "default_exchange")]
    pub exchange: String,
}

fn default_exchange() -> String {
    "cbtc.events".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    #[serde(default = "default_jwt_ttl_secs")]
    pub jwt_ttl_secs: i64,
}

fn default_jwt_ttl_secs() -> i64 {
    3600
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    pub port: u16,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

/// Fault-injection knobs, read by `cbtc-mq` and `cbtc-db` middleware.
/// All disabled/zero by default; toggled per-service-instance via env
/// vars for failure-simulation scenarios, never enabled by application
/// code at runtime.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FaultInjectionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub latency_ms_min: u64,
    #[serde(default)]
    pub latency_ms_max: u64,
    #[serde(default)]
    pub packet_loss_probability: f64,
    #[serde(default)]
    pub duplicate_message_probability: f64,
    #[serde(default)]
    pub message_delay_ms_max: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub service_name: String,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub rabbitmq: RabbitMqConfig,
    pub auth: AuthConfig,
    #[serde(default)]
    pub fault_injection: FaultInjectionConfig,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to load configuration: {0}")]
    Load(#[from] config::ConfigError),
}

impl AppConfig {
    /// Load configuration for `service_name`. Looks for `config/default.toml`
    /// relative to the current working directory (optional — every value
    /// can also come purely from environment), then layers environment
    /// variables of the form `CBTC__DATABASE__URL`, `CBTC__SERVER__PORT`,
    /// etc. (double-underscore separator, matching the `config` crate's
    /// `Environment::separator` convention).
    pub fn load(service_name: &str) -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();

        let builder = config::Config::builder()
            .set_default("service_name", service_name)?
            .add_source(config::File::with_name("config/default").required(false))
            .add_source(
                config::Environment::with_prefix("CBTC")
                    .separator("__")
                    .try_parsing(true),
            );

        let settings = builder.build()?;
        settings.try_deserialize().map_err(ConfigError::from)
    }
}
