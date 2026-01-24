//! Policy enforcement middleware for MCP servers
//!
//! This module provides transparent policy enforcement that works with any
//! ServerHandler implementation without modifying the MCP protocol.

use std::sync::Arc;

use crate::{
    error::ErrorData,
    handler::server::ServerHandler,
    model::*,
    service::{NotificationContext, RequestContext, RoleServer},
};

/// A server handler wrapper that enforces policies transparently
///
/// This wrapper can be applied to any existing ServerHandler implementation
/// to add policy enforcement without changing the MCP protocol or breaking
/// backwards compatibility.
#[derive(Clone)]
pub struct PolicyEnabledServer<H: ServerHandler> {
    inner: H,
    policy: Option<Arc<mcpkit_rs_policy::CompiledPolicy>>,
}

impl<H: ServerHandler> PolicyEnabledServer<H> {
    /// Create a new policy-enabled server wrapping an existing handler
    pub fn new(inner: H) -> Self {
        Self {
            inner,
            policy: None,
        }
    }

    /// Create a new policy-enabled server with a specific policy
    pub fn with_policy(inner: H, policy: mcpkit_rs_policy::Policy) -> Result<Self, ErrorData> {
        let compiled =
            mcpkit_rs_policy::CompiledPolicy::compile(&policy).map_err(|e| ErrorData {
                code: crate::model::ErrorCode(-32603),
                message: format!("Failed to compile policy: {}", e).into(),
                data: None,
            })?;

        Ok(Self {
            inner,
            policy: Some(Arc::new(compiled)),
        })
    }

    /// Create from a pre-compiled policy
    pub fn with_compiled_policy(inner: H, policy: Arc<mcpkit_rs_policy::CompiledPolicy>) -> Self {
        Self {
            inner,
            policy: Some(policy),
        }
    }

    /// Get a reference to the inner handler
    pub fn inner(&self) -> &H {
        &self.inner
    }

    /// Get a mutable reference to the inner handler
    pub fn inner_mut(&mut self) -> &mut H {
        &mut self.inner
    }

    /// Check if policy enforcement is enabled
    pub fn has_policy(&self) -> bool {
        self.policy.is_some()
    }

    /// Standard MCP error for permission denied
    fn permission_denied(action: &str, resource: &str) -> ErrorData {
        ErrorData {
            code: crate::model::ErrorCode(-32602), // Invalid params - standard JSON-RPC error
            message: format!("Access denied: {} for {}", action, resource).into(),
            data: None,
        }
    }
}

impl<H: ServerHandler> ServerHandler for PolicyEnabledServer<H> {
    async fn initialize(
        &self,
        params: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        self.inner.initialize(params, context).await
    }

    async fn ping(&self, context: RequestContext<RoleServer>) -> Result<(), ErrorData> {
        self.inner.ping(context).await
    }

    async fn list_tools(
        &self,
        params: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let result = self.inner.list_tools(params, context).await?;

        // Filter tools based on policy if enabled
        if let Some(policy) = &self.policy {
            let filtered_tools = result
                .tools
                .into_iter()
                .filter(|tool| policy.is_tool_allowed(&tool.name))
                .collect();

            Ok(ListToolsResult {
                tools: filtered_tools,
                next_cursor: result.next_cursor,
                meta: result.meta,
            })
        } else {
            Ok(result)
        }
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        // Check tool permission if policy is enabled
        if let Some(policy) = &self.policy {
            if !policy.is_tool_allowed(&params.name) {
                return Err(Self::permission_denied("tool", &params.name));
            }
        }

        self.inner.call_tool(params, context).await
    }

    async fn list_resources(
        &self,
        params: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        self.inner.list_resources(params, context).await
    }

    async fn read_resource(
        &self,
        params: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        // Check resource permission if policy is enabled
        if let Some(policy) = &self.policy {
            if !policy.is_storage_allowed(&params.uri, "read") {
                return Err(Self::permission_denied("resource", &params.uri));
            }
        }

        self.inner.read_resource(params, context).await
    }

    async fn list_prompts(
        &self,
        params: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        self.inner.list_prompts(params, context).await
    }

    async fn get_prompt(
        &self,
        params: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, ErrorData> {
        self.inner.get_prompt(params, context).await
    }

    async fn complete(
        &self,
        params: CompleteRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CompleteResult, ErrorData> {
        self.inner.complete(params, context).await
    }

    async fn set_level(
        &self,
        params: SetLevelRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.inner.set_level(params, context).await
    }

    async fn list_resource_templates(
        &self,
        params: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        self.inner.list_resource_templates(params, context).await
    }

    async fn subscribe(
        &self,
        params: SubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.inner.subscribe(params, context).await
    }

    async fn unsubscribe(
        &self,
        params: UnsubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.inner.unsubscribe(params, context).await
    }

    async fn on_custom_request(
        &self,
        request: CustomRequest,
        context: RequestContext<RoleServer>,
    ) -> Result<CustomResult, ErrorData> {
        self.inner.on_custom_request(request, context).await
    }

    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        self.inner.on_initialized(context).await
    }

    async fn on_custom_notification(
        &self,
        notification: CustomNotification,
        context: NotificationContext<RoleServer>,
    ) {
        self.inner
            .on_custom_notification(notification, context)
            .await
    }
}
