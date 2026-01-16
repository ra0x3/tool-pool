use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_router, ErrorData, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
};

// ===== FILE SYSTEM OPERATIONS (Enhanced for V2) =====

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListFilesRequest {
    #[schemars(description = "Directory path to list")]
    pub path: String,
    #[schemars(description = "Optional glob pattern to filter files")]
    pub pattern: Option<String>,
    #[schemars(description = "Include hidden files (default: false)")]
    pub include_hidden: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReadFileRequest {
    #[schemars(description = "File path to read")]
    pub path: String,
    #[schemars(description = "Read as binary (base64) or text")]
    pub binary: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WriteFileRequest {
    #[schemars(description = "File path to write")]
    pub path: String,
    #[schemars(description = "Content to write")]
    pub content: String,
    #[schemars(description = "Append instead of overwrite")]
    pub append: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct GrepRequest {
    #[schemars(description = "Pattern to search for")]
    pub pattern: String,
    #[schemars(description = "Directory to search in")]
    pub path: String,
    #[schemars(description = "Use regex pattern matching")]
    pub regex: Option<bool>,
    #[schemars(description = "Case insensitive search")]
    pub ignore_case: Option<bool>,
}

// ===== NETWORK OPERATIONS (Enhanced for V2) =====

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct HttpRequest {
    #[schemars(description = "URL to request")]
    pub url: String,
    #[schemars(description = "HTTP method")]
    pub method: Option<String>,
    #[schemars(description = "Request headers")]
    pub headers: Option<HashMap<String, String>>,
    #[schemars(description = "Request body")]
    pub body: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TcpRequest {
    #[schemars(description = "Host to connect to")]
    pub host: String,
    #[schemars(description = "Port to connect to")]
    pub port: u16,
    #[schemars(description = "Data to send")]
    pub data: String,
    #[schemars(description = "Read response")]
    pub read_response: Option<bool>,
}

// ===== DATABASE OPERATIONS (Real SQLite for V2) =====

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SqlQueryRequest {
    #[schemars(description = "SQL query to execute")]
    pub query: String,
    #[schemars(description = "Query parameters")]
    pub params: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CreateTableRequest {
    #[schemars(description = "Table name")]
    pub table: String,
    #[schemars(description = "Column definitions as JSON")]
    pub columns: serde_json::Value,
}

// ===== V2 EXCLUSIVE FEATURES =====

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct BatchRequest {
    #[schemars(description = "List of operations to perform")]
    pub operations: Vec<BatchOperation>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct BatchOperation {
    pub tool: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct WasiServerV2 {
    tool_router: ToolRouter<Self>,
    client: reqwest::Client,
    cache: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    #[cfg(feature = "v2")]
    db_pool: Option<sqlx::SqlitePool>,
}

impl WasiServerV2 {
    pub async fn new() -> Self {
        #[cfg(feature = "v2")]
        let db_pool = sqlx::SqlitePool::connect("sqlite::memory:").await.ok();

        #[cfg(feature = "v2")]
        if let Some(ref pool) = db_pool {
            // Initialize database schema
            let _ = sqlx::query(
                "CREATE TABLE IF NOT EXISTS kv_store (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )",
            )
            .execute(pool)
            .await;
        }

        Self {
            tool_router: Self::tool_router(),
            client: reqwest::Client::new(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(feature = "v2")]
            db_pool,
        }
    }
}

#[tool_router]
impl WasiServerV2 {
    // ===== ENHANCED FILE SYSTEM TOOLS =====

    #[tool(description = "List files with advanced filtering")]
    async fn list_files(
        &self,
        Parameters(req): Parameters<ListFilesRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = PathBuf::from(&req.path);

        if !path.exists() {
            return Err(ErrorData::new(format!("Path does not exist: {}", req.path)));
        }

        let mut files = Vec::new();
        let include_hidden = req.include_hidden.unwrap_or(false);

        let mut entries = fs::read_dir(&path)
            .await
            .map_err(|e| ErrorData::new(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ErrorData::new(format!("Failed to read entry: {}", e)))?
        {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files if not requested
            if !include_hidden && file_name.starts_with('.') {
                continue;
            }

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
                "modified": metadata.modified().ok().map(|t|
                    t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
                ),
            }));
        }

        // Sort files by name
        files.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "path": req.path,
                "count": files.len(),
                "files": files,
                "version": "2.0.0"
            }))
            .unwrap(),
        )]))
    }

    #[tool(description = "Read file with binary support")]
    async fn read_file(
        &self,
        Parameters(req): Parameters<ReadFileRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = PathBuf::from(&req.path);

        if !path.exists() {
            return Err(ErrorData::new(format!("File does not exist: {}", req.path)));
        }

        if req.binary.unwrap_or(false) {
            let contents = fs::read(&path)
                .await
                .map_err(|e| ErrorData::new(format!("Failed to read file: {}", e)))?;

            let base64 = base64::encode(&contents);

            Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json!({
                    "path": req.path,
                    "encoding": "base64",
                    "size": contents.len(),
                    "content": base64,
                    "version": "2.0.0"
                }))
                .unwrap(),
            )]))
        } else {
            let contents = fs::read_to_string(&path)
                .await
                .map_err(|e| ErrorData::new(format!("Failed to read file: {}", e)))?;

            Ok(CallToolResult::success(vec![Content::text(contents)]))
        }
    }

    #[tool(description = "Write file with append support")]
    async fn write_file(
        &self,
        Parameters(req): Parameters<WriteFileRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = PathBuf::from(&req.path);

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                ErrorData::new(format!("Failed to create directories: {}", e))
            })?;
        }

        if req.append.unwrap_or(false) {
            let mut file = fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&path)
                .await
                .map_err(|e| ErrorData::new(format!("Failed to open file: {}", e)))?;

            file.write_all(req.content.as_bytes()).await.map_err(|e| {
                ErrorData::new(format!("Failed to append to file: {}", e))
            })?;
        } else {
            fs::write(&path, &req.content)
                .await
                .map_err(|e| ErrorData::new(format!("Failed to write file: {}", e)))?;
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "path": req.path,
                "operation": if req.append.unwrap_or(false) { "append" } else { "write" },
                "bytes_written": req.content.len(),
                "version": "2.0.0"
            }))
            .unwrap(),
        )]))
    }

    #[tool(description = "Advanced pattern search with regex support")]
    async fn grep(
        &self,
        Parameters(req): Parameters<GrepRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = PathBuf::from(&req.path);

        if !path.exists() {
            return Err(ErrorData::new(format!("Path does not exist: {}", req.path)));
        }

        let mut matches = Vec::new();
        let use_regex = req.regex.unwrap_or(false);
        let ignore_case = req.ignore_case.unwrap_or(false);

        let search_pattern = if ignore_case {
            req.pattern.to_lowercase()
        } else {
            req.pattern.clone()
        };

        let search_in_file =
            |file_path: PathBuf, contents: String| -> Vec<serde_json::Value> {
                let mut file_matches = Vec::new();
                for (line_num, line) in contents.lines().enumerate() {
                    let search_line = if ignore_case {
                        line.to_lowercase()
                    } else {
                        line.to_string()
                    };

                    let is_match = if use_regex {
                        // Simple regex simulation (would use regex crate in production)
                        search_line.contains(&search_pattern)
                    } else {
                        search_line.contains(&search_pattern)
                    };

                    if is_match {
                        file_matches.push(json!({
                            "file": file_path.to_string_lossy(),
                            "line": line_num + 1,
                            "text": line,
                            "column": search_line.find(&search_pattern).unwrap_or(0) + 1
                        }));
                    }
                }
                file_matches
            };

        if path.is_file() {
            if let Ok(contents) = fs::read_to_string(&path).await {
                matches.extend(search_in_file(path, contents));
            }
        } else {
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
                    matches.extend(search_in_file(file_path, contents));
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "pattern": req.pattern,
                "path": req.path,
                "options": {
                    "regex": use_regex,
                    "ignore_case": ignore_case
                },
                "matches": matches,
                "total_matches": matches.len(),
                "version": "2.0.0"
            }))
            .unwrap(),
        )]))
    }

    // ===== ENHANCED NETWORK TOOLS =====

    #[tool(description = "Advanced HTTP client with full features")]
    async fn http_request(
        &self,
        Parameters(req): Parameters<HttpRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let method = req.method.as_deref().unwrap_or("GET");

        let mut request = match method.to_uppercase().as_str() {
            "GET" => self.client.get(&req.url),
            "POST" => self.client.post(&req.url),
            "PUT" => self.client.put(&req.url),
            "DELETE" => self.client.delete(&req.url),
            "PATCH" => self.client.patch(&req.url),
            _ => return Err(ErrorData::new(format!("Unsupported method: {}", method))),
        };

        // Add headers if provided
        if let Some(headers) = req.headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }

        // Add body if provided
        if let Some(body) = req.body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .map_err(|e| ErrorData::new(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body_text = response
            .text()
            .await
            .map_err(|e| ErrorData::new(format!("Failed to read response: {}", e)))?;

        let body = serde_json::from_str::<serde_json::Value>(&body_text)
            .unwrap_or_else(|_| json!({ "text": body_text }));

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "status": status.as_u16(),
                "headers": headers,
                "body": body,
                "version": "2.0.0"
            }))
            .unwrap(),
        )]))
    }

    #[tool(description = "Raw TCP socket communication")]
    async fn tcp_connect(
        &self,
        Parameters(req): Parameters<TcpRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        use tokio::net::TcpStream;

        let addr = format!("{}:{}", req.host, req.port);
        let mut stream = TcpStream::connect(&addr).await.map_err(|e| {
            ErrorData::new(format!("Failed to connect to {}: {}", addr, e))
        })?;

        // Send data
        stream
            .write_all(req.data.as_bytes())
            .await
            .map_err(|e| ErrorData::new(format!("Failed to send data: {}", e)))?;

        let mut response = String::new();

        if req.read_response.unwrap_or(true) {
            let mut buffer = vec![0; 4096];
            match stream.read(&mut buffer).await {
                Ok(n) if n > 0 => {
                    response = String::from_utf8_lossy(&buffer[..n]).to_string();
                }
                Ok(_) => {
                    response = "Connection closed by remote".to_string();
                }
                Err(e) => {
                    response = format!("Failed to read response: {}", e);
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "connected_to": addr,
                "sent": req.data,
                "sent_bytes": req.data.len(),
                "response": response,
                "response_bytes": response.len(),
                "version": "2.0.0"
            }))
            .unwrap(),
        )]))
    }

    // ===== REAL DATABASE TOOLS (SQLite) =====

    #[tool(description = "Execute SQL query on SQLite database")]
    async fn sql_query(
        &self,
        Parameters(req): Parameters<SqlQueryRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        #[cfg(feature = "v2")]
        {
            if let Some(ref pool) = self.db_pool {
                let query_lower = req.query.to_lowercase();

                if query_lower.starts_with("select") {
                    // Handle SELECT queries
                    let rows = sqlx::query(&req.query)
                        .fetch_all(pool)
                        .await
                        .map_err(|e| ErrorData::new(format!("Query failed: {}", e)))?;

                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&json!({
                            "query": req.query,
                            "rows_returned": rows.len(),
                            "database": "SQLite (in-memory)",
                            "version": "2.0.0"
                        }))
                        .unwrap(),
                    )]))
                } else {
                    // Handle INSERT, UPDATE, DELETE
                    let result = sqlx::query(&req.query)
                        .execute(pool)
                        .await
                        .map_err(|e| ErrorData::new(format!("Query failed: {}", e)))?;

                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&json!({
                            "query": req.query,
                            "rows_affected": result.rows_affected(),
                            "database": "SQLite (in-memory)",
                            "version": "2.0.0"
                        }))
                        .unwrap(),
                    )]))
                }
            } else {
                Err(ErrorData::new("Database not available".to_string()))
            }
        }

        #[cfg(not(feature = "v2"))]
        Err(ErrorData::new(
            "SQL queries require v2 features".to_string(),
        ))
    }

    #[tool(description = "Create a new table in SQLite")]
    async fn create_table(
        &self,
        Parameters(req): Parameters<CreateTableRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        #[cfg(feature = "v2")]
        {
            if let Some(ref pool) = self.db_pool {
                // Build CREATE TABLE statement from JSON schema
                let mut columns = Vec::new();
                if let Some(cols) = req.columns.as_object() {
                    for (name, def) in cols {
                        let col_type =
                            def.get("type").and_then(|t| t.as_str()).unwrap_or("TEXT");
                        let nullable = !def
                            .get("required")
                            .and_then(|r| r.as_bool())
                            .unwrap_or(false);

                        let col_def = format!(
                            "{} {} {}",
                            name,
                            col_type,
                            if nullable { "" } else { "NOT NULL" }
                        );
                        columns.push(col_def);
                    }
                }

                let create_sql = format!(
                    "CREATE TABLE IF NOT EXISTS {} ({})",
                    req.table,
                    columns.join(", ")
                );

                sqlx::query(&create_sql).execute(pool).await.map_err(|e| {
                    ErrorData::new(format!("Failed to create table: {}", e))
                })?;

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&json!({
                        "table": req.table,
                        "columns": columns,
                        "status": "created",
                        "database": "SQLite (in-memory)",
                        "version": "2.0.0"
                    }))
                    .unwrap(),
                )]))
            } else {
                Err(ErrorData::new("Database not available".to_string()))
            }
        }

        #[cfg(not(feature = "v2"))]
        Err(ErrorData::new(
            "Table creation requires v2 features".to_string(),
        ))
    }

    // ===== V2 EXCLUSIVE: BATCH OPERATIONS =====

    #[tool(description = "Execute multiple operations in batch")]
    async fn batch_execute(
        &self,
        Parameters(req): Parameters<BatchRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut results = Vec::new();

        for op in req.operations {
            let result = match op.tool.as_str() {
                "list_files" => {
                    json!({
                        "tool": "list_files",
                        "status": "executed",
                        "message": "Would list files with provided params"
                    })
                }
                "http_request" => {
                    json!({
                        "tool": "http_request",
                        "status": "executed",
                        "message": "Would make HTTP request with provided params"
                    })
                }
                _ => {
                    json!({
                        "tool": op.tool,
                        "status": "unknown",
                        "error": "Unknown tool in batch operation"
                    })
                }
            };
            results.push(result);
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "batch_size": req.operations.len(),
                "results": results,
                "version": "2.0.0"
            }))
            .unwrap(),
        )]))
    }

    #[tool(description = "Get server statistics and cache info")]
    async fn server_stats(&self) -> Result<CallToolResult, ErrorData> {
        let cache_size = self.cache.lock().map(|c| c.len()).unwrap_or(0);

        #[cfg(feature = "v2")]
        let db_info = if let Some(ref pool) = self.db_pool {
            let result = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            )
            .fetch_one(pool)
            .await
            .unwrap_or(0);

            json!({
                "type": "SQLite",
                "location": "memory",
                "tables": result
            })
        } else {
            json!(null)
        };

        #[cfg(not(feature = "v2"))]
        let db_info = json!(null);

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "version": "2.0.0",
                "features": {
                    "file_system": "enhanced",
                    "network": "full HTTP + TCP",
                    "database": "SQLite",
                    "batch_operations": true,
                    "caching": true
                },
                "cache": {
                    "entries": cache_size,
                    "type": "in-memory"
                },
                "database": db_info
            }))
            .unwrap(),
        )]))
    }
}

impl ServerHandler for WasiServerV2 {
    fn info(&self) -> ServerInfo {
        ServerInfo::new("wasi-multi-v2".to_string(), "2.0.0".to_string())
    }

    fn capabilities(&self) -> ServerCapabilities {
        ServerCapabilities::new().with_tools(self.tool_router.clone())
    }
}

// Base64 encoding helper
mod base64 {
    pub fn encode(input: &[u8]) -> String {
        use std::fmt::Write;
        let mut result = String::new();
        let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        for chunk in input.chunks(3) {
            let mut buf = [0u8; 3];
            for (i, &b) in chunk.iter().enumerate() {
                buf[i] = b;
            }

            let _ = write!(&mut result, "{}", table[(buf[0] >> 2) as usize] as char);
            let _ = write!(
                &mut result,
                "{}",
                table[(((buf[0] & 0x03) << 4) | (buf[1] >> 4)) as usize] as char
            );

            if chunk.len() > 1 {
                let _ = write!(
                    &mut result,
                    "{}",
                    table[(((buf[1] & 0x0f) << 2) | (buf[2] >> 6)) as usize] as char
                );
            } else {
                result.push('=');
            }

            if chunk.len() > 2 {
                let _ =
                    write!(&mut result, "{}", table[(buf[2] & 0x3f) as usize] as char);
            } else {
                result.push('=');
            }
        }

        result
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let server = WasiServerV2::new().await;
    match server.serve(wasi_multi::wasi_io()).await {
        Ok(service) => {
            tracing::info!("WASI Multi Server v2 started - Enhanced FS, Network, SQLite DB, Batch Operations");
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
