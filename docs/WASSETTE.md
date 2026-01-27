# Wassette Policy Mechanism Documentation

## Overview

The Wassette policy mechanism is a comprehensive security and resource management system for WebAssembly (WASM) components. It provides fine-grained control over what resources and capabilities each component can access, implementing a **deny-by-default** security model where components must be explicitly granted permissions.

## Architecture

### Core Components

```
┌─────────────────────────────────────────────────────────┐
│                     Policy System                        │
├───────────────┬─────────────────┬───────────────────────┤
│  Policy Crate │  PolicyManager  │  WasiStateTemplate   │
│   (Parsing &  │   (Runtime      │    (Enforcement)      │
│  Validation)  │   Management)   │                       │
└───────────────┴─────────────────┴───────────────────────┘
```

### 1. **Policy Crate** (`crates/policy/`)
The foundational library for policy parsing, validation, and type definitions.

**Key Files:**
- `lib.rs` - Main policy document structure
- `types.rs` - Permission type definitions
- `parser.rs` - YAML parsing and serialization

### 2. **PolicyManager** (`crates/wassette/src/policy_internal.rs`)
Manages policy lifecycle, storage, and runtime registry.

**Responsibilities:**
- Attaching/detaching policies to components
- Granular permission grants/revokes
- Policy persistence and restoration
- WASI template generation

### 3. **WasiStateTemplate** (`crates/wassette/src/wasistate.rs`)
Bridges policies to WASI runtime enforcement.

**Features:**
- Converts policies to WASI configurations
- Enforces permissions at runtime
- Tracks permission violations

## Policy Structure

### YAML Format

Policies are defined in YAML files with a specific schema:

```yaml
$schema: https://raw.githubusercontent.com/microsoft/policy-mcp/main/schema/policy-v1.0.schema.json
version: "1.0"
description: "Description of the policy"
permissions:
  storage:
    allow:
      - uri: "fs://work/agent/**"
        access: ["read", "write"]
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

  runtime:
    docker:
      security:
        privileged: false
        no_new_privileges: true
        capabilities:
          drop: ["ALL"]
          add: ["NET_BIND_SERVICE"]

  resources:
    limits:
      cpu: "500m"      # 500 millicores
      memory: "512Mi"  # 512 MiB
```

## Permission Types

### 1. Storage Permissions

Controls filesystem access for components.

```rust
pub struct StoragePermission {
    pub uri: String,          // e.g., "fs://work/data/**"
    pub access: Vec<AccessType>, // [Read, Write]
}
```

**URI Patterns:**
- `fs://path/to/file` - Specific file
- `fs://path/*` - All files in directory
- `fs://path/**` - Recursive directory access

**Example:**
```yaml
storage:
  allow:
    - uri: "fs://tmp/**"
      access: ["read", "write"]
    - uri: "fs://config/app.yaml"
      access: ["read"]
```

### 2. Network Permissions

Controls network access for HTTP requests and socket connections.

```rust
pub enum NetworkPermission {
    Host(NetworkHostPermission),  // Domain-based
    Cidr(NetworkCidrPermission),  // IP range-based
}
```

**Host Patterns:**
- `api.example.com` - Exact host match
- `*.example.com` - Wildcard subdomain
- `*` - All hosts (use with caution)

**CIDR Notation:**
- `10.0.0.0/8` - Private network range
- `192.168.1.0/24` - Subnet access

**Example:**
```yaml
network:
  allow:
    - host: "api.openai.com"
    - host: "*.googleapis.com"
    - cidr: "10.0.0.0/8"
```

### 3. Environment Variables

Controls which environment variables components can access.

```rust
pub struct EnvironmentPermission {
    pub key: String,  // Variable name (no wildcards)
}
```

**Example:**
```yaml
environment:
  allow:
    - key: "DATABASE_URL"
    - key: "API_TOKEN"
    - key: "NODE_ENV"
```

### 4. Runtime Configuration

Platform-specific runtime settings (Docker, Hyperlight).

**Docker Security:**
```yaml
runtime:
  docker:
    security:
      privileged: false
      no_new_privileges: true
      capabilities:
        drop: ["ALL"]
        add: ["NET_BIND_SERVICE", "SYS_TIME"]
```

### 5. Resource Limits

CPU and memory constraints using Kubernetes-style notation.

```yaml
resources:
  limits:
    cpu: "2"        # 2 cores
    memory: "1Gi"   # 1 GiB
```

**CPU Formats:**
- `"500m"` - 500 millicores (0.5 core)
- `"2"` - 2 cores
- `"0.25"` - Quarter core

**Memory Formats:**
- `"512Mi"` - 512 MiB
- `"2Gi"` - 2 GiB
- `"256Ki"` - 256 KiB

## Enforcement Flow

### 1. Policy Loading
```rust
// From file
let policy = PolicyParser::parse_file("policy.yaml")?;

// From string
let policy = PolicyParser::parse_str(yaml_content)?;

// Validation
policy.validate()?;
```

### 2. Template Creation
```rust
// Convert policy to WASI state template
let template = create_wasi_state_template_from_policy(
    &policy,
    component_dir,
    &environment_vars,
    secrets.as_ref(),
)?;
```

### 3. Runtime Enforcement

The `WasiStateTemplate` configures the WASI runtime:

```rust
impl WasiStateTemplate {
    pub fn build(&self) -> Result<WasiState> {
        let mut ctx_builder = WasiCtxBuilder::new();

        // Network permissions
        if self.network_perms.allow_tcp {
            ctx_builder.allow_tcp(true);
            ctx_builder.allow_ip_name_lookup(true);
        }

        // Storage permissions
        for preopened_dir in &self.preopened_dirs {
            ctx_builder.preopened_dir(
                preopened_dir.host_path,
                preopened_dir.guest_path,
                preopened_dir.dir_perms,
                preopened_dir.file_perms,
            )?;
        }

        // Environment variables
        for (k, v) in &self.config_vars {
            ctx_builder.env(k, v);
        }

        // Build WASI context
        Ok(WasiState {
            ctx: ctx_builder.build(),
            // ... other fields
        })
    }
}
```

### 4. Permission Violation Tracking

When a component violates permissions:

```rust
pub enum PermissionError {
    NetworkDenied { host: String, uri: String },
    StorageDenied { path: String, access_type: String },
}

// User-friendly error messages with remediation
impl PermissionError {
    pub fn to_user_message(&self, component_id: &str) -> String {
        match self {
            NetworkDenied { host, .. } => format!(
                "Network permission denied for '{}'.\n\
                 To grant access:\n  \
                 grant-network-permission --component-id=\"{}\" --host=\"{}\"",
                host, component_id, host
            ),
            // ... other cases
        }
    }
}
```

## Management Operations

### Policy Attachment

```bash
# Attach a policy file to a component
wassette attach-policy --component-id="my-component" --policy="file://policy.yaml"

# From OCI registry
wassette attach-policy --component-id="my-component" --policy="oci://registry/policy:v1"
```

### Granular Permission Management

```bash
# Grant network permission
wassette grant-permission --component-id="my-component" \
    --type="network" --host="api.example.com"

# Grant storage permission
wassette grant-permission --component-id="my-component" \
    --type="storage" --uri="fs:///data/**" --access="read,write"

# Grant environment variable access
wassette grant-permission --component-id="my-component" \
    --type="environment" --key="DATABASE_URL"

# Grant memory limit
wassette grant-permission --component-id="my-component" \
    --type="resource" --memory="512Mi"

# Revoke permission
wassette revoke-permission --component-id="my-component" \
    --type="network" --host="api.example.com"

# Reset all permissions
wassette reset-permission --component-id="my-component"
```

### Policy Files Structure

Policies are stored alongside components:

```
wassette-root/
├── components/
│   └── my-component/
│       ├── my-component.wasm       # Component binary
│       ├── my-component.policy.yaml # Co-located policy
│       └── my-component.metadata.json # Policy metadata
```

## Validation Rules

### Storage URIs
- Must start with protocol (`fs://`)
- `**` must be its own path segment
- No triple wildcards (`***`)
- Single `*` matches within segment only

### Network Hosts
- Wildcards only at start (`*.example.com`)
- Maximum one wildcard per host
- CIDR must include slash notation

### Environment Keys
- No wildcards allowed
- Must be non-empty
- Exact match only

### Resource Limits
- CPU: Non-negative values
- Memory: Non-zero values
- Valid unit suffixes required

## Security Model

### Deny-by-Default
- Components start with zero permissions
- All access must be explicitly granted
- Network, storage, and environment access blocked by default

### Principle of Least Privilege
- Grant minimum required permissions
- Use specific paths over wildcards
- Prefer allow lists over deny lists

### Capability-Based Security
- Permissions tied to component identity
- Non-transferable between components
- Auditable permission grants

## Integration with WASI

The policy system integrates deeply with WASI (WebAssembly System Interface):

1. **Storage** → WASI preopened directories
2. **Network** → WASI socket capabilities
3. **Environment** → WASI environment variables
4. **Resources** → WASI store limits

Example transformation:

```yaml
# Policy definition
storage:
  allow:
    - uri: "fs://data/**"
      access: ["read", "write"]
```

```rust
// Runtime enforcement
ctx_builder.preopened_dir(
    "/data",           // Host path
    "/data",           // Guest path
    DirPerms::all(),   // Directory permissions
    FilePerms::all(),  // File permissions
)?;
```

## Best Practices

### 1. Start Restrictive
Begin with minimal permissions and add as needed:

```yaml
version: "1.0"
description: "Minimal starting policy"
permissions: {}  # No permissions
```

### 2. Use Specific Paths
Prefer exact paths over wildcards:

```yaml
# Good
storage:
  allow:
    - uri: "fs://app/config/settings.json"
      access: ["read"]

# Avoid
storage:
  allow:
    - uri: "fs://app/**"
      access: ["read", "write"]
```

### 3. Document Permissions
Always include descriptions:

```yaml
description: "Web service requiring database and S3 access"
permissions:
  network:
    allow:
      - host: "postgres.internal"  # Database
      - host: "s3.amazonaws.com"    # File storage
```

### 4. Regular Audits
Review and remove unnecessary permissions periodically.

### 5. Environment-Specific Policies
Use different policies for dev/staging/production:

```yaml
# development.policy.yaml
permissions:
  network:
    allow:
      - host: "localhost"
      - host: "*.test.local"

# production.policy.yaml
permissions:
  network:
    allow:
      - host: "api.production.com"
```

## Error Handling

The system provides detailed error messages for permission violations:

```
Network permission denied: Component 'weather-service' attempted to access
'https://unauthorized-api.com/data' but does not have permission for host
'unauthorized-api.com'.

To grant network access, use:
  grant-network-permission --component-id="weather-service" --host="unauthorized-api.com"
```

## Future Enhancements

### Planned Features
- **IPC Permissions**: Inter-process communication controls
- **Hyperlight Runtime**: Alternative sandboxing runtime
- **Dynamic Permissions**: Runtime permission negotiation
- **Audit Logging**: Comprehensive access logs
- **Permission Templates**: Reusable permission profiles

### Under Consideration
- Time-based permissions
- Rate limiting
- Conditional permissions based on context
- Permission inheritance hierarchies
- Capability delegation between components

## Summary

The Wassette policy mechanism provides a robust, type-safe, and user-friendly system for managing WebAssembly component permissions. By combining deny-by-default security with flexible granular controls, it enables secure multi-tenant component execution while maintaining operational simplicity.

Key benefits:
- **Security**: Deny-by-default with explicit grants
- **Flexibility**: Granular permission management
- **Usability**: Clear error messages with remediation steps
- **Compatibility**: WASI-native integration
- **Extensibility**: Plugin architecture for new permission types