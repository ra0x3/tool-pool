//! Wasmtime runtime backend implementation

use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    core::{HostFunction, HostFunctions, PolicyState, RuntimeConfig, RuntimeEnforcer, Violation},
    error::Result,
};

/// Wasmtime backend for policy enforcement
pub struct WasmtimeBackend {
    #[cfg(feature = "wasmtime-backend")]
    engine: wasmtime::Engine,
    policy_state: Arc<PolicyState>,
}

impl WasmtimeBackend {
    /// Create a new Wasmtime backend
    pub fn new() -> Result<Self> {
        #[cfg(feature = "wasmtime-backend")]
        {
            let engine = wasmtime::Engine::default();
            let policy_state = Arc::new(PolicyState {
                policy: Arc::new(crate::compiled::CompiledPolicy::compile(
                    &crate::permissions::Policy {
                        version: "1.0".to_string(),
                        description: None,
                        core: Default::default(),
                        extensions: Default::default(),
                    },
                )?),
                violations: Arc::new(tokio::sync::Mutex::new(Vec::new())),
                metrics: Arc::new(crate::core::Metrics::default()),
            });

            Ok(WasmtimeBackend {
                engine,
                policy_state,
            })
        }

        #[cfg(not(feature = "wasmtime-backend"))]
        {
            Err(crate::error::PolicyError::RuntimeError(
                "Wasmtime backend not enabled".to_string(),
            ))
        }
    }

    #[cfg(feature = "wasmtime-backend")]
    fn add_mcp_host_functions(
        &self,
        linker: &mut wasmtime::Linker<PolicyState>,
        policy: Arc<crate::compiled::CompiledPolicy>,
    ) -> Result<()> {
        use wasmtime::Caller;

        let policy_clone = policy.clone();
        linker.func_wrap(
            "mcp",
            "tool_execute",
            move |mut caller: Caller<'_, PolicyState>,
                  name_ptr: i32,
                  name_len: i32,
                  _args_ptr: i32,
                  _args_len: i32|
                  -> i32 {
                let name = {
                    let memory = match caller.get_export("memory") {
                        Some(wasmtime::Extern::Memory(mem)) => mem,
                        _ => return -1,
                    };

                    let data = memory.data(&caller);
                    let name_bytes = &data[name_ptr as usize..(name_ptr + name_len) as usize];
                    match std::str::from_utf8(name_bytes) {
                        Ok(s) => s.to_string(),
                        Err(_) => return -1,
                    }
                };

                if !policy_clone.is_tool_allowed(&name) {
                    let state = caller.data_mut();
                    let violation = Violation::ToolDenied {
                        tool: name.clone(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };

                    let violations = state.violations.clone();
                    let metrics = state.metrics.clone();
                    tokio::spawn(async move {
                        let mut v = violations.lock().await;
                        v.push(violation);
                        metrics
                            .total_violations
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    });

                    return -2;
                }

                0
            },
        )?;

        Ok(())
    }
}

#[async_trait]
impl RuntimeEnforcer for WasmtimeBackend {
    fn runtime_name(&self) -> &str {
        "wasmtime"
    }

    async fn enforce(&mut self, config: RuntimeConfig) -> Result<()> {
        if !self.is_compatible(&config) {
            return Err(crate::error::PolicyError::IncompatibleRuntime {
                runtime: self.runtime_name().to_string(),
            });
        }

        // Apply configuration
        self.policy_state = Arc::new(PolicyState {
            policy: Arc::new(crate::compiled::CompiledPolicy::compile(
                &crate::permissions::Policy {
                    version: "1.0".to_string(),
                    description: None,
                    core: Default::default(),
                    extensions: Default::default(),
                },
            )?),
            violations: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            metrics: Arc::new(crate::core::Metrics::default()),
        });

        Ok(())
    }

    async fn create_host_functions(&self, _config: &RuntimeConfig) -> Result<HostFunctions> {
        let mut functions = Vec::new();

        // Create MCP host functions
        functions.push(HostFunction {
            module: "mcp".to_string(),
            name: "tool_execute".to_string(),
            implementation: Arc::new(|_args| {
                // Implementation would go here
                Ok(vec![])
            }),
        });

        functions.push(HostFunction {
            module: "mcp".to_string(),
            name: "prompt_get".to_string(),
            implementation: Arc::new(|_args| {
                // Implementation would go here
                Ok(vec![])
            }),
        });

        functions.push(HostFunction {
            module: "mcp".to_string(),
            name: "resource_read".to_string(),
            implementation: Arc::new(|_args| {
                // Implementation would go here
                Ok(vec![])
            }),
        });

        Ok(HostFunctions { functions })
    }
}

impl Default for WasmtimeBackend {
    fn default() -> Self {
        Self::new().expect("Failed to create Wasmtime backend")
    }
}
