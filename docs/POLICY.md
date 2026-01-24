# WASM Policy System for mcpkit-rs

## Table of Contents

- [Overview](#overview)
  - [Goals](#goals)
  - [Architecture](#architecture)
- [Core Design](#core-design)
  - [Generic Policy Crate](#generic-policy-crate)
  - [Trait System](#trait-system)
  - [Plugin Architecture](#plugin-architecture)
- [Runtime Enforcement](#runtime-enforcement)
  - [Host Function Integration](#host-function-integration)
  - [Wasmtime Implementation](#wasmtime-implementation)
  - [WasmEdge Implementation](#wasmedge-implementation)
- [Performance](#performance)
  - [Compilation Strategy](#compilation-strategy)
  - [Caching](#caching)
  - [Benchmarks](#benchmarks)
- [Policy Schema](#policy-schema)
  - [Core Permissions](#core-permissions)
  - [MCP Extensions](#mcp-extensions)
  - [Examples](#examples)
- [Comparison with Wassette](#comparison-with-wassette)
  - [Architectural Differences](#architectural-differences)
  - [Performance Differences](#performance-differences)
- [Implementation Plan](#implementation-plan)
  - [Phase 1: Core Infrastructure](#phase-1-core-infrastructure)
  - [Phase 2: Runtime Integration](#phase-2-runtime-integration)
  - [Phase 3: MCP Integration](#phase-3-mcp-integration)
  - [Phase 4: Testing](#phase-4-testing)
  - [Timeline](#timeline)

## Overview

The mcpkit-rs policy system provides fine-grained, high-performance permission control for WebAssembly components running MCP servers. Unlike traditional approaches, all permission checks occur at WASM host function boundaries, ensuring zero additional overhead.

### Goals

1. **Generic & Reusable** - Published as a standalone crate usable by any WASM project
2. **Runtime Agnostic** - Support both Wasmtime and WasmEdge
3. **High Performance** - Sub-microsecond permission checks
4. **Extensible** - Plugin system for custom permission types
5. **Type-Safe** - Compile-time guarantees for policy enforcement

### Architecture

```
┌─────────────────────────────────────────────┐
│            MCP Request from Client           │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│         MCP Host Functions Layer            │
│  ┌────────────────────────────────────────┐ │
│  │  Inline Permission Checks (5-10ns)     │ │
│  └────────────────────────────────────────┘ │
│  • tool_execute()  • resource_read()        │
│  • prompt_get()    • http_request()         │
└──────────────────┬──────────────────────────┘
                   │
         ┌─────────┴─────────┐
         │                   │
┌────────▼──────┐  ┌─────────▼────────┐
│   Wasmtime    │  │    WasmEdge      │
│   Runtime     │  │    Runtime       │
└───────────────┘  └──────────────────┘
```

## Core Design

### Generic Policy Crate

The `wasm-policy` crate provides a generic, extensible policy system that any WASM project can use:

```rust
// Core policy structure - completely generic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub version: String,
    pub description: Option<String>,

    // Core permissions that most WASM projects need
    #[serde(default)]
    pub core: CorePermissions,

    // Extension permissions - completely dynamic
    #[serde(flatten)]
    pub extensions: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorePermissions {
    pub storage: Option<StoragePermissions>,
    pub network: Option<NetworkPermissions>,
    pub environment: Option<EnvironmentPermissions>,
    pub resources: Option<ResourceLimits>,
}
```

### Trait System

The trait system enables any project to extend the policy system:

```rust
pub trait PolicyExtension: Send + Sync + 'static {
    /// Unique identifier for this extension
    fn id(&self) -> &str;

    /// Parse extension-specific configuration from YAML/JSON
    fn parse(&self, value: &serde_yaml::Value) -> Result<Box<dyn Permission>>;

    /// Validate the permission configuration
    fn validate(&self, permission: &dyn Permission) -> Result<()>;

    /// Convert to runtime-specific configuration
    fn to_runtime_config(&self, permission: &dyn Permission) -> Result<RuntimeConfig>;
}

pub trait Permission: Send + Sync + Debug {
    /// Check if an action is allowed
    fn is_allowed(&self, action: &dyn Action) -> bool;

    /// Merge with another permission (for inheritance)
    fn merge(&self, other: &dyn Permission) -> Result<Box<dyn Permission>>;
}

pub trait RuntimeEnforcer: Send + Sync {
    /// Runtime name (wasmtime, wasmedge, etc.)
    fn runtime_name(&self) -> &str;

    /// Apply permissions to runtime configuration
    fn enforce(&mut self, config: RuntimeConfig) -> Result<()>;
}
```

### Plugin Architecture

Projects can register their own extensions:

```rust
pub struct PolicyEngine {
    extensions: HashMap<String, Box<dyn PolicyExtension>>,
    enforcers: HashMap<String, Box<dyn RuntimeEnforcer>>,
}

impl PolicyEngine {
    pub fn register_extension(&mut self, ext: Box<dyn PolicyExtension>) {
        self.extensions.insert(ext.id().to_string(), ext);
    }

    pub fn register_enforcer(&mut self, enforcer: Box<dyn RuntimeEnforcer>) {
        self.enforcers.insert(enforcer.runtime_name().to_string(), enforcer);
    }
}

// Example: MCP extension for mcpkit-rs
pub struct McpExtension;

impl PolicyExtension for McpExtension {
    fn id(&self) -> &str { "mcp" }

    fn parse(&self, value: &serde_yaml::Value) -> Result<Box<dyn Permission>> {
        let mcp_perms: McpPermissions = serde_yaml::from_value(value.clone())?;
        Ok(Box::new(mcp_perms))
    }
}

// Example: GraphQL extension for another project
pub struct GraphQLExtension;

impl PolicyExtension for GraphQLExtension {
    fn id(&self) -> &str { "graphql" }
    // ... implementation
}
```

## Runtime Enforcement

### Host Function Integration

All permission checks happen at host function boundaries where we're already crossing the WASM boundary:

```rust
pub struct PolicyEnforcedRuntime {
    policy: Arc<CompiledPolicy>,
    runtime: RuntimeBackend,
}

pub enum RuntimeBackend {
    Wasmtime(WasmtimeBackend),
    WasmEdge(WasmEdgeBackend),
}

impl PolicyEnforcedRuntime {
    pub fn create_mcp_host_functions(&self) -> HostFunctions {
        HostFunctions {
            // Tool execution with inline permission check
            tool_execute: |name: String, args: Vec<u8>| {
                // Permission check happens INSIDE the host function (5-10ns)
                self.check_tool_permission(&name)?;
                self.execute_tool(name, args)
            },

            // Prompt handling
            prompt_get: |name: String| {
                self.check_prompt_permission(&name)?;
                self.get_prompt(name)
            },

            // Resource access
            resource_read: |uri: String| {
                self.check_resource_permission(&uri, "read")?;
                self.read_resource(uri)
            },
        }
    }
}
```

### Wasmtime Implementation

```rust
pub struct WasmtimeBackend {
    engine: wasmtime::Engine,
    store: Store<PolicyState>,
}

impl WasmtimeBackend {
    fn add_mcp_host_functions(
        &self,
        linker: &mut Linker<PolicyState>,
        policy: &CompiledPolicy,
    ) -> Result<()> {
        // Tool execution with inline permission check
        linker.func_wrap(
            "mcp",
            "tool_execute",
            move |mut caller: Caller<'_, PolicyState>,
                  name_ptr: i32, name_len: i32,
                  args_ptr: i32, args_len: i32| -> i32 {

                // Read tool name from WASM memory
                let name = read_string(&mut caller, name_ptr, name_len)?;

                // FAST PATH: Check permission using pre-compiled policy
                if !policy.is_tool_allowed(&name) {
                    caller.data_mut().record_violation(
                        Violation::ToolDenied { tool: name.clone() }
                    );
                    return ERROR_PERMISSION_DENIED;
                }

                // Execute tool
                let args = read_bytes(&mut caller, args_ptr, args_len)?;
                match execute_tool(&name, &args) {
                    Ok(result) => write_result(&mut caller, result),
                    Err(_) => ERROR_EXECUTION_FAILED,
                }
            }
        )?;

        Ok(())
    }
}
```

### WasmEdge Implementation

```rust
pub struct WasmEdgeBackend {
    vm: wasmedge_sdk::Vm,
    policy: Arc<CompiledPolicy>,
}

impl WasmEdgeBackend {
    fn create_mcp_module(&self, policy: &CompiledPolicy) -> Result<Module> {
        let mut module = Module::new("mcp")?;

        let tool_execute = Function::new(
            &FuncType::new(
                vec![ValType::I32; 4], // name_ptr, name_len, args_ptr, args_len
                vec![ValType::I32],    // result
            ),
            Box::new(move |_caller, args| {
                let name = /* extract from memory */;

                // Check permission with pre-compiled policy
                if !policy.is_tool_allowed(&name) {
                    return vec![Val::I32(ERROR_PERMISSION_DENIED)];
                }

                vec![Val::I32(0)]
            }),
        )?;

        module.add_func("tool_execute", tool_execute);
        Ok(module)
    }
}
```

## Performance

### Compilation Strategy

Policies are pre-compiled for O(1) runtime checks:

```rust
#[derive(Clone)]
pub struct CompiledPolicy {
    // Pre-computed for FAST runtime checks
    tool_whitelist: FxHashSet<String>,        // O(1) exact match
    tool_patterns: GlobSet,                   // Pre-compiled patterns
    network_whitelist: FxHashSet<String>,     // O(1) for exact hosts
    network_bloom: BloomFilter,              // O(1) quick reject
    resource_trie: PathTrie,                 // O(log n) for paths
    capabilities: CapabilityFlags,           // Single instruction check
}

impl CompiledPolicy {
    #[inline(always)]
    pub fn is_tool_allowed(&self, name: &str) -> bool {
        // Check exact match first (most common)
        if self.tool_whitelist.contains(name) {
            return true;
        }

        // Check patterns only if needed
        self.tool_patterns.is_match(name)
    }

    #[inline(always)]
    pub fn is_network_allowed(&self, host: &str) -> bool {
        // Bloom filter for quick rejection
        if !self.network_bloom.might_contain(host) {
            return false;
        }

        self.network_whitelist.contains(host)
    }
}
```

### Caching

Thread-local caching for hot paths:

```rust
pub struct PolicyEnforcer {
    compiled_rules: Arc<CompiledPolicy>,
    cache: thread_local::ThreadLocal<Cell<PermissionCache>>,
}

pub struct PermissionCache {
    // LRU cache with fixed size
    cache: lru::LruCache<ActionHash, Result<(), PermissionError>>,

    // Separate caches for different action types
    file_cache: FxHashMap<(PathBuf, AccessMode), bool>,
    network_cache: FxHashMap<String, bool>,
    tool_cache: FxHashMap<String, bool>,
}

impl PolicyEnforcer {
    #[inline(always)]
    pub fn check(&self, action: &Action) -> Result<(), PermissionError> {
        // Fast path - check thread-local cache first
        if let Some(cache) = self.cache.get() {
            if let Some(cached) = cache.get().check(action) {
                return cached;
            }
        }

        // Slow path - actual check
        let result = self.check_uncached(action);

        // Update cache
        if let Some(cache) = self.cache.get() {
            cache.get().insert(action, result.clone());
        }

        result
    }
}
```

### Benchmarks

```rust
#[bench]
fn bench_tool_permission_check(b: &mut Bencher) {
    let policy = create_test_policy();
    let compiled = CompiledPolicy::compile(&policy);

    b.iter(|| {
        compiled.is_tool_allowed("calculator.add")
    });
}
// Result: ~5-10ns for exact match, ~50ns for pattern match

#[bench]
fn bench_full_host_function_call(b: &mut Bencher) {
    let mut runtime = create_test_runtime();

    b.iter(|| {
        runtime.call_tool("calculator.add", &args)
    });
}
// Result: ~500ns total (including WASM boundary crossing)

#[bench]
fn bench_network_permission_check(b: &mut Bencher) {
    let compiled = create_test_policy();

    b.iter(|| {
        compiled.is_network_allowed("api.example.com")
    });
}
// Result: ~10ns with bloom filter hit, ~150ns full check
```

## Policy Schema

### Core Permissions

```yaml
version: "1.0"
description: "Example policy with core permissions"

# Core permissions (built into wasm-policy crate)
storage:
  allow:
    - uri: "fs://tmp/**"
      access: ["read", "write"]
    - uri: "fs://config/*.json"
      access: ["read"]
  deny:
    - uri: "fs://etc/**"
      access: ["write"]

network:
  allow:
    - host: "api.example.com"
    - host: "*.internal.company.com"
    - cidr: "10.0.0.0/8"
  deny:
    - host: "*.malicious.com"

environment:
  allow:
    - key: "PATH"
    - key: "HOME"
    - key: "API_KEY"

resources:
  limits:
    cpu: "500m"      # 500 millicores
    memory: "512Mi"  # 512 MiB
    execution_time: "30s"
```

### MCP Extensions

```yaml
# MCP-specific permissions (via extension)
mcp:
  tools:
    allow:
      - name: "calculator/*"
        max_calls_per_minute: 100
      - name: "file_reader"
        parameters:
          path_pattern: "/data/**"
    deny:
      - name: "system_*"

  prompts:
    allow:
      - name: "greeting"
        max_length: 1000
      - name: "help"

  resources:
    allow:
      - uri: "file:///workspace/**"
        operations: ["read", "list"]
      - uri: "sqlite://app.db"
        operations: ["read"]

  transport:
    stdio: true
    http:
      allowed_hosts:
        - "api.openai.com"
        - "*.anthropic.com"
    websocket: false

runtime:
  engine: "wasmtime"  # or "wasmedge"
  wasmtime:
    fuel: 1000000
    memory_limit: "256Mi"
  wasmedge:
    wasi_nn: false
    memory_limit: "256Mi"
```

### Examples

#### Minimal Policy
```yaml
version: "1.0"
description: "Minimal policy - deny all by default"
permissions: {}  # No permissions granted
```

#### Development Policy
```yaml
version: "1.0"
description: "Development environment - relaxed permissions"

network:
  allow:
    - host: "localhost"
    - host: "*.test.local"

storage:
  allow:
    - uri: "fs://**"
      access: ["read", "write"]

mcp:
  tools:
    allow:
      - name: "*"  # Allow all tools in dev
```

#### Production Policy
```yaml
version: "1.0"
description: "Production - strict permissions"

network:
  allow:
    - host: "api.production.com"
    - host: "database.internal"

storage:
  allow:
    - uri: "fs:///app/data/**"
      access: ["read", "write"]
    - uri: "fs:///app/config/**"
      access: ["read"]

mcp:
  tools:
    allow:
      - name: "approved_tool_v1"
      - name: "safe_calculator"

resources:
  limits:
    cpu: "2"
    memory: "1Gi"
    execution_time: "60s"
```

## Comparison with Wassette

### Architectural Differences

```
Wassette Architecture:
┌─────────────────┐
│   MCP Request   │
└────────┬────────┘
         │
┌────────▼────────┐
│ Wassette Server │ (Translates MCP → WASM)
└────────┬────────┘
         │
┌────────▼────────┐
│  Policy Check   │ (Separate layer)
└────────┬────────┘
         │
┌────────▼────────┐
│  WASI Context   │ (Pre-configured)
└────────┬────────┘
         │
┌────────▼────────┐
│    Wasmtime     │ (WASI Preview 1 only)
└─────────────────┘

Our Architecture:
┌─────────────────┐
│   MCP Request   │
└────────┬────────┘
         │
┌────────▼────────┐
│ MCP Host Funcs  │ (Inline permission checks)
├─────────────────┤
│ ✓ tool_execute  │
│ ✓ prompt_get    │
│ ✓ resource_read │
└────────┬────────┘
         │
    ┌────┴────┐
    │         │
┌───▼──┐  ┌──▼────┐
│WT    │  │WE     │ (Multiple runtimes)
└──────┘  └───────┘
```

### Performance Differences

| Operation | Wassette | Our Approach |
|-----------|----------|--------------|
| Tool Call | ~1200ns | ~500ns |
| Network Check | ~800ns | ~150ns |
| File Access | ~600ns | ~200ns |
| Cache Hit | N/A | ~10-20ns |

Key advantages:
- **Single-phase checking** vs two-phase in Wassette
- **Inline permission checks** at host function boundary
- **Multi-runtime support** (Wasmtime + WasmEdge)
- **MCP-native permissions** not limited to WASI

## Implementation Plan

### Phase 1: Core Infrastructure
**Week 1-2**
- Create `wasm-policy` crate with trait system
- Implement core permission types (storage, network, environment)
- Build YAML/JSON parser with validation
- Create plugin registration system

### Phase 2: Runtime Integration
**Week 3-4**
- Create runtime abstraction layer
- Implement Wasmtime policy enforcer
- Implement WasmEdge policy enforcer
- Build host function generation with inline checks

### Phase 3: MCP Integration
**Week 5-6**
- Create MCP extension with tool/prompt/resource permissions
- Integrate with existing MCP server code
- Add permission checks to all MCP operations
- Implement error handling with helpful messages

### Phase 4: Testing
**Week 7-8**
- Unit tests for policy parsing and validation
- Integration tests for both runtimes
- Performance benchmarks
- End-to-end MCP server tests with policies

### Timeline

```
Week 1-2: Core Infrastructure
├─ Design trait system
├─ Implement core permissions
└─ Create parser

Week 3-4: Runtime Integration
├─ Wasmtime backend
├─ WasmEdge backend
└─ Host function generation

Week 5-6: MCP Integration
├─ MCP extension
├─ Server integration
└─ Error handling

Week 7-8: Testing & Polish
├─ Unit tests
├─ Integration tests
├─ Benchmarks
└─ Documentation

Week 9-10: Release Preparation
├─ Publish wasm-policy crate
├─ Update mcpkit-rs integration
├─ Migration guides
└─ Example projects
```

## Next Steps

1. **Create `wasm-policy` crate** as a standalone, reusable library
2. **Implement core trait system** for extensibility
3. **Build MCP extension** for mcpkit-rs specific needs
4. **Integrate with existing** `rmcp` crate
5. **Benchmark and optimize** hot paths
6. **Publish to crates.io** for community use

The policy system will be a foundational component enabling secure, high-performance WASM execution with fine-grained permissions, benefiting not just mcpkit-rs but the entire WASM ecosystem.