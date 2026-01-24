//! WASM tool executor

use std::{sync::Arc, time::Duration};

use super::{WasmContext, WasmError, WasmToolRegistry};
use crate::{
    ErrorData,
    model::{CallToolResult, Content, JsonObject},
};

/// Executor for WASM tools
#[derive(Clone)]
pub struct WasmToolExecutor {
    /// Tool registry
    registry: Arc<WasmToolRegistry>,

    #[cfg(feature = "config")]
    /// Optional server configuration
    config: Option<Arc<crate::config::ServerConfig>>,
}

impl WasmToolExecutor {
    /// Create a new executor
    pub fn new(registry: Arc<WasmToolRegistry>) -> Self {
        Self {
            registry,
            #[cfg(feature = "config")]
            config: None,
        }
    }

    #[cfg(feature = "config")]
    /// Create a new executor with configuration
    pub fn with_config(
        registry: Arc<WasmToolRegistry>,
        config: Arc<crate::config::ServerConfig>,
    ) -> Self {
        Self {
            registry,
            config: Some(config),
        }
    }

    /// Execute a WASM tool
    pub async fn execute(
        &self,
        tool_name: &str,
        arguments: JsonObject,
    ) -> Result<CallToolResult, ErrorData> {
        // Check policy if configured
        #[cfg(feature = "config")]
        if let Some(ref config) = self.config {
            if !config.is_tool_allowed(tool_name) {
                return Err(ErrorData::invalid_request(
                    format!("Tool '{}' is not allowed by policy", tool_name),
                    None,
                ));
            }
        }

        // Get the tool
        let tool = self
            .registry
            .get_tool(tool_name)
            .ok_or_else(|| WasmError::ToolNotFound(tool_name.to_string()))?;

        // Serialize arguments to JSON for stdin
        let input_json = serde_json::to_vec(&arguments).map_err(|e| {
            ErrorData::invalid_params(format!("Failed to serialize input: {}", e), None)
        })?;

        // Prepare execution context
        #[cfg(feature = "config")]
        let mut context = if let Some(ref config) = self.config {
            // Use config-based context with policy limits
            config
                .create_wasm_context()
                .with_stdin(input_json)
                .with_timeout(Duration::from_secs(tool.manifest.timeout_seconds))
        } else {
            WasmContext::new()
                .with_stdin(input_json)
                .with_timeout(Duration::from_secs(tool.manifest.timeout_seconds))
        };

        #[cfg(not(feature = "config"))]
        let mut context = WasmContext::new()
            .with_stdin(input_json)
            .with_timeout(Duration::from_secs(tool.manifest.timeout_seconds));

        // Set fuel limit if configured
        if let Some(max_fuel) = tool.manifest.max_fuel {
            context = context.with_max_fuel(max_fuel);
        }

        // Set static environment variables
        for env_var in &tool.manifest.env_vars {
            context = context.with_env(env_var.name.clone(), env_var.value.clone());
        }

        // Resolve and inject credentials
        for credential_req in &tool.manifest.credentials {
            match self
                .registry
                .credential_provider()
                .resolve(credential_req)
                .await
            {
                Ok(cred_value) => {
                    // Get the environment variable name
                    let env_name = tool
                        .manifest
                        .get_credential_env_var(&credential_req.name)
                        .unwrap_or_else(|| credential_req.name.to_uppercase().replace('-', "_"));

                    // Set the main credential value
                    context = context.with_env(env_name.clone(), cred_value.to_env_value());

                    // Set any additional environment variables (e.g., for BasicAuth)
                    for (key, value) in cred_value.additional_env_vars(&env_name) {
                        context = context.with_env(key, value);
                    }
                }
                Err(e) => {
                    if credential_req.required {
                        return Err(ErrorData::internal_error(
                            format!(
                                "Failed to resolve required credential '{}': {}",
                                credential_req.name, e
                            ),
                            None,
                        ));
                    }
                    // Optional credential - log warning but continue
                    tracing::warn!(
                        "Failed to resolve optional credential '{}': {}",
                        credential_req.name,
                        e
                    );
                }
            }
        }

        // Execute the WASM module
        let output = self
            .registry
            .runtime()
            .execute(&tool.compiled_module, context)
            .await
            .map_err(|e| match e {
                WasmError::Timeout => ErrorData::internal_error(
                    format!("Tool '{}' execution timeout", tool_name),
                    None,
                ),
                _ => ErrorData::internal_error(
                    format!("Tool '{}' execution failed: {}", tool_name, e),
                    None,
                ),
            })?;

        // Parse the output as JSON
        let output_json: serde_json::Value = serde_json::from_slice(&output).map_err(|e| {
            ErrorData::internal_error(
                format!("Tool '{}' produced invalid JSON output: {}", tool_name, e),
                None,
            )
        })?;

        // Convert to CallToolResult
        // The tool should output a JSON object with optional "error" and "content" fields
        if let Some(obj) = output_json.as_object() {
            // Check if it's an error response
            if let Some(error) = obj.get("error") {
                if let Some(error_str) = error.as_str() {
                    return Ok(CallToolResult::error(vec![Content::text(error_str)]));
                }
            }

            // Extract content (default to the entire object if no "content" field)
            let content = if let Some(content_value) = obj.get("content") {
                // If there's a specific content field, use it
                vec![Content::text(content_value.to_string())]
            } else {
                // Otherwise, use the entire output
                vec![Content::text(output_json.to_string())]
            };

            Ok(CallToolResult {
                content,
                structured_content: None,
                is_error: Some(false),
                meta: None,
            })
        } else {
            // Non-object output - treat as plain content
            Ok(CallToolResult::success(vec![Content::text(
                output_json.to_string(),
            )]))
        }
    }

    /// Check if a tool is available
    pub fn has_tool(&self, name: &str) -> bool {
        self.registry.has_tool(name)
    }

    /// List all available tools
    pub fn list_tools(&self) -> Vec<crate::model::Tool> {
        self.registry.list_tools()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::{credentials::InMemoryCredentialProvider, runtime::WasmRuntime};

    #[tokio::test]
    async fn test_executor_tool_not_found() {
        let runtime = Arc::new(WasmRuntime::new().unwrap());
        let provider = Arc::new(InMemoryCredentialProvider::new());
        let registry = Arc::new(WasmToolRegistry::new(provider, runtime));
        let executor = WasmToolExecutor::new(registry);

        let result = executor.execute("nonexistent", JsonObject::new()).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("not found"));
    }

    #[test]
    fn test_executor_has_tool() {
        let runtime = Arc::new(WasmRuntime::new().unwrap());
        let provider = Arc::new(InMemoryCredentialProvider::new());
        let registry = Arc::new(WasmToolRegistry::new(provider, runtime));
        let executor = WasmToolExecutor::new(registry);

        assert!(!executor.has_tool("test"));
        assert_eq!(executor.list_tools().len(), 0);
    }

    // More comprehensive tests would require actual WASM modules
    // or mock implementations
}
