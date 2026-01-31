//! Cache-specific tests

use mcpkit_rs::bundle::{Bundle, BundleCache};
use tempfile::TempDir;

/// Test cache corruption detection
#[test]
fn test_cache_corruption_detection() {
    let temp_dir = TempDir::new().unwrap();
    let cache = BundleCache::new(temp_dir.path()).unwrap();

    // Create and cache a bundle
    let bundle = Bundle::new(
        vec![0x00, 0x61, 0x73, 0x6d],
        b"version: 1.0".to_vec(),
        "test".to_string(),
        "1.0.0".to_string(),
    );

    let uri = "oci://test/bundle:v1";
    cache.put(uri, &bundle).unwrap();

    // Corrupt the cached WASM file
    let cache_path = cache.uri_to_path(uri).unwrap();
    let wasm_path = cache_path.join("module.wasm");
    std::fs::write(&wasm_path, b"corrupted").unwrap();

    // Verify should detect corruption
    let corrupted = cache.verify().unwrap();
    assert!(!corrupted.is_empty());
    assert!(corrupted.contains(&uri.to_string()));
}

/// Test cache size limits and cleanup
#[test]
fn test_cache_size_management() {
    let temp_dir = TempDir::new().unwrap();
    let cache = BundleCache::new(temp_dir.path()).unwrap();

    // Add multiple bundles
    for i in 0..10 {
        let bundle = Bundle::new(
            vec![0x00, 0x61, 0x73, 0x6d],
            format!("version: {}", i).into_bytes(),
            "test".to_string(),
            format!("{}.0.0", i),
        );

        let uri = format!("oci://test/bundle:v{}", i);
        cache.put(&uri, &bundle).unwrap();
    }

    // Check stats
    let stats = cache.stats().unwrap();
    assert_eq!(stats.bundle_count, 10);

    // Clear cache
    cache.clear().unwrap();
    let stats = cache.stats().unwrap();
    assert_eq!(stats.bundle_count, 0);
}