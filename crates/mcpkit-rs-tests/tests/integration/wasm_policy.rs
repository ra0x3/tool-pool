//! WASM and policy integration tests

use mcpkit_rs_policy::{Policy, PolicyEngine};
use mcpkit_rs_config::Config;

/// Test policy loading from configuration
#[test]
fn test_policy_from_config() {
    let yaml = r#"
version: "1.0"

server:
  name: policy-test
  version: 1.0.0
  bind: 127.0.0.1
  port: 3000

transport:
  type: stdio
  settings:
    buffer_size: 8192

runtime:
  type: wasmtime

mcp:
  protocol_version: "2024-11-05"

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
      deny:
        - uri: "fs:///**"
          access: ["read", "write", "execute"]

    environment:
      allow:
        - key: "HOME"
        - key: "USER"
      deny:
        - key: "*_TOKEN"
        - key: "*_KEY"

    resources:
      limits:
        cpu: "100m"
        memory: "128Mi"
        execution_time: "30s"
"#;

    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert!(config.policy.is_some());

    let policy = config.policy.unwrap();
    assert_eq!(policy.version, "1.0");

    // Verify policy structure
    assert!(policy.core.network.is_some());
    assert!(policy.core.storage.is_some());
    assert!(policy.core.environment.is_some());
    assert!(policy.core.resources.is_some());
}

/// Test policy enforcement engine
#[test]
fn test_policy_engine_creation() {
    let policy = Policy {
        version: "1.0".to_string(),
        description: Some("Test policy".to_string()),
        core: mcpkit_rs_policy::CorePermissions {
            network: Some(mcpkit_rs_policy::NetworkPermissions {
                allow: vec![],
                deny: vec![mcpkit_rs_policy::NetworkRule {
                    host: "*".to_string(),
                    port: None,
                }],
            }),
            storage: None,
            environment: None,
            resources: None,
        },
        mcp: None,
    };

    // Create policy engine
    let engine = PolicyEngine::new(policy);

    // Test network permission check
    assert!(!engine.check_network_access("example.com", None));
    assert!(!engine.check_network_access("localhost", Some(8080)));
}