//! Filesystem permission mapping from policy to WASI
//!
//! This module translates high-level policy permissions ("read", "write", "execute")
//! into WASI-specific directory and file permissions.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use wasmtime_wasi::{DirPerms, FilePerms};

/// Maps filesystem permissions from policy to WASI
#[derive(Debug, Clone)]
pub struct FsPermissionMapper {
    policy: Option<Arc<mcpkit_rs_policy::CompiledPolicy>>,
}

/// Represents a directory to preopen with its permissions
#[derive(Debug, Clone)]
pub struct PreopenDir {
    /// Host path to preopen
    pub host_path: PathBuf,
    /// Guest path (how it appears inside WASM)
    pub guest_path: PathBuf,
    /// Directory permissions
    pub dir_perms: DirPerms,
    /// File permissions
    pub file_perms: FilePerms,
}

impl FsPermissionMapper {
    /// Create a new permission mapper with optional policy
    pub fn new(policy: Option<Arc<mcpkit_rs_policy::CompiledPolicy>>) -> Self {
        Self { policy }
    }

    /// Map policy access strings to WASI permissions
    pub fn map_to_wasi(access: &[String]) -> (DirPerms, FilePerms) {
        let mut dir_perms = DirPerms::empty();
        let mut file_perms = FilePerms::empty();

        for op in access {
            match op.as_str() {
                "read" => {
                    dir_perms |= DirPerms::READ;
                    file_perms |= FilePerms::READ;
                }
                "write" => {
                    dir_perms |= DirPerms::READ | DirPerms::MUTATE;
                    file_perms |= FilePerms::READ | FilePerms::WRITE;
                }
                "execute" => {
                    dir_perms |= DirPerms::READ;
                    file_perms |= FilePerms::READ;
                }
                _ => {
                    tracing::warn!("Unknown filesystem permission: {}", op);
                }
            }
        }

        (dir_perms, file_perms)
    }

    /// Extract directories to preopen from policy
    pub fn get_preopen_dirs(&self) -> Vec<PreopenDir> {
        let Some(policy) = &self.policy else {
            return vec![];
        };

        let mut preopen_dirs = Vec::new();
        let mut seen_paths = HashSet::new();

        for (pattern, access_set) in &policy.storage_access_map {
            let dir_path = Self::pattern_to_dir_path(pattern);

            if !seen_paths.insert(dir_path.clone()) {
                continue;
            }

            let access_vec: Vec<String> = access_set.iter().cloned().collect();
            let (dir_perms, file_perms) = Self::map_to_wasi(&access_vec);

            preopen_dirs.push(PreopenDir {
                host_path: dir_path.clone(),
                guest_path: dir_path,
                dir_perms,
                file_perms,
            });
        }

        preopen_dirs
    }

    /// Convert a glob pattern to the base directory that needs to be opened
    fn pattern_to_dir_path(pattern: &str) -> PathBuf {
        let pattern = pattern.strip_prefix("fs://").unwrap_or(pattern);

        let mut path = PathBuf::new();
        for component in Path::new(pattern).components() {
            let component_str = component.as_os_str().to_string_lossy();
            if component_str.contains('*') || component_str.contains('?') {
                break;
            }
            path.push(component);
        }

        if path.as_os_str().is_empty() {
            return PathBuf::from("/tmp");
        }

        path
    }

    /// Check if a specific operation is allowed for a path
    pub fn is_operation_allowed(&self, path: &str, operation: &str) -> bool {
        let Some(policy) = &self.policy else {
            return false;
        };

        policy.is_storage_allowed(path, operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_permission_mapping() {
        let access = vec!["read".to_string()];
        let (dir_perms, file_perms) = FsPermissionMapper::map_to_wasi(&access);

        assert!(dir_perms.contains(DirPerms::READ));
        assert!(file_perms.contains(FilePerms::READ));
        assert!(!dir_perms.contains(DirPerms::MUTATE));
        assert!(!file_perms.contains(FilePerms::WRITE));
    }

    #[test]
    fn test_write_permission_mapping() {
        let access = vec!["write".to_string()];
        let (dir_perms, file_perms) = FsPermissionMapper::map_to_wasi(&access);

        assert!(dir_perms.contains(DirPerms::READ | DirPerms::MUTATE));
        assert!(file_perms.contains(FilePerms::READ | FilePerms::WRITE));
    }

    #[test]
    fn test_combined_permissions() {
        let access = vec!["read".to_string(), "write".to_string()];
        let (dir_perms, file_perms) = FsPermissionMapper::map_to_wasi(&access);

        assert!(dir_perms.contains(DirPerms::READ | DirPerms::MUTATE));
        assert!(file_perms.contains(FilePerms::READ | FilePerms::WRITE));
    }

    #[test]
    fn test_pattern_to_dir_path() {
        assert_eq!(
            FsPermissionMapper::pattern_to_dir_path("/tmp/test/**"),
            PathBuf::from("/tmp/test")
        );

        assert_eq!(
            FsPermissionMapper::pattern_to_dir_path("fs:///var/log/*.log"),
            PathBuf::from("/var/log")
        );

        assert_eq!(
            FsPermissionMapper::pattern_to_dir_path("/home/user/docs/"),
            PathBuf::from("/home/user/docs")
        );
    }

    #[test]
    fn test_no_policy_returns_empty_dirs() {
        let mapper = FsPermissionMapper::new(None);
        let dirs = mapper.get_preopen_dirs();
        assert!(dirs.is_empty());
    }
}
