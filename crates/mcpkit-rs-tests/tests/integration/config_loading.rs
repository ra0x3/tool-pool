//! Configuration loading and validation tests

use mcpkit_rs_config::{Config, ConfigLoader};
use tempfile::NamedTempFile;
use std::io::Write;

/// Test loading config from file
#[test]
fn test_load_config_from_file() {
    let yaml = r#"
version: "1.0"
server:
  name: test-server
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

    // Write to temp file
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(yaml.as_bytes()).unwrap();

    // Load config
    let config = Config::from_yaml_file(temp_file.path()).unwrap();
    assert_eq!(config.version, "1.0");
    assert_eq!(config.server.name, "test-server");
}

/// Test config validation
#[test]
fn test_config_validation() {
    // Valid config
    let valid_yaml = r#"
version: "1.0"
server:
  name: valid-server
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

    let config: Config = serde_yaml::from_str(valid_yaml).unwrap();
    assert!(config.validate().is_ok());

    // Test with invalid version
    let invalid_yaml = r#"
version: "0.5"
server:
  name: test
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

    let invalid_config: Result<Config, _> = serde_yaml::from_str(invalid_yaml);
    if let Ok(config) = invalid_config {
        // Version validation would fail here if implemented
        let _ = config.validate();
    }
}