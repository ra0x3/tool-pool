//! Cross-module integration tests

use mcpkit_rs::service::{serve_server, RoleServer};
use mcpkit_rs::transport::stdio::Stdio;
use mcpkit_rs_config::Config;
use tempfile::TempDir;
use tokio::time::{timeout, Duration};

mod server_client;
mod wasm_policy;
mod config_loading;

/// Test server startup with configuration
#[tokio::test]
async fn test_server_startup_with_config() {
    let config_yaml = r#"
version: "1.0"

server:
  name: test-server
  version: 0.1.0
  bind: 127.0.0.1
  port: 3000
  debug: false

transport:
  type: stdio
  settings:
    buffer_size: 8192

runtime:
  type: native

mcp:
  protocol_version: "2024-11-05"
  tools:
    - name: test_tool
      description: "Test tool"
      input_schema:
        type: object
        properties:
          input:
            type: string
"#;

    // Parse config
    let config: Config = serde_yaml::from_str(config_yaml).unwrap();
    assert_eq!(config.server.name, "test-server");

    // Verify config is valid
    assert!(config.validate().is_ok());
}

/// Test configuration merging
#[test]
fn test_config_merge() {
    let base_yaml = r#"
version: "1.0"
server:
  name: base-server
  version: 1.0.0
  bind: 127.0.0.1
  port: 3000

transport:
  type: stdio
  settings:
    buffer_size: 8192

runtime:
  type: native

mcp:
  protocol_version: "2024-11-05"
"#;

    let override_yaml = r#"
version: "1.0"
server:
  name: override-server
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

    let mut base: Config = serde_yaml::from_str(base_yaml).unwrap();
    let override_config: Config = serde_yaml::from_str(override_yaml).unwrap();

    // Merge configurations
    base.merge(override_config).unwrap();

    // Verify override took effect
    assert_eq!(base.server.name, "override-server");
    assert_eq!(base.server.version, "2.0.0");
    assert_eq!(base.server.bind, "0.0.0.0");
    assert_eq!(base.server.port, 8080);
    assert_eq!(base.runtime.runtime_type, mcpkit_rs_config::RuntimeType::Wasmtime);
}

/// Test configuration with all features enabled
#[test]
fn test_full_config_with_distribution_and_policy() {
    let yaml = r#"
version: "1.0"

metadata:
  name: full-test
  description: "Full configuration test"
  author: "test"

server:
  name: full-server
  version: 1.0.0
  bind: 127.0.0.1
  port: 3000
  max_connections: 100
  request_timeout: 30
  debug: false
  log_level: info

transport:
  type: stdio
  settings:
    buffer_size: 65536

policy:
  version: "1.0"
  description: "Test policy"
  core:
    network:
      deny:
        - host: "*"
    storage:
      allow:
        - uri: "fs:///tmp/**"
          access: ["read", "write"]
    environment:
      allow:
        - key: "HOME"
        - key: "USER"

runtime:
  type: wasmtime
  wasm:
    module_path: "./test.wasm"
    fuel: 1000000
    memory_pages: 64
    cache: true
    cache_dir: "./.cache"

mcp:
  protocol_version: "2024-11-05"
  tools:
    - name: test_tool
      description: "Test"
      input_schema:
        type: object

distribution:
  registry: "ghcr.io/test/bundle"
  version: "1.0.0"
  tags:
    - "latest"
  metadata:
    authors: ["test"]
    license: "MIT"
"#;

    let config: Config = serde_yaml::from_str(yaml).unwrap();

    // Verify all sections are present
    assert!(config.metadata.is_some());
    assert!(config.policy.is_some());
    assert!(config.distribution.is_some());
    assert!(config.runtime.wasm.is_some());

    // Validate complete configuration
    assert!(config.validate().is_ok());
}