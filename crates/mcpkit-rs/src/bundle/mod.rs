//! Bundle distribution system for mcpkit-rs
//!
//! This module provides functionality for distributing WASM bundles via OCI registries.

use std::path::Path;

use sha2::{Digest, Sha256};

pub mod cache;
pub mod oci;

pub use cache::BundleCache;
pub use oci::{BundleClient, OciError};

use crate::ErrorData;

/// A bundle consisting of WASM module and configuration
#[derive(Debug, Clone)]
pub struct Bundle {
    /// The WASM module bytes
    pub wasm: Vec<u8>,

    /// The configuration YAML bytes
    pub config: Vec<u8>,

    /// Bundle metadata
    pub metadata: BundleMetadata,
}

/// Bundle metadata
#[derive(Debug, Clone)]
pub struct BundleMetadata {
    /// Registry URI
    pub registry: String,

    /// Bundle version
    pub version: String,

    /// SHA256 digest of WASM module
    pub wasm_digest: String,

    /// SHA256 digest of config
    pub config_digest: String,

    /// Pull timestamp
    pub pulled_at: std::time::SystemTime,
}

/// Bundle distribution errors
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("OCI operation failed: {0}")]
    OciError(#[from] OciError),

    #[error("Cache operation failed: {0}")]
    CacheError(#[from] cache::CacheError),

    #[error("Digest mismatch - expected: {expected}, computed: {computed}")]
    DigestMismatch { expected: String, computed: String },

    #[error("Bundle not found: {0}")]
    NotFound(String),

    #[error("Invalid URI: {0}")]
    InvalidUri(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

impl From<BundleError> for ErrorData {
    fn from(err: BundleError) -> Self {
        match err {
            BundleError::NotFound(name) => {
                ErrorData::invalid_request(format!("Bundle not found: {}", name), None)
            }
            BundleError::AuthenticationFailed(msg) => {
                ErrorData::invalid_request(format!("Authentication failed: {}", msg), None)
            }
            _ => ErrorData::internal_error(err.to_string(), None),
        }
    }
}

/// Compute SHA256 digest of content
pub fn compute_digest(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Verify content against expected digest
pub fn verify_digest(content: &[u8], expected: &str) -> Result<(), BundleError> {
    let computed = compute_digest(content);
    if computed != expected {
        return Err(BundleError::DigestMismatch {
            expected: expected.to_string(),
            computed,
        });
    }
    Ok(())
}

/// Parse OCI URI into registry, repository, and tag
pub fn parse_oci_uri(uri: &str) -> Result<(String, String, Option<String>), BundleError> {
    if !uri.starts_with("oci://") {
        return Err(BundleError::InvalidUri(format!(
            "URI must start with 'oci://': {}",
            uri
        )));
    }

    let uri = &uri[6..]; // Remove "oci://" prefix

    // Split into registry and path
    let parts: Vec<&str> = uri.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(BundleError::InvalidUri(format!(
            "Invalid OCI URI format: {}",
            uri
        )));
    }

    let registry = parts[0];
    let path_and_tag = parts[1];

    // Split repository and tag/digest
    let (repository, tag) = if let Some(at_pos) = path_and_tag.rfind('@') {
        // Digest reference (e.g., @sha256:abc123)
        let repo = &path_and_tag[..at_pos];
        let digest = &path_and_tag[at_pos + 1..];
        (repo.to_string(), Some(digest.to_string()))
    } else if let Some(colon_pos) = path_and_tag.rfind(':') {
        // Tag reference (e.g., :v1.0.0)
        let repo = &path_and_tag[..colon_pos];
        let tag = &path_and_tag[colon_pos + 1..];
        (repo.to_string(), Some(tag.to_string()))
    } else {
        // No tag or digest
        (path_and_tag.to_string(), None)
    };

    Ok((registry.to_string(), repository, tag))
}

impl Bundle {
    /// Create a new bundle from WASM and config bytes
    pub fn new(wasm: Vec<u8>, config: Vec<u8>, registry: String, version: String) -> Self {
        let wasm_digest = compute_digest(&wasm);
        let config_digest = compute_digest(&config);

        Self {
            wasm,
            config,
            metadata: BundleMetadata {
                registry,
                version,
                wasm_digest,
                config_digest,
                pulled_at: std::time::SystemTime::now(),
            },
        }
    }

    /// Load bundle from filesystem
    pub fn from_directory(path: &Path) -> Result<Self, BundleError> {
        let wasm_path = path.join("module.wasm");
        let config_path = path.join("config.yaml");
        let metadata_path = path.join("metadata.json");

        if !wasm_path.exists() || !config_path.exists() {
            return Err(BundleError::NotFound(path.display().to_string()));
        }

        let wasm = std::fs::read(&wasm_path)?;
        let config = std::fs::read(&config_path)?;

        // Load metadata if it exists
        let metadata = if metadata_path.exists() {
            let metadata_str = std::fs::read_to_string(&metadata_path)?;
            serde_json::from_str(&metadata_str)
                .map_err(|e| BundleError::ConfigError(e.to_string()))?
        } else {
            // Create default metadata
            BundleMetadata {
                registry: String::new(),
                version: String::new(),
                wasm_digest: compute_digest(&wasm),
                config_digest: compute_digest(&config),
                pulled_at: std::time::SystemTime::now(),
            }
        };

        Ok(Self {
            wasm,
            config,
            metadata,
        })
    }

    /// Save bundle to filesystem
    pub fn save_to_directory(&self, path: &Path) -> Result<(), BundleError> {
        std::fs::create_dir_all(path)?;

        std::fs::write(path.join("module.wasm"), &self.wasm)?;
        std::fs::write(path.join("config.yaml"), &self.config)?;

        let metadata_json = serde_json::to_string_pretty(&self.metadata)
            .map_err(|e| BundleError::ConfigError(e.to_string()))?;
        std::fs::write(path.join("metadata.json"), metadata_json)?;

        Ok(())
    }

    /// Verify bundle integrity
    pub fn verify(&self) -> Result<(), BundleError> {
        verify_digest(&self.wasm, &self.metadata.wasm_digest)?;
        verify_digest(&self.config, &self.metadata.config_digest)?;
        Ok(())
    }
}

// Implement Serialize for BundleMetadata so it can be saved
impl serde::Serialize for BundleMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("BundleMetadata", 5)?;
        state.serialize_field("registry", &self.registry)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("wasm_digest", &self.wasm_digest)?;
        state.serialize_field("config_digest", &self.config_digest)?;

        // Serialize SystemTime as ISO8601 string
        let duration = self
            .pulled_at
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let timestamp = duration.as_secs();
        state.serialize_field("pulled_at", &timestamp)?;

        state.end()
    }
}

// Implement Deserialize for BundleMetadata
impl<'de> serde::Deserialize<'de> for BundleMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Helper {
            registry: String,
            version: String,
            wasm_digest: String,
            config_digest: String,
            pulled_at: u64,
        }

        let helper = Helper::deserialize(deserializer)?;

        Ok(BundleMetadata {
            registry: helper.registry,
            version: helper.version,
            wasm_digest: helper.wasm_digest,
            config_digest: helper.config_digest,
            pulled_at: std::time::UNIX_EPOCH + std::time::Duration::from_secs(helper.pulled_at),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_oci_uri() {
        // Test with tag
        let (registry, repo, tag) = parse_oci_uri("oci://ghcr.io/org/tool:v1.0.0").unwrap();
        assert_eq!(registry, "ghcr.io");
        assert_eq!(repo, "org/tool");
        assert_eq!(tag, Some("v1.0.0".to_string()));

        // Test with digest
        let (registry, repo, tag) =
            parse_oci_uri("oci://docker.io/org/tool@sha256:abc123").unwrap();
        assert_eq!(registry, "docker.io");
        assert_eq!(repo, "org/tool");
        assert_eq!(tag, Some("sha256:abc123".to_string()));

        // Test without tag
        let (registry, repo, tag) = parse_oci_uri("oci://ghcr.io/org/tool").unwrap();
        assert_eq!(registry, "ghcr.io");
        assert_eq!(repo, "org/tool");
        assert_eq!(tag, None);

        // Test invalid URI
        assert!(parse_oci_uri("https://ghcr.io/org/tool").is_err());
    }

    #[test]
    fn test_compute_digest() {
        let content = b"test content";
        let digest = compute_digest(content);
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest.len(), 71); // "sha256:" + 64 hex chars
    }

    #[test]
    fn test_verify_digest() {
        let content = b"test content";
        let digest = compute_digest(content);

        // Valid digest
        assert!(verify_digest(content, &digest).is_ok());

        // Invalid digest
        assert!(verify_digest(b"different content", &digest).is_err());
    }

    #[test]
    fn test_bundle_creation() {
        let wasm = vec![0x00, 0x61, 0x73, 0x6d]; // WASM magic number
        let config = b"version: 1.0".to_vec();

        let bundle = Bundle::new(
            wasm.clone(),
            config.clone(),
            "ghcr.io/test/bundle".to_string(),
            "1.0.0".to_string(),
        );

        assert_eq!(bundle.wasm, wasm);
        assert_eq!(bundle.config, config);
        assert_eq!(bundle.metadata.registry, "ghcr.io/test/bundle");
        assert_eq!(bundle.metadata.version, "1.0.0");

        // Verify integrity
        assert!(bundle.verify().is_ok());
    }
}
