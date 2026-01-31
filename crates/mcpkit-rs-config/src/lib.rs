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

    /// Distribution configuration for OCI/registry publishing
    pub distribution: Option<DistributionConfig>,

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
#[derive(Debug, Clone, Serialize)]
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
    #[serde(rename = "wasmedge")]
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
#[serde(untagged)]
pub enum McpCapabilities {
    List(Vec<String>),
    Struct {
        tools: Option<bool>,
        prompts: Option<bool>,
        resources: Option<bool>,
        logging: Option<bool>,
        experimental: Option<HashMap<String, bool>>,
    },
}

impl McpCapabilities {
    pub fn has_tools(&self) -> bool {
        match self {
            McpCapabilities::List(caps) => caps.contains(&"tools".to_string()),
            McpCapabilities::Struct { tools, .. } => tools.unwrap_or(false),
        }
    }

    pub fn has_prompts(&self) -> bool {
        match self {
            McpCapabilities::List(caps) => caps.contains(&"prompts".to_string()),
            McpCapabilities::Struct { prompts, .. } => prompts.unwrap_or(false),
        }
    }

    pub fn has_resources(&self) -> bool {
        match self {
            McpCapabilities::List(caps) => caps.contains(&"resources".to_string()),
            McpCapabilities::Struct { resources, .. } => resources.unwrap_or(false),
        }
    }

    pub fn has_logging(&self) -> bool {
        match self {
            McpCapabilities::List(caps) => caps.contains(&"logging".to_string()),
            McpCapabilities::Struct { logging, .. } => logging.unwrap_or(false),
        }
    }
}

/// Distribution configuration for OCI registry publishing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionConfig {
    /// OCI registry URI for this bundle (e.g., ghcr.io/org/bundle)
    pub registry: String,

    /// Version to publish (defaults to server.version)
    pub version: Option<String>,

    /// Tags to apply to the OCI image
    #[serde(default)]
    pub tags: Vec<String>,

    /// Bundle metadata for registry
    pub metadata: Option<BundleMetadata>,

    /// Files to include in bundle (defaults to module.wasm + config.yaml)
    #[serde(default)]
    pub include: Vec<String>,

    /// Registry authentication configuration
    pub auth: Option<RegistryAuth>,
}

/// Bundle metadata for distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleMetadata {
    /// Bundle authors
    #[serde(default)]
    pub authors: Vec<String>,

    /// License identifier (e.g., MIT, Apache-2.0)
    pub license: Option<String>,

    /// Source repository URL
    pub repository: Option<String>,

    /// Keywords for discovery
    #[serde(default)]
    pub keywords: Vec<String>,

    /// Bundle homepage
    pub homepage: Option<String>,

    /// Documentation URL
    pub documentation: Option<String>,
}

/// Registry authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryAuth {
    /// Registry username (supports env var interpolation)
    pub username: Option<String>,

    /// Registry password/token (supports env var interpolation)
    pub password: Option<String>,

    /// Path to auth file (e.g., ~/.docker/config.json)
    pub auth_file: Option<PathBuf>,

    /// Use system keychain for credentials
    #[serde(default)]
    pub use_keychain: bool,
}

#[cfg(test)]
mod tests;

// Custom deserializer for TransportConfig to properly match settings with transport type
impl<'de> Deserialize<'de> for TransportConfig {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use std::fmt;

        use serde::de::{self, MapAccess, Visitor};

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            #[serde(rename = "type")]
            Type,
            Settings,
        }

        struct TransportConfigVisitor;

        impl<'de> Visitor<'de> for TransportConfigVisitor {
            type Value = TransportConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct TransportConfig")
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<TransportConfig, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut transport_type: Option<TransportType> = None;
                let mut settings_value: Option<serde_json::Value> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Type => {
                            if transport_type.is_some() {
                                return Err(de::Error::duplicate_field("type"));
                            }
                            transport_type = Some(map.next_value()?);
                        }
                        Field::Settings => {
                            if settings_value.is_some() {
                                return Err(de::Error::duplicate_field("settings"));
                            }
                            settings_value = Some(map.next_value()?);
                        }
                    }
                }

                let transport_type =
                    transport_type.ok_or_else(|| de::Error::missing_field("type"))?;
                let settings_value =
                    settings_value.ok_or_else(|| de::Error::missing_field("settings"))?;

                // Deserialize settings based on transport type
                let settings = match transport_type {
                    TransportType::Stdio => {
                        let stdio_settings: StdioSettings = serde_json::from_value(settings_value)
                            .map_err(|e| {
                                de::Error::custom(format!("Invalid stdio settings: {}", e))
                            })?;
                        TransportSettings::Stdio(stdio_settings)
                    }
                    TransportType::Http => {
                        let http_settings: HttpSettings = serde_json::from_value(settings_value)
                            .map_err(|e| {
                                de::Error::custom(format!("Invalid HTTP settings: {}", e))
                            })?;
                        TransportSettings::Http(http_settings)
                    }
                    TransportType::WebSocket => {
                        let ws_settings: WebSocketSettings = serde_json::from_value(settings_value)
                            .map_err(|e| {
                                de::Error::custom(format!("Invalid WebSocket settings: {}", e))
                            })?;
                        TransportSettings::WebSocket(ws_settings)
                    }
                    TransportType::Grpc => {
                        let grpc_settings: GrpcSettings = serde_json::from_value(settings_value)
                            .map_err(|e| {
                                de::Error::custom(format!("Invalid gRPC settings: {}", e))
                            })?;
                        TransportSettings::Grpc(grpc_settings)
                    }
                };

                Ok(TransportConfig {
                    transport_type,
                    settings,
                })
            }
        }

        const FIELDS: &[&str] = &["type", "settings"];
        deserializer.deserialize_struct("TransportConfig", FIELDS, TransportConfigVisitor)
    }
}

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

        if let Some(distribution) = other.distribution {
            self.distribution = Some(distribution);
        }

        self.extensions.extend(other.extensions);

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        defaults::default_config()
    }
}
