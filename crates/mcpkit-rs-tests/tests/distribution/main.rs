//! Distribution system integration tests

use std::env;
use std::path::PathBuf;

use mcpkit_rs::bundle::{Bundle, BundleCache, BundleClient};
use mcpkit_rs_config::{Config, RegistryAuth};
use tempfile::TempDir;

mod push_pull;
mod cache;
mod registry;

/// Test bundle push and pull cycle with a local registry
///
/// This test requires a local OCI registry to be running.
/// You can start one with: `docker run -d -p 5000:5000 registry:2`
#[tokio::test]
#[ignore] // Ignored by default since it requires a registry
async fn test_push_pull_cycle() {
    // Check if test registry is available
    let registry_url = env::var("TEST_REGISTRY_URL").unwrap_or_else(|_| "localhost:5000".to_string());

    // Create test WASM module and config
    let wasm_content = include_bytes!("../../../../examples/wasm/wasmtime/calculator/calculator.wasm");
    let config_content = include_bytes!("../../../../examples/wasm/wasmtime/calculator/config.yaml");

    // Create bundle client
    let client = BundleClient::new();

    // Test URI (no auth needed for local registry)
    let uri = format!("oci://{}/test/calculator:test", registry_url);

    // Push bundle
    let push_result = client.push(
        wasm_content,
        config_content,
        &uri,
        None, // No auth for local registry
    ).await;

    // Allow push to fail if registry isn't running
    if push_result.is_err() {
        eprintln!("Skipping test - registry not available at {}", registry_url);
        return;
    }

    let digest = push_result.unwrap();
    assert!(!digest.is_empty());

    // Pull bundle back
    let bundle = client.pull(&uri, None).await.unwrap();

    // Verify content matches
    assert_eq!(bundle.wasm, wasm_content);
    assert_eq!(bundle.config, config_content);

    // Verify bundle integrity
    assert!(bundle.verify().is_ok());
}

/// Test bundle caching functionality
#[test]
fn test_bundle_cache() {
    // Create temporary cache directory
    let temp_dir = TempDir::new().unwrap();
    let cache = BundleCache::new(temp_dir.path()).unwrap();

    // Create test bundle
    let bundle = Bundle::new(
        vec![0x00, 0x61, 0x73, 0x6d], // WASM magic
        b"version: 1.0".to_vec(),
        "test-registry".to_string(),
        "1.0.0".to_string(),
    );

    let uri = "oci://test-registry/org/tool:v1.0.0";

    // Cache should be empty initially
    assert!(!cache.exists(uri));

    // Store bundle in cache
    cache.put(uri, &bundle).unwrap();
    assert!(cache.exists(uri));

    // Retrieve from cache
    let retrieved = cache.get(uri).unwrap();
    assert_eq!(retrieved.wasm, bundle.wasm);
    assert_eq!(retrieved.config, bundle.config);

    // List cached bundles
    let list = cache.list().unwrap();
    assert_eq!(list.len(), 1);
    assert!(list.contains(&uri.to_string()));

    // Get cache stats
    let stats = cache.stats().unwrap();
    assert_eq!(stats.bundle_count, 1);
    assert!(stats.total_size > 0);

    // Verify cache integrity
    let corrupted = cache.verify().unwrap();
    assert!(corrupted.is_empty());

    // Remove from cache
    cache.remove(uri).unwrap();
    assert!(!cache.exists(uri));
}

/// Test configuration with distribution section
#[test]
fn test_config_with_distribution() {
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

distribution:
  registry: "ghcr.io/test/bundle"
  version: "1.0.0"
  tags:
    - "latest"
    - "v1.0.0"
  metadata:
    authors:
      - "Test Author"
    license: "MIT"
    repository: "https://github.com/test/repo"
    keywords:
      - "test"
      - "bundle"
  include:
    - "module.wasm"
    - "config.yaml"
  auth:
    username: "${TEST_USER}"
    password: "${TEST_TOKEN}"
"#;

    // Parse configuration
    let config: Config = serde_yaml::from_str(yaml).unwrap();

    // Verify distribution section
    assert!(config.distribution.is_some());
    let dist = config.distribution.unwrap();
    assert_eq!(dist.registry, "ghcr.io/test/bundle");
    assert_eq!(dist.version, Some("1.0.0".to_string()));
    assert_eq!(dist.tags, vec!["latest", "v1.0.0"]);

    // Verify metadata
    assert!(dist.metadata.is_some());
    let meta = dist.metadata.unwrap();
    assert_eq!(meta.authors, vec!["Test Author"]);
    assert_eq!(meta.license, Some("MIT".to_string()));
    assert_eq!(meta.keywords, vec!["test", "bundle"]);

    // Verify auth
    assert!(dist.auth.is_some());
    let auth = dist.auth.unwrap();
    assert_eq!(auth.username, Some("${TEST_USER}".to_string()));
    assert_eq!(auth.password, Some("${TEST_TOKEN}".to_string()));
}

/// Test GitHub Container Registry integration
///
/// This test requires GitHub credentials to be set:
/// - GITHUB_USER: Your GitHub username
/// - GITHUB_TOKEN: A GitHub personal access token with `write:packages` scope
#[tokio::test]
#[ignore] // Ignored by default since it requires credentials
async fn test_github_registry() {
    // Check for GitHub credentials
    let username = env::var("GITHUB_USER");
    let token = env::var("GITHUB_TOKEN");

    if username.is_err() || token.is_err() {
        eprintln!("Skipping test - GITHUB_USER and GITHUB_TOKEN not set");
        return;
    }

    // Create test bundle
    let wasm = b"fake wasm content for testing".to_vec();
    let config = b"version: 1.0\ntest: true".to_vec();

    // Create bundle client with cache
    let temp_dir = TempDir::new().unwrap();
    let cache = BundleCache::new(temp_dir.path()).unwrap();
    let client = BundleClient::with_cache(cache);

    // Test URI - use a test namespace to avoid conflicts
    let uri = format!("oci://ghcr.io/{}/mcpkit-test:integration", username.unwrap());

    // Create auth config
    let auth = RegistryAuth {
        username: Some("${GITHUB_USER}".to_string()),
        password: Some("${GITHUB_TOKEN}".to_string()),
        auth_file: None,
        use_keychain: false,
    };

    // Push to GitHub Container Registry
    let push_result = client.push(&wasm, &config, &uri, Some(&auth)).await;

    match push_result {
        Ok(digest) => {
            println!("Successfully pushed to GitHub: {}", digest);

            // Pull back from registry
            let bundle = client.pull(&uri, Some(&auth)).await.unwrap();
            assert_eq!(bundle.wasm, wasm);
            assert_eq!(bundle.config, config);
        }
        Err(e) => {
            eprintln!("Failed to push to GitHub (may need permissions): {}", e);
        }
    }
}

/// Test distribution with calculator example
#[tokio::test]
async fn test_calculator_distribution() {
    // Load calculator example files
    let calc_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/wasm/wasmtime/calculator");

    let wasm_path = calc_dir.join("calculator.wasm");
    let config_path = calc_dir.join("config.yaml");

    if !wasm_path.exists() || !config_path.exists() {
        eprintln!("Calculator example not built - skipping test");
        return;
    }

    // Load files
    let wasm = std::fs::read(&wasm_path).unwrap();
    let config_bytes = std::fs::read(&config_path).unwrap();

    // Parse config to verify distribution section
    let config: Config = serde_yaml::from_slice(&config_bytes).unwrap();
    assert!(config.distribution.is_some());

    let dist = config.distribution.unwrap();
    assert_eq!(dist.registry, "ghcr.io/ra0x3/mcpkit-calculator");
    assert!(dist.tags.contains(&"latest".to_string()));

    // Create bundle
    let bundle = Bundle::new(
        wasm,
        config_bytes,
        dist.registry.clone(),
        dist.version.unwrap_or_else(|| config.server.version.clone()),
    );

    // Verify bundle
    assert!(bundle.verify().is_ok());

    // Test caching
    let temp_dir = TempDir::new().unwrap();
    let cache = BundleCache::new(temp_dir.path()).unwrap();

    let uri = format!("oci://{}:latest", dist.registry);
    cache.put(&uri, &bundle).unwrap();

    // Verify cached bundle
    let cached = cache.get(&uri).unwrap();
    assert_eq!(cached.wasm.len(), bundle.wasm.len());
    assert_eq!(cached.config, bundle.config);
}