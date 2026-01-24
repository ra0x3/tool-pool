//! WASM tool manifest types and parsing

use std::{
    convert::TryFrom,
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use toml;

use crate::model::JsonObject;

/// Manifest describing a WASM tool's metadata and requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmToolManifest {
    /// Unique name of the tool
    pub name: String,

    /// Tool version
    pub version: String,

    /// Description of what the tool does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Path to the WASM module file (relative to manifest)
    pub wasm_module: PathBuf,

    /// Credentials required by this tool
    #[serde(default)]
    pub credentials: Vec<CredentialRequirement>,

    /// JSON Schema for tool input parameters
    pub input_schema: JsonObject,

    /// Optional JSON Schema for tool output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<JsonObject>,

    /// Execution timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// Maximum memory in bytes (default: 50MB)
    #[serde(default = "default_max_memory")]
    pub max_memory_bytes: usize,

    /// Maximum fuel for execution (to prevent DOS). If None, uses default based on memory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_fuel: Option<u64>,

    /// Environment variables to set (in addition to credentials)
    #[serde(default)]
    pub env_vars: Vec<EnvVar>,
}

/// A credential requirement for a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialRequirement {
    /// Name/identifier for this credential
    pub name: String,

    /// Type of credential
    #[serde(flatten)]
    pub credential_type: CredentialType,

    /// Whether this credential is required
    #[serde(default = "default_true")]
    pub required: bool,

    /// Environment variable name to inject (defaults to uppercase name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,

    /// Human-readable description of what this credential is for
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Types of credentials a tool can require
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CredentialType {
    /// OAuth2 token with optional scopes
    #[serde(rename = "oauth2")]
    OAuth2 {
        #[serde(default)]
        scopes: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
    },

    /// Simple API key
    #[serde(rename = "api_key")]
    ApiKey,

    /// HTTP Basic authentication
    #[serde(rename = "basic_auth")]
    BasicAuth,

    /// Bearer token
    #[serde(rename = "bearer_token")]
    BearerToken,

    /// Custom credential type
    #[serde(rename = "custom")]
    Custom { schema: JsonObject },
}

/// Environment variable to set for the tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    /// Environment variable name
    pub name: String,

    /// Value to set
    pub value: String,
}

fn default_timeout() -> u64 {
    30
}

fn default_max_memory() -> usize {
    50 * 1024 * 1024 // 50MB
}

fn default_true() -> bool {
    true
}

/// Bundle manifest for MCP WASM applications
/// This is typically loaded from a TOML file that accompanies the WASM binary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    /// Bundle metadata
    pub metadata: BundleMetadata,

    /// MCP server configuration
    pub server: ServerConfig,

    /// Runtime requirements
    pub runtime: RuntimeRequirements,

    /// Bundle contents and files
    pub bundle: BundleContents,

    /// External dependencies
    #[serde(default)]
    pub dependencies: BundleDependencies,
}

/// Bundle metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleMetadata {
    /// Bundle name
    pub name: String,

    /// Bundle version (semantic versioning)
    pub version: String,

    /// Bundle description
    pub description: String,

    /// Bundle author
    pub author: String,

    /// License identifier
    pub license: String,

    /// ISO 8601 creation timestamp
    pub created_at: String,

    /// SHA256 hash of the bundle contents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_hash: Option<String>,
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// MCP protocol version
    pub protocol_version: String,

    /// Transport type (stdio, http, etc.)
    pub transport: String,

    /// Server capabilities
    #[serde(default)]
    pub capabilities: Vec<String>,

    /// Available MCP tools
    #[serde(default)]
    pub tools: Vec<McpToolInfo>,
}

/// Information about an MCP tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    /// Tool name
    pub name: String,

    /// Tool description
    pub description: String,

    /// Required features for this tool to work
    #[serde(default)]
    pub required_features: Vec<String>,
}

/// Runtime requirements for the bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRequirements {
    /// Target WASM runtime (wasmedge, wasmtime, etc.)
    pub target: String,

    /// WASI version (wasip1, wasip2)
    pub wasi_version: String,

    /// Required runtime features
    #[serde(default)]
    pub required_features: Vec<String>,

    /// Environment variables needed
    #[serde(default)]
    pub environment: Vec<BundleEnvVar>,
}

/// Environment variable requirement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleEnvVar {
    /// Variable name
    pub name: String,

    /// Variable description
    pub description: String,

    /// Whether this variable is required
    #[serde(default)]
    pub required: bool,

    /// Default value if not provided
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// Bundle contents information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleContents {
    /// Main WASM binary filename
    pub binary: String,

    /// Additional files included in the bundle
    #[serde(default)]
    pub files: Vec<String>,

    /// Total bundle size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// Bundle dependencies
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BundleDependencies {
    /// External services required
    #[serde(default)]
    pub services: Vec<ServiceDependency>,

    /// OAuth providers supported
    #[serde(default)]
    pub oauth_providers: Vec<String>,
}

/// External service dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDependency {
    /// Service name
    pub name: String,

    /// Service type (database, cache, etc.)
    #[serde(rename = "type")]
    pub service_type: String,

    /// Version requirement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Connection string template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_template: Option<String>,
}

/// Trait for loading manifests from various sources
pub trait ManifestLoader: Sized {
    /// Load from a file path
    fn load<P: AsRef<Path>>(path: P) -> Result<Self, crate::wasm::WasmError>;

    /// Parse from string content
    fn parse(content: &str) -> Result<Self, crate::wasm::WasmError>;
}

/// Trait for saving manifests to various formats
pub trait ManifestSaver {
    /// Save to a file path
    fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), crate::wasm::WasmError>;

    /// Serialize to string
    fn to_string_pretty(&self) -> Result<String, crate::wasm::WasmError>;
}

/// Trait for bundle verification
pub trait BundleVerifier {
    /// Calculate bundle hash
    fn hash<P: AsRef<Path>>(&self, bundle_dir: P) -> Result<String, crate::wasm::WasmError>;

    /// Verify bundle integrity
    fn verify<P: AsRef<Path>>(&self, bundle_dir: P) -> Result<bool, crate::wasm::WasmError>;
}

impl ManifestLoader for BundleManifest {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self, crate::wasm::WasmError> {
        let content = fs::read_to_string(path.as_ref()).map_err(|e| {
            crate::wasm::WasmError::ManifestError(format!("Failed to read manifest file: {}", e))
        })?;
        Self::parse(&content)
    }

    fn parse(content: &str) -> Result<Self, crate::wasm::WasmError> {
        toml::from_str(content).map_err(|e| {
            crate::wasm::WasmError::ManifestError(format!("Failed to parse TOML manifest: {}", e))
        })
    }
}

impl ManifestSaver for BundleManifest {
    fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), crate::wasm::WasmError> {
        let content = self.to_string_pretty()?;
        fs::write(path.as_ref(), content).map_err(|e| {
            crate::wasm::WasmError::ManifestError(format!("Failed to write manifest file: {}", e))
        })?;
        Ok(())
    }

    fn to_string_pretty(&self) -> Result<String, crate::wasm::WasmError> {
        toml::to_string_pretty(&self).map_err(|e| {
            crate::wasm::WasmError::ManifestError(format!(
                "Failed to serialize manifest to TOML: {}",
                e
            ))
        })
    }
}

impl TryFrom<&Path> for BundleManifest {
    type Error = crate::wasm::WasmError;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        <Self as ManifestLoader>::load(path)
    }
}

impl TryFrom<PathBuf> for BundleManifest {
    type Error = crate::wasm::WasmError;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        <Self as ManifestLoader>::load(&path)
    }
}

impl TryFrom<&str> for BundleManifest {
    type Error = crate::wasm::WasmError;

    fn try_from(content: &str) -> Result<Self, Self::Error> {
        <Self as ManifestLoader>::parse(content)
    }
}

impl TryFrom<String> for BundleManifest {
    type Error = crate::wasm::WasmError;

    fn try_from(content: String) -> Result<Self, Self::Error> {
        <Self as ManifestLoader>::parse(&content)
    }
}

impl fmt::Display for BundleManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_string_pretty() {
            Ok(toml) => write!(f, "{}", toml),
            Err(e) => write!(f, "Failed to serialize manifest: {}", e),
        }
    }
}

impl BundleVerifier for BundleManifest {
    fn hash<P: AsRef<Path>>(&self, bundle_dir: P) -> Result<String, crate::wasm::WasmError> {
        let bundle_path = bundle_dir.as_ref();
        let mut hasher = Sha256::new();

        // Hash the binary file
        let binary_path = bundle_path.join(&self.bundle.binary);
        if binary_path.exists() {
            let content = fs::read(&binary_path).map_err(|e| {
                crate::wasm::WasmError::ManifestError(format!("Failed to read binary file: {}", e))
            })?;
            hasher.update(&content);
        }

        // Hash additional files
        for file in &self.bundle.files {
            let file_path = bundle_path.join(file);
            if file_path.exists() {
                let content = fs::read(&file_path).map_err(|e| {
                    crate::wasm::WasmError::ManifestError(format!(
                        "Failed to read file {}: {}",
                        file, e
                    ))
                })?;
                hasher.update(&content);
            }
        }

        // Include manifest content (excluding the hash field itself)
        let mut manifest_copy = self.clone();
        manifest_copy.metadata.bundle_hash = None;
        if let Ok(manifest_toml) = manifest_copy.to_string_pretty() {
            hasher.update(manifest_toml.as_bytes());
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    fn verify<P: AsRef<Path>>(&self, bundle_dir: P) -> Result<bool, crate::wasm::WasmError> {
        if let Some(expected_hash) = &self.metadata.bundle_hash {
            let calculated_hash = self.hash(bundle_dir)?;
            Ok(calculated_hash == *expected_hash)
        } else {
            Ok(true) // No hash to verify
        }
    }
}

impl WasmToolManifest {
    /// Load a manifest from a JSON file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, crate::wasm::WasmError> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            crate::wasm::WasmError::ManifestError(format!("Failed to read manifest: {}", e))
        })?;

        Self::from_json_str(&content)
    }

    /// Parse a manifest from JSON string
    pub fn from_json_str(json: &str) -> Result<Self, crate::wasm::WasmError> {
        serde_json::from_str(json).map_err(|e| {
            crate::wasm::WasmError::ManifestError(format!("Failed to parse manifest: {}", e))
        })
    }

    /// Validate the manifest
    pub fn validate(&self) -> Result<(), crate::wasm::WasmError> {
        if self.name.is_empty() {
            return Err(crate::wasm::WasmError::ManifestError(
                "Tool name cannot be empty".to_string(),
            ));
        }

        if self.version.is_empty() {
            return Err(crate::wasm::WasmError::ManifestError(
                "Tool version cannot be empty".to_string(),
            ));
        }

        if self.timeout_seconds == 0 {
            return Err(crate::wasm::WasmError::ManifestError(
                "Timeout must be greater than 0".to_string(),
            ));
        }

        if self.max_memory_bytes == 0 {
            return Err(crate::wasm::WasmError::ManifestError(
                "Max memory must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Get the environment variable name for a credential
    pub fn get_credential_env_var(&self, cred_name: &str) -> Option<String> {
        self.credentials
            .iter()
            .find(|c| c.name == cred_name)
            .map(|c| {
                c.env_var
                    .clone()
                    .unwrap_or_else(|| c.name.to_uppercase().replace('-', "_"))
            })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_manifest_parsing() {
        let json = json!({
            "name": "example-tool",
            "version": "1.0.0",
            "description": "An example WASM tool",
            "wasm_module": "./tool.wasm",
            "credentials": [
                {
                    "name": "github_token",
                    "type": "bearer_token",
                    "description": "GitHub API token"
                },
                {
                    "name": "openai_key",
                    "type": "api_key",
                    "required": false
                }
            ],
            "input_schema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            },
            "timeout_seconds": 60
        });

        let manifest: WasmToolManifest =
            serde_json::from_value(json).expect("Failed to parse manifest");

        assert_eq!(manifest.name, "example-tool");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.credentials.len(), 2);
        assert_eq!(manifest.timeout_seconds, 60);
        assert_eq!(manifest.max_memory_bytes, 50 * 1024 * 1024);
    }

    #[test]
    fn test_credential_env_var_name() {
        let manifest = WasmToolManifest {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            wasm_module: PathBuf::from("test.wasm"),
            credentials: vec![
                CredentialRequirement {
                    name: "api-key".to_string(),
                    credential_type: CredentialType::ApiKey,
                    required: true,
                    env_var: Some("CUSTOM_API_KEY".to_string()),
                    description: None,
                },
                CredentialRequirement {
                    name: "github-token".to_string(),
                    credential_type: CredentialType::BearerToken,
                    required: true,
                    env_var: None,
                    description: None,
                },
            ],
            input_schema: JsonObject::new(),
            output_schema: None,
            timeout_seconds: 30,
            max_memory_bytes: 50 * 1024 * 1024,
            max_fuel: None,
            env_vars: vec![],
        };

        assert_eq!(
            manifest.get_credential_env_var("api-key"),
            Some("CUSTOM_API_KEY".to_string())
        );
        assert_eq!(
            manifest.get_credential_env_var("github-token"),
            Some("GITHUB_TOKEN".to_string())
        );
    }
}
