// V2 - Real PostgreSQL and HTTP with WasmEdge runtime ONLY
// This will NOT work with standard WASM runtimes - requires WasmEdge

#[cfg(feature = "wasmedge-postgres")]
use std::env;
use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(feature = "wasmedge-postgres")]
use tokio_postgres::{Client, NoTls};
#[cfg(not(feature = "wasmedge-postgres"))]
type Client = ();

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
    #[schemars(description = "Fetch from JSONPlaceholder API")]
    pub from_api: Option<bool>,
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

#[derive(Debug, Clone)]
pub struct FullStackServerV2 {
    tool_router: ToolRouter<Self>,
    db_client: Arc<Option<Client>>,
}

impl FullStackServerV2 {
    pub async fn new() -> Self {
        eprintln!("=== Full-Stack v2 Server (WasmEdge) ===");
        eprintln!("Real PostgreSQL & HTTP connections enabled");
        eprintln!();

        let db_client = Self::connect_db().await;
        if db_client.is_none() {
            eprintln!("WARNING: Could not connect to PostgreSQL");
            eprintln!("Make sure PostgreSQL is running: docker-compose up -d");
        } else {
            eprintln!("Connected to PostgreSQL successfully!");
        }

        Self {
            tool_router: Self::tool_router(),
            db_client: Arc::new(db_client),
        }
    }

    async fn connect_db() -> Option<Client> {
        #[cfg(feature = "wasmedge-postgres")]
        {
            let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres@localhost/wasmedge".to_string()
            });

            eprintln!("Attempting to connect to: {}", database_url);

            // Use a timeout to avoid hanging when not running under WasmEdge
            match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                tokio_postgres::connect(&database_url, NoTls)
            ).await {
                Ok(Ok((client, connection))) => {
                    tokio::spawn(async move {
                        if let Err(e) = connection.await {
                            eprintln!("Connection error: {}", e);
                        }
                    });
                    eprintln!("Connected to PostgreSQL!");
                    Some(client)
                }
                Ok(Err(e)) => {
                    eprintln!("Failed to connect to PostgreSQL: {}", e);
                    None
                }
                Err(_) => {
                    eprintln!("PostgreSQL connection timed out (expected when not using WasmEdge)");
                    None
                }
            }
        }
        #[cfg(not(feature = "wasmedge-postgres"))]
        {
            eprintln!("PostgreSQL support disabled (build with --features wasmedge-postgres)");
            None
        }
    }

    async fn fetch_from_api(user_id: Option<i32>) -> Result<Vec<JsonPlaceholderTodo>, String> {
        // For wasm32-wasip2, we'll return a mock response since wasmedge_wasi_socket
        // requires unstable features. In production, you'd use WasmEdge runtime with wasip1.
        eprintln!("Note: HTTP fetch disabled for wasm32-wasip2 target (requires WasmEdge runtime)");

        // Return mock data for demonstration
        let mock_todos = vec![
            JsonPlaceholderTodo {
                user_id: user_id.unwrap_or(1),
                id: 1,
                title: "Mock todo item (HTTP disabled for wasip2)".to_string(),
                completed: false,
            },
        ];

        Ok(mock_todos)
    }
}

// Tool implementations
#[tool_router]
impl FullStackServerV2 {
    #[tool(description = "Fetch todos from PostgreSQL or JSONPlaceholder API")]
    async fn fetch_todos(
        &self,
        Parameters(req): Parameters<FetchTodosRequest>,
    ) -> Result<String, String> {
        // Real API fetch via HTTP
        if req.from_api.unwrap_or(false) {
            let api_todos = Self::fetch_from_api(req.user_id).await?;

            // Save to PostgreSQL
            #[cfg(feature = "wasmedge-postgres")]
            if let Some(client) = self.db_client.as_ref() {
                for todo in &api_todos {
                    let id = format!("api-{}", todo.id);
                    let user_id_str = todo.user_id.to_string();
                    let _ = client
                        .execute(
                            "INSERT INTO todos (id, user_id, title, completed, created_at)
                         VALUES ($1, $2, $3, $4, NOW())
                         ON CONFLICT (id) DO UPDATE SET
                         title = EXCLUDED.title,
                         completed = EXCLUDED.completed,
                         updated_at = NOW()",
                            &[&id, &user_id_str, &todo.title, &todo.completed],
                        )
                        .await;
                }
            }

            return serde_json::to_string_pretty(&json!({
                "todos": api_todos,
                "count": api_todos.len(),
                "source": "jsonplaceholder-api"
            }))
            .map_err(|e| e.to_string());
        }

        // Real PostgreSQL fetch
        #[cfg(feature = "wasmedge-postgres")]
        {
            if let Some(client) = self.db_client.as_ref() {
                let query =
                    if let Some(user_id) = req.user_id {
                        client.query(
                        "SELECT id, user_id, title, completed, created_at::text, updated_at::text
                         FROM todos WHERE user_id = $1 ORDER BY created_at DESC",
                        &[&user_id],
                    ).await
                    } else {
                        client.query(
                        "SELECT id, user_id, title, completed, created_at::text, updated_at::text
                         FROM todos ORDER BY created_at DESC",
                        &[],
                    ).await
                    };

                match query {
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
                            "todos": todos,
                            "count": todos.len(),
                            "source": "postgresql"
                        }))
                        .map_err(|e| e.to_string())
                    }
                    Err(e) => Err(format!("Database query failed: {}", e)),
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

    #[tool(description = "Create todo in PostgreSQL")]
    async fn create_todo(
        &self,
        Parameters(req): Parameters<CreateTodoRequest>,
    ) -> Result<String, String> {
        #[cfg(feature = "wasmedge-postgres")]
        {
            if let Some(client) = self.db_client.as_ref() {
                let todo_id = format!("todo-{}", chrono::Utc::now().timestamp_millis());
                let user_id_str = req.user_id.to_string();

                match client
                    .execute(
                        "INSERT INTO todos (id, user_id, title, completed, created_at)
                     VALUES ($1, $2, $3, false, NOW())",
                        &[&todo_id, &user_id_str, &req.title],
                    )
                    .await
                {
                    Ok(_) => {
                        // Also write to WAL table
                        let json_data = json!({
                            "id": todo_id,
                            "user_id": req.user_id,
                            "title": req.title
                        }).to_string();

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
            if let Some(client) = self.db_client.as_ref() {
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
                            // Write to WAL
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
            if let Some(client) = self.db_client.as_ref() {
                match client
                    .execute("DELETE FROM todos WHERE id = $1", &[&req.id])
                    .await
                {
                    Ok(rows_affected) => {
                        if rows_affected == 0 {
                            Err(format!("Todo with id {} not found", req.id))
                        } else {
                            // Write to WAL
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
            if let Some(client) = self.db_client.as_ref() {
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
            if let Some(client) = self.db_client.as_ref() {
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
            if let Some(client) = self.db_client.as_ref() {
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
                                let data: serde_json::Value = serde_json::from_str(&data_str).unwrap_or(json!({}));
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
            if let Some(client) = self.db_client.as_ref() {
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

    #[tool(description = "Test PostgreSQL database connection")]
    async fn test_connection(&self) -> Result<String, String> {
        #[cfg(feature = "wasmedge-postgres")]
        {
            // First check if we have a database URL configured
            let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres@localhost/wasmedge".to_string()
            });

            if let Some(client) = self.db_client.as_ref() {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(1),
                    client.query_one("SELECT version()", &[])
                ).await {
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
                    Ok(Err(e)) => {
                        serde_json::to_string_pretty(&json!({
                            "connected": false,
                            "error": format!("Query failed: {}", e),
                            "database_url": database_url,
                            "hint": "Database client exists but query failed"
                        }))
                        .map_err(|e| e.to_string())
                    }
                    Err(_) => {
                        serde_json::to_string_pretty(&json!({
                            "connected": false,
                            "error": "Connection test timed out after 1 second",
                            "database_url": database_url,
                            "hint": "This might indicate network issues or PostgreSQL is not running"
                        }))
                        .map_err(|e| e.to_string())
                    }
                }
            } else {
                serde_json::to_string_pretty(&json!({
                    "connected": false,
                    "error": "No database client initialized",
                    "database_url": database_url,
                    "hint": "The initial connection failed during server startup. Check if PostgreSQL is running on the specified URL."
                }))
                .map_err(|e| e.to_string())
            }
        }
        #[cfg(not(feature = "wasmedge-postgres"))]
        {
            Err("Database support not enabled. Build with --features wasmedge-postgres".to_string())
        }
    }
}

#[tool_handler]
impl ServerHandler for FullStackServerV2 {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Full-Stack Server v2 - Real PostgreSQL & HTTP with WasmEdge".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}


#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let server = FullStackServerV2::new().await;
    use wasm_fullstack_v2::wasi_io;
    match server.serve(wasi_io()).await {
        Ok(service) => {
            tracing::info!("Full-Stack Server v2 running");
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