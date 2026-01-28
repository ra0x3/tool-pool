#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use crate::*;

    // Complex real-world configuration example with stdio transport
    const COMPLEX_CONFIG_YAML: &str = r#"# Configuration for WasmEdge Fullstack Example
version: "1.0"

metadata:
  name: "fullstack-example"
  description: "Fullstack WasmEdge MCP server example with multiple tools"
  author: "mcpkit-rs"

server:
  name: "fullstack-server"
  version: "0.1.0"
  description: "A fullstack MCP server with data management tools"
  bind: "0.0.0.0"
  port: 3000
  request_timeout: 60
  debug: false
  log_level: "info"

transport:
  type: stdio
  settings:
    stdio:
      buffer_size: 8192

runtime:
  type: wasmedge
  wasm:
    memory_pages: 1024  # 64MB
    fuel: 10000000      # Legacy configuration (kept for compatibility)

    # Modern metering configuration for WasmEdge
    metering:
      enabled: true
      max_compute_units: 100_000_000  # 100M units (gas in WasmEdge)
      display_format: detailed         # Show detailed metrics for development

      # Memory limits with defense-in-depth
      memory_limits:
        max_memory: "256Mi"           # 256 MiB for fullstack operations
        soft_limit: "200Mi"           # Warn at 200 MiB
        max_tables: 20                # More tables for complex app
        max_instances: 10             # Support multiple instances

      # Enable monitoring for development
      enable_monitoring: true

      # Sampling configuration
      sampling:
        unit_threshold: 100_000       # Sample every 100K units
        time_threshold_ms: 100        # Sample every 100ms
        adaptive: true                # Adapt to workload

      # Warning mode for development
      enforcement:
        warning:
          threshold: 0.85             # Warn at 85% usage

      # Generous limits for fullstack app
      limits:
        soft_limit: 85_000_000        # Warn at 85M units
        hard_limit: 100_000_000       # Stop at 100M units

      # Fullstack operation quotas
      quotas:
        per_request: 10_000_000       # 10M per request
        per_minute: 500_000_000       # 500M per minute
        per_hour: 10_000_000_000      # 10B per hour

  limits:
    execution_time: "30s"
    memory: "256Mi"     # Updated to match metering config
    cpu: "1000m"

mcp:
  protocol_version: "2024-11-05"
  capabilities:
    - tools
    - prompts
    - resources
    - logging

policy:
  version: "1.0"
  core:
    tools:
      allow:
        - name: "test_connection"
        - name: "fetch_todos"
        - name: "create_todo"
        - name: "update_todo"
        - name: "delete_todo"
        - name: "batch_process"
        - name: "search_todos"
        - name: "db_stats"
        - name: "read_wal"
      deny: []
    network:
      allow:
        - host: "localhost:*"
        - host: "127.0.0.1:*"
      deny:
        - host: "*"
    storage:
      allow:
        - uri: "/tmp/**"
          access: ["read", "write"]
        - uri: "/var/tmp/**"
          access: ["read", "write"]
      deny:
        - uri: "/etc/**"
          access: ["read", "write"]
        - uri: "/usr/**"
          access: ["read", "write"]
        - uri: "/sys/**"
          access: ["read", "write"]
    environment:
      allow:
        - key: "HOME"
        - key: "USER"
        - key: "TMPDIR"
        - key: "WASMEDGE_*"
      deny:
        - key: "*_TOKEN"
        - key: "*_KEY"
        - key: "*_SECRET"
  resources:
    execution:
      max_fuel: 10000000
      timeout_seconds: 30
    memory:
      max_pages: 1024
    capabilities:
      - filesystem_read
      - filesystem_write
      - network_connect
"#;

    // HTTP transport version of the complex config
    const COMPLEX_HTTP_CONFIG_YAML: &str = r#"# Configuration for WasmEdge Fullstack Example
version: "1.0"

metadata:
  name: "fullstack-example"
  description: "Fullstack WasmEdge MCP server example with multiple tools"
  author: "mcpkit-rs"

server:
  name: "fullstack-server"
  version: "0.1.0"
  description: "A fullstack MCP server with data management tools"
  bind: "0.0.0.0"
  port: 3000
  request_timeout: 60
  debug: false
  log_level: "info"

transport:
  type: http
  settings:
    cors_enabled: true
    max_body_size: 10485760  # 10MB
    compression: true

runtime:
  type: wasmedge
  wasm:
    memory_pages: 1024  # 64MB
    fuel: 10000000      # Legacy configuration (kept for compatibility)

mcp:
  protocol_version: "2024-11-05"
  capabilities:
    - tools
    - prompts
    - resources
    - logging

policy:
  version: "1.0"
  core:
    tools:
      allow:
        - name: "test_connection"
        - name: "fetch_todos"
      deny: []
"#;

    #[test]
    fn test_complex_http_config() {
        // Test the HTTP transport version
        let config = Config::from_yaml(COMPLEX_HTTP_CONFIG_YAML).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.server.name, "fullstack-server");
        assert_eq!(config.transport.transport_type, TransportType::Http);
    }

    #[test]
    fn test_complex_real_world_config() {
        // Test a complex real-world configuration
        let config = Config::from_yaml(COMPLEX_CONFIG_YAML).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.server.name, "fullstack-server");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.runtime.runtime_type, RuntimeType::WasmEdge);

        // Verify policy loaded correctly
        assert!(config.policy.is_some());
        let policy = config.policy.unwrap();
        assert_eq!(policy.version, "1.0");

        // Verify capabilities as list
        assert!(config.mcp.capabilities.is_some());
        match config.mcp.capabilities.unwrap() {
            McpCapabilities::List(caps) => {
                assert!(caps.contains(&"tools".to_string()));
                assert!(caps.contains(&"prompts".to_string()));
                assert!(caps.contains(&"resources".to_string()));
                assert!(caps.contains(&"logging".to_string()));
            }
            _ => panic!("Expected capabilities to be a list"),
        }
    }

    #[test]
    fn test_minimal_config() {
        let yaml = r#"
version: "1.0"
server:
  name: test-server
  version: 0.1.0
  bind: 127.0.0.1
  port: 3000
transport:
  type: stdio
  settings:
    buffer_size: 65536
runtime:
  type: native
mcp:
  protocol_version: "2024-11-05"
"#;

        let config = Config::from_yaml(yaml).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.server.name, "test-server");
        assert_eq!(config.server.port, 3000);
    }

    #[test]
    fn test_config_with_policy() {
        let yaml = r#"
version: "1.0"
server:
  name: test-server
  version: 0.1.0
  bind: 127.0.0.1
  port: 3000
transport:
  type: stdio
  settings:
    buffer_size: 65536
runtime:
  type: native
mcp:
  protocol_version: "2024-11-05"
policy:
  version: "1.0"
  core:
    network:
      allow:
        - host: "api.example.com"
      deny:
        - host: "malicious.com"
"#;

        let config = Config::from_yaml(yaml).unwrap();
        assert!(config.policy.is_some());

        let policy = config.policy.unwrap();
        assert_eq!(policy.version, "1.0");
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.server.name, "mcpkit-rs-server");
        assert_eq!(config.transport.transport_type, TransportType::Stdio);
    }

    #[test]
    fn test_config_merge() {
        let mut config1 = Config::default();

        let yaml2 = r#"
version: "1.0"
server:
  name: merged-server
  version: 2.0.0
  bind: 0.0.0.0
  port: 8080
transport:
  type: http
  settings:
    cors_enabled: true
runtime:
  type: wasmtime
mcp:
  protocol_version: "2024-11-05"
"#;

        let config2 = Config::from_yaml(yaml2).unwrap();
        config1.merge(config2).unwrap();

        assert_eq!(config1.server.name, "merged-server");
        assert_eq!(config1.server.port, 8080);
        assert_eq!(config1.transport.transport_type, TransportType::Http);
    }

    #[test]
    fn test_version_mismatch() {
        let mut config1 = Config::default();
        config1.version = "1.0".to_string();

        let mut config2 = Config::default();
        config2.version = "2.0".to_string();

        let result = config1.merge(config2);
        assert!(result.is_err());

        if let Err(ConfigError::VersionMismatch { expected, found }) = result {
            assert_eq!(expected, "1.0");
            assert_eq!(found, "2.0");
        } else {
            panic!("Expected version mismatch error");
        }
    }

    #[test]
    fn test_validation_empty_server_name() {
        let yaml = r#"
version: "1.0"
server:
  name: ""
  version: 0.1.0
  bind: 127.0.0.1
  port: 3000
transport:
  type: stdio
  settings: {}
runtime:
  type: native
mcp:
  protocol_version: "2024-11-05"
"#;

        let result = Config::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_invalid_port() {
        let yaml = r#"
version: "1.0"
server:
  name: test
  version: 0.1.0
  bind: 127.0.0.1
  port: 0
transport:
  type: stdio
  settings: {}
runtime:
  type: native
mcp:
  protocol_version: "2024-11-05"
"#;

        let result = Config::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_loader_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("mcpkit.yaml");

        let yaml = r#"
version: "1.0"
server:
  name: file-test-server
  version: 1.0.0
  bind: 127.0.0.1
  port: 4000
transport:
  type: stdio
  settings: {}
runtime:
  type: native
mcp:
  protocol_version: "2024-11-05"
"#;

        fs::write(&config_path, yaml).unwrap();

        let mut loader = ConfigLoader::new();
        loader.add_search_path(temp_dir.path());

        let config = loader.load().unwrap();
        assert_eq!(config.server.name, "file-test-server");
        assert_eq!(config.server.port, 4000);
    }

    #[test]
    fn test_json_config() {
        let json = r#"{
  "version": "1.0",
  "server": {
    "name": "json-server",
    "version": "1.0.0",
    "bind": "127.0.0.1",
    "port": 5000
  },
  "transport": {
    "type": "http",
    "settings": {
      "cors_enabled": true
    }
  },
  "runtime": {
    "type": "native"
  },
  "mcp": {
    "protocol_version": "2024-11-05"
  }
}"#;

        let config = Config::from_json(json).unwrap();
        assert_eq!(config.server.name, "json-server");
        assert_eq!(config.server.port, 5000);
        assert_eq!(config.transport.transport_type, TransportType::Http);
    }

    #[test]
    fn test_resource_limits_validation() {
        let yaml = r#"
version: "1.0"
server:
  name: test
  version: 0.1.0
  bind: 127.0.0.1
  port: 3000
transport:
  type: stdio
  settings: {}
runtime:
  type: native
  limits:
    cpu: "500m"
    memory: "512Mi"
    execution_time: "30s"
    max_requests_per_minute: 100
mcp:
  protocol_version: "2024-11-05"
"#;

        let config = Config::from_yaml(yaml).unwrap();
        let limits = config.runtime.limits.unwrap();
        assert_eq!(limits.cpu, Some("500m".to_string()));
        assert_eq!(limits.memory, Some("512Mi".to_string()));
        assert_eq!(limits.execution_time, Some("30s".to_string()));
    }

    #[test]
    fn test_mcp_tools_config() {
        let yaml = r#"
version: "1.0"
server:
  name: test
  version: 0.1.0
  bind: 127.0.0.1
  port: 3000
transport:
  type: stdio
  settings: {}
runtime:
  type: native
mcp:
  protocol_version: "2024-11-05"
  tools:
    - name: calculator
      description: "Math tool"
      input_schema:
        type: object
        properties:
          expression:
            type: string
      handler: calc_handler
"#;

        let config = Config::from_yaml(yaml).unwrap();
        let tools = config.mcp.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "calculator");
        assert_eq!(tools[0].description, "Math tool");
    }

    #[test]
    fn test_extensions_config() {
        let yaml = r#"
version: "1.0"
server:
  name: test
  version: 0.1.0
  bind: 127.0.0.1
  port: 3000
transport:
  type: stdio
  settings: {}
runtime:
  type: native
mcp:
  protocol_version: "2024-11-05"
extensions:
  custom_auth:
    enabled: true
    provider: oauth2
  monitoring:
    enabled: false
"#;

        let config = Config::from_yaml(yaml).unwrap();
        assert_eq!(config.extensions.len(), 2);
        assert!(config.extensions.contains_key("custom_auth"));
        assert!(config.extensions.contains_key("monitoring"));
    }
}
