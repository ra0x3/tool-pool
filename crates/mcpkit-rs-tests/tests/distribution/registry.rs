//! Registry interaction tests

use wiremock::{Mock, MockServer, ResponseTemplate};
use wiremock::matchers::{method, path};

/// Test OCI registry error handling
#[tokio::test]
async fn test_registry_error_responses() {
    // Start mock registry server
    let mock_server = MockServer::start().await;

    // Mock 404 response for manifest
    Mock::given(method("GET"))
        .and(path("/v2/test/repo/manifests/latest"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let client = mcpkit_rs::bundle::BundleClient::new();

    // Try to pull non-existent bundle
    let uri = format!("oci://{}/test/repo:latest", mock_server.address());
    let result = client.pull(&uri, None).await;

    assert!(result.is_err());
    let error = result.unwrap_err().to_string();
    assert!(error.contains("404") || error.contains("not found"));
}

/// Test registry authentication flow
#[tokio::test]
async fn test_registry_auth_flow() {
    let mock_server = MockServer::start().await;

    // Mock 401 response without auth
    Mock::given(method("GET"))
        .and(path("/v2/test/repo/manifests/latest"))
        .respond_with(
            ResponseTemplate::new(401)
                .append_header("WWW-Authenticate", "Bearer realm=\"test\"")
        )
        .mount(&mock_server)
        .await;

    let client = mcpkit_rs::bundle::BundleClient::new();

    // Pull without auth should fail
    let uri = format!("oci://{}/test/repo:latest", mock_server.address());
    let result = client.pull(&uri, None).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Authentication"));
}