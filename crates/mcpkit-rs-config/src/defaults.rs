//! Default configuration values

use std::collections::HashMap;

use crate::*;

/// Create a default configuration
pub fn default_config() -> Config {
    Config {
        version: "1.0".to_string(),
        metadata: None,
        server: default_server_config(),
        transport: default_transport_config(),
        policy: None,
        runtime: default_runtime_config(),
        mcp: default_mcp_config(),
        extensions: HashMap::new(),
    }
}

pub fn default_server_config() -> ServerConfig {
    ServerConfig {
        name: "mcpkit-rs-server".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: Some("MCP server powered by mcpkit-rs".to_string()),
        bind: "127.0.0.1".to_string(),
        port: 3000,
        max_connections: Some(100),
        request_timeout: Some(30),
        debug: false,
        log_level: Some("info".to_string()),
    }
}

pub fn default_transport_config() -> TransportConfig {
    TransportConfig {
        transport_type: TransportType::Stdio,
        settings: TransportSettings::Stdio(StdioSettings {
            buffer_size: Some(65536),
        }),
    }
}

pub fn default_runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        runtime_type: RuntimeType::Native,
        wasm: None,
        limits: Some(ResourceLimits {
            cpu: Some("1000m".to_string()),
            memory: Some("512Mi".to_string()),
            execution_time: Some("60s".to_string()),
            max_requests_per_minute: Some(1000),
        }),
    }
}

pub fn default_mcp_config() -> McpConfig {
    McpConfig {
        protocol_version: "2024-11-05".to_string(),
        tools: None,
        prompts: None,
        resources: None,
        capabilities: Some(McpCapabilities::List(vec![
            "tools".to_string(),
            "prompts".to_string(),
            "resources".to_string(),
            "logging".to_string(),
        ])),
    }
}
