# Filesystem Permission Mapping

This document describes how mcpkit-rs maps policy-defined filesystem permissions to WASI (WebAssembly System Interface) permissions.

## Overview

The filesystem permission system translates high-level policy permissions (`"read"`, `"write"`, `"execute"`) into WASI-specific directory and file permissions that control what WASM modules can access.

## Implementation Components

### 1. Permission Mapper (`crates/mcpkit-rs/src/wasm/fs.rs`)

The `FsPermissionMapper` struct handles the translation from policy to WASI permissions:

- Maps policy access strings to WASI `DirPerms` and `FilePerms`
- Extracts directories to preopen from policy patterns
- Converts glob patterns to actual directory paths

### 2. WASM Context Integration (`crates/mcpkit-rs/src/wasm/runtime.rs`)

The `WasmContext` now supports policy-based filesystem access:

- Accepts an optional `CompiledPolicy` via `with_policy()`
- Automatically preopens directories based on policy during WASI context creation
- Applies appropriate permissions to each preopened directory

### 3. Configuration Integration (`crates/mcpkit-rs/src/config.rs`)

The `ServerConfig` automatically applies filesystem policies when creating WASM contexts.

## Permission Mapping

| Policy Operation | WASI DirPerms | WASI FilePerms | Allowed Actions |
|-----------------|---------------|----------------|-----------------|
| `"read"` | READ | READ | List directories, read files |
| `"write"` | READ \| MUTATE | READ \| WRITE | Create/delete files, modify content |
| `"execute"` | READ | READ | (Not distinct in WASI) |

## Filesystem Operations

| Operation | Required Policy | WASI Calls |
|-----------|-----------------|------------|
| Read file | `["read"]` | `fd_read` |
| Write file | `["write"]` | `fd_write` |
| Create file | `["write"]` | `path_open` with CREATE |
| Delete file | `["write"]` | `path_unlink_file` |
| Create directory | `["write"]` | `path_create_directory` |
| Remove directory | `["write"]` | `path_remove_directory` |
| Create symlink | `["write"]` | `path_symlink` |
| List directory | `["read"]` | `fd_readdir` |

## Policy Configuration

Example policy configuration in YAML:

```yaml
version: "1.0"
core:
  storage:
    allow:
      - uri: "fs:///tmp/**"
        access: ["read", "write"]
      - uri: "fs:///var/log/*.log"
        access: ["read"]
    deny:
      - uri: "fs:///etc/**"
        access: ["read", "write"]
```

## Usage Example

```rust
use mcpkit_rs::wasm::{WasmContext, WasmRuntime};
use mcpkit_rs_policy::{Policy, CompiledPolicy};
use std::sync::Arc;

// Define policy
let policy_yaml = r#"
    version: "1.0"
    core:
      storage:
        allow:
          - uri: "fs:///tmp/**"
            access: ["read", "write"]
"#;

// Compile policy
let policy = Policy::from_yaml(policy_yaml).unwrap();
let compiled_policy = Arc::new(CompiledPolicy::compile(&policy).unwrap());

// Create WASM context with policy
let context = WasmContext::new()
    .with_policy(compiled_policy);

// Execute WASM module with filesystem access
let runtime = WasmRuntime::new().unwrap();
let module = runtime.compile_module(&wasm_bytes).unwrap();
let result = runtime.execute(&module, context).await.unwrap();
```

## Security Considerations

1. **Deny by Default**: No filesystem access is granted unless explicitly allowed in the policy
2. **Path Normalization**: The system strips `fs://` prefixes and normalizes paths
3. **Glob Pattern Matching**: Uses glob patterns to match allowed/denied paths
4. **Directory Isolation**: Each preopened directory is isolated with specific permissions
5. **No Root Access**: Empty patterns don't open root; defaults to safe directory like `/tmp`

## Testing

The implementation includes:

1. **Unit Tests** (`fs::tests`):
   - Permission mapping validation
   - Pattern to directory path conversion
   - Policy compilation and application

2. **Integration Tests** (`test_wasm_fs_access.rs`):
   - No-policy scenarios (no filesystem access)
   - Read-only access enforcement
   - Write permission validation
   - Multiple directory access with different permissions
   - Forbidden directory access prevention

3. **Real-World Testing** (`examples/wasm/wasmedge/fullstack/test_harness.sh`):
   - Filesystem access tests in the fullstack example
   - Happy and unhappy path scenarios
   - Policy enforcement validation

## Implementation Status

✅ **Completed**:
- Permission mapping from policy strings to WASI permissions
- Directory preopen based on policy patterns
- Integration with WasmContext and ServerConfig
- Unit tests for permission mapping
- Integration test framework
- Updated fullstack example with filesystem tests

⚠️ **Known Limitations**:
- WASI doesn't distinguish execute permissions from read
- MCP protocol currently only defines `read_resource`, not write operations
- The actual enforcement depends on the WASM runtime's WASI implementation

## Future Enhancements

1. **Fine-grained Permissions**: Support for more specific file operations
2. **Dynamic Permission Updates**: Allow runtime permission changes
3. **Audit Logging**: Log all filesystem access attempts
4. **Quota Management**: Implement storage quotas per WASM module
5. **Symbolic Link Handling**: Special handling for symlinks
6. **Mount Points**: Support for virtual filesystems and mount points
