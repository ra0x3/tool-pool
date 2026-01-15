//! WASM tool loading and registry

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use wasmtime::Module;

use super::{CredentialProvider, WasmError, WasmRuntime, WasmToolManifest};
use crate::model::Tool;

/// A loaded WASM tool with its compiled module
#[derive(Clone)]
pub struct LoadedWasmTool {
    /// Tool manifest
    pub manifest: WasmToolManifest,

    /// Compiled WASM module
    pub compiled_module: Module,

    /// Path to the tool directory (for relative paths)
    pub base_path: PathBuf,
}

impl LoadedWasmTool {
    /// Convert to MCP Tool descriptor
    pub fn to_tool(&self) -> Tool {
        Tool {
            name: std::borrow::Cow::Owned(self.manifest.name.clone()),
            title: None,
            description: self
                .manifest
                .description
                .as_ref()
                .map(|d| std::borrow::Cow::Owned(d.clone())),
            input_schema: Arc::new(self.manifest.input_schema.clone()),
            output_schema: self
                .manifest
                .output_schema
                .as_ref()
                .map(|s| Arc::new(s.clone())),
            annotations: None,
            icons: None,
            meta: None,
        }
    }
}

/// Registry of loaded WASM tools
pub struct WasmToolRegistry {
    /// Loaded tools indexed by name
    tools: HashMap<String, LoadedWasmTool>,

    /// Credential provider
    credential_provider: Arc<dyn CredentialProvider>,

    /// Shared WASM runtime
    runtime: Arc<WasmRuntime>,
}

impl WasmToolRegistry {
    /// Create a new registry
    pub fn new(
        credential_provider: Arc<dyn CredentialProvider>,
        runtime: Arc<WasmRuntime>,
    ) -> Self {
        Self {
            tools: HashMap::new(),
            credential_provider,
            runtime,
        }
    }

    /// Load tools from a directory
    pub fn load_from_directory(
        tool_dir: impl AsRef<Path>,
        credential_provider: Arc<dyn CredentialProvider>,
    ) -> Result<Self, WasmError> {
        let tool_dir = tool_dir.as_ref();

        if !tool_dir.exists() {
            return Err(WasmError::LoadError(format!(
                "Tool directory does not exist: {}",
                tool_dir.display()
            )));
        }

        let runtime = Arc::new(WasmRuntime::new()?);
        let mut registry = Self::new(credential_provider, runtime);

        // Find all manifest files
        let entries = std::fs::read_dir(tool_dir)
            .map_err(|e| WasmError::LoadError(format!("Failed to read tool directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                WasmError::LoadError(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();

            // Look for directories with manifest.json files
            if path.is_dir() {
                let manifest_path = path.join("manifest.json");
                if manifest_path.exists() {
                    match registry.load_tool_from_manifest(&manifest_path) {
                        Ok(name) => {
                            tracing::info!("Loaded WASM tool: {}", name);
                        }
                        Err(e) => {
                            tracing::error!("Failed to load tool from {:?}: {}", manifest_path, e);
                        }
                    }
                }
            }
        }

        Ok(registry)
    }

    /// Load a single tool from a manifest file
    pub fn load_tool_from_manifest(
        &mut self,
        manifest_path: impl AsRef<Path>,
    ) -> Result<String, WasmError> {
        let manifest_path = manifest_path.as_ref();
        let base_path = manifest_path
            .parent()
            .ok_or_else(|| WasmError::LoadError("Invalid manifest path".to_string()))?;

        // Load and validate manifest
        let manifest = WasmToolManifest::from_file(manifest_path)?;
        manifest.validate()?;

        // Load WASM module
        let wasm_path = if manifest.wasm_module.is_absolute() {
            manifest.wasm_module.clone()
        } else {
            base_path.join(&manifest.wasm_module)
        };

        if !wasm_path.exists() {
            return Err(WasmError::LoadError(format!(
                "WASM module not found: {}",
                wasm_path.display()
            )));
        }

        let wasm_bytes = std::fs::read(&wasm_path)
            .map_err(|e| WasmError::LoadError(format!("Failed to read WASM module: {}", e)))?;

        // Compile the module
        let compiled_module = self.runtime.compile_module(&wasm_bytes)?;

        // Store the loaded tool
        let tool_name = manifest.name.clone();
        let loaded_tool = LoadedWasmTool {
            manifest,
            compiled_module,
            base_path: base_path.to_path_buf(),
        };

        self.tools.insert(tool_name.clone(), loaded_tool);

        Ok(tool_name)
    }

    /// Register a pre-loaded tool
    pub fn register_tool(&mut self, tool: LoadedWasmTool) -> Result<(), WasmError> {
        if self.tools.contains_key(&tool.manifest.name) {
            return Err(WasmError::LoadError(format!(
                "Tool already registered: {}",
                tool.manifest.name
            )));
        }

        self.tools.insert(tool.manifest.name.clone(), tool);
        Ok(())
    }

    /// Get a tool by name
    pub fn get_tool(&self, name: &str) -> Option<&LoadedWasmTool> {
        self.tools.get(name)
    }

    /// List all loaded tools
    pub fn list_tools(&self) -> Vec<Tool> {
        self.tools.values().map(|t| t.to_tool()).collect()
    }

    /// Get the credential provider
    pub fn credential_provider(&self) -> &Arc<dyn CredentialProvider> {
        &self.credential_provider
    }

    /// Get the runtime
    pub fn runtime(&self) -> &Arc<WasmRuntime> {
        &self.runtime
    }

    /// Get the number of loaded tools
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Check if a tool is loaded
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Unload a tool
    pub fn unload_tool(&mut self, name: &str) -> Option<LoadedWasmTool> {
        self.tools.remove(name)
    }

    /// Clear all tools
    pub fn clear(&mut self) {
        self.tools.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::credentials::InMemoryCredentialProvider;

    #[test]
    fn test_registry_creation() {
        let runtime = Arc::new(WasmRuntime::new().unwrap());
        let provider = Arc::new(InMemoryCredentialProvider::new());
        let registry = WasmToolRegistry::new(provider, runtime);

        assert_eq!(registry.tool_count(), 0);
        assert!(!registry.has_tool("test"));
    }

    #[test]
    fn test_loaded_tool_to_mcp_tool() {
        use serde_json::json;

        let manifest = WasmToolManifest {
            name: "test-tool".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A test tool".to_string()),
            wasm_module: PathBuf::from("test.wasm"),
            credentials: vec![],
            input_schema: json!({"type": "object"}).as_object().unwrap().clone(),
            output_schema: None,
            timeout_seconds: 30,
            max_memory_bytes: 50 * 1024 * 1024,
            max_fuel: None,
            env_vars: vec![],
        };

        // Create a minimal valid WASM module for testing
        // This is the smallest valid WASM module (empty module)
        let wasm_bytes = &[
            0x00, 0x61, 0x73, 0x6d, // WASM magic number
            0x01, 0x00, 0x00, 0x00, // Version 1
        ];

        let runtime = WasmRuntime::new().unwrap();
        let module = runtime
            .compile_module(wasm_bytes)
            .expect("Should compile minimal WASM module");

        let loaded_tool = LoadedWasmTool {
            manifest: manifest.clone(),
            compiled_module: module,
            base_path: PathBuf::from("/test"),
        };

        let tool = loaded_tool.to_tool();
        assert_eq!(tool.name, "test-tool");
        assert_eq!(
            tool.description,
            Some(std::borrow::Cow::Owned("A test tool".to_string()))
        );
    }
}
