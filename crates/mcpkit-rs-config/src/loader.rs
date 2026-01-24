//! Configuration loader with multiple source support

use std::{
    env,
    path::{Path, PathBuf},
};

use crate::{Config, ConfigError, Result};

/// Configuration loader that supports multiple sources
pub struct ConfigLoader {
    search_paths: Vec<PathBuf>,
    env_prefix: String,
}

impl ConfigLoader {
    /// Create a new configuration loader
    pub fn new() -> Self {
        Self {
            search_paths: default_search_paths(),
            env_prefix: "MCPKIT".to_string(),
        }
    }

    /// Add a search path
    pub fn add_search_path<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.search_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Set environment variable prefix
    pub fn set_env_prefix(&mut self, prefix: impl Into<String>) -> &mut Self {
        self.env_prefix = prefix.into();
        self
    }

    /// Load configuration from all available sources
    pub fn load(&self) -> Result<Config> {
        let mut config = Config::default();

        if let Some(file_config) = self.load_from_file()? {
            config.merge(file_config)?;
        }

        self.apply_env_overrides(&mut config)?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a specific file
    pub fn load_from_specific_file<P: AsRef<Path>>(&self, path: P) -> Result<Config> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(ConfigError::NotFound(path.display().to_string()));
        }

        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        match extension {
            "yaml" | "yml" => Config::from_yaml_file(path),
            "json" => Config::from_json_file(path),
            _ => Err(ConfigError::ValidationError(format!(
                "Unsupported config file extension: {}",
                extension
            ))),
        }
    }

    /// Load configuration from the first found config file
    fn load_from_file(&self) -> Result<Option<Config>> {
        let config_names = [
            "mcpkit.yaml",
            "mcpkit.yml",
            "mcpkit.json",
            ".mcpkit.yaml",
            ".mcpkit.yml",
            ".mcpkit.json",
            "config.yaml",
            "config.yml",
            "config.json",
        ];

        for dir in &self.search_paths {
            for name in &config_names {
                let path = dir.join(name);
                if path.exists() {
                    tracing::debug!("Loading config from: {}", path.display());
                    return self.load_from_specific_file(path).map(Some);
                }
            }
        }

        Ok(None)
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&self, config: &mut Config) -> Result<()> {
        if let Ok(bind) = env::var(format!("{}_SERVER_BIND", self.env_prefix)) {
            config.server.bind = bind;
        }

        if let Ok(port) = env::var(format!("{}_SERVER_PORT", self.env_prefix)) {
            config.server.port = port.parse().map_err(|_| {
                ConfigError::ValidationError(format!("Invalid port number from env: {}", port))
            })?;
        }

        if let Ok(debug) = env::var(format!("{}_DEBUG", self.env_prefix)) {
            config.server.debug = debug.to_lowercase() == "true" || debug == "1";
        }

        if let Ok(log_level) = env::var(format!("{}_LOG_LEVEL", self.env_prefix)) {
            config.server.log_level = Some(log_level);
        }

        Ok(())
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Get default configuration search paths
fn default_search_paths() -> Vec<PathBuf> {
    let mut paths = vec![env::current_dir().unwrap_or_default()];

    if let Ok(config_dir) = env::var("MCPKIT_CONFIG_DIR") {
        paths.push(PathBuf::from(config_dir));
    }

    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".config").join("mcpkit"));
        paths.push(home.join(".mcpkit"));
    }

    if let Some(config_home) = dirs::config_dir() {
        paths.push(config_home.join("mcpkit"));
    }

    paths.push(PathBuf::from("/etc/mcpkit"));

    paths
}
