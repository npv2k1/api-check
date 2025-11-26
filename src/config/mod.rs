//! Configuration module for api-check
//!
//! Supports configuration via file and environment variables.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind the server to
    #[serde(default = "default_host")]
    pub host: String,
    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

/// Proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyConfig {
    /// Whether proxy mode is enabled
    #[serde(default)]
    pub enabled: bool,
    /// Target URL to forward requests to
    #[serde(default)]
    pub target: Option<String>,
}

/// API testing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    /// Number of times to call the API
    #[serde(default = "default_num_calls")]
    pub num_calls: u32,
    /// Frequency in milliseconds between calls
    #[serde(default = "default_frequency_ms")]
    pub frequency_ms: u64,
    /// HTTP method (GET, POST, PUT, DELETE, etc.)
    #[serde(default = "default_method")]
    pub method: String,
    /// Target URL for testing (defaults to dev server)
    #[serde(default)]
    pub target_url: Option<String>,
    /// Request body (for POST/PUT)
    #[serde(default)]
    pub body: Option<String>,
    /// Custom headers as key-value pairs
    #[serde(default)]
    pub headers: Vec<(String, String)>,
}

fn default_num_calls() -> u32 {
    10
}

fn default_frequency_ms() -> u64 {
    100
}

fn default_method() -> String {
    "GET".to_string()
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            num_calls: default_num_calls(),
            frequency_ms: default_frequency_ms(),
            method: default_method(),
            target_url: None,
            body: None,
            headers: Vec::new(),
        }
    }
}

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,
    /// Proxy configuration
    #[serde(default)]
    pub proxy: ProxyConfig,
    /// Test configuration
    #[serde(default)]
    pub test: TestConfig,
}

impl AppConfig {
    /// Load configuration from file and environment
    pub fn load() -> anyhow::Result<Self> {
        // Try to load .env file (ignore if not found)
        let _ = dotenvy::dotenv();

        let mut config = config::Config::builder();

        // Add default config
        config = config.add_source(config::Config::try_from(&AppConfig::default())?);

        // Try to load from config file if it exists
        if std::path::Path::new("config.toml").exists() {
            config = config.add_source(config::File::with_name("config").required(false));
        }

        // Override with environment variables (prefixed with API_CHECK_)
        config = config.add_source(
            config::Environment::with_prefix("API_CHECK")
                .separator("_")
                .try_parsing(true),
        );

        let config = config.build()?;
        let app_config: AppConfig = config.try_deserialize()?;

        Ok(app_config)
    }

    /// Load configuration from a specific file
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: AppConfig =
            toml::from_str(&contents).or_else(|_| serde_json::from_str(&contents))?;
        Ok(config)
    }
}

/// Shared application state that holds runtime configuration
#[derive(Debug, Clone)]
pub struct SharedConfig {
    inner: Arc<RwLock<AppConfig>>,
}

impl SharedConfig {
    /// Create a new shared configuration
    pub fn new(config: AppConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(config)),
        }
    }

    /// Get a read-only copy of the configuration
    pub fn get(&self) -> AppConfig {
        self.inner.read().clone()
    }

    /// Update the proxy configuration
    pub fn update_proxy(&self, proxy: ProxyConfig) {
        self.inner.write().proxy = proxy;
    }

    /// Update the test configuration
    pub fn update_test(&self, test: TestConfig) {
        self.inner.write().test = test;
    }

    /// Update the entire configuration
    pub fn update(&self, config: AppConfig) {
        *self.inner.write() = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert!(!config.proxy.enabled);
        assert_eq!(config.test.num_calls, 10);
    }

    #[test]
    fn test_shared_config() {
        let config = AppConfig::default();
        let shared = SharedConfig::new(config);

        let proxy = ProxyConfig {
            enabled: true,
            target: Some("http://example.com".to_string()),
        };
        shared.update_proxy(proxy.clone());

        let updated = shared.get();
        assert!(updated.proxy.enabled);
        assert_eq!(updated.proxy.target, Some("http://example.com".to_string()));
    }
}
