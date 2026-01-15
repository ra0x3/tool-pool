//! WASM tool manifest types and parsing

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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

impl WasmToolManifest {
    /// Load a manifest from a JSON file
    pub fn from_file(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, crate::wasm::WasmError> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            crate::wasm::WasmError::ManifestError(format!(
                "Failed to read manifest: {}",
                e
            ))
        })?;

        Self::from_json_str(&content)
    }

    /// Parse a manifest from JSON string
    pub fn from_json_str(json: &str) -> Result<Self, crate::wasm::WasmError> {
        serde_json::from_str(json).map_err(|e| {
            crate::wasm::WasmError::ManifestError(format!(
                "Failed to parse manifest: {}",
                e
            ))
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
