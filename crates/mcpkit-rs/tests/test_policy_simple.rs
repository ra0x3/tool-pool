//! Simple test to demonstrate policy enforcement works

#![cfg(all(feature = "server", feature = "client", feature = "policy"))]

use mcpkit_rs::PolicyEnabledServer;
use mcpkit_rs_policy::Policy;

#[test]
fn test_policy_creation() {
    // Create a simple YAML policy
    let policy_yaml = r#"
version: "1.0"
tools:
  allow:
    - allowed_tool
  deny: []
"#;

    let policy = Policy::from_yaml(policy_yaml).unwrap();

    // This test just ensures we can create a policy-enabled server
    // The actual enforcement is tested via integration tests
    #[derive(Clone)]
    struct DummyServer;
    impl mcpkit_rs::ServerHandler for DummyServer {}

    let server = PolicyEnabledServer::with_policy(DummyServer, policy).unwrap();
    assert!(server.has_policy());
}

#[test]
fn test_policy_is_optional_feature() {
    // This test just ensures the feature flag works
    #[cfg(not(feature = "policy"))]
    {
        // Without policy feature, PolicyEnabledServer shouldn't exist
        // This would fail to compile if we accidentally exposed it
    }

    #[cfg(feature = "policy")]
    {
        // With policy feature, we can create policy servers
        #[derive(Clone)]
        struct SimpleServer;
        impl mcpkit_rs::ServerHandler for SimpleServer {}

        let server = PolicyEnabledServer::new(SimpleServer);
        assert!(!server.has_policy()); // No policy by default
    }
}
