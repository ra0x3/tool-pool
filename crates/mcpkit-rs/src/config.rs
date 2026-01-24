//! Configuration integration for mcpkit-rs

use std::sync::Arc;

#[cfg(feature = "config")]
use mcpkit_rs_config::{Config, RuntimeType, TransportType};
#[cfg(feature = "config")]
use mcpkit_rs_policy::{CompiledPolicy, PolicyEngine};

/// Server configuration with policy enforcement
#[cfg(feature = "config")]
pub struct ServerConfig {
    pub config: Arc<Config>,
    pub policy_engine: Arc<PolicyEngine>,
    pub compiled_policy: Option<Arc<CompiledPolicy>>,
}

#[cfg(feature = "config")]
impl ServerConfig {
    /// Load configuration from file
    pub async fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config = Config::from_yaml_file(path)?;
        Self::from_config(config).await
    }

    /// Create from a Config instance
    pub async fn from_config(config: Config) -> Result<Self, Box<dyn std::error::Error>> {
        let policy_engine = PolicyEngine::new();

        let compiled_policy = if let Some(ref policy) = config.policy {
            Some(Arc::new(CompiledPolicy::compile(policy)?))
        } else {
            None
        };

        if let Some(ref policy) = config.policy {
            policy_engine.load_policy(policy.clone()).await?;
        }

        Ok(Self {
            config: Arc::new(config),
            policy_engine: Arc::new(policy_engine),
            compiled_policy,
        })
    }

    /// Get server bind address
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.config.server.bind, self.config.server.port)
    }

    /// Check if debug mode is enabled
    pub fn is_debug(&self) -> bool {
        self.config.server.debug
    }

    /// Get log level
    pub fn log_level(&self) -> &str {
        self.config.server.log_level.as_deref().unwrap_or("info")
    }

    /// Get transport type
    pub fn transport_type(&self) -> &TransportType {
        &self.config.transport.transport_type
    }

    /// Get runtime type
    pub fn runtime_type(&self) -> &RuntimeType {
        &self.config.runtime.runtime_type
    }

    /// Create WASM context from config
    #[cfg(feature = "wasm-tools")]
    pub fn create_wasm_context(&self) -> crate::wasm::WasmContext {
        let mut ctx = crate::wasm::WasmContext::new();

        // Apply runtime limits
        if let Some(ref limits) = self.config.runtime.limits {
            if let Some(ref exec_time) = limits.execution_time {
                if let Some(duration) = parse_duration(exec_time) {
                    ctx.timeout = duration;
                }
            }

            if let Some(ref memory) = limits.memory {
                if let Some(bytes) = parse_memory_limit(memory) {
                    ctx.max_memory_bytes = bytes;
                }
            }
        }

        // Apply WASM-specific settings
        if let Some(ref wasm) = self.config.runtime.wasm {
            if let Some(fuel) = wasm.fuel {
                ctx.max_fuel = Some(fuel);
            }

            if let Some(memory_pages) = wasm.memory_pages {
                ctx.max_memory_bytes = (memory_pages as usize) * 65536;
            }
        }

        // Apply environment variables from policy
        if let Some(ref policy) = self.config.policy {
            if let Some(ref env) = policy.core.environment {
                for rule in &env.allow {
                    // Add allowed environment variables
                    if let Ok(value) = std::env::var(&rule.key) {
                        ctx.env_vars.insert(rule.key.clone(), value);
                    }
                }
            }
        }

        ctx
    }

    /// Check if a tool is allowed by policy
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        if let Some(ref policy) = self.compiled_policy {
            policy.is_tool_allowed(tool_name)
        } else {
            true // No policy means everything is allowed
        }
    }

    /// Check if network access is allowed
    pub fn is_network_allowed(&self, host: &str) -> bool {
        if let Some(ref policy) = self.compiled_policy {
            policy.is_network_allowed(host)
        } else {
            true
        }
    }

    /// Check if file access is allowed
    pub fn is_storage_allowed(&self, path: &str, operation: &str) -> bool {
        if let Some(ref policy) = self.compiled_policy {
            policy.is_storage_allowed(path, operation)
        } else {
            true
        }
    }
}

/// Parse duration string (e.g., "30s", "5m", "1h")
#[cfg(feature = "wasm-tools")]
fn parse_duration(s: &str) -> Option<std::time::Duration> {
    if let Some(stripped) = s.strip_suffix("ms") {
        stripped
            .parse::<u64>()
            .ok()
            .map(std::time::Duration::from_millis)
    } else if let Some(stripped) = s.strip_suffix('s') {
        stripped
            .parse::<u64>()
            .ok()
            .map(std::time::Duration::from_secs)
    } else if let Some(stripped) = s.strip_suffix('m') {
        stripped
            .parse::<u64>()
            .ok()
            .map(|m| std::time::Duration::from_secs(m * 60))
    } else if let Some(stripped) = s.strip_suffix('h') {
        stripped
            .parse::<u64>()
            .ok()
            .map(|h| std::time::Duration::from_secs(h * 3600))
    } else {
        None
    }
}

/// Parse memory limit string (e.g., "512Mi", "2Gi")
#[cfg(feature = "wasm-tools")]
fn parse_memory_limit(s: &str) -> Option<usize> {
    if let Some(stripped) = s.strip_suffix("Ki") {
        stripped.parse::<usize>().ok().map(|v| v * 1024)
    } else if let Some(stripped) = s.strip_suffix("Mi") {
        stripped.parse::<usize>().ok().map(|v| v * 1024 * 1024)
    } else if let Some(stripped) = s.strip_suffix("Gi") {
        stripped
            .parse::<usize>()
            .ok()
            .map(|v| v * 1024 * 1024 * 1024)
    } else {
        s.parse::<usize>().ok()
    }
}
