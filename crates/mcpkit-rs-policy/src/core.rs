//! Core trait definitions for the policy system

use std::{fmt::Debug, sync::Arc};

use async_trait::async_trait;
use serde_yaml::Value;

use crate::error::Result;

/// Type alias for host function implementation
pub type HostFunctionImpl = Arc<dyn Fn(&[u8]) -> Result<Vec<u8>> + Send + Sync>;

/// Represents an action that can be permission-checked
pub trait Action: Send + Sync + Debug {
    /// Returns the type of action (e.g., "tool", "network", "storage")
    fn action_type(&self) -> &str;

    /// Returns the specific resource being accessed
    fn resource(&self) -> &str;

    /// Returns any additional context for this action
    fn context(&self) -> Option<&dyn std::any::Any>;
}

/// Core permission trait for authorization checks
#[async_trait]
pub trait Permission: Send + Sync + Debug {
    /// Check if an action is allowed
    fn is_allowed(&self, action: &dyn Action) -> bool;

    /// Merge with another permission (for inheritance)
    fn merge(&self, other: &dyn Permission) -> Result<Box<dyn Permission>>;

    /// Convert to a cacheable representation
    fn to_cache_key(&self) -> String;

    /// Validate the permission configuration
    fn validate(&self) -> Result<()>;
}

/// Extension system for custom permission types
pub trait PolicyExtension: Send + Sync + 'static {
    /// Unique identifier for this extension
    fn id(&self) -> &str;

    /// Parse extension-specific configuration from YAML/JSON
    fn parse(&self, value: &Value) -> Result<Box<dyn Permission>>;

    /// Validate the permission configuration
    fn validate(&self, permission: &dyn Permission) -> Result<()>;

    /// Convert to runtime-specific configuration
    fn to_runtime_config(&self, permission: &dyn Permission) -> Result<RuntimeConfig>;
}

/// Runtime-specific configuration
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Runtime type (wasmtime, wasmedge, etc.)
    pub runtime: String,

    /// Configuration data specific to the runtime
    pub config: serde_json::Value,

    /// Pre-computed permission flags
    pub flags: CapabilityFlags,
}

/// Capability flags for single-instruction permission checks
#[derive(Debug, Clone, Copy, Default)]
pub struct CapabilityFlags {
    /// Can execute tools
    pub can_execute_tools: bool,

    /// Can access network
    pub can_access_network: bool,

    /// Can access filesystem
    pub can_access_filesystem: bool,

    /// Can read environment variables
    pub can_read_environment: bool,

    /// Can access resources
    pub can_access_resources: bool,

    /// Custom flags (bitfield for extension-specific capabilities)
    pub custom_flags: u64,
}

impl CapabilityFlags {
    /// Check if all required flags are set
    #[inline(always)]
    pub fn has_all(&self, required: CapabilityFlags) -> bool {
        self.can_execute_tools >= required.can_execute_tools
            && self.can_access_network >= required.can_access_network
            && self.can_access_filesystem >= required.can_access_filesystem
            && self.can_read_environment >= required.can_read_environment
            && self.can_access_resources >= required.can_access_resources
            && (self.custom_flags & required.custom_flags) == required.custom_flags
    }

    /// Check if any of the required flags are set
    #[inline(always)]
    pub fn has_any(&self, required: CapabilityFlags) -> bool {
        (self.can_execute_tools && required.can_execute_tools)
            || (self.can_access_network && required.can_access_network)
            || (self.can_access_filesystem && required.can_access_filesystem)
            || (self.can_read_environment && required.can_read_environment)
            || (self.can_access_resources && required.can_access_resources)
            || (self.custom_flags & required.custom_flags) != 0
    }
}

/// Runtime enforcer trait for applying permissions
#[async_trait]
pub trait RuntimeEnforcer: Send + Sync {
    /// Runtime name (wasmtime, wasmedge, etc.)
    fn runtime_name(&self) -> &str;

    /// Apply permissions to runtime configuration
    async fn enforce(&mut self, config: RuntimeConfig) -> Result<()>;

    /// Create host functions with embedded permission checks
    async fn create_host_functions(&self, config: &RuntimeConfig) -> Result<HostFunctions>;

    /// Check if runtime is compatible with given configuration
    fn is_compatible(&self, config: &RuntimeConfig) -> bool {
        self.runtime_name() == config.runtime
    }
}

/// Container for host functions with embedded permission checks
pub struct HostFunctions {
    /// Functions exposed to WASM modules
    pub functions: Vec<HostFunction>,
}

/// Individual host function with permission checks
pub struct HostFunction {
    /// Module name (e.g., "mcp", "wasi")
    pub module: String,

    /// Function name (e.g., "tool_execute", "fd_write")
    pub name: String,

    /// Function implementation with inline permission checks
    pub implementation: HostFunctionImpl,
}

/// Policy enforcement state for tracking violations
#[derive(Debug, Clone)]
pub struct PolicyState {
    /// Current policy being enforced
    pub policy: Arc<crate::compiled::CompiledPolicy>,

    /// Recorded violations for audit
    pub violations: Arc<tokio::sync::Mutex<Vec<Violation>>>,

    /// Performance metrics
    pub metrics: Arc<Metrics>,
}

/// Security violation record
#[derive(Debug, Clone)]
pub enum Violation {
    /// Tool execution was denied
    ToolDenied {
        /// Name of the denied tool
        tool: String,
        /// Unix timestamp of the violation
        timestamp: u64,
    },

    /// Network access was denied
    NetworkDenied {
        /// Host/URL that was denied
        host: String,
        /// Unix timestamp of the violation
        timestamp: u64,
    },

    /// File access was denied
    FileDenied {
        /// Path to the denied file
        path: String,
        /// Operation that was denied (read/write/execute)
        operation: String,
        /// Unix timestamp of the violation
        timestamp: u64,
    },

    /// Resource limit exceeded
    ResourceLimitExceeded {
        /// Resource type that exceeded limit
        resource: String,
        /// The configured limit
        limit: u64,
        /// The amount requested
        requested: u64,
    },

    /// Custom violation from extensions
    Custom {
        /// Extension that raised the violation
        extension: String,
        /// Violation message
        message: String,
        /// Unix timestamp of the violation
        timestamp: u64,
    },
}

/// Performance metrics for policy enforcement
#[derive(Debug, Default)]
pub struct Metrics {
    /// Total permission checks performed
    pub total_checks: std::sync::atomic::AtomicU64,

    /// Cache hits
    pub cache_hits: std::sync::atomic::AtomicU64,

    /// Cache misses
    pub cache_misses: std::sync::atomic::AtomicU64,

    /// Average check time in nanoseconds
    pub avg_check_time_ns: std::sync::atomic::AtomicU64,

    /// Total violations
    pub total_violations: std::sync::atomic::AtomicU64,
}

impl PolicyState {
    /// Record a violation
    pub async fn record_violation(&self, violation: Violation) {
        let mut violations = self.violations.lock().await;
        violations.push(violation);
        self.metrics
            .total_violations
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}
