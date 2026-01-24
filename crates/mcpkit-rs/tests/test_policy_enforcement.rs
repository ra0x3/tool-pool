//! Integration tests for policy enforcement middleware
//!
//! These tests demonstrate that:
//! 1. Vanilla MCP servers work unchanged without policy
//! 2. Policy enforcement is transparent to the MCP protocol
//! 3. Policy-enabled servers return standard MCP errors

#![cfg(all(feature = "server", feature = "client"))]

mod common;

use std::sync::Arc;

use common::handlers::TestClientHandler;
use mcpkit_rs::{ErrorData, ServerHandler, ServiceExt, model::*, service::RequestContext};
use tokio::sync::Mutex;

/// Test server that tracks tool calls
#[derive(Clone)]
struct TestToolServer {
    calls: Arc<Mutex<Vec<String>>>,
}

impl TestToolServer {
    fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn get_calls(&self) -> Vec<String> {
        self.calls.lock().await.clone()
    }
}

impl ServerHandler for TestToolServer {
    async fn list_tools(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: RequestContext<mcpkit_rs::service::RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: vec![
                Tool {
                    name: "read_file".into(),
                    title: None,
                    description: Some("Read a file".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" }
                            }
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    output_schema: None,
                    annotations: None,
                    icons: None,
                    meta: None,
                },
                Tool {
                    name: "write_file".into(),
                    title: None,
                    description: Some("Write a file".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "content": { "type": "string" }
                            }
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    output_schema: None,
                    annotations: None,
                    icons: None,
                    meta: None,
                },
                Tool {
                    name: "dangerous_tool".into(),
                    title: None,
                    description: Some("A dangerous operation".into()),
                    input_schema: Arc::new(serde_json::json!({}).as_object().unwrap().clone()),
                    output_schema: None,
                    annotations: None,
                    icons: None,
                    meta: None,
                },
            ],
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParams,
        _context: RequestContext<mcpkit_rs::service::RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        self.calls.lock().await.push(params.name.to_string());

        Ok(CallToolResult {
            content: vec![Content::text(format!("Executed: {}", params.name))],
            is_error: None,
            meta: None,
            structured_content: None,
        })
    }
}

#[tokio::test]
async fn test_vanilla_server_works_without_policy() {
    let server = TestToolServer::new();

    // Create a duplex channel for bidirectional communication
    let (server_transport, client_transport) = tokio::io::duplex(65536);

    // Spawn server
    let server_clone = server.clone();
    let server_handle = tokio::spawn(async move {
        let service = server_clone.serve(server_transport).await?;
        service.waiting().await?;
        anyhow::Ok(())
    });

    // Spawn client
    let client_handle = tokio::spawn(async move {
        let mut client = TestClientHandler::new(true, true)
            .serve(client_transport)
            .await?;

        // List tools - should see all tools
        let tools = client.peer().list_tools(None).await?;
        assert_eq!(tools.tools.len(), 3);

        // Call each tool - all should work
        for tool_name in ["read_file", "write_file", "dangerous_tool"] {
            let result = client
                .peer()
                .call_tool(CallToolRequestParams {
                    name: tool_name.into(),
                    arguments: Some(serde_json::json!({}).as_object().unwrap().clone()),
                    task: None,
                    meta: None,
                })
                .await?;

            assert_eq!(result.content.len(), 1);
        }

        client.close().await?;
        anyhow::Ok(())
    });

    // Wait for both to complete
    let (server_result, client_result) = tokio::join!(server_handle, client_handle);
    server_result.unwrap().unwrap();
    client_result.unwrap().unwrap();

    // Verify all tools were called
    let calls = server.get_calls().await;
    assert_eq!(calls.len(), 3);
    assert!(calls.contains(&"read_file".to_string()));
    assert!(calls.contains(&"write_file".to_string()));
    assert!(calls.contains(&"dangerous_tool".to_string()));
}

#[cfg(feature = "policy")]
#[tokio::test]
async fn test_policy_enforcement_transparent_errors() {
    use mcpkit_rs::PolicyEnabledServer;
    use mcpkit_rs_policy::Policy;

    let server = TestToolServer::new();

    // Create policy that only allows read_file
    let policy_yaml = r#"
version: "1.0"
extensions:
  mcp:
    tools:
      allow:
        - name: "read_file"
      deny: []
"#;
    let policy = Policy::from_yaml(policy_yaml).unwrap();

    let policy_server = PolicyEnabledServer::with_policy(server.clone(), policy).unwrap();

    // Create duplex channel
    let (server_transport, client_transport) = tokio::io::duplex(65536);

    // Spawn server with policy
    let server_handle = tokio::spawn(async move {
        let service = policy_server.serve(server_transport).await?;
        service.waiting().await?;
        anyhow::Ok(())
    });

    // Spawn client - it doesn't know about policy
    let client_handle = tokio::spawn(async move {
        let mut client = TestClientHandler::new(true, true)
            .serve(client_transport)
            .await?;

        // List tools - should only see allowed tool
        let tools = client.peer().list_tools(None).await?;
        assert_eq!(tools.tools.len(), 1);
        assert_eq!(tools.tools[0].name, "read_file");

        // Call allowed tool - should work
        let result = client
            .peer()
            .call_tool(CallToolRequestParams {
                name: "read_file".into(),
                arguments: Some(
                    serde_json::json!({"path": "/allowed/path"})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
                task: None,
                meta: None,
            })
            .await?;

        assert_eq!(result.content.len(), 1);

        // Call denied tool - should get standard MCP error
        let error = client
            .peer()
            .call_tool(CallToolRequestParams {
                name: "dangerous_tool".into(),
                arguments: Some(serde_json::json!({}).as_object().unwrap().clone()),
                task: None,
                meta: None,
            })
            .await
            .unwrap_err();

        // Error should contain access denied message
        let error_msg = error.to_string();
        assert!(error_msg.contains("Access denied") || error_msg.contains("-32602"));

        client.close().await?;
        anyhow::Ok(())
    });

    // Wait for completion
    let (server_result, client_result) = tokio::join!(server_handle, client_handle);
    server_result.unwrap().unwrap();
    client_result.unwrap().unwrap();

    // Only allowed tool should have been called
    let calls = server.get_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], "read_file");
}

#[cfg(feature = "policy")]
#[tokio::test]
async fn test_policy_with_resource_permissions() {
    use mcpkit_rs::PolicyEnabledServer;
    use mcpkit_rs_policy::Policy;

    #[derive(Clone)]
    struct ResourceServer;

    impl ServerHandler for ResourceServer {
        async fn list_resources(
            &self,
            _params: Option<PaginatedRequestParams>,
            _context: RequestContext<mcpkit_rs::service::RoleServer>,
        ) -> Result<ListResourcesResult, ErrorData> {
            Ok(ListResourcesResult {
                resources: vec![
                    Resource {
                        raw: mcpkit_rs::model::RawResource {
                            uri: "file:///etc/passwd".into(),
                            name: "System Password File".to_string(),
                            title: None,
                            description: None,
                            mime_type: None,
                            size: None,
                            icons: None,
                            meta: None,
                        },
                        annotations: None,
                    },
                    Resource {
                        raw: mcpkit_rs::model::RawResource {
                            uri: "file:///home/user/document.txt".into(),
                            name: "User Document".to_string(),
                            title: None,
                            description: None,
                            mime_type: None,
                            size: None,
                            icons: None,
                            meta: None,
                        },
                        annotations: None,
                    },
                ],
                next_cursor: None,
                meta: None,
            })
        }

        async fn read_resource(
            &self,
            params: ReadResourceRequestParams,
            _context: RequestContext<mcpkit_rs::service::RoleServer>,
        ) -> Result<ReadResourceResult, ErrorData> {
            Ok(ReadResourceResult {
                contents: vec![mcpkit_rs::model::ResourceContents::TextResourceContents {
                    text: format!("Contents of {}", params.uri),
                    uri: params.uri.clone(),
                    mime_type: None,
                    meta: None,
                }],
            })
        }
    }

    // Create policy that only allows /home/** paths
    let policy_yaml = r#"
version: "1.0"
core:
  storage:
    allow:
      - uri: "file:///home/**"
        access: ["read", "write"]
    deny: []
"#;
    let policy = Policy::from_yaml(policy_yaml).unwrap();

    let policy_server = PolicyEnabledServer::with_policy(ResourceServer, policy).unwrap();

    // Create duplex channel
    let (server_transport, client_transport) = tokio::io::duplex(65536);

    // Spawn server
    let server_handle = tokio::spawn(async move {
        let service = policy_server.serve(server_transport).await?;
        service.waiting().await?;
        anyhow::Ok(())
    });

    // Spawn client
    let client_handle = tokio::spawn(async move {
        let mut client = TestClientHandler::new(true, true)
            .serve(client_transport)
            .await?;

        // List resources - should see all (no filtering for list)
        let resources = client.peer().list_resources(None).await?;
        assert_eq!(resources.resources.len(), 2);

        // Try to read allowed resource
        let result = client
            .peer()
            .read_resource(ReadResourceRequestParams {
                uri: "file:///home/user/document.txt".into(),
                meta: None,
            })
            .await?;

        assert_eq!(result.contents.len(), 1);

        // Try to read denied resource
        let error = client
            .peer()
            .read_resource(ReadResourceRequestParams {
                uri: "file:///etc/passwd".into(),
                meta: None,
            })
            .await
            .unwrap_err();

        // Error should contain access denied message
        let error_msg = error.to_string();
        assert!(error_msg.contains("Access denied") || error_msg.contains("-32602"));

        client.close().await?;
        anyhow::Ok(())
    });

    let (server_result, client_result) = tokio::join!(server_handle, client_handle);
    server_result.unwrap().unwrap();
    client_result.unwrap().unwrap();
}

#[cfg(feature = "policy")]
#[tokio::test]
async fn test_policy_runtime_agnostic() {
    use mcpkit_rs::PolicyEnabledServer;
    use mcpkit_rs_policy::Policy;

    // Create identical policies
    let policy_yaml = r#"
version: "1.0"
core: {}
"#;
    let policy1 = Policy::from_yaml(policy_yaml).unwrap();
    let policy2 = policy1.clone();

    // Can be used with any ServerHandler implementation
    let server1 = PolicyEnabledServer::with_policy(TestToolServer::new(), policy1).unwrap();
    let _server2 = PolicyEnabledServer::with_policy(TestToolServer::new(), policy2).unwrap();

    // Policy enforcement is transport-agnostic
    assert!(server1.has_policy());
}
