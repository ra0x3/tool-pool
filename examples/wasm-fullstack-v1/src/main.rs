use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Todo {
    pub id: String,
    pub user_id: i32,
    pub title: String,
    pub completed: bool,
    pub created_at: String,
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

#[derive(Debug, Clone)]
pub struct FullStackServerV1 {
    tool_router: ToolRouter<Self>,
    todos: Arc<Mutex<HashMap<String, Todo>>>,
}

impl FullStackServerV1 {
    pub async fn new() -> Self {
        let server = Self {
            tool_router: Self::tool_router(),
            todos: Arc::new(Mutex::new(HashMap::new())),
        };

        if true {
            let mut todos = HashMap::new();
            todos.insert(
                "todo-1".to_string(),
                Todo {
                    id: "todo-1".to_string(),
                    user_id: 1,
                    title: "Setup PostgreSQL database".to_string(),
                    completed: true,
                    created_at: chrono::Utc::now().to_rfc3339(),
                },
            );
            if let Ok(mut t) = server.todos.lock() {
                *t = todos;
            }
        }

        server
    }
}

#[tool_router]
impl FullStackServerV1 {
    #[tool(description = "Fetch todos")]
    async fn fetch_todos(
        &self,
        Parameters(req): Parameters<FetchTodosRequest>,
    ) -> Result<String, String> {
        let todos = self.todos.lock().map_err(|e| e.to_string())?;
        let filtered: Vec<Todo> = if let Some(user_id) = req.user_id {
            todos
                .values()
                .filter(|t| t.user_id == user_id)
                .cloned()
                .collect()
        } else {
            todos.values().cloned().collect()
        };

        serde_json::to_string_pretty(&json!({
            "todos": filtered,
            "count": filtered.len(),
            "source": "in-memory"
        }))
        .map_err(|e| e.to_string())
    }

    #[tool(description = "Create a new todo")]
    async fn create_todo(
        &self,
        Parameters(req): Parameters<CreateTodoRequest>,
    ) -> Result<String, String> {
        let todo_id = format!("todo-{}", uuid::new_v4());
        let todo = Todo {
            id: todo_id.clone(),
            user_id: req.user_id,
            title: req.title.clone(),
            completed: false,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let mut todos = self.todos.lock().map_err(|e| e.to_string())?;
        todos.insert(todo.id.clone(), todo.clone());

        serde_json::to_string_pretty(&json!({
            "id": todo_id,
            "title": req.title,
            "user_id": req.user_id,
            "source": "in-memory"
        }))
        .map_err(|e| e.to_string())
    }

    #[tool(description = "Update a todo")]
    async fn update_todo(
        &self,
        Parameters(req): Parameters<UpdateTodoRequest>,
    ) -> Result<String, String> {
        let mut todos = self.todos.lock().map_err(|e| e.to_string())?;
        if let Some(todo) = todos.get_mut(&req.id) {
            if let Some(title) = req.title {
                todo.title = title;
            }
            if let Some(completed) = req.completed {
                todo.completed = completed;
            }
            return serde_json::to_string_pretty(&json!({
                "id": req.id,
                "updated": true,
                "source": "in-memory"
            }))
            .map_err(|e| e.to_string());
        }

        Err(format!("Todo {} not found", req.id))
    }

    #[tool(description = "Delete a todo")]
    async fn delete_todo(
        &self,
        Parameters(req): Parameters<DeleteTodoRequest>,
    ) -> Result<String, String> {
        let mut todos = self.todos.lock().map_err(|e| e.to_string())?;
        if todos.remove(&req.id).is_some() {
            return serde_json::to_string_pretty(&json!({
                "id": req.id,
                "deleted": true,
                "source": "in-memory"
            }))
            .map_err(|e| e.to_string());
        }

        Err(format!("Todo {} not found", req.id))
    }
}

#[tool_handler]
impl ServerHandler for FullStackServerV1 {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Full-Stack Server v1 - In-Memory Storage".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

mod uuid {
    pub fn new_v4() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let random = (timestamp * 1103515245 + 12345) & 0x7fffffff;
        format!("{:x}-{:x}", timestamp & 0xffffffff, random)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let server = FullStackServerV1::new().await;

    match server.serve(wasm_fullstack_v1::wasi_io()).await {
        Ok(service) => {
            tracing::info!("Full-Stack Server v1 running");
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
