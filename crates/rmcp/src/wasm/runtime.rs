//! WASM runtime for executing tools in isolated environments

use std::{collections::HashMap, sync::Arc, time::Duration};

use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::{
    p2::{
        WasiCtxBuilder,
        pipe::{MemoryInputPipe, MemoryOutputPipe},
    },
    preview1::WasiP1Ctx,
};

use super::WasmError;

/// Wrapper for WASI context with output pipes
struct WasiWithPipes {
    wasi: WasiP1Ctx,
    stdout: MemoryOutputPipe,
    #[allow(dead_code)]
    stderr: MemoryOutputPipe,
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
}

impl WasmContext {
    /// Create a new WASM context
    pub fn new() -> Self {
        Self {
            stdin: Vec::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
            env_vars: HashMap::new(),
            timeout: Duration::from_secs(30),
            max_memory_bytes: 50 * 1024 * 1024,
            max_fuel: None,
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

        // Note: In wasmtime 28, clock access is configured differently
        // The new API doesn't use ambient_authority() anymore

        // Build the preview1 context
        let wasi = builder.build_p1();

        Ok(WasiWithPipes {
            wasi,
            stdout,
            stderr,
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
        mut context: WasmContext,
    ) -> Result<Vec<u8>, WasmError> {
        // Build WASI preview1 context with pipes
        let wasi_with_pipes = context.build_wasi()?;

        // Clone the engine for use in spawn_blocking
        let engine = self.engine.clone();
        let module = module.clone();
        // Use configured fuel limit or default based on memory
        let fuel_limit = context.max_fuel.unwrap_or_else(|| {
            // Default: 100 fuel per KB of memory, with minimum of 1M
            std::cmp::max(1_000_000, (context.max_memory_bytes as u64 / 1024) * 100)
        });

        // Use spawn_blocking to avoid tokio runtime conflicts with WASI
        let result = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, WasmError> {
            // Create store with WASI and pipes
            let mut store = Store::new(&engine, wasi_with_pipes);

            // Set fuel for execution limits to prevent DOS
            store
                .set_fuel(fuel_limit)
                .map_err(|e| WasmError::RuntimeError(format!("Failed to set fuel: {}", e)))?;

            // Create linker and add WASI preview 1 functions
            let mut linker = Linker::new(&engine);
            // For WASI preview 1 with modules (not components)
            wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |ctx: &mut WasiWithPipes| {
                &mut ctx.wasi
            })
            .map_err(|e| WasmError::RuntimeError(format!("Failed to link WASI: {}", e)))?;

            // Instantiate the module
            let instance = linker
                .instantiate(&mut store, &module)
                .map_err(|e| WasmError::RuntimeError(format!("Failed to instantiate: {}", e)))?;

            // Get the _start function (WASI convention)
            let start = instance
                .get_typed_func::<(), ()>(&mut store, "_start")
                .map_err(|e| {
                    WasmError::RuntimeError(format!("Failed to get _start function: {}", e))
                })?;

            // Execute the WASM function
            let exec_result = start.call(&mut store, ());

            // Get stdout from the pipe after execution regardless of result
            let wasi_with_pipes = store.data();
            let output_bytes = wasi_with_pipes.stdout.contents();

            // Now check if the execution succeeded
            match exec_result {
                Ok(_) => Ok(output_bytes.to_vec()),
                Err(e) => {
                    // Check if it's a fuel exhaustion error
                    if let Some(trap) = e.downcast_ref::<wasmtime::Trap>() {
                        if trap.to_string().contains("fuel") {
                            return Err(WasmError::RuntimeError(
                                "Execution exceeded fuel limit (possible DOS attempt)".to_string(),
                            ));
                        }
                    }
                    Err(WasmError::RuntimeError(format!("Execution failed: {}", e)))
                }
            }
        })
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
}

/// Shared runtime instance
pub type SharedRuntime = Arc<WasmRuntime>;

#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    async fn test_runtime_creation() {
        let runtime = WasmRuntime::new();
        assert!(runtime.is_ok());
    }

    // Note: Full execution tests would require actual WASM modules
    // These would typically be added as test fixtures
}
