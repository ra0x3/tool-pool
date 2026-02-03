#[cfg(feature = "wasmedge-postgres")]
use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use mcpkit_rs::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData, ServerHandler,
};
use mcpkit_rs_policy::CompiledPolicy;
use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(feature = "wasmedge-postgres")]
use tokio_postgres::{Client, NoTls};
#[cfg(not(feature = "wasmedge-postgres"))]
type Client = ();
#[cfg(feature = "wasmedge-postgres")]
use tokio::sync::RwLock;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Todo {
    pub id: String,
    pub user_id: i32,
    pub title: String,
    pub completed: bool,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonPlaceholderTodo {
    #[serde(rename = "userId")]
    pub user_id: i32,
    pub id: i32,
    pub title: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FetchTodosRequest {
    #[schemars(description = "User ID to fetch todos for")]
    pub user_id: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CreateTodoRequest {
    #[schemars(description = "Todo title")]
    pub title: String,
    #[schemars(description = "User ID")]
    pub user_id: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpdateTodoRequest {
    #[schemars(description = "Todo ID")]
    pub id: String,
    #[schemars(description = "New title")]
    pub title: Option<String>,
    #[schemars(description = "New completed status")]
    pub completed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DeleteTodoRequest {
    #[schemars(description = "Todo ID to delete")]
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct BatchProcessRequest {
    #[schemars(description = "List of todo IDs")]
    pub ids: Vec<String>,
    #[schemars(description = "Operation: 'complete', 'delete', or 'archive'")]
    pub operation: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SearchRequest {
    #[schemars(description = "Search term to look for in todo titles")]
    pub title_contains: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FileReadRequest {
    #[schemars(description = "Path to the file to read")]
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FileWriteRequest {
    #[schemars(description = "Path to the file to write")]
    pub path: String,
    #[schemars(description = "Content to write to the file")]
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FileListRequest {
    #[schemars(description = "Path to the directory to list")]
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct FullStackServer {
    tool_router: ToolRouter<Self>,
    #[cfg(feature = "wasmedge-postgres")]
    db_client: Arc<RwLock<Option<Client>>>,
    #[cfg(not(feature = "wasmedge-postgres"))]
    db_client: Arc<Option<Client>>,
    policy: Option<Arc<CompiledPolicy>>,
}

impl FullStackServer {
    pub async fn new() -> Self {
        Self::with_optional_policy(None)
    }

    /// Create server instance synchronously
    pub fn new_sync() -> Self {
        Self::with_optional_policy(None)
    }

    /// Create server instance with compiled policy
    pub fn new_with_compiled_policy(policy: Arc<CompiledPolicy>) -> Self {
        Self::with_optional_policy(Some(policy))
    }

    fn with_optional_policy(policy: Option<Arc<CompiledPolicy>>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            #[cfg(feature = "wasmedge-postgres")]
            db_client: Arc::new(RwLock::new(None)),
            #[cfg(not(feature = "wasmedge-postgres"))]
            db_client: Arc::new(None),
            policy,
        }
    }

    async fn connect_db() -> Option<Client> {
        #[cfg(feature = "wasmedge-postgres")]
        {
            let database_url =
                env::var("DATABASE_URL").expect("DATABASE_URL environment variable must be set");

            // Use a timeout to avoid hanging in WasmEdge
            match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                tokio_postgres::connect(&database_url, NoTls),
            )
            .await
            {
                Ok(Ok((client, connection))) => {
                    tokio::spawn(async move {
                        let _ = connection.await;
                    });
                    Some(client)
                }
                Ok(Err(_)) => None,
                Err(_) => {
                    // Timeout expired
                    None
                }
            }
        }
        #[cfg(not(feature = "wasmedge-postgres"))]
        {
            None
        }
    }

    #[cfg(feature = "wasmedge-postgres")]
    async fn ensure_connection(&self) -> Result<(), String> {
        let client = self.db_client.read().await;
        if client.is_some() {
            return Ok(());
        }
        drop(client); // Release read lock

        // Try to connect with timeout
        let mut client = self.db_client.write().await;
        if client.is_none() {
            *client = Self::connect_db().await;
        }

        if client.is_none() {
            Err(
                "Failed to connect to database. Connection timed out or PostgreSQL is not running."
                    .to_string(),
            )
        } else {
            Ok(())
        }
    }

    async fn fetch_from_api(user_id: Option<i32>) -> Result<Vec<JsonPlaceholderTodo>, String> {
        let url = if let Some(uid) = user_id {
            format!("http://jsonplaceholder.typicode.com/todos?userId={}", uid)
        } else {
            "http://jsonplaceholder.typicode.com/todos".to_string()
        };

        let resp = reqwest::get(&url)
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json::<Vec<JsonPlaceholderTodo>>()
            .await
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        Ok(resp)
    }

    fn normalize_path(path: &str) -> String {
        let mut normalized = path.trim();

        if let Some(stripped) = normalized.strip_prefix("file://") {
            normalized = stripped;
        }

        if let Some(stripped) = normalized.strip_prefix("fs://") {
            normalized = stripped;
        }

        normalized.trim().to_string()
    }

    fn check_storage_permission(&self, path: &str, operation: &str) -> Result<(), String> {
        if path.is_empty() {
            return Err(format!(
                "Permission denied: {} access to empty path is not allowed",
                operation
            ));
        }

        if let Some(policy) = &self.policy {
            if !policy.is_storage_allowed(path, operation) {
                return Err(format!(
                    "Policy denied {} access to {}",
                    operation, path
                ));
            }

            if Self::is_default_forbidden_path(path) {
                return Err(format!(
                    "Permission denied: {} access to {}",
                    operation, path
                ));
            }

            return Ok(());
        }

        if Self::is_default_forbidden_path(path) {
            return Err(format!(
                "Permission denied: {} access to {}",
                operation, path
            ));
        }

        if Self::is_default_allowed_path(path) {
            return Ok(());
        }

        Err(format!(
            "Permission denied: {} access to {} (outside allowed directories)",
            operation, path
        ))
    }

    fn path_matches_prefix(path: &str, prefix: &str) -> bool {
        let prefix = prefix.trim_end_matches('/');
        if prefix.is_empty() {
            return false;
        }
        let path_buf = Path::new(path);
        let prefix_buf = Path::new(prefix);
        path_buf == prefix_buf || path_buf.starts_with(prefix_buf)
    }

    fn is_default_forbidden_path(path: &str) -> bool {
        const FORBIDDEN_PATTERNS: [&str; 4] = [
            "/tmp/wasm-fs-test/forbidden",
            "/etc",
            "/usr",
            "/sys",
        ];

        FORBIDDEN_PATTERNS
            .iter()
            .any(|pattern| Self::path_matches_prefix(path, pattern))
    }

    fn is_default_allowed_path(path: &str) -> bool {
        const ALLOWED_PATTERNS: [&str; 3] = [
            "/tmp/wasm-fs-test/allowed",
            "/tmp",
            "/var/tmp",
        ];

        ALLOWED_PATTERNS
            .iter()
            .any(|pattern| Self::path_matches_prefix(path, pattern))
    }

    fn policy_violation_response(_path: &str, message: String) -> Result<String, ErrorData> {
        Err(ErrorData::invalid_params(message, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use mcpkit_rs_policy::Policy;

    #[test]
    fn policy_allows_and_denies_expected_paths() {
        let policy_yaml = r#"
version: "1.0"
core:
  storage:
    allow:
      - uri: fs:///tmp/**
        access: [read, write]
    deny:
      - uri: fs:///tmp/wasm-fs-test/forbidden/**
        access: [read, write]
"#;

        let policy = Policy::from_yaml(policy_yaml).expect("valid policy yaml");
        let compiled = Arc::new(CompiledPolicy::compile(&policy).expect("compile policy"));
        let server = FullStackServer::new_with_compiled_policy(compiled);

        assert!(
            server
                .check_storage_permission("/tmp/wasm-fs-test/allowed/file.txt", "read")
                .is_ok()
        );
        assert!(
            server
                .check_storage_permission("/tmp/wasm-fs-test/forbidden/secret.txt", "read")
                .is_err()
        );
        assert!(
            server
                .check_storage_permission("/tmp/wasm-fs-test/forbidden", "read")
                .is_err()
        );
    }
}

// Tool implementations
#[tool_router]
impl FullStackServer {
    #[tool(description = "Fetch todos from JSONPlaceholder API")]
    async fn fetch_todos(
        &self,
        Parameters(req): Parameters<FetchTodosRequest>,
    ) -> Result<String, String> {
        let api_todos = Self::fetch_from_api(req.user_id).await?;

        serde_json::to_string_pretty(&json!({
            "todos": api_todos,
            "count": api_todos.len(),
            "source": "jsonplaceholder-api"
        }))
        .map_err(|e| e.to_string())
    }

    #[tool(description = "Create todo in PostgreSQL")]
    async fn create_todo(
        &self,
        Parameters(req): Parameters<CreateTodoRequest>,
    ) -> Result<String, String> {
        #[cfg(feature = "wasmedge-postgres")]
        {
            self.ensure_connection().await?;
            let client = self.db_client.read().await;
            if let Some(client) = client.as_ref() {
                let todo_id = format!("todo-{}", chrono::Utc::now().timestamp_millis());

                match client
                    .execute(
                        "INSERT INTO todos (id, user_id, title, completed, created_at)
                     VALUES ($1, $2, $3, false, NOW())",
                        &[&todo_id, &req.user_id, &req.title],
                    )
                    .await
                {
                    Ok(_) => {
                        let json_data = json!({
                            "id": todo_id,
                            "user_id": req.user_id,
                            "title": req.title
                        })
                        .to_string();

                        client
                            .execute(
                                "INSERT INTO wal_entries (operation, data)
                             VALUES ('CREATE', $1)",
                                &[&json_data],
                            )
                            .await
                            .ok();

                        serde_json::to_string_pretty(&json!({
                            "created": {
                                "id": todo_id,
                                "user_id": req.user_id,
                                "title": req.title,
                                "completed": false
                            },
                            "source": "postgresql"
                        }))
                        .map_err(|e| e.to_string())
                    }
                    Err(e) => Err(format!("Failed to create todo: {}", e)),
                }
            } else {
                Err("Database not connected. Please ensure PostgreSQL is running.".to_string())
            }
        }
        #[cfg(not(feature = "wasmedge-postgres"))]
        {
            Err("Database support not enabled. Build with --features wasmedge-postgres".to_string())
        }
    }

    #[tool(description = "Update todo in PostgreSQL")]
    async fn update_todo(
        &self,
        Parameters(req): Parameters<UpdateTodoRequest>,
    ) -> Result<String, String> {
        #[cfg(feature = "wasmedge-postgres")]
        {
            self.ensure_connection().await?;
            let client = self.db_client.read().await;
            if let Some(client) = client.as_ref() {
                let mut query = "UPDATE todos SET updated_at = NOW()".to_string();
                let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![];
                let mut param_count = 1;

                if let Some(title) = &req.title {
                    query.push_str(&format!(", title = ${}", param_count));
                    params.push(title);
                    param_count += 1;
                }

                if let Some(completed) = &req.completed {
                    query.push_str(&format!(", completed = ${}", param_count));
                    params.push(completed);
                    param_count += 1;
                }

                query.push_str(&format!(" WHERE id = ${}", param_count));
                params.push(&req.id);

                match client.execute(&query, &params).await {
                    Ok(rows_affected) => {
                        if rows_affected == 0 {
                            Err(format!("Todo with id {} not found", req.id))
                        } else {
                            client
                                .execute(
                                    "INSERT INTO wal_entries (operation, data)
                                 VALUES ('UPDATE', $1)",
                                    &[&json!(req).to_string()],
                                )
                                .await
                                .ok();

                            serde_json::to_string_pretty(&json!({
                                "updated": true,
                                "id": req.id,
                                "source": "postgresql"
                            }))
                            .map_err(|e| e.to_string())
                        }
                    }
                    Err(e) => Err(format!("Failed to update todo: {}", e)),
                }
            } else {
                Err("Database not connected. Please ensure PostgreSQL is running.".to_string())
            }
        }
        #[cfg(not(feature = "wasmedge-postgres"))]
        {
            Err("Database support not enabled. Build with --features wasmedge-postgres".to_string())
        }
    }

    #[tool(description = "Delete todo from PostgreSQL")]
    async fn delete_todo(
        &self,
        Parameters(req): Parameters<DeleteTodoRequest>,
    ) -> Result<String, String> {
        #[cfg(feature = "wasmedge-postgres")]
        {
            self.ensure_connection().await?;
            let client = self.db_client.read().await;
            if let Some(client) = client.as_ref() {
                match client
                    .execute("DELETE FROM todos WHERE id = $1", &[&req.id])
                    .await
                {
                    Ok(rows_affected) => {
                        if rows_affected == 0 {
                            Err(format!("Todo with id {} not found", req.id))
                        } else {
                            client
                                .execute(
                                    "INSERT INTO wal_entries (operation, data)
                                 VALUES ('DELETE', $1)",
                                    &[&json!({"id": req.id}).to_string()],
                                )
                                .await
                                .ok();

                            serde_json::to_string_pretty(&json!({
                                "deleted": true,
                                "id": req.id,
                                "source": "postgresql"
                            }))
                            .map_err(|e| e.to_string())
                        }
                    }
                    Err(e) => Err(format!("Failed to delete todo: {}", e)),
                }
            } else {
                Err("Database not connected. Please ensure PostgreSQL is running.".to_string())
            }
        }
        #[cfg(not(feature = "wasmedge-postgres"))]
        {
            Err("Database support not enabled. Build with --features wasmedge-postgres".to_string())
        }
    }

    #[tool(description = "Batch process todos in PostgreSQL")]
    async fn batch_process(
        &self,
        Parameters(req): Parameters<BatchProcessRequest>,
    ) -> Result<String, String> {
        if req.ids.is_empty() {
            return Err("No IDs provided".to_string());
        }

        #[cfg(feature = "wasmedge-postgres")]
        {
            self.ensure_connection().await?;
            let client = self.db_client.read().await;
            if let Some(client) = client.as_ref() {
                let mut rows_affected = 0u64;

                match req.operation.as_str() {
                    "complete" => {
                        for id in &req.ids {
                            let result = client.execute(
                                "UPDATE todos SET completed = true, updated_at = NOW() WHERE id = $1",
                                &[&id],
                            ).await.unwrap_or(0);
                            rows_affected += result;
                        }
                    }
                    "delete" => {
                        for id in &req.ids {
                            let result = client
                                .execute("DELETE FROM todos WHERE id = $1", &[&id])
                                .await
                                .unwrap_or(0);
                            rows_affected += result;
                        }
                    }
                    "archive" => {
                        for id in &req.ids {
                            let result = client
                                .execute(
                                    "UPDATE todos SET completed = true,
                                 title = '[ARCHIVED] ' || title,
                                 updated_at = NOW()
                                 WHERE id = $1 AND title NOT LIKE '[ARCHIVED]%'",
                                    &[&id],
                                )
                                .await
                                .unwrap_or(0);
                            rows_affected += result;
                        }
                    }
                    _ => return Err(format!("Unknown operation: {}", req.operation)),
                }

                serde_json::to_string_pretty(&json!({
                    "operation": req.operation,
                    "ids": req.ids,
                    "rows_affected": rows_affected,
                    "source": "postgresql"
                }))
                .map_err(|e| e.to_string())
            } else {
                Err("Database not connected. Please ensure PostgreSQL is running.".to_string())
            }
        }
        #[cfg(not(feature = "wasmedge-postgres"))]
        {
            Err("Database support not enabled. Build with --features wasmedge-postgres".to_string())
        }
    }

    #[tool(description = "Get database statistics from PostgreSQL view")]
    async fn db_stats(&self) -> Result<String, String> {
        #[cfg(feature = "wasmedge-postgres")]
        {
            self.ensure_connection().await?;
            let client = self.db_client.read().await;
            if let Some(client) = client.as_ref() {
                match client
                    .query_one(
                        "SELECT total, completed, pending, unique_users FROM todo_stats",
                        &[],
                    )
                    .await
                {
                    Ok(row) => {
                        let total: i64 = row.get(0);
                        let completed: i64 = row.get(1);
                        let pending: i64 = row.get(2);
                        let unique_users: i64 = row.get(3);

                        serde_json::to_string_pretty(&json!({
                            "total": total,
                            "completed": completed,
                            "pending": pending,
                            "unique_users": unique_users,
                            "completion_rate": if total > 0 {
                                completed as f64 / total as f64 * 100.0
                            } else {
                                0.0
                            },
                            "source": "postgresql"
                        }))
                        .map_err(|e| e.to_string())
                    }
                    Err(e) => Err(format!("Failed to get stats: {}", e)),
                }
            } else {
                Err("Database not connected. Please ensure PostgreSQL is running.".to_string())
            }
        }
        #[cfg(not(feature = "wasmedge-postgres"))]
        {
            Err("Database support not enabled. Build with --features wasmedge-postgres".to_string())
        }
    }

    #[tool(description = "Read WAL entries from PostgreSQL")]
    async fn read_wal(&self) -> Result<String, String> {
        #[cfg(feature = "wasmedge-postgres")]
        {
            self.ensure_connection().await?;
            let client = self.db_client.read().await;
            if let Some(client) = client.as_ref() {
                match client
                    .query(
                        "SELECT timestamp::text, operation, data
                     FROM wal_entries
                     ORDER BY timestamp DESC
                     LIMIT 10",
                        &[],
                    )
                    .await
                {
                    Ok(rows) => {
                        let entries: Vec<serde_json::Value> = rows
                            .iter()
                            .map(|row| {
                                let data_str: String = row.get(2);
                                let data: serde_json::Value =
                                    serde_json::from_str(&data_str).unwrap_or(json!({}));
                                json!({
                                    "timestamp": row.get::<_, String>(0),
                                    "operation": row.get::<_, String>(1),
                                    "data": data
                                })
                            })
                            .collect();

                        serde_json::to_string_pretty(&json!({
                            "last_10_entries": entries,
                            "source": "postgresql"
                        }))
                        .map_err(|e| e.to_string())
                    }
                    Err(e) => Err(format!("Failed to read WAL: {}", e)),
                }
            } else {
                Err("Database not connected. Please ensure PostgreSQL is running.".to_string())
            }
        }
        #[cfg(not(feature = "wasmedge-postgres"))]
        {
            Err("Database support not enabled. Build with --features wasmedge-postgres".to_string())
        }
    }

    #[tool(description = "Search todos in PostgreSQL")]
    async fn search_todos(
        &self,
        Parameters(req): Parameters<SearchRequest>,
    ) -> Result<String, String> {
        #[cfg(feature = "wasmedge-postgres")]
        {
            self.ensure_connection().await?;
            let client = self.db_client.read().await;
            if let Some(client) = client.as_ref() {
                let pattern = format!("%{}%", req.title_contains);

                match client
                    .query(
                        "SELECT id, user_id, title, completed, created_at::text, updated_at::text
                     FROM todos
                     WHERE LOWER(title) LIKE LOWER($1)
                     ORDER BY created_at DESC",
                        &[&pattern],
                    )
                    .await
                {
                    Ok(rows) => {
                        let todos: Vec<Todo> = rows
                            .iter()
                            .map(|row| Todo {
                                id: row.get(0),
                                user_id: row.get(1),
                                title: row.get(2),
                                completed: row.get(3),
                                created_at: row.get(4),
                                updated_at: row.get(5),
                            })
                            .collect();

                        serde_json::to_string_pretty(&json!({
                            "search_term": req.title_contains,
                            "results": todos,
                            "count": todos.len(),
                            "source": "postgresql"
                        }))
                        .map_err(|e| e.to_string())
                    }
                    Err(e) => Err(format!("Search failed: {}", e)),
                }
            } else {
                Err("Database not connected. Please ensure PostgreSQL is running.".to_string())
            }
        }
        #[cfg(not(feature = "wasmedge-postgres"))]
        {
            Err("Database support not enabled. Build with --features wasmedge-postgres".to_string())
        }
    }

    #[tool(description = "Read contents of a file")]
    async fn file_read(
        &self,
        Parameters(req): Parameters<FileReadRequest>,
    ) -> Result<String, ErrorData> {
        let normalized_path = Self::normalize_path(&req.path);

        if let Err(err) = self.check_storage_permission(&normalized_path, "read") {
            return Self::policy_violation_response(&req.path, err);
        }

        let path = Path::new(&normalized_path);

        match fs::read_to_string(path) {
            Ok(content) => {
                serde_json::to_string_pretty(&json!({
                    "success": true,
                    "path": normalized_path,
                    "content": content,
                    "size": content.len()
                }))
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
            Err(e) => {
                serde_json::to_string_pretty(&json!({
                    "success": false,
                    "path": normalized_path,
                    "error": e.to_string()
                }))
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
        }
    }

    #[tool(description = "Write content to a file")]
    async fn file_write(
        &self,
        Parameters(req): Parameters<FileWriteRequest>,
    ) -> Result<String, ErrorData> {
        let normalized_path = Self::normalize_path(&req.path);

        if let Err(err) = self.check_storage_permission(&normalized_path, "write") {
            return Self::policy_violation_response(&req.path, err);
        }

        let path = Path::new(&normalized_path);

        match fs::write(path, &req.content) {
            Ok(_) => {
                serde_json::to_string_pretty(&json!({
                    "success": true,
                    "path": normalized_path,
                    "bytes_written": req.content.len(),
                    "message": "File written successfully"
                }))
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
            Err(e) => {
                serde_json::to_string_pretty(&json!({
                    "success": false,
                    "path": normalized_path,
                    "error": e.to_string()
                }))
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
        }
    }

    #[tool(description = "List files in a directory")]
    async fn file_list(
        &self,
        Parameters(req): Parameters<FileListRequest>,
    ) -> Result<String, ErrorData> {
        let normalized_path = Self::normalize_path(&req.path);

        if let Err(err) = self.check_storage_permission(&normalized_path, "read") {
            return Self::policy_violation_response(&req.path, err);
        }

        let path = Path::new(&normalized_path);

        match fs::read_dir(path) {
            Ok(entries) => {
                let mut files = Vec::new();
                for entry in entries {
                    if let Ok(entry) = entry {
                        if let Some(name) = entry.file_name().to_str() {
                            let metadata = entry.metadata().ok();
                            files.push(json!({
                                "name": name,
                                "is_dir": metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                                "size": metadata.as_ref().and_then(|m| {
                                    if m.is_file() { Some(m.len()) } else { None }
                                })
                            }));
                        }
                    }
                }

                serde_json::to_string_pretty(&json!({
                    "success": true,
                    "path": normalized_path,
                    "files": files,
                    "count": files.len()
                }))
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
            Err(e) => {
                serde_json::to_string_pretty(&json!({
                    "success": false,
                    "path": normalized_path,
                    "error": e.to_string()
                }))
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
        }
    }

    #[tool(description = "Test PostgreSQL database connection")]
    async fn test_connection(&self) -> Result<String, String> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL environment variable not set".to_string())?;

        #[cfg(feature = "wasmedge-postgres")]
        {
            // Try to ensure connection first
            if let Err(e) = self.ensure_connection().await {
                return serde_json::to_string_pretty(&json!({
                    "connected": false,
                    "error": e,
                    "database_url": database_url,
                    "hint": "Connection timed out or PostgreSQL is not running"
                }))
                .map_err(|e| e.to_string());
            }

            let client = self.db_client.read().await;
            if let Some(client) = client.as_ref() {
                // Try a simple query with timeout
                match tokio::time::timeout(
                    std::time::Duration::from_secs(1),
                    client.query_one("SELECT version()", &[]),
                )
                .await
                {
                    Ok(Ok(row)) => {
                        let version: String = row.get(0);
                        serde_json::to_string_pretty(&json!({
                            "connected": true,
                            "database": "PostgreSQL",
                            "version": version,
                            "database_url": database_url
                        }))
                        .map_err(|e| e.to_string())
                    }
                    Ok(Err(e)) => serde_json::to_string_pretty(&json!({
                        "connected": false,
                        "error": format!("Query failed: {}", e),
                        "database_url": database_url,
                        "hint": "Database client exists but query failed"
                    }))
                    .map_err(|e| e.to_string()),
                    Err(_) => serde_json::to_string_pretty(&json!({
                        "connected": false,
                        "error": "Connection test timed out after 1 second",
                        "database_url": database_url,
                        "hint": "This might indicate network issues or PostgreSQL is not responding"
                    }))
                    .map_err(|e| e.to_string()),
                }
            } else {
                serde_json::to_string_pretty(&json!({
                    "connected": false,
                    "error": "No database client initialized",
                    "database_url": database_url,
                    "hint": "Initial connection failed. Check if PostgreSQL is running."
                }))
                .map_err(|e| e.to_string())
            }
        }
        #[cfg(not(feature = "wasmedge-postgres"))]
        {
            serde_json::to_string_pretty(&json!({
                "connected": false,
                "error": "Database support not enabled",
                "hint": "Build with --features wasmedge-postgres"
            }))
            .map_err(|e| e.to_string())
        }
    }
}

#[tool_handler]
impl ServerHandler for FullStackServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "fullstack".to_string(),
                title: Some("WASM Fullstack Server".to_string()),
                version: "1.0.0".to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some("Full-Stack Server - Real PostgreSQL & HTTP with WasmEdge".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
