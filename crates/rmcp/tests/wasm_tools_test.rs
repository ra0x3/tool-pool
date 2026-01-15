//! Integration tests for WASM tool support

#![cfg(feature = "wasm-tools")]

use std::{path::PathBuf, sync::Arc};

use rmcp::wasm::{
    WasmContext, WasmRuntime, WasmToolExecutor, WasmToolRegistry,
    credentials::{CredentialProvider, CredentialValue, InMemoryCredentialProvider},
    manifest::{CredentialRequirement, CredentialType, WasmToolManifest},
};
use serde_json::json;

#[tokio::test]
async fn test_wasm_runtime_creation() {
    let runtime = WasmRuntime::new();
    assert!(runtime.is_ok(), "Should create WASM runtime successfully");
}

#[tokio::test]
async fn test_credential_provider() {
    let provider = InMemoryCredentialProvider::new();

    // Add some credentials
    provider
        .add_credential("api-key", CredentialValue::ApiKey("secret123".to_string()))
        .await;
    provider
        .add_credential(
            "auth",
            CredentialValue::BasicAuth {
                username: "user".to_string(),
                password: "pass".to_string(),
            },
        )
        .await;

    // Test resolution
    let req = CredentialRequirement {
        name: "api-key".to_string(),
        credential_type: CredentialType::ApiKey,
        required: true,
        env_var: None,
        description: None,
    };

    let result = provider.resolve(&req).await;
    assert!(result.is_ok());

    match result.unwrap() {
        CredentialValue::ApiKey(key) => assert_eq!(key, "secret123"),
        _ => panic!("Wrong credential type"),
    }

    // Test has_credential
    assert!(provider.has_credential("api-key").await);
    assert!(provider.has_credential("auth").await);
    assert!(!provider.has_credential("missing").await);
}

#[tokio::test]
async fn test_manifest_validation() {
    let valid_manifest = WasmToolManifest {
        name: "test-tool".to_string(),
        version: "1.0.0".to_string(),
        description: Some("Test tool".to_string()),
        wasm_module: PathBuf::from("tool.wasm"),
        credentials: vec![],
        input_schema: json!({"type": "object"}).as_object().unwrap().clone(),
        output_schema: None,
        timeout_seconds: 30,
        max_memory_bytes: 50 * 1024 * 1024,
        max_fuel: None,
        env_vars: vec![],
    };

    assert!(valid_manifest.validate().is_ok());

    // Test invalid manifest
    let invalid_manifest = WasmToolManifest {
        name: "".to_string(), // Empty name
        version: "1.0.0".to_string(),
        description: None,
        wasm_module: PathBuf::from("tool.wasm"),
        credentials: vec![],
        input_schema: json!({"type": "object"}).as_object().unwrap().clone(),
        output_schema: None,
        timeout_seconds: 30,
        max_memory_bytes: 50 * 1024 * 1024,
        max_fuel: None,
        env_vars: vec![],
    };

    assert!(invalid_manifest.validate().is_err());
}

#[tokio::test]
async fn test_wasm_context_builder() {
    let context = WasmContext::new()
        .with_stdin(b"test input".to_vec())
        .with_env("TEST_VAR".to_string(), "value".to_string())
        .with_timeout(std::time::Duration::from_secs(60));

    assert_eq!(context.stdin, b"test input");
    assert_eq!(context.env_vars.get("TEST_VAR"), Some(&"value".to_string()));
    assert_eq!(context.timeout, std::time::Duration::from_secs(60));
}

#[tokio::test]
async fn test_registry_operations() {
    let runtime = Arc::new(WasmRuntime::new().unwrap());
    let provider = Arc::new(InMemoryCredentialProvider::new());
    let registry = WasmToolRegistry::new(provider.clone(), runtime.clone());

    // Registry should start empty
    assert_eq!(registry.tool_count(), 0);
    assert!(!registry.has_tool("test"));
    assert_eq!(registry.list_tools().len(), 0);

    // Test loading non-existent directory
    let result =
        WasmToolRegistry::load_from_directory("/nonexistent/directory", provider.clone());
    assert!(result.is_err());
}

#[tokio::test]
async fn test_executor_tool_not_found() {
    let runtime = Arc::new(WasmRuntime::new().unwrap());
    let provider = Arc::new(InMemoryCredentialProvider::new());
    let registry = Arc::new(WasmToolRegistry::new(provider, runtime));
    let executor = WasmToolExecutor::new(registry);

    // Try to execute non-existent tool
    let result = executor
        .execute("nonexistent", serde_json::Map::new())
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message.contains("not found"));
}

#[tokio::test]
async fn test_credential_value_conversions() {
    // Test API key
    let api_key = CredentialValue::ApiKey("secret".to_string());
    assert_eq!(api_key.to_env_value(), "secret");

    // Test basic auth
    let basic_auth = CredentialValue::BasicAuth {
        username: "user".to_string(),
        password: "pass".to_string(),
    };
    assert_eq!(basic_auth.to_env_value(), "user:pass");

    let additional = basic_auth.additional_env_vars("AUTH");
    assert_eq!(additional.get("AUTH_USERNAME"), Some(&"user".to_string()));
    assert_eq!(additional.get("AUTH_PASSWORD"), Some(&"pass".to_string()));

    // Test bearer token
    let bearer = CredentialValue::BearerToken("token123".to_string());
    assert_eq!(bearer.to_env_value(), "token123");

    // Test OAuth2
    let oauth = CredentialValue::OAuth2Token("access_token".to_string());
    assert_eq!(oauth.to_env_value(), "access_token");
}

// Note: Full integration tests with actual WASM modules would require
// compiling test WASM modules, which is beyond the scope of this test file.
// In a production environment, you would have test fixtures with pre-compiled
// WASM modules to test actual execution.
