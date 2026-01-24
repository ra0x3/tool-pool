//! Runtime backend implementations

#[cfg(feature = "wasmtime-backend")]
pub mod wasmtime;

#[cfg(feature = "wasmedge-backend")]
pub mod wasmedge;

use crate::error::Result;

/// Common runtime backend functionality
pub struct PolicyEnforcedRuntime {
    /// Compiled policy for runtime enforcement
    pub policy: std::sync::Arc<crate::compiled::CompiledPolicy>,
    /// Selected runtime backend implementation
    pub backend: RuntimeBackend,
}

/// Available runtime backends
pub enum RuntimeBackend {
    #[cfg(feature = "wasmtime-backend")]
    Wasmtime(wasmtime::WasmtimeBackend),

    #[cfg(feature = "wasmedge-backend")]
    WasmEdge(wasmedge::WasmEdgeBackend),
}

impl PolicyEnforcedRuntime {
    /// Check tool permission
    #[inline(always)]
    pub fn check_tool_permission(&self, name: &str) -> Result<()> {
        if !self.policy.is_tool_allowed(name) {
            return Err(crate::error::PolicyError::PermissionDenied {
                action: "tool_execute".to_string(),
                resource: name.to_string(),
            });
        }
        Ok(())
    }

    /// Check network permission
    #[inline(always)]
    pub fn check_network_permission(&self, host: &str) -> Result<()> {
        if !self.policy.is_network_allowed(host) {
            return Err(crate::error::PolicyError::PermissionDenied {
                action: "network_access".to_string(),
                resource: host.to_string(),
            });
        }
        Ok(())
    }

    /// Check storage permission
    #[inline(always)]
    pub fn check_storage_permission(&self, path: &str, operation: &str) -> Result<()> {
        if !self.policy.is_storage_allowed(path, operation) {
            return Err(crate::error::PolicyError::PermissionDenied {
                action: format!("storage_{}", operation),
                resource: path.to_string(),
            });
        }
        Ok(())
    }

    /// Check environment permission
    #[inline(always)]
    pub fn check_env_permission(&self, key: &str) -> Result<()> {
        if !self.policy.is_env_allowed(key) {
            return Err(crate::error::PolicyError::PermissionDenied {
                action: "env_read".to_string(),
                resource: key.to_string(),
            });
        }
        Ok(())
    }
}
