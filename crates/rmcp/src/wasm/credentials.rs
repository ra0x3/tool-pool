//! Credential provider interface for WASM tools

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;

use super::manifest::CredentialRequirement;

/// Credential values that can be provided to WASM tools
#[derive(Debug, Clone)]
pub enum CredentialValue {
    /// OAuth2 access token
    OAuth2Token(String),

    /// API key
    ApiKey(String),

    /// HTTP Basic authentication credentials
    BasicAuth { username: String, password: String },

    /// Bearer token
    BearerToken(String),

    /// Custom credential as JSON
    Custom(serde_json::Value),
}

impl CredentialValue {
    /// Convert the credential to environment variable value(s)
    pub fn to_env_value(&self) -> String {
        match self {
            CredentialValue::OAuth2Token(token) => token.clone(),
            CredentialValue::ApiKey(key) => key.clone(),
            CredentialValue::BasicAuth { username, password } => {
                // For basic auth, we'll use the standard format
                format!("{}:{}", username, password)
            }
            CredentialValue::BearerToken(token) => token.clone(),
            CredentialValue::Custom(value) => value.to_string(),
        }
    }

    /// Get additional environment variables for complex credential types
    pub fn additional_env_vars(&self, base_name: &str) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        if let CredentialValue::BasicAuth { username, password } = self {
            // For basic auth, also provide separate username/password vars
            vars.insert(format!("{}_USERNAME", base_name), username.clone());
            vars.insert(format!("{}_PASSWORD", base_name), password.clone());
        }

        vars
    }
}

/// Error type for credential resolution
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("Credential not found: {0}")]
    NotFound(String),

    #[error("Credential type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("Invalid credential format: {0}")]
    InvalidFormat(String),

    #[error("Provider error: {0}")]
    ProviderError(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),
}

/// Trait for providing credentials to WASM tools
#[async_trait]
pub trait CredentialProvider: Send + Sync {
    /// Resolve a credential requirement
    async fn resolve(
        &self,
        requirement: &CredentialRequirement,
    ) -> Result<CredentialValue, CredentialError>;

    /// Check if a credential is available without fetching it
    async fn has_credential(&self, name: &str) -> bool {
        // Default implementation - try to resolve and see if it succeeds
        // Implementors can override for more efficient checking
        let dummy_req = CredentialRequirement {
            name: name.to_string(),
            credential_type: super::manifest::CredentialType::ApiKey,
            required: false,
            env_var: None,
            description: None,
        };
        self.resolve(&dummy_req).await.is_ok()
    }
}

/// A simple in-memory credential provider for testing
#[derive(Debug, Clone, Default)]
pub struct InMemoryCredentialProvider {
    credentials: Arc<tokio::sync::RwLock<HashMap<String, CredentialValue>>>,
}

impl InMemoryCredentialProvider {
    /// Create a new in-memory provider
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a credential
    pub async fn add_credential(&self, name: impl Into<String>, value: CredentialValue) {
        let mut creds = self.credentials.write().await;
        creds.insert(name.into(), value);
    }

    /// Remove a credential
    pub async fn remove_credential(&self, name: &str) -> Option<CredentialValue> {
        let mut creds = self.credentials.write().await;
        creds.remove(name)
    }

    /// Clear all credentials
    pub async fn clear(&self) {
        let mut creds = self.credentials.write().await;
        creds.clear();
    }
}

#[async_trait]
impl CredentialProvider for InMemoryCredentialProvider {
    async fn resolve(
        &self,
        requirement: &CredentialRequirement,
    ) -> Result<CredentialValue, CredentialError> {
        let creds = self.credentials.read().await;

        let value = creds
            .get(&requirement.name)
            .ok_or_else(|| CredentialError::NotFound(requirement.name.clone()))?;

        // Validate type matches if possible
        match (&requirement.credential_type, value) {
            (
                super::manifest::CredentialType::OAuth2 { .. },
                CredentialValue::OAuth2Token(_),
            )
            | (super::manifest::CredentialType::ApiKey, CredentialValue::ApiKey(_))
            | (
                super::manifest::CredentialType::BasicAuth,
                CredentialValue::BasicAuth { .. },
            )
            | (
                super::manifest::CredentialType::BearerToken,
                CredentialValue::BearerToken(_),
            )
            | (
                super::manifest::CredentialType::Custom { .. },
                CredentialValue::Custom(_),
            ) => Ok(value.clone()),
            _ => {
                // Type mismatch - for now we'll be lenient and convert
                // In production, you might want to be stricter
                Ok(value.clone())
            }
        }
    }

    async fn has_credential(&self, name: &str) -> bool {
        let creds = self.credentials.read().await;
        creds.contains_key(name)
    }
}

/// A credential provider that always returns errors (for testing)
#[derive(Debug, Clone)]
pub struct DenyAllCredentialProvider;

#[async_trait]
impl CredentialProvider for DenyAllCredentialProvider {
    async fn resolve(
        &self,
        requirement: &CredentialRequirement,
    ) -> Result<CredentialValue, CredentialError> {
        Err(CredentialError::AccessDenied(format!(
            "Credential '{}' is not available",
            requirement.name
        )))
    }

    async fn has_credential(&self, _name: &str) -> bool {
        false
    }
}

/// A credential provider that chains multiple providers
#[derive(Clone)]
pub struct ChainedCredentialProvider {
    providers: Vec<Arc<dyn CredentialProvider>>,
}

impl ChainedCredentialProvider {
    /// Create a new chained provider
    pub fn new(providers: Vec<Arc<dyn CredentialProvider>>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl CredentialProvider for ChainedCredentialProvider {
    async fn resolve(
        &self,
        requirement: &CredentialRequirement,
    ) -> Result<CredentialValue, CredentialError> {
        for provider in &self.providers {
            match provider.resolve(requirement).await {
                Ok(value) => return Ok(value),
                Err(CredentialError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }

        Err(CredentialError::NotFound(requirement.name.clone()))
    }

    async fn has_credential(&self, name: &str) -> bool {
        for provider in &self.providers {
            if provider.has_credential(name).await {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_provider() {
        let provider = InMemoryCredentialProvider::new();

        provider
            .add_credential("test_key", CredentialValue::ApiKey("secret123".to_string()))
            .await;

        let requirement = CredentialRequirement {
            name: "test_key".to_string(),
            credential_type: super::super::manifest::CredentialType::ApiKey,
            required: true,
            env_var: None,
            description: None,
        };

        let result = provider.resolve(&requirement).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CredentialValue::ApiKey(key) => assert_eq!(key, "secret123"),
            _ => panic!("Wrong credential type"),
        }

        assert!(provider.has_credential("test_key").await);
        assert!(!provider.has_credential("missing_key").await);
    }

    #[test]
    fn test_credential_to_env() {
        let cred = CredentialValue::BasicAuth {
            username: "user".to_string(),
            password: "pass".to_string(),
        };

        assert_eq!(cred.to_env_value(), "user:pass");

        let additional = cred.additional_env_vars("AUTH");
        assert_eq!(additional.get("AUTH_USERNAME"), Some(&"user".to_string()));
        assert_eq!(additional.get("AUTH_PASSWORD"), Some(&"pass".to_string()));
    }
}
