use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_router, ErrorData, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{fs, io::AsyncReadExt};

// ===== FILE SYSTEM OPERATIONS =====

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListFilesRequest {
    #[schemars(description = "Directory path to list")]
    pub path: String,
    #[schemars(description = "Optional glob pattern to filter files")]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReadFileRequest {
    #[schemars(description = "File path to read")]
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GrepRequest {
    #[schemars(description = "Pattern to search for")]
    pub pattern: String,
    #[schemars(description = "Directory to search in")]
    pub path: String,
}

// ===== NETWORK OPERATIONS =====

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct HttpRequest {
    #[schemars(description = "URL to request")]
    pub url: String,
    #[schemars(description = "HTTP method (default: GET)")]
    pub method: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TcpPingRequest {
    #[schemars(description = "Host to ping")]
    pub host: String,
    #[schemars(description = "Port to connect to")]
    pub port: u16,
}

// ===== DATABASE OPERATIONS =====

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct StoreDataRequest {
    #[schemars(description = "Key for the data")]
    pub key: String,
    #[schemars(description = "Value to store")]
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GetDataRequest {
    #[schemars(description = "Key to retrieve")]
    pub key: String,
}

#[derive(Debug, Clone)]
pub struct WasiServerV1 {
    tool_router: ToolRouter<Self>,
    client: reqwest::Client,
    // Simple in-memory key-value store for v1
    store: std::sync::Arc<std::sync::Mutex<HashMap<String, serde_json::Value>>>,
}

impl WasiServerV1 {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            client: reqwest::Client::new(),
            store: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

#[tool_router]
impl WasiServerV1 {
    // ===== FILE SYSTEM TOOLS =====

    #[tool(description = "List files in a directory")]
    async fn list_files(
        &self,
        Parameters(req): Parameters<ListFilesRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = PathBuf::from(&req.path);

        if !path.exists() {
            return Err(ErrorData::new(format!("Path does not exist: {}", req.path)));
        }

        let mut files = Vec::new();
        let mut entries = fs::read_dir(&path)
            .await
            .map_err(|e| ErrorData::new(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ErrorData::new(format!("Failed to read entry: {}", e)))?
        {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Apply pattern filter if provided
            if let Some(pattern) = &req.pattern {
                let glob_pattern = glob::Pattern::new(pattern).map_err(|e| {
                    ErrorData::new(format!("Invalid glob pattern: {}", e))
                })?;
                if !glob_pattern.matches(&file_name) {
                    continue;
                }
            }

            let metadata = entry
                .metadata()
                .await
                .map_err(|e| ErrorData::new(format!("Failed to get metadata: {}", e)))?;

            files.push(json!({
                "name": file_name,
                "type": if metadata.is_dir() { "directory" } else { "file" },
                "size": metadata.len(),
            }));
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "path": req.path,
                "count": files.len(),
                "files": files,
                "version": "1.0.0"
            }))
            .unwrap(),
        )]))
    }

    #[tool(description = "Read contents of a file")]
    async fn read_file(
        &self,
        Parameters(req): Parameters<ReadFileRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = PathBuf::from(&req.path);

        if !path.exists() {
            return Err(ErrorData::new(format!("File does not exist: {}", req.path)));
        }

        let contents = fs::read_to_string(&path)
            .await
            .map_err(|e| ErrorData::new(format!("Failed to read file: {}", e)))?;

        Ok(CallToolResult::success(vec![Content::text(contents)]))
    }

    #[tool(description = "Search for a pattern in files (grep)")]
    async fn grep(
        &self,
        Parameters(req): Parameters<GrepRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = PathBuf::from(&req.path);

        if !path.exists() {
            return Err(ErrorData::new(format!("Path does not exist: {}", req.path)));
        }

        let mut matches = Vec::new();

        if path.is_file() {
            // Search in single file
            if let Ok(contents) = fs::read_to_string(&path).await {
                for (line_num, line) in contents.lines().enumerate() {
                    if line.contains(&req.pattern) {
                        matches.push(json!({
                            "file": path.to_string_lossy(),
                            "line": line_num + 1,
                            "text": line
                        }));
                    }
                }
            }
        } else {
            // Search in directory
            let mut entries = fs::read_dir(&path).await.map_err(|e| {
                ErrorData::new(format!("Failed to read directory: {}", e))
            })?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| ErrorData::new(format!("Failed to read entry: {}", e)))?
            {
                let file_path = entry.path();
                if file_path.is_dir() {
                    continue;
                }

                if let Ok(contents) = fs::read_to_string(&file_path).await {
                    for (line_num, line) in contents.lines().enumerate() {
                        if line.contains(&req.pattern) {
                            matches.push(json!({
                                "file": file_path.to_string_lossy(),
                                "line": line_num + 1,
                                "text": line
                            }));
                        }
                    }
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "pattern": req.pattern,
                "path": req.path,
                "matches": matches,
                "total_matches": matches.len(),
                "version": "1.0.0"
            }))
            .unwrap(),
        )]))
    }

    // ===== NETWORK TOOLS =====

    #[tool(description = "Make HTTP requests")]
    async fn http_request(
        &self,
        Parameters(req): Parameters<HttpRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let method = req.method.as_deref().unwrap_or("GET");

        let response = match method.to_uppercase().as_str() {
            "GET" => self.client.get(&req.url).send().await,
            "POST" => self.client.post(&req.url).send().await,
            _ => return Err(ErrorData::new(format!("Unsupported method: {}", method))),
        }
        .map_err(|e| ErrorData::new(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .map_err(|e| ErrorData::new(format!("Failed to read response: {}", e)))?;

        // Try to parse as JSON, otherwise return as text
        let body = serde_json::from_str::<serde_json::Value>(&body_text)
            .unwrap_or_else(|_| json!({ "text": body_text }));

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "status": status.as_u16(),
                "body": body,
                "version": "1.0.0"
            }))
            .unwrap(),
        )]))
    }

    #[tool(description = "Test TCP connectivity")]
    async fn tcp_ping(
        &self,
        Parameters(req): Parameters<TcpPingRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        use tokio::{
            net::TcpStream,
            time::{timeout, Duration},
        };

        let addr = format!("{}:{}", req.host, req.port);

        match timeout(Duration::from_secs(5), TcpStream::connect(&addr)).await {
            Ok(Ok(_)) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json!({
                    "host": req.host,
                    "port": req.port,
                    "status": "reachable",
                    "message": format!("Successfully connected to {}", addr),
                    "version": "1.0.0"
                }))
                .unwrap(),
            )])),
            Ok(Err(e)) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json!({
                    "host": req.host,
                    "port": req.port,
                    "status": "unreachable",
                    "error": e.to_string(),
                    "version": "1.0.0"
                }))
                .unwrap(),
            )])),
            Err(_) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json!({
                    "host": req.host,
                    "port": req.port,
                    "status": "timeout",
                    "message": "Connection timed out after 5 seconds",
                    "version": "1.0.0"
                }))
                .unwrap(),
            )])),
        }
    }

    // ===== DATABASE TOOLS (In-Memory for V1) =====

    #[tool(description = "Store data in key-value store")]
    async fn store_data(
        &self,
        Parameters(req): Parameters<StoreDataRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut store = self
            .store
            .lock()
            .map_err(|e| ErrorData::new(format!("Failed to lock store: {}", e)))?;

        store.insert(req.key.clone(), req.value.clone());

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "operation": "store",
                "key": req.key,
                "status": "success",
                "message": "Data stored in memory",
                "version": "1.0.0"
            }))
            .unwrap(),
        )]))
    }

    #[tool(description = "Retrieve data from key-value store")]
    async fn get_data(
        &self,
        Parameters(req): Parameters<GetDataRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let store = self
            .store
            .lock()
            .map_err(|e| ErrorData::new(format!("Failed to lock store: {}", e)))?;

        match store.get(&req.key) {
            Some(value) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json!({
                    "key": req.key,
                    "value": value,
                    "found": true,
                    "version": "1.0.0"
                }))
                .unwrap(),
            )])),
            None => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json!({
                    "key": req.key,
                    "found": false,
                    "message": "Key not found",
                    "version": "1.0.0"
                }))
                .unwrap(),
            )])),
        }
    }

    #[tool(description = "List all keys in the store")]
    async fn list_keys(&self) -> Result<CallToolResult, ErrorData> {
        let store = self
            .store
            .lock()
            .map_err(|e| ErrorData::new(format!("Failed to lock store: {}", e)))?;

        let keys: Vec<String> = store.keys().cloned().collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "keys": keys,
                "count": keys.len(),
                "storage": "in-memory",
                "version": "1.0.0"
            }))
            .unwrap(),
        )]))
    }
}

impl ServerHandler for WasiServerV1 {
    fn info(&self) -> ServerInfo {
        ServerInfo::new("wasi-multi-v1".to_string(), "1.0.0".to_string())
    }

    fn capabilities(&self) -> ServerCapabilities {
        ServerCapabilities::new().with_tools(self.tool_router.clone())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let server = WasiServerV1::new();
    match server.serve(wasi_multi::wasi_io()).await {
        Ok(service) => {
            tracing::info!(
                "WASI Multi Server v1 started - Basic FS, Network, and Memory Store"
            );
            if let Err(e) = service.waiting().await {
                tracing::error!("Server error: {:?}", e);
            }
        }
        Err(e) => {
            tracing::error!("Failed to start server: {:?}", e);
        }
    }

    Ok(())
}
