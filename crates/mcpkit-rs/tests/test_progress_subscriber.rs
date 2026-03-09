#![cfg(all(feature = "client", feature = "server", feature = "macros"))]

use futures::StreamExt;
use mcpkit_rs::{
    ClientHandler, Peer, RoleServer, ServerHandler, ServiceExt,
    handler::{client::progress::ProgressDispatcher, server::tool::ToolRouter},
    model::{CallToolRequestParams, ClientRequest, Meta, ProgressNotificationParam, Request},
    service::PeerRequestOptions,
    tool, tool_handler, tool_router,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct MyClient {
    progress_handler: ProgressDispatcher,
}

impl MyClient {
    pub fn new() -> Self {
        Self {
            progress_handler: ProgressDispatcher::new(),
        }
    }
}

impl Default for MyClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientHandler for MyClient {
    async fn on_progress(
        &self,
        params: mcpkit_rs::model::ProgressNotificationParam,
        _context: mcpkit_rs::service::NotificationContext<mcpkit_rs::RoleClient>,
    ) {
        tracing::info!("Received progress notification: {:?}", params);
        self.progress_handler.handle_notification(params).await;
    }
}

pub struct MyServer {
    tool_router: ToolRouter<Self>,
}

impl MyServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for MyServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl MyServer {
    #[tool]
    pub async fn some_progress(
        meta: Meta,
        client: Peer<RoleServer>,
    ) -> Result<(), mcpkit_rs::ErrorData> {
        let progress_token =
            meta.get_progress_token()
                .ok_or(mcpkit_rs::ErrorData::invalid_params(
                    "Progress token is required for this tool",
                    None,
                ))?;
        // This test server processes requests inline; sending notifications synchronously
        // from within the request handler can deadlock the service loop.
        tokio::spawn(async move {
            for step in 0..10 {
                let _ = client
                    .notify_progress(ProgressNotificationParam {
                        progress_token: progress_token.clone(),
                        progress: step as f64,
                        total: Some(10.0),
                        message: Some("Some message".into()),
                    })
                    .await;
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });
        Ok(())
    }
}

#[tool_handler]
impl ServerHandler for MyServer {}

#[tokio::test]
async fn test_progress_subscriber() -> anyhow::Result<()> {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
    let client = MyClient::new();

    let server = MyServer::new();
    let (transport_server, transport_client) = tokio::io::duplex(4096);
    tokio::spawn(async move {
        let service = server.serve(transport_server).await?;
        service.waiting().await?;
        anyhow::Ok(())
    });
    let client_service = client.serve(transport_client).await?;
    let handle = client_service
        .send_cancellable_request(
            ClientRequest::CallToolRequest(Request::new(CallToolRequestParams::new(
                "some_progress",
            ))),
            PeerRequestOptions::no_options(),
        )
        .await?;
    let mut progress_subscriber = client_service
        .service()
        .progress_handler
        .subscribe(handle.progress_token.clone())
        .await;
    let _response = handle.await_response().await?;

    for step in 0..10 {
        let notification = tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            progress_subscriber.next(),
        )
        .await?
        .ok_or_else(|| anyhow::anyhow!("progress stream closed before step {step}"))?;
        assert_eq!(notification.progress, step as f64);
        assert_eq!(notification.total, Some(10.0));
    }

    Ok(())
}
