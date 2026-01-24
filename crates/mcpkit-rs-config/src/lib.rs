//! Configuration system for mcpkit-rs

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use mcpkit_rs_policy::Policy as PolicyConfig;
use serde::{Deserialize, Serialize};

pub mod defaults;
pub mod error;
pub mod loader;
pub mod validation;

pub use error::{ConfigError, Result};
pub use loader::ConfigLoader;

/// Main configuration structure for mcpkit-rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// Configuration format version
    pub version: String,

    /// Metadata about this configuration
    pub metadata: Option<Metadata>,

    /// Server configuration
    pub server: ServerConfig,

    /// Transport configuration
    pub transport: TransportConfig,

    /// Security policy configuration
    pub policy: Option<PolicyConfig>,

    /// Runtime configuration
    pub runtime: RuntimeConfig,

    /// MCP-specific configuration
    pub mcp: McpConfig,

    /// Extension configurations
    #[serde(default)]
    pub extensions: HashMap<String, serde_json::Value>,
}

/// Metadata about the configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server name
    pub name: String,

    /// Server version
    pub version: String,

    /// Server description
    pub description: Option<String>,

    /// Bind address
    pub bind: String,

    /// Port number
    pub port: u16,

    /// Max connections
    pub max_connections: Option<usize>,

    /// Request timeout in seconds
    pub request_timeout: Option<u64>,

    /// Enable debug mode
    #[serde(default)]
    pub debug: bool,

    /// Log level (trace, debug, info, warn, error)
    pub log_level: Option<String>,
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TransportConfig {
    /// Transport type
    #[serde(rename = "type")]
    pub transport_type: TransportType,

    /// Transport-specific settings
    pub settings: TransportSettings,
}

/// Available transport types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransportType {
    Stdio,
    Http,
    WebSocket,
    Grpc,
}

/// Transport-specific settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TransportSettings {
    Stdio(StdioSettings),
    Http(HttpSettings),
    WebSocket(WebSocketSettings),
    Grpc(GrpcSettings),
}

/// Stdio transport settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StdioSettings {
    pub buffer_size: Option<usize>,
}

/// HTTP transport settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpSettings {
    pub cors_enabled: Option<bool>,
    pub cors_origins: Option<Vec<String>>,
    pub max_body_size: Option<usize>,
    pub compression: Option<bool>,
    pub tls: Option<TlsConfig>,
}

/// WebSocket transport settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketSettings {
    pub ping_interval: Option<u64>,
    pub max_frame_size: Option<usize>,
    pub compression: Option<bool>,
}

/// gRPC transport settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcSettings {
    pub reflection: Option<bool>,
    pub max_message_size: Option<usize>,
    pub tls: Option<TlsConfig>,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_file: PathBuf,
    pub key_file: PathBuf,
    pub ca_file: Option<PathBuf>,
    pub verify_client: Option<bool>,
}

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Runtime type (native, wasmtime, wasmedge)
    #[serde(rename = "type")]
    pub runtime_type: RuntimeType,

    /// WASM-specific settings
    pub wasm: Option<WasmConfig>,

    /// Resource limits
    pub limits: Option<ResourceLimits>,
}

/// Runtime types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeType {
    Native,
    Wasmtime,
    WasmEdge,
}

/// WASM runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    /// Path to WASM module
    pub module_path: Option<PathBuf>,

    /// Enable fuel metering
    pub fuel: Option<u64>,

    /// Memory pages limit
    pub memory_pages: Option<u32>,

    /// Enable caching
    pub cache: Option<bool>,

    /// Cache directory
    pub cache_dir: Option<PathBuf>,
}

/// Resource limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub cpu: Option<String>,
    pub memory: Option<String>,
    pub execution_time: Option<String>,
    pub max_requests_per_minute: Option<u32>,
}

/// MCP-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Protocol version
    pub protocol_version: String,

    /// Available tools
    pub tools: Option<Vec<ToolConfig>>,

    /// Available prompts
    pub prompts: Option<Vec<PromptConfig>>,

    /// Available resources
    pub resources: Option<Vec<ResourceConfig>>,

    /// Capabilities
    pub capabilities: Option<McpCapabilities>,
}

/// Tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub handler: Option<String>,
}

/// Prompt configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    pub name: String,
    pub description: String,
    pub arguments: Option<Vec<PromptArgument>>,
}

/// Prompt argument
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
    pub default: Option<serde_json::Value>,
}

/// Resource configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    pub name: String,
    pub uri: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// MCP capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCapabilities {
    pub tools: Option<bool>,
    pub prompts: Option<bool>,
    pub resources: Option<bool>,
    pub logging: Option<bool>,
    pub experimental: Option<HashMap<String, bool>>,
}

#[cfg(test)]
mod tests;

impl Config {
    /// Load configuration from a YAML file
    pub fn from_yaml_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        Self::from_yaml(&contents)
    }

    /// Load configuration from a YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let config: Config = serde_yaml::from_str(yaml)?;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a JSON file
    pub fn from_json_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        Self::from_json(&contents)
    }

    /// Load configuration from a JSON string
    pub fn from_json(json: &str) -> Result<Self> {
        let config: Config = serde_json::from_str(json)?;
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to a YAML file
    pub fn to_yaml_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        std::fs::write(path, yaml)?;
        Ok(())
    }

    /// Save configuration to a JSON file
    pub fn to_json_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        validation::validate_config(self)
    }

    /// Merge with another configuration (other takes precedence)
    pub fn merge(&mut self, other: Config) -> Result<()> {
        if other.version != self.version {
            return Err(ConfigError::VersionMismatch {
                expected: self.version.clone(),
                found: other.version,
            });
        }

        if let Some(metadata) = other.metadata {
            self.metadata = Some(metadata);
        }

        self.server = other.server;
        self.transport = other.transport;

        if let Some(policy) = other.policy {
            self.policy = Some(policy);
        }

        self.runtime = other.runtime;
        self.mcp = other.mcp;
        self.extensions.extend(other.extensions);

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        defaults::default_config()
    }
}
