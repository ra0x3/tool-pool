//! Push and pull operation tests

use mcpkit_rs::bundle::BundleClient;
use serial_test::serial;

/// Test pushing to a registry that requires authentication without providing credentials
#[tokio::test]
#[serial]
async fn test_push_without_auth() {
    let client = BundleClient::new();

    let wasm = b"test".to_vec();
    let config = b"version: 1.0".to_vec();

    // Try to push to GitHub without auth
    let result = client.push(
        &wasm,
        &config,
        "oci://ghcr.io/test/unauthorized:latest",
        None,
    ).await;

    assert!(result.is_err());
    let error = result.unwrap_err().to_string();
    assert!(error.contains("Authentication") || error.contains("401"));
}

/// Test pulling from a private repository without authentication
#[tokio::test]
#[serial]
async fn test_pull_private_without_auth() {
    let client = BundleClient::new();

    // Try to pull a private bundle without auth
    let result = client.pull(
        "oci://ghcr.io/private/repo:latest",
        None,
    ).await;

    // Should fail with auth error
    assert!(result.is_err());
}