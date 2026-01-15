//! WASM tool execution support for MCP servers
//!
//! This module provides the ability to load and execute WebAssembly-based tools
//! alongside native Rust tool handlers. WASM tools run in isolated sandboxed
//! environments with controlled access to credentials and resources.

pub mod credentials;
pub mod executor;
pub mod integration;
pub mod loader;
pub mod manifest;
pub mod runtime;

pub use credentials::{CredentialProvider, CredentialValue};
pub use executor::WasmToolExecutor;
pub use integration::{
    CompositeToolHandler, WasmToolHandler, load_wasm_tools_from_directory,
};
pub use loader::{LoadedWasmTool, WasmToolRegistry};
pub use manifest::{CredentialRequirement, CredentialType, WasmToolManifest};
pub use runtime::{WasmContext, WasmRuntime};

use crate::ErrorData;

/// Errors that can occur during WASM tool operations
#[derive(Debug, thiserror::Error)]
pub enum WasmError {
    #[error("Failed to load WASM module: {0}")]
    LoadError(String),

    #[error("Failed to compile WASM module: {0}")]
    CompileError(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("Credential resolution failed: {0}")]
    CredentialError(String),

    #[error("Tool execution timeout")]
    Timeout,

    #[error("Invalid manifest: {0}")]
    ManifestError(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Invalid output from WASM tool: {0}")]
    InvalidOutput(String),
}

impl From<WasmError> for ErrorData {
    fn from(err: WasmError) -> Self {
        match err {
            WasmError::ToolNotFound(name) => {
                ErrorData::invalid_request(format!("WASM tool not found: {}", name), None)
            }
            WasmError::Timeout => {
                ErrorData::internal_error("WASM tool execution timeout".to_string(), None)
            }
            _ => ErrorData::internal_error(err.to_string(), None),
        }
    }
}
