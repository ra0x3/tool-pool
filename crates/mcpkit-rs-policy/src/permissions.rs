//! Core permission types and policy structure

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Main policy structure containing all permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// Policy format version
    pub version: String,
    /// Optional description of the policy
    pub description: Option<String>,

    /// Core permissions configuration
    #[serde(default)]
    pub core: CorePermissions,

    /// Extension permissions as raw YAML values
    #[serde(flatten)]
    pub extensions: HashMap<String, serde_yaml::Value>,
}

/// Core permissions that most WASM projects need
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorePermissions {
    /// Storage/filesystem access permissions
    pub storage: Option<StoragePermissions>,
    /// Network access permissions
    pub network: Option<NetworkPermissions>,
    /// Environment variable access permissions
    pub environment: Option<EnvironmentPermissions>,
    /// Resource usage limits
    pub resources: Option<ResourceLimits>,
}

/// Storage/filesystem permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePermissions {
    /// List of allowed storage access rules
    #[serde(default)]
    pub allow: Vec<StorageRule>,

    /// List of denied storage access rules
    #[serde(default)]
    pub deny: Vec<StorageRule>,
}

/// Individual storage access rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageRule {
    /// URI pattern for the storage resource
    pub uri: String,
    /// Allowed access modes (read, write, execute)
    pub access: Vec<String>,
}

/// Network access permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPermissions {
    /// List of allowed network access rules
    #[serde(default)]
    pub allow: Vec<NetworkRule>,

    /// List of denied network access rules
    #[serde(default)]
    pub deny: Vec<NetworkRule>,
}

/// Individual network access rule
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NetworkRule {
    /// Host-based network rule
    Host {
        /// Hostname or domain pattern
        host: String,
    },
    /// CIDR-based network rule
    Cidr {
        /// CIDR notation for IP range
        cidr: String,
    },
}

/// Environment variable access permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentPermissions {
    /// List of allowed environment variables
    #[serde(default)]
    pub allow: Vec<EnvironmentRule>,

    /// List of denied environment variables
    #[serde(default)]
    pub deny: Vec<EnvironmentRule>,
}

/// Individual environment variable rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentRule {
    /// Environment variable name/key
    pub key: String,
}

/// Resource limits for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Resource limit values configuration
    pub limits: ResourceLimitValues,
}

/// Actual resource limit values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimitValues {
    /// CPU limit (e.g., "100m" for 100 millicores, "0.5" for half a core)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,

    /// Memory limit (e.g., "128Mi", "1Gi")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,

    /// Execution time limit (e.g., "30s", "5m", "1000ms")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time: Option<String>,

    /// WebAssembly fuel limit for instruction counting
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuel: Option<u64>,

    /// Legacy memory limit field (deprecated, use 'memory' instead)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<String>,
}

impl Policy {
    /// Load policy from YAML string
    pub fn from_yaml(yaml: &str) -> crate::error::Result<Self> {
        serde_yaml::from_str(yaml).map_err(|e| crate::error::PolicyError::ParseError(e.to_string()))
    }

    /// Load policy from JSON string
    pub fn from_json(json: &str) -> crate::error::Result<Self> {
        serde_json::from_str(json).map_err(|e| crate::error::PolicyError::ParseError(e.to_string()))
    }

    /// Validate policy structure
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.version.is_empty() {
            return Err(crate::error::PolicyError::ValidationError(
                "Policy version is required".to_string(),
            ));
        }

        if !self.version.starts_with("1.") {
            return Err(crate::error::PolicyError::ValidationError(format!(
                "Unsupported policy version: {}",
                self.version
            )));
        }

        Ok(())
    }

    /// Merge with another policy (for inheritance)
    pub fn merge(&mut self, other: Policy) -> crate::error::Result<()> {
        // Merge storage permissions
        if let Some(other_storage) = other.core.storage {
            match &mut self.core.storage {
                Some(storage) => {
                    storage.allow.extend(other_storage.allow);
                    storage.deny.extend(other_storage.deny);
                }
                None => self.core.storage = Some(other_storage),
            }
        }

        // Merge network permissions
        if let Some(other_network) = other.core.network {
            match &mut self.core.network {
                Some(network) => {
                    network.allow.extend(other_network.allow);
                    network.deny.extend(other_network.deny);
                }
                None => self.core.network = Some(other_network),
            }
        }

        // Merge environment permissions
        if let Some(other_env) = other.core.environment {
            match &mut self.core.environment {
                Some(env) => {
                    env.allow.extend(other_env.allow);
                    env.deny.extend(other_env.deny);
                }
                None => self.core.environment = Some(other_env),
            }
        }

        // Resource limits use the most restrictive values
        if let Some(other_resources) = other.core.resources {
            match &mut self.core.resources {
                Some(_) => {
                    // Keep existing limits (more restrictive)
                }
                None => self.core.resources = Some(other_resources),
            }
        }

        // Merge extensions
        self.extensions.extend(other.extensions);

        Ok(())
    }
}
