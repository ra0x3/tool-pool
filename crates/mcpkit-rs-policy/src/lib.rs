//! # WASM Policy System for mcpkit-rs
//!
//! A high-performance, extensible policy system for WebAssembly components
//! running MCP servers. Provides fine-grained permission control with
//! sub-microsecond runtime checks.
//!
//! ## Features
//! - Generic and reusable across any WASM project
//! - Runtime agnostic (Wasmtime and WasmEdge support)
//! - Extensible plugin system for custom permissions
//! - Zero-overhead permission checks at host function boundaries
//! - Pre-compiled policies for O(1) runtime checks

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod cache;
pub mod compiled;
pub mod core;
pub mod engine;
pub mod error;
pub mod extensions;
pub mod permissions;
pub mod runtime;

// Re-export main types
pub use core::{Action, Permission, PolicyExtension, RuntimeEnforcer};

pub use compiled::CompiledPolicy;
pub use engine::PolicyEngine;
pub use error::{PolicyError, Result};
pub use permissions::{
    CorePermissions, EnvironmentPermissions, NetworkPermissions, Policy, ResourceLimits,
    StoragePermissions,
};
#[cfg(feature = "wasmedge-backend")]
pub use runtime::wasmedge::WasmEdgeBackend;
// Re-export runtime backends when features are enabled
#[cfg(feature = "wasmtime-backend")]
pub use runtime::wasmtime::WasmtimeBackend;

/// Current version of the policy format
pub const POLICY_VERSION: &str = "1.0";

#[cfg(test)]
mod tests;
