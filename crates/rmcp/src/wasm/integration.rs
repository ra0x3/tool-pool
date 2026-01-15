//! Integration module for using WASM tools with MCP servers
//!
//! This module provides handlers and helpers to integrate WASM tools
//! with the existing ServerHandler trait.

use std::sync::Arc;

use super::{WasmToolExecutor, WasmToolRegistry};
use crate::{
    ErrorData,
    handler::server::ServerHandler,
    model::{CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams},
    service::{RequestContext, RoleServer},
};

/// A server handler that wraps WASM tools
#[derive(Clone)]
pub struct WasmToolHandler {
    /// The WASM tool executor
    executor: WasmToolExecutor,

    /// The tool registry
    registry: Arc<WasmToolRegistry>,
}

impl WasmToolHandler {
    /// Create a new WASM tool handler
    pub fn new(registry: Arc<WasmToolRegistry>) -> Self {
        let executor = WasmToolExecutor::new(registry.clone());
        Self { executor, registry }
    }

    /// Get the executor
    pub fn executor(&self) -> &WasmToolExecutor {
        &self.executor
    }

    /// Get the registry
    pub fn registry(&self) -> &Arc<WasmToolRegistry> {
        &self.registry
    }
}

impl ServerHandler for WasmToolHandler {
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = self.executor.list_tools();
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let arguments = request.arguments.unwrap_or_default();
        self.executor.execute(&request.name, arguments).await
    }
}

/// A composite handler that combines native and WASM tools
#[derive(Clone)]
pub struct CompositeToolHandler<H: ServerHandler> {
    /// The native tool handler
    native_handler: H,

    /// The WASM tool handler
    wasm_handler: WasmToolHandler,
}

impl<H: ServerHandler> CompositeToolHandler<H> {
    /// Create a new composite handler
    pub fn new(native_handler: H, wasm_registry: Arc<WasmToolRegistry>) -> Self {
        let wasm_handler = WasmToolHandler::new(wasm_registry);
        Self {
            native_handler,
            wasm_handler,
        }
    }
}

impl<H: ServerHandler + Send + Sync> ServerHandler for CompositeToolHandler<H> {
    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // Get tools from both handlers
        let mut native_result = self
            .native_handler
            .list_tools(request.clone(), context.clone())
            .await?;
        let wasm_result = self.wasm_handler.list_tools(request, context).await?;

        // Combine the tool lists
        native_result.tools.extend(wasm_result.tools);
        Ok(native_result)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        // Check if it's a WASM tool first
        if self.wasm_handler.executor.has_tool(&request.name) {
            self.wasm_handler.call_tool(request, context).await
        } else {
            // Fall back to native handler
            self.native_handler.call_tool(request, context).await
        }
    }
}

/// Helper to load WASM tools from a directory and create a handler
pub async fn load_wasm_tools_from_directory(
    tool_dir: impl AsRef<std::path::Path>,
    credential_provider: Arc<dyn super::CredentialProvider>,
) -> Result<WasmToolHandler, super::WasmError> {
    let registry = Arc::new(WasmToolRegistry::load_from_directory(
        tool_dir,
        credential_provider,
    )?);
    Ok(WasmToolHandler::new(registry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::credentials::InMemoryCredentialProvider;

    #[tokio::test]
    async fn test_wasm_handler_creation() {
        let runtime = Arc::new(super::super::runtime::WasmRuntime::new().unwrap());
        let provider = Arc::new(InMemoryCredentialProvider::new());
        let registry = Arc::new(WasmToolRegistry::new(provider, runtime));
        let handler = WasmToolHandler::new(registry);

        // Test without needing a real RequestContext
        // Just verify the handler is created correctly
        assert_eq!(handler.registry().tool_count(), 0);
        assert_eq!(handler.executor().list_tools().len(), 0);
    }
}
