//! WASM runtime for executing tools in isolated environments

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use wasmtime::{Config, Engine, Linker, Module, ResourceLimiter, Store, Trap};
use wasmtime_wasi::{
    I32Exit, WasiCtxBuilder,
    pipe::{MemoryInputPipe, MemoryOutputPipe},
    preview1::WasiP1Ctx,
};

use super::{
    WasmError,
    fs::FsPermissionMapper,
    metering::{
        ComputeUnits, DisplayFormat, EnforcementMode, FuelMetrics, FuelUpdate, MemoryLimits,
        MeteringConfig, MeteringMonitor, SamplingStrategy,
    },
};

/// Wrapper for WASI context with output pipes and metering
struct WasiWithPipes {
    wasi: WasiP1Ctx,
    stdout: MemoryOutputPipe,
    #[allow(dead_code)]
    stderr: MemoryOutputPipe,
    limiter: CustomResourceLimiter,
}

/// Custom resource limiter for defense-in-depth
struct CustomResourceLimiter {
    memory_limit: usize,
    memory_used: AtomicU64,
}

impl CustomResourceLimiter {
    fn new(memory_limit: usize) -> Self {
        Self {
            memory_limit,
            memory_used: AtomicU64::new(0),
        }
    }
}

impl ResourceLimiter for CustomResourceLimiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let allowed = desired <= self.memory_limit;
        if allowed {
            self.memory_used.store(desired as u64, Ordering::Relaxed);
        }
        Ok(allowed)
    }

    fn table_growing(
        &mut self,
        _current: u32,
        desired: u32,
        maximum: Option<u32>,
    ) -> wasmtime::Result<bool> {
        let max = maximum.unwrap_or(10);
        Ok(desired <= max)
    }
}

/// Context for WASM execution
pub struct WasmContext {
    /// Input to provide via stdin
    pub stdin: Vec<u8>,

    /// Buffer to collect stdout
    pub stdout: Vec<u8>,

    /// Buffer to collect stderr
    pub stderr: Vec<u8>,

    /// Environment variables
    pub env_vars: HashMap<String, String>,

    /// Execution timeout
    pub timeout: Duration,

    /// Maximum memory in bytes
    pub max_memory_bytes: usize,

    /// Maximum fuel for execution (optional, defaults to memory-based calculation)
    pub max_fuel: Option<u64>,

    /// Metering configuration (if enabled)
    pub metering: Option<MeteringConfig>,

    /// Monitoring channel for real-time updates
    pub monitor: Option<MeteringMonitor>,

    /// Policy for filesystem permissions
    pub policy: Option<Arc<mcpkit_rs_policy::CompiledPolicy>>,
}

impl WasmContext {
    /// Create a new WASM context with default metering enabled
    pub fn new() -> Self {
        let default_metering = MeteringConfig {
            enabled: true,
            max_compute_units: None,
            memory_limits: MemoryLimits::default(),
            enable_monitoring: false,
            sampling: SamplingStrategy::default(),
            display_format: DisplayFormat::default(),
            enforcement: EnforcementMode::Tracking,
        };

        Self {
            stdin: Vec::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
            env_vars: HashMap::new(),
            timeout: Duration::from_secs(30),
            max_memory_bytes: 50 * 1024 * 1024,
            max_fuel: None,
            metering: Some(default_metering),
            monitor: None,
            policy: None,
        }
    }

    /// Set stdin data
    pub fn with_stdin(mut self, data: Vec<u8>) -> Self {
        self.stdin = data;
        self
    }

    /// Add environment variable
    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.env_vars.insert(key, value);
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set maximum fuel
    pub fn with_max_fuel(mut self, max_fuel: u64) -> Self {
        self.max_fuel = Some(max_fuel);
        self
    }

    /// Enable metering with configuration
    pub fn with_metering(mut self, config: MeteringConfig) -> Self {
        // Apply metering limits
        if let Some(max_cu) = config.max_compute_units {
            self.max_fuel = Some(max_cu);
        }

        // Apply memory limits
        self.max_memory_bytes = config.memory_limits.max_memory;

        self.metering = Some(config);
        self
    }

    /// Set monitoring channel
    pub fn with_monitor(mut self, monitor: MeteringMonitor) -> Self {
        self.monitor = Some(monitor);
        self
    }

    /// Disable metering (for backward compatibility or special cases)
    pub fn without_metering(mut self) -> Self {
        self.metering = None;
        self
    }

    /// Set policy for filesystem permissions
    pub fn with_policy(mut self, policy: Arc<mcpkit_rs_policy::CompiledPolicy>) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Build WASI context for preview1
    fn build_wasi(&mut self) -> Result<WasiWithPipes, WasmError> {
        let mut builder = WasiCtxBuilder::new();

        // Set up stdin
        let stdin = MemoryInputPipe::new(self.stdin.clone());
        builder.stdin(stdin);

        // Set up stdout
        let stdout = MemoryOutputPipe::new(self.max_memory_bytes);
        builder.stdout(stdout.clone());

        // Set up stderr
        let stderr = MemoryOutputPipe::new(self.max_memory_bytes);
        builder.stderr(stderr.clone());

        // Set environment variables
        for (key, value) in &self.env_vars {
            builder.env(key, value);
        }

        // Add preopened directories based on policy
        if let Some(policy) = &self.policy {
            let mapper = FsPermissionMapper::new(Some(policy.clone()));
            let preopen_dirs = mapper.get_preopen_dirs();

            for dir in preopen_dirs {
                builder
                    .preopened_dir(
                        &dir.host_path,
                        dir.guest_path.to_string_lossy(),
                        dir.dir_perms,
                        dir.file_perms,
                    )
                    .map_err(|e| {
                        WasmError::RuntimeError(format!("Failed to preopen directory: {}", e))
                    })?;
            }
        }

        // Build the WASI preview1 context
        let wasi = builder.build_p1();

        // Create resource limiter
        let limiter = CustomResourceLimiter::new(self.max_memory_bytes);

        Ok(WasiWithPipes {
            wasi,
            stdout,
            stderr,
            limiter,
        })
    }
}

impl Default for WasmContext {
    fn default() -> Self {
        Self::new()
    }
}

/// WASM runtime for executing tools
pub struct WasmRuntime {
    pub(crate) engine: Engine,
}

impl WasmRuntime {
    /// Create a new WASM runtime
    pub fn new() -> Result<Self, WasmError> {
        let mut config = Config::new();

        // Enable required features
        config.wasm_threads(false);
        config.wasm_simd(true);
        config.wasm_bulk_memory(true);
        config.wasm_multi_value(true);

        // Set resource limits
        config.max_wasm_stack(1024 * 1024); // 1MB stack
        config.async_support(false); // Disable async to avoid runtime conflicts

        // Enable fuel metering for execution limits
        config.consume_fuel(true);

        let engine = Engine::new(&config)
            .map_err(|e| WasmError::RuntimeError(format!("Failed to create engine: {}", e)))?;

        Ok(Self { engine })
    }

    /// Compile a WASM module
    pub fn compile_module(&self, wasm_bytes: &[u8]) -> Result<Module, WasmError> {
        Module::new(&self.engine, wasm_bytes)
            .map_err(|e| WasmError::CompileError(format!("Failed to compile module: {}", e)))
    }

    /// Execute a WASM module with the given context
    pub async fn execute(
        &self,
        module: &Module,
        context: WasmContext,
    ) -> Result<Vec<u8>, WasmError> {
        let result = self.execute_with_metering(module, context).await?;
        Ok(result.0)
    }

    /// Execute a WASM module with metering support
    pub async fn execute_with_metering(
        &self,
        module: &Module,
        mut context: WasmContext,
    ) -> Result<(Vec<u8>, Option<FuelMetrics>), WasmError> {
        // Build WASI preview1 context with pipes
        let wasi_with_pipes = context.build_wasi()?;

        // Clone the engine for use in spawn_blocking
        let engine = self.engine.clone();
        let module = module.clone();
        // Determine fuel limit based on enforcement mode
        let fuel_limit = if let Some(ref metering) = context.metering {
            match metering.enforcement {
                EnforcementMode::Tracking => {
                    // In tracking mode, use a very high limit to avoid enforcement
                    u64::MAX / 2
                }
                _ => {
                    // Use configured limit or default
                    context.max_fuel.unwrap_or_else(|| {
                        std::cmp::max(1_000_000, (context.max_memory_bytes as u64 / 1024) * 100)
                    })
                }
            }
        } else {
            // Use configured fuel limit or default based on memory
            context.max_fuel.unwrap_or_else(|| {
                std::cmp::max(1_000_000, (context.max_memory_bytes as u64 / 1024) * 100)
            })
        };

        let metering_enabled = context
            .metering
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(false);
        let monitor = context.monitor.take();

        // Use spawn_blocking to avoid tokio runtime conflicts with WASI
        let result = tokio::task::spawn_blocking(
            move || -> Result<(Vec<u8>, Option<FuelMetrics>), WasmError> {
                let start_time = Instant::now();
                // Keep reference to stdout pipe for later
                let stdout_pipe = wasi_with_pipes.stdout.clone();

                // Create store with WASI pipes wrapper
                let mut store = Store::new(&engine, wasi_with_pipes);

                // Apply resource limiter
                store.limiter(|state| &mut state.limiter);

                // Set fuel for execution limits to prevent DOS
                store
                    .set_fuel(fuel_limit)
                    .map_err(|e| WasmError::RuntimeError(format!("Failed to set fuel: {}", e)))?;

                // Create linker and add WASI preview1 functions
                let mut linker: Linker<WasiWithPipes> = Linker::new(&engine);
                // Add WASI functions to the linker
                wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |ctx| &mut ctx.wasi)
                    .map_err(|e| WasmError::RuntimeError(format!("Failed to link WASI: {}", e)))?;

                // Instantiate the module
                let instance = linker.instantiate(&mut store, &module).map_err(|e| {
                    WasmError::RuntimeError(format!("Failed to instantiate: {}", e))
                })?;

                // Get the _start function (WASI convention)
                let start = instance
                    .get_typed_func::<(), ()>(&mut store, "_start")
                    .map_err(|e| {
                        WasmError::RuntimeError(format!("Failed to get _start function: {}", e))
                    })?;

                // Track fuel consumption before execution (if monitoring)
                let _initial_fuel = if metering_enabled {
                    fuel_limit - store.get_fuel().unwrap_or(fuel_limit)
                } else {
                    0
                };

                // Start monitoring task if requested
                if let Some(ref monitor) = monitor {
                    // We can't easily do real-time monitoring in spawn_blocking
                    // So we'll just send start/end updates
                    monitor.send_update(FuelUpdate {
                        consumed: ComputeUnits::new(0),
                        remaining: Some(ComputeUnits::new(fuel_limit)),
                        rate: 0,
                        timestamp: Instant::now(),
                    });
                }

                // Execute the WASM function
                let exec_result = start.call(&mut store, ());

                // Get fuel consumption after execution
                let fuel_consumed = if metering_enabled {
                    fuel_limit - store.get_fuel().unwrap_or(0)
                } else {
                    0
                };

                // Get stdout from the pipe after execution regardless of result
                let output_bytes = stdout_pipe.contents();

                // Calculate metrics if metering enabled
                let metrics = if metering_enabled {
                    let execution_time = start_time.elapsed();
                    let units_per_second = if execution_time.as_secs() > 0 {
                        fuel_consumed / execution_time.as_secs()
                    } else if execution_time.as_nanos() > 0 {
                        (fuel_consumed as u128 * 1_000_000_000 / execution_time.as_nanos()) as u64
                    } else {
                        0
                    };

                    // Send final update if monitoring
                    if let Some(monitor) = monitor {
                        monitor.send_final(FuelUpdate {
                            consumed: ComputeUnits::new(fuel_consumed),
                            remaining: Some(ComputeUnits::new(
                                fuel_limit.saturating_sub(fuel_consumed),
                            )),
                            rate: units_per_second,
                            timestamp: Instant::now(),
                        });
                    }

                    Some(FuelMetrics {
                        compute_units: ComputeUnits::new(fuel_consumed),
                        execution_time,
                        units_per_second,
                        peak_rate: None, // Could be tracked with periodic sampling
                        instruction_count: None, // Wasmtime doesn't expose this directly
                    })
                } else {
                    None
                };

                // Now check if the execution succeeded
                match exec_result {
                    Ok(_) => Ok((output_bytes.to_vec(), metrics)),
                    Err(err) => {
                        if let Some(exit) = err.downcast_ref::<I32Exit>() {
                            if exit.0 == 0 {
                                return Ok((output_bytes.to_vec(), metrics));
                            }
                            return Err(WasmError::RuntimeError(format!(
                                "Execution exited with status {}",
                                exit.0
                            )));
                        }

                        if let Some(trap) = err.downcast_ref::<Trap>() {
                            if trap.to_string().contains("fuel") {
                                return Err(WasmError::RuntimeError(
                                    "Execution exceeded fuel limit (possible DOS attempt)"
                                        .to_string(),
                                ));
                            }
                        }

                        Err(WasmError::RuntimeError(format!(
                            "Execution failed: {}",
                            err
                        )))
                    }
                }
            },
        )
        .await
        .map_err(|e| WasmError::RuntimeError(format!("Failed to spawn blocking task: {}", e)))??;

        Ok(result)
    }

    /// Execute a WASM module from bytes
    pub async fn execute_bytes(
        &self,
        wasm_bytes: &[u8],
        context: WasmContext,
    ) -> Result<Vec<u8>, WasmError> {
        let module = self.compile_module(wasm_bytes)?;
        self.execute(&module, context).await
    }

    /// Execute a WASM module from bytes with metering
    pub async fn execute_bytes_with_metering(
        &self,
        wasm_bytes: &[u8],
        context: WasmContext,
    ) -> Result<(Vec<u8>, Option<FuelMetrics>), WasmError> {
        let module = self.compile_module(wasm_bytes)?;
        self.execute_with_metering(&module, context).await
    }
}

/// Shared runtime instance
pub type SharedRuntime = Arc<WasmRuntime>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::metering::{DisplayFormat, EnforcementMode, MemoryLimits, SamplingStrategy};

    #[test]
    fn test_context_builder() {
        let context = WasmContext::new()
            .with_stdin(b"test input".to_vec())
            .with_env("TEST_VAR".to_string(), "test_value".to_string())
            .with_timeout(Duration::from_secs(60));

        assert_eq!(context.stdin, b"test input");
        assert_eq!(
            context.env_vars.get("TEST_VAR"),
            Some(&"test_value".to_string())
        );
        assert_eq!(context.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_context_with_metering() {
        let metering_config = MeteringConfig {
            enabled: true,
            max_compute_units: Some(5_000_000),
            memory_limits: MemoryLimits {
                max_memory: 1024 * 1024 * 10,      // 10MB
                soft_limit: Some(1024 * 1024 * 8), // 8MB
                max_tables: 5,
                max_instances: 2,
            },
            enable_monitoring: true,
            sampling: SamplingStrategy::default(),
            display_format: DisplayFormat::Detailed,
            enforcement: EnforcementMode::Strict,
        };

        let context = WasmContext::new().with_metering(metering_config.clone());

        assert_eq!(context.max_fuel, Some(5_000_000));
        assert_eq!(context.max_memory_bytes, 10 * 1024 * 1024);
        assert!(context.metering.is_some());

        let metering = context.metering.unwrap();
        assert!(metering.enabled);
        assert_eq!(metering.max_compute_units, Some(5_000_000));
        assert_eq!(metering.enforcement, EnforcementMode::Strict);
    }

    #[test]
    fn test_memory_limits_parsing() {
        // Test valid formats
        assert_eq!(MemoryLimits::parse_size("512Ki").unwrap(), 524_288);
        assert_eq!(MemoryLimits::parse_size("256Mi").unwrap(), 268_435_456);
        assert_eq!(MemoryLimits::parse_size("1Gi").unwrap(), 1_073_741_824);
        assert_eq!(MemoryLimits::parse_size("100").unwrap(), 100);

        // Test invalid formats
        assert!(MemoryLimits::parse_size("abc").is_err());
        assert!(MemoryLimits::parse_size("512Xi").is_err());
    }

    #[test]
    fn test_custom_resource_limiter() {
        let mut limiter = CustomResourceLimiter::new(1024 * 1024); // 1MB

        // Should allow growth within limit
        assert!(limiter.memory_growing(0, 512 * 1024, None).unwrap());
        assert!(
            limiter
                .memory_growing(512 * 1024, 1024 * 1024, None)
                .unwrap()
        );

        // Should deny growth beyond limit
        assert!(
            !limiter
                .memory_growing(1024 * 1024, 2 * 1024 * 1024, None)
                .unwrap()
        );

        // Check memory tracking
        assert_eq!(limiter.memory_used.load(Ordering::Relaxed), 1024 * 1024);
    }

    #[tokio::test]
    async fn test_runtime_creation() {
        let runtime = WasmRuntime::new();
        assert!(runtime.is_ok());
    }

    #[tokio::test]
    async fn test_monitoring_channel() {
        use crate::wasm::metering::FuelUpdate;

        let monitor = MeteringMonitor::new(10);
        let receiver = monitor.receiver.clone();

        // Send some updates
        monitor.send_update(FuelUpdate {
            consumed: ComputeUnits::new(1000),
            remaining: Some(ComputeUnits::new(9000)),
            rate: 1000,
            timestamp: Instant::now(),
        });

        monitor.send_update(FuelUpdate {
            consumed: ComputeUnits::new(2000),
            remaining: Some(ComputeUnits::new(8000)),
            rate: 2000,
            timestamp: Instant::now(),
        });

        monitor.send_final(FuelUpdate {
            consumed: ComputeUnits::new(3000),
            remaining: Some(ComputeUnits::new(7000)),
            rate: 3000,
            timestamp: Instant::now(),
        });

        // Check that we can receive updates
        let mut rx = receiver.lock().await;
        let update1 = rx.recv().await.unwrap();
        assert_eq!(update1.consumed.0, 1000);

        let update2 = rx.recv().await.unwrap();
        assert_eq!(update2.consumed.0, 2000);

        let update3 = rx.recv().await.unwrap();
        assert_eq!(update3.consumed.0, 3000);

        assert!(monitor.is_closed());
    }

    // Note: Full execution tests with metering would require actual WASM modules
    // These would typically be added as test fixtures

    #[tokio::test]
    async fn test_execute_with_metering_simple() {
        // This test would require a simple WASM module
        // For now, we just test that the API works
        let runtime = WasmRuntime::new().unwrap();

        // Simple WASM module that just returns (wat format)
        // (module (func (export "_start")))
        let wasm_bytes = vec![
            0x00, 0x61, 0x73, 0x6d, // Magic
            0x01, 0x00, 0x00,
            0x00, // Version
                  // Minimal module with _start function
        ];

        let context = WasmContext::new().with_metering(MeteringConfig {
            enabled: true,
            max_compute_units: Some(1000),
            ..Default::default()
        });

        // This will fail with invalid WASM but tests the API
        let result = runtime
            .execute_bytes_with_metering(&wasm_bytes, context)
            .await;
        assert!(result.is_err()); // Expected to fail with invalid WASM
    }
}
