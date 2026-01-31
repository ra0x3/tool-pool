//! Local cache for bundles

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::RwLock,
};

use super::{Bundle, BundleError, parse_oci_uri};

/// Cache errors
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Bundle not found in cache: {0}")]
    NotFound(String),

    #[error("Cache corrupted: {0}")]
    Corrupted(String),

    #[error("Lock poisoned")]
    LockPoisoned,
}

/// Bundle cache for local storage
pub struct BundleCache {
    cache_dir: PathBuf,
    index: RwLock<HashMap<String, PathBuf>>,
}

impl BundleCache {
    /// Create a new bundle cache
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Result<Self, CacheError> {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&cache_dir)?;

        let mut cache = Self {
            cache_dir,
            index: RwLock::new(HashMap::new()),
        };

        cache.rebuild_index()?;
        Ok(cache)
    }

    /// Get default cache directory
    pub fn default_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".mcpkit")
            .join("bundles")
    }

    /// Rebuild the cache index by scanning the filesystem
    fn rebuild_index(&mut self) -> Result<(), CacheError> {
        let mut index = self.index.write().map_err(|_| CacheError::LockPoisoned)?;
        index.clear();

        // Scan cache directory for bundles
        self.scan_directory(&self.cache_dir.clone(), &mut index)?;

        Ok(())
    }

    /// Recursively scan directory for bundles
    fn scan_directory(
        &self,
        dir: &Path,
        index: &mut HashMap<String, PathBuf>,
    ) -> Result<(), CacheError> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Check if this is a bundle directory (contains module.wasm and config.yaml)
                let wasm_path = path.join("module.wasm");
                let config_path = path.join("config.yaml");

                if wasm_path.exists() && config_path.exists() {
                    // Try to reconstruct the URI from the path
                    if let Some(uri) = self.path_to_uri(&path) {
                        index.insert(uri, path.clone());
                    }
                } else {
                    // Recurse into subdirectory
                    self.scan_directory(&path, index)?;
                }
            }
        }

        Ok(())
    }

    /// Convert cache path back to URI
    fn path_to_uri(&self, path: &Path) -> Option<String> {
        let relative = path.strip_prefix(&self.cache_dir).ok()?;
        let components: Vec<&str> = relative
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();

        if components.len() >= 3 {
            // Format: registry/org/repo/version
            // Convert back to: oci://registry/org/repo:version
            let registry = components[0];
            let repo = components[1..components.len() - 1].join("/");
            let version = components[components.len() - 1];

            Some(format!("oci://{}/{}:{}", registry, repo, version))
        } else {
            None
        }
    }

    /// Get the cache path for a URI
    pub fn uri_to_path(&self, uri: &str) -> Result<PathBuf, BundleError> {
        let (registry, repository, tag) = parse_oci_uri(uri)?;
        let tag = tag.unwrap_or_else(|| "latest".to_string());

        // Create path: cache_dir/registry/repository/tag/
        let path = self.cache_dir.join(&registry).join(&repository).join(&tag);

        Ok(path)
    }

    /// Store a bundle in cache
    pub fn put(&self, uri: &str, bundle: &Bundle) -> Result<(), CacheError> {
        let path = self
            .uri_to_path(uri)
            .map_err(|e| CacheError::Corrupted(e.to_string()))?;

        // Create directory and save bundle
        std::fs::create_dir_all(&path)?;
        bundle
            .save_to_directory(&path)
            .map_err(|e| CacheError::IoError(std::io::Error::other(e.to_string())))?;

        // Update index
        let mut index = self.index.write().map_err(|_| CacheError::LockPoisoned)?;
        index.insert(uri.to_string(), path);

        Ok(())
    }

    /// Get a bundle from cache
    pub fn get(&self, uri: &str) -> Result<Bundle, CacheError> {
        let index = self.index.read().map_err(|_| CacheError::LockPoisoned)?;

        let path = index
            .get(uri)
            .ok_or_else(|| CacheError::NotFound(uri.to_string()))?;

        Bundle::from_directory(path)
            .map_err(|e| CacheError::IoError(std::io::Error::other(e.to_string())))
    }

    /// Check if a bundle exists in cache
    pub fn exists(&self, uri: &str) -> bool {
        let index = self.index.read().ok();
        index.is_some_and(|idx| idx.contains_key(uri))
    }

    /// Remove a bundle from cache
    pub fn remove(&self, uri: &str) -> Result<(), CacheError> {
        let mut index = self.index.write().map_err(|_| CacheError::LockPoisoned)?;

        if let Some(path) = index.remove(uri) {
            if path.exists() {
                std::fs::remove_dir_all(&path)?;
            }
        }

        Ok(())
    }

    /// Clear entire cache
    pub fn clear(&self) -> Result<(), CacheError> {
        let mut index = self.index.write().map_err(|_| CacheError::LockPoisoned)?;

        // Remove all cached bundles
        for path in index.values() {
            if path.exists() {
                std::fs::remove_dir_all(path)?;
            }
        }

        index.clear();
        Ok(())
    }

    /// List all cached bundles
    pub fn list(&self) -> Result<Vec<String>, CacheError> {
        let index = self.index.read().map_err(|_| CacheError::LockPoisoned)?;

        Ok(index.keys().cloned().collect())
    }

    /// Get cache statistics
    pub fn stats(&self) -> Result<CacheStats, CacheError> {
        let index = self.index.read().map_err(|_| CacheError::LockPoisoned)?;

        let mut total_size = 0u64;
        let mut bundle_count = 0usize;

        for path in index.values() {
            if path.exists() {
                bundle_count += 1;
                total_size += Self::dir_size(path)?;
            }
        }

        Ok(CacheStats {
            bundle_count,
            total_size,
            cache_dir: self.cache_dir.clone(),
        })
    }

    /// Calculate directory size recursively
    fn dir_size(path: &Path) -> Result<u64, CacheError> {
        let mut size = 0u64;

        if path.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    size += Self::dir_size(&path)?;
                } else {
                    size += entry.metadata()?.len();
                }
            }
        } else {
            size = std::fs::metadata(path)?.len();
        }

        Ok(size)
    }

    /// Verify cache integrity
    pub fn verify(&self) -> Result<Vec<String>, CacheError> {
        let index = self.index.read().map_err(|_| CacheError::LockPoisoned)?;

        let mut corrupted = Vec::new();

        for (uri, path) in index.iter() {
            // Check if files exist
            let wasm_path = path.join("module.wasm");
            let config_path = path.join("config.yaml");

            if !wasm_path.exists() || !config_path.exists() {
                corrupted.push(uri.clone());
                continue;
            }

            // Try to load and verify bundle
            if let Ok(bundle) = Bundle::from_directory(path) {
                if bundle.verify().is_err() {
                    corrupted.push(uri.clone());
                }
            } else {
                corrupted.push(uri.clone());
            }
        }

        Ok(corrupted)
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub bundle_count: usize,
    pub total_size: u64,
    pub cache_dir: PathBuf,
}

impl CacheStats {
    /// Format size in human-readable format
    pub fn format_size(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = self.total_size as f64;
        let mut unit_idx = 0;

        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_cache_operations() {
        let temp_dir = TempDir::new().unwrap();
        let cache = BundleCache::new(temp_dir.path()).unwrap();

        // Test URI to path conversion
        let uri = "oci://ghcr.io/org/tool:v1.0.0";
        let path = cache.uri_to_path(uri).unwrap();
        assert!(path.ends_with("ghcr.io/org/tool/v1.0.0"));

        // Test cache existence check
        assert!(!cache.exists(uri));

        // Create and store a bundle
        let bundle = Bundle::new(
            vec![0x00, 0x61, 0x73, 0x6d],
            b"version: 1.0".to_vec(),
            "ghcr.io/org/tool".to_string(),
            "v1.0.0".to_string(),
        );

        cache.put(uri, &bundle).unwrap();
        assert!(cache.exists(uri));

        // Retrieve bundle
        let retrieved = cache.get(uri).unwrap();
        assert_eq!(retrieved.wasm, bundle.wasm);
        assert_eq!(retrieved.config, bundle.config);

        // List cached bundles
        let list = cache.list().unwrap();
        assert_eq!(list.len(), 1);
        assert!(list.contains(&uri.to_string()));

        // Get stats
        let stats = cache.stats().unwrap();
        assert_eq!(stats.bundle_count, 1);
        assert!(stats.total_size > 0);

        // Verify cache
        let corrupted = cache.verify().unwrap();
        assert!(corrupted.is_empty());

        // Remove bundle
        cache.remove(uri).unwrap();
        assert!(!cache.exists(uri));

        // Clear cache
        cache.put(uri, &bundle).unwrap();
        cache.clear().unwrap();
        assert!(cache.list().unwrap().is_empty());
    }

    #[test]
    fn test_cache_path_conversion() {
        let temp_dir = TempDir::new().unwrap();
        let cache = BundleCache::new(temp_dir.path()).unwrap();

        // Test various URI formats
        let test_cases = vec![
            ("oci://ghcr.io/org/tool:latest", "ghcr.io/org/tool/latest"),
            ("oci://docker.io/user/app:v2.0", "docker.io/user/app/v2.0"),
            (
                "oci://localhost:5000/test/bundle:tag",
                "localhost:5000/test/bundle/tag",
            ),
        ];

        for (uri, expected_suffix) in test_cases {
            let path = cache.uri_to_path(uri).unwrap();
            assert!(path.to_string_lossy().ends_with(expected_suffix));
        }
    }

    #[test]
    fn test_stats_format_size() {
        let stats = CacheStats {
            bundle_count: 5,
            total_size: 1536, // 1.5 KB
            cache_dir: PathBuf::from("/tmp/cache"),
        };

        assert_eq!(stats.format_size(), "1.50 KB");

        let stats_mb = CacheStats {
            bundle_count: 10,
            total_size: 5_242_880, // 5 MB
            cache_dir: PathBuf::from("/tmp/cache"),
        };

        assert_eq!(stats_mb.format_size(), "5.00 MB");
    }
}
