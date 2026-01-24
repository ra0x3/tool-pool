//! Policy engine for managing extensions and runtime enforcers

use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use crate::{
    compiled::CompiledPolicy,
    core::{PolicyExtension, RuntimeEnforcer},
    error::Result,
    permissions::Policy,
};

/// Main policy engine that manages extensions and enforcers
pub struct PolicyEngine {
    extensions: HashMap<String, Box<dyn PolicyExtension>>,
    enforcers: HashMap<String, Box<dyn RuntimeEnforcer>>,
    compiled_policy: Arc<RwLock<Option<CompiledPolicy>>>,
}

impl PolicyEngine {
    /// Create a new policy engine
    pub fn new() -> Self {
        PolicyEngine {
            extensions: HashMap::new(),
            enforcers: HashMap::new(),
            compiled_policy: Arc::new(RwLock::new(None)),
        }
    }

    /// Register a policy extension
    pub fn register_extension(&mut self, ext: Box<dyn PolicyExtension>) {
        self.extensions.insert(ext.id().to_string(), ext);
    }

    /// Register a runtime enforcer
    pub fn register_enforcer(&mut self, enforcer: Box<dyn RuntimeEnforcer>) {
        self.enforcers
            .insert(enforcer.runtime_name().to_string(), enforcer);
    }

    /// Load and compile a policy
    pub async fn load_policy(&self, policy: Policy) -> Result<()> {
        policy.validate()?;
        let compiled = CompiledPolicy::compile(&policy)?;

        let mut guard = self.compiled_policy.write().await;
        *guard = Some(compiled);

        Ok(())
    }

    /// Load policy from YAML string
    pub async fn load_policy_yaml(&self, yaml: &str) -> Result<()> {
        let policy = Policy::from_yaml(yaml)?;
        self.load_policy(policy).await
    }

    /// Load policy from JSON string
    pub async fn load_policy_json(&self, json: &str) -> Result<()> {
        let policy = Policy::from_json(json)?;
        self.load_policy(policy).await
    }

    /// Get the compiled policy
    pub async fn get_compiled_policy(&self) -> Option<CompiledPolicy> {
        let guard = self.compiled_policy.read().await;
        guard.clone()
    }

    /// Get a specific extension
    pub fn get_extension(&self, id: &str) -> Option<&dyn PolicyExtension> {
        self.extensions.get(id).map(|ext| ext.as_ref())
    }

    /// Get a specific enforcer
    pub fn get_enforcer(&self, runtime: &str) -> Option<&dyn RuntimeEnforcer> {
        self.enforcers
            .get(runtime)
            .map(|enforcer| enforcer.as_ref())
    }

    /// Apply policy to a runtime
    pub async fn apply_to_runtime(&mut self, runtime_name: &str) -> Result<()> {
        let guard = self.compiled_policy.read().await;
        let compiled = guard.as_ref().ok_or_else(|| {
            crate::error::PolicyError::RuntimeError("No policy loaded".to_string())
        })?;

        let enforcer = self.enforcers.get_mut(runtime_name).ok_or_else(|| {
            crate::error::PolicyError::RuntimeError(format!(
                "No enforcer registered for runtime: {}",
                runtime_name
            ))
        })?;

        // Create runtime config
        let config = crate::core::RuntimeConfig {
            runtime: runtime_name.to_string(),
            config: serde_json::json!({}),
            flags: compiled.capabilities,
        };

        enforcer.enforce(config).await?;

        Ok(())
    }

    /// List all registered extensions
    pub fn list_extensions(&self) -> Vec<String> {
        self.extensions.keys().cloned().collect()
    }

    /// List all registered enforcers
    pub fn list_enforcers(&self) -> Vec<String> {
        self.enforcers.keys().cloned().collect()
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}
