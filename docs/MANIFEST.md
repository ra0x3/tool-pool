# Configuration & Manifest

## Overview

mcpkit-rs uses a YAML-based configuration file to define MCP server metadata, transport settings, security policies, runtime configuration, and tool definitions. The configuration file is typically named `config.yaml` or `manifest.yaml` and should be placed in your project root.

The configuration system provides:
- **Server metadata** – Name, version, and description of your MCP server
- **Transport configuration** – stdio, HTTP, or WebSocket transport settings
- **Security policies** – Fine-grained access control for tools, resources, and system capabilities
- **Runtime settings** – WASM runtime configuration, resource limits, and caching
- **MCP specifications** – Tool definitions, prompts, and resource declarations

## Configuration Structure

### Top-Level Configuration

The configuration file has the following top-level sections:

- `version` – Configuration file version (currently "1.0")
- `metadata` – Optional server metadata and description
- `server` – Server configuration including bind address and logging
- `transport` – Transport protocol configuration
- `policy` – Security policy definitions
- `runtime` – WASM runtime and resource limits
- `mcp` – MCP protocol configuration and tool definitions

### Complete Example

Below is a comprehensive example showing all available configuration options:

```yaml
# Configuration file version
version: "1.0"

# Server metadata (optional)
metadata:
  name: "Calculator MCP Server"
  description: "WASM-based calculator with strict security policy"
  author: "Your Organization"
  repository: "https://github.com/youorg/calculator"

# Server configuration
server:
  name: wasm-calculator
  version: 0.1.0
  bind: 127.0.0.1
  port: 3000
  debug: false
  log_level: info  # trace, debug, info, warn, error

# Transport configuration
transport:
  type: stdio  # stdio, http, or websocket
  settings:
    buffer_size: 32768
    # For HTTP transport:
    # cors_origins: ["http://localhost:3000"]
    # timeout: "30s"
    # For WebSocket transport:
    # ping_interval: "30s"
    # max_connections: 100

# Security policy
policy:
  version: "1.0"
  description: "Strict sandbox policy for calculator"

  core:
    # Network access control
    network:
      allow:
        - host: "localhost"
          ports: [5432]  # PostgreSQL
        - host: "127.0.0.1"
          ports: [5432]
      deny:
        - host: "*"  # Deny all other hosts

    # File system access control
    storage:
      allow:
        - uri: "fs:///tmp/**"
          access: ["read", "write"]
        - uri: "fs:///var/tmp/**"
          access: ["read", "write"]
      deny:
        - uri: "fs:///**"
          access: ["read", "write", "execute"]

    # Environment variable access
    environment:
      allow:
        - key: "HOME"
        - key: "USER"
        - key: "TMPDIR"
        - key: "DATABASE_URL"
      deny:
        - key: "*_TOKEN"
        - key: "*_KEY"
        - key: "*_SECRET"

    # Resource limits
    resources:
      limits:
        cpu: "1000m"           # 1 core
        memory: "64Mi"         # 64 MiB
        execution_time: "30s"  # 30 seconds max
        fuel: 10000000        # WASM fuel units

  # MCP-specific permissions
  mcp:
    tools:
      allow:
        - name: "add"
          max_calls_per_minute: 1000
        - name: "subtract"
          max_calls_per_minute: 1000
        - name: "multiply"
          max_calls_per_minute: 1000
        - name: "divide"
          max_calls_per_minute: 1000
      deny:
        - name: "*"  # Deny all other tools

    prompts:
      allow:
        - name: "calculation_help"
      deny:
        - name: "*"

    resources:
      allow:
        - uri: "file:///tmp/**"
          operations: ["read", "write"]
      deny:
        - uri: "*"
          operations: ["read", "write", "delete"]

    transport:
      stdio: true
      http:
        allowed_hosts: ["localhost", "127.0.0.1"]
        allowed_origins: ["http://localhost:*"]
      websocket: false

# Runtime configuration
runtime:
  type: wasmtime  # wasmtime or wasmedge
  wasm:
    module_path: ./calculator.wasm
    fuel: 10000000      # WASM fuel units
    memory_pages: 16    # 1MB memory (16 * 64KB)
    cache: true
    cache_dir: ./.wasm-cache
    # WasmEdge specific:
    # enable_all: true  # Enable all WasmEdge extensions
  limits:
    cpu: "100m"
    memory: "32Mi"
    execution_time: "5s"
    max_requests_per_minute: 1000

# MCP protocol configuration
mcp:
  protocol_version: "2024-11-05"

  # Server information
  server_info:
    name: "calculator"
    version: "1.0.0"
    capabilities:
      tools: true
      prompts: false
      resources: false
      logging: true

  # Tool definitions
  tools:
    - name: add
      description: "Add two numbers"
      input_schema:
        type: object
        properties:
          a:
            type: number
            description: "First number"
          b:
            type: number
            description: "Second number"
        required: ["a", "b"]

    - name: subtract
      description: "Subtract two numbers"
      input_schema:
        type: object
        properties:
          a:
            type: number
            description: "First number"
          b:
            type: number
            description: "Second number"
        required: ["a", "b"]

  # Prompt definitions (optional)
  prompts:
    - name: calculation_help
      description: "Get help with calculations"
      arguments:
        - name: operation
          description: "The operation you need help with"
          required: true

  # Resource definitions (optional)
  resources:
    - uri: "file:///tmp/results.json"
      name: "Calculation Results"
      mime_type: "application/json"
      description: "Stored calculation results"
```

## Configuration Options

### Metadata Section

#### metadata (optional)

Server metadata providing information about the MCP server.

```yaml
metadata:
  name: "My MCP Server"
  description: "Description of what this server does"
  author: "Author Name"
  repository: "https://github.com/org/repo"
  license: "MIT"
```

### Server Configuration

#### server (required)

Core server configuration including network binding and logging.

##### server.name (required)
Internal name for the server instance.

```yaml
server:
  name: my-mcp-server
```

##### server.version (required)
Server version following semantic versioning.

```yaml
server:
  version: 1.0.0
```

##### server.bind (optional)
IP address to bind to. Defaults to "127.0.0.1".

```yaml
server:
  bind: 0.0.0.0  # Listen on all interfaces
```

##### server.port (optional)
Port number for HTTP/WebSocket transports. Ignored for stdio.

```yaml
server:
  port: 8080
```

##### server.debug (optional)
Enable debug mode with verbose logging. Defaults to false.

```yaml
server:
  debug: true
```

##### server.log_level (optional)
Logging level. Valid values: "trace", "debug", "info", "warn", "error".

```yaml
server:
  log_level: "info"
```

### Transport Configuration

#### transport (required)

Transport protocol configuration for client-server communication.

##### transport.type (required)
Transport type. Valid values: "stdio", "http", "websocket".

```yaml
transport:
  type: stdio
```

##### transport.settings (optional)
Transport-specific settings.

For stdio transport:
```yaml
transport:
  type: stdio
  settings:
    buffer_size: 32768  # Buffer size in bytes
```

For HTTP transport:
```yaml
transport:
  type: http
  settings:
    cors_origins: ["http://localhost:3000", "https://app.example.com"]
    timeout: "30s"
    max_body_size: "10MB"
    tls:
      cert_file: "/path/to/cert.pem"
      key_file: "/path/to/key.pem"
```

For WebSocket transport:
```yaml
transport:
  type: websocket
  settings:
    ping_interval: "30s"
    max_connections: 100
    message_size_limit: "1MB"
```

### Security Policy

#### policy (required)

Comprehensive security policy controlling access to system resources and MCP capabilities.

##### policy.version (required)
Policy version for compatibility checking.

```yaml
policy:
  version: "1.0"
```

##### policy.core (required)
Core security policies for system resources.

###### policy.core.network
Network access control rules.

```yaml
policy:
  core:
    network:
      allow:
        - host: "api.example.com"
          ports: [443]
          protocols: ["https"]
        - host: "localhost"
          ports: [5432, 6379]  # PostgreSQL, Redis
      deny:
        - host: "*"  # Deny all other hosts
```

###### policy.core.storage
File system access control.

```yaml
policy:
  core:
    storage:
      allow:
        - uri: "fs:///app/data/**"
          access: ["read", "write"]
        - uri: "fs:///tmp/**"
          access: ["read", "write", "create", "delete"]
      deny:
        - uri: "fs:///etc/**"
          access: ["read", "write", "execute"]
        - uri: "fs:///sys/**"
          access: ["read", "write", "execute"]
```

Access modes:
- `read` – Read file contents
- `write` – Modify file contents
- `create` – Create new files
- `delete` – Delete files
- `execute` – Execute files

###### policy.core.environment
Environment variable access control.

```yaml
policy:
  core:
    environment:
      allow:
        - key: "NODE_ENV"
        - key: "DATABASE_*"  # Wildcard patterns supported
        - key: "API_*"
      deny:
        - key: "*_SECRET"
        - key: "*_TOKEN"
        - key: "AWS_*"
```

###### policy.core.resources
Resource limits for the server process.

```yaml
policy:
  core:
    resources:
      limits:
        cpu: "2000m"           # 2 cores
        memory: "512Mi"        # 512 MiB
        execution_time: "60s"  # Max execution time
        fuel: 100000000       # WASM fuel units (if applicable)
```

##### policy.mcp (required)
MCP-specific access control.

###### policy.mcp.tools
Tool access and rate limiting.

```yaml
policy:
  mcp:
    tools:
      allow:
        - name: "query_database"
          max_calls_per_minute: 100
          max_calls_per_hour: 5000
        - name: "send_email"
          max_calls_per_minute: 10
          require_confirmation: true  # Require user confirmation
      deny:
        - name: "delete_*"  # Deny all delete operations
```

###### policy.mcp.prompts
Prompt access control.

```yaml
policy:
  mcp:
    prompts:
      allow:
        - name: "help_*"
        - name: "tutorial_*"
      deny:
        - name: "admin_*"
```

###### policy.mcp.resources
Resource access control.

```yaml
policy:
  mcp:
    resources:
      allow:
        - uri: "file:///app/public/**"
          operations: ["read"]
        - uri: "sqlite:///app/data/app.db"
          operations: ["read", "write"]
      deny:
        - uri: "file:///app/private/**"
          operations: ["read", "write", "delete"]
```

###### policy.mcp.transport
Transport-specific policies.

```yaml
policy:
  mcp:
    transport:
      stdio: true
      http:
        allowed_hosts: ["localhost", "127.0.0.1", "app.example.com"]
        allowed_origins: ["http://localhost:3000", "https://app.example.com"]
        allowed_methods: ["GET", "POST"]
      websocket: true
```

### Runtime Configuration

#### runtime (required for WASM)

Runtime configuration for WASM modules.

##### runtime.type (required)
WASM runtime type. Valid values: "wasmtime", "wasmedge".

```yaml
runtime:
  type: wasmtime
```

##### runtime.wasm (required for WASM)
WASM-specific configuration.

```yaml
runtime:
  wasm:
    module_path: ./target/wasm32-wasip1/release/server.wasm
    fuel: 100000000      # Fuel units (computational limit)
    memory_pages: 256    # 16MB memory (256 * 64KB)
    cache: true          # Enable module caching
    cache_dir: ./.wasm-cache
    # WasmEdge specific options:
    enable_all: true     # Enable all WasmEdge extensions
    # Wasmtime specific options:
    epoch_interruption: true
    debug_info: false
```

##### runtime.limits (optional)
Runtime resource limits.

```yaml
runtime:
  limits:
    cpu: "500m"                    # 0.5 cores
    memory: "128Mi"                # 128 MiB
    execution_time: "30s"          # Max execution time per request
    max_requests_per_minute: 1000  # Rate limiting
    max_concurrent_requests: 10    # Concurrency limit
```

### MCP Protocol Configuration

#### mcp (required)

Model Context Protocol configuration and capability definitions.

##### mcp.protocol_version (required)
MCP protocol version for compatibility.

```yaml
mcp:
  protocol_version: "2024-11-05"
```

##### mcp.server_info (optional)
Server information returned to clients.

```yaml
mcp:
  server_info:
    name: "my-server"
    version: "1.0.0"
    description: "My MCP Server"
    capabilities:
      tools: true
      prompts: true
      resources: true
      logging: true
      experimental: false
```

##### mcp.tools (optional)
Tool definitions exposed to clients.

```yaml
mcp:
  tools:
    - name: query_database
      description: "Query the application database"
      input_schema:
        type: object
        properties:
          query:
            type: string
            description: "SQL query to execute"
          parameters:
            type: array
            items:
              type: string
            description: "Query parameters"
        required: ["query"]
      output_schema:
        type: object
        properties:
          rows:
            type: array
            description: "Query results"
          affected:
            type: integer
            description: "Number of affected rows"
```

Each tool requires:
- `name` – Unique tool identifier
- `description` – Human-readable description
- `input_schema` – JSON Schema for input validation
- `output_schema` (optional) – Expected output format

##### mcp.prompts (optional)
Prompt templates for common operations.

```yaml
mcp:
  prompts:
    - name: database_help
      description: "Get help with database queries"
      arguments:
        - name: table
          description: "The table you need help with"
          required: true
        - name: operation
          description: "The operation type (select, insert, update, delete)"
          required: false
          default: "select"
      template: |
        Help me write a {operation} query for the {table} table.

        Available columns: {available_columns}
        Example: {example_query}
```

##### mcp.resources (optional)
Static resources exposed by the server.

```yaml
mcp:
  resources:
    - uri: "file:///app/docs/api.md"
      name: "API Documentation"
      mime_type: "text/markdown"
      description: "Complete API documentation"

    - uri: "sqlite:///app/data/app.db"
      name: "Application Database"
      mime_type: "application/x-sqlite3"
      description: "Main application database"

    - uri: "template:///emails/welcome"
      name: "Welcome Email Template"
      mime_type: "text/html"
      description: "Welcome email template"
```

## Environment Variables

Configuration values can reference environment variables using `${VAR_NAME}` syntax:

```yaml
server:
  port: ${PORT:-3000}  # Use PORT env var, default to 3000

mcp:
  tools:
    - name: api_call
      description: "Call external API"
      input_schema:
        type: object
        properties:
          endpoint:
            type: string
            default: ${API_ENDPOINT}  # From environment
```

## Configuration Precedence

When multiple configuration sources are present, they are applied in the following order (highest to lowest precedence):

1. Environment variables
2. Command-line arguments
3. Configuration file (`config.yaml`)
4. Built-in defaults

## Validation

The configuration is validated at startup to ensure:
- Required fields are present
- Values are within acceptable ranges
- File paths and URIs are properly formatted
- Policy rules don't conflict
- Tool schemas are valid JSON Schema

Validation errors will prevent the server from starting and provide detailed error messages indicating the problematic configuration section.

## Examples

### Minimal Configuration

Simplest possible configuration for a basic MCP server:

```yaml
version: "1.0"

server:
  name: simple-server
  version: 0.1.0

transport:
  type: stdio

policy:
  version: "1.0"
  core:
    network:
      deny: ["*"]
  mcp:
    tools:
      allow: ["*"]

mcp:
  protocol_version: "2024-11-05"
  tools:
    - name: hello
      description: "Say hello"
      input_schema:
        type: object
        properties:
          name:
            type: string
        required: ["name"]
```

### PostgreSQL-Enabled Server

Configuration for a server with database access:

```yaml
version: "1.0"

server:
  name: database-server
  version: 1.0.0
  log_level: info

transport:
  type: stdio

policy:
  version: "1.0"
  core:
    network:
      allow:
        - host: "localhost"
          ports: [5432]
    environment:
      allow:
        - key: "DATABASE_URL"
        - key: "POSTGRES_*"

runtime:
  type: wasmedge  # WasmEdge supports networking
  wasm:
    module_path: ./server.wasm
    enable_all: true

mcp:
  protocol_version: "2024-11-05"
  tools:
    - name: query_db
      description: "Execute database query"
      input_schema:
        type: object
        properties:
          query:
            type: string
        required: ["query"]
```

### Multi-Transport Configuration

Server supporting multiple transport protocols:

```yaml
version: "1.0"

server:
  name: multi-transport
  version: 2.0.0
  bind: 0.0.0.0
  port: 8080

# Primary transport
transport:
  type: http
  settings:
    cors_origins: ["*"]
    timeout: "60s"

# Alternative transports can be enabled via command-line flags
# --transport=stdio
# --transport=websocket

policy:
  version: "1.0"
  core:
    network:
      allow:
        - host: "*"
          ports: [443]
  mcp:
    transport:
      stdio: true
      http:
        allowed_origins: ["*"]
      websocket: true

mcp:
  protocol_version: "2024-11-05"
  server_info:
    capabilities:
      tools: true
      prompts: true
      resources: true
```

## See Also

- [Policy Documentation](./POLICY.md) – Detailed security policy reference
- [MCP Specification](https://modelcontextprotocol.io/specification) – Model Context Protocol specification
- [Examples](../examples/) – Example configurations for various use cases