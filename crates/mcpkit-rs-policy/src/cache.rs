//! Thread-local caching for hot paths

use std::{
    cell::RefCell,
    hash::Hash,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use lru::LruCache;
use rustc_hash::FxHashMap;

use crate::error::Result;

thread_local! {
    static CACHE: RefCell<PermissionCache> = RefCell::new(PermissionCache::new(1024));
}

/// Hash type for action caching
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ActionHash {
    /// Tool execution action with tool name
    Tool(String),
    /// Network access action with host/URL
    Network(String),
    /// Storage access action with path and operation mode
    Storage(PathBuf, String),
    /// Environment variable access action with variable name
    Environment(String),
}

/// Thread-local permission cache for hot paths
pub struct PermissionCache {
    cache: LruCache<ActionHash, bool>,
    file_cache: FxHashMap<(PathBuf, AccessMode), bool>,
    network_cache: FxHashMap<String, bool>,
    tool_cache: FxHashMap<String, bool>,
    env_cache: FxHashMap<String, bool>,

    hits: u64,
    misses: u64,
}

/// File access mode for caching
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum AccessMode {
    /// Read access to file
    Read,
    /// Write access to file
    Write,
    /// Execute access to file
    Execute,
}

impl PermissionCache {
    /// Create a new permission cache with specified size
    pub fn new(size: usize) -> Self {
        PermissionCache {
            cache: LruCache::new(
                NonZeroUsize::new(size).unwrap_or(NonZeroUsize::new(1024).unwrap()),
            ),
            file_cache: FxHashMap::default(),
            network_cache: FxHashMap::default(),
            tool_cache: FxHashMap::default(),
            env_cache: FxHashMap::default(),
            hits: 0,
            misses: 0,
        }
    }

    /// Check cache for an action
    pub fn check(&mut self, action: &ActionHash) -> Option<bool> {
        let result = self.cache.get(action).copied();
        if result.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        result
    }

    /// Insert a result into the cache
    pub fn insert(&mut self, action: ActionHash, allowed: bool) {
        self.cache.put(action.clone(), allowed);

        // Also update specialized caches
        match action {
            ActionHash::Tool(name) => {
                self.tool_cache.insert(name, allowed);
            }
            ActionHash::Network(host) => {
                self.network_cache.insert(host, allowed);
            }
            ActionHash::Storage(path, mode) => {
                let access_mode = match mode.as_str() {
                    "read" => AccessMode::Read,
                    "write" => AccessMode::Write,
                    "execute" => AccessMode::Execute,
                    _ => AccessMode::Read,
                };
                self.file_cache.insert((path, access_mode), allowed);
            }
            ActionHash::Environment(key) => {
                self.env_cache.insert(key, allowed);
            }
        }
    }

    /// Check tool permission cache
    #[inline(always)]
    pub fn check_tool(&mut self, name: &str) -> Option<bool> {
        let result = self.tool_cache.get(name).copied();
        if result.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        result
    }

    /// Check network permission cache
    #[inline(always)]
    pub fn check_network(&mut self, host: &str) -> Option<bool> {
        let result = self.network_cache.get(host).copied();
        if result.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        result
    }

    /// Check file permission cache
    #[inline(always)]
    pub fn check_file(&mut self, path: &Path, mode: AccessMode) -> Option<bool> {
        let result = self.file_cache.get(&(path.to_path_buf(), mode)).copied();
        if result.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        result
    }

    /// Clear all caches
    pub fn clear(&mut self) {
        self.cache.clear();
        self.file_cache.clear();
        self.network_cache.clear();
        self.tool_cache.clear();
        self.env_cache.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            hit_rate: if self.hits + self.misses > 0 {
                self.hits as f64 / (self.hits + self.misses) as f64
            } else {
                0.0
            },
            total_items: self.cache.len()
                + self.file_cache.len()
                + self.network_cache.len()
                + self.tool_cache.len()
                + self.env_cache.len(),
        }
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Cache hit rate (0.0 to 1.0)
    pub hit_rate: f64,
    /// Total number of items in all caches
    pub total_items: usize,
}

/// Check permission with caching
pub fn check_with_cache<F>(action: ActionHash, check_fn: F) -> Result<bool>
where
    F: FnOnce() -> Result<bool>,
{
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();

        // Check cache first
        if let Some(result) = cache.check(&action) {
            return Ok(result);
        }

        // Perform actual check
        let result = check_fn()?;

        // Cache the result
        cache.insert(action, result);

        Ok(result)
    })
}

/// Clear thread-local cache
pub fn clear_cache() {
    CACHE.with(|cache| {
        cache.borrow_mut().clear();
    });
}

/// Get cache statistics
pub fn cache_stats() -> CacheStats {
    CACHE.with(|cache| cache.borrow().stats())
}
