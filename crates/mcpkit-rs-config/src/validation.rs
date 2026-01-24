//! Configuration validation

use semver::Version;

use crate::{Config, ConfigError, Result};

/// Validate a configuration
pub fn validate_config(config: &Config) -> Result<()> {
    validate_version(&config.version)?;
    validate_server_config(&config.server)?;
    validate_transport_config(&config.transport)?;
    validate_runtime_config(&config.runtime)?;
    validate_mcp_config(&config.mcp)?;

    if let Some(ref policy) = config.policy {
        policy.validate().map_err(ConfigError::PolicyError)?;
    }

    Ok(())
}

fn validate_version(version: &str) -> Result<()> {
    if version.is_empty() {
        return Err(ConfigError::ValidationError(
            "Version cannot be empty".to_string(),
        ));
    }

    if !version.starts_with("1.") && version != "1" {
        return Err(ConfigError::ValidationError(format!(
            "Unsupported configuration version: {}",
            version
        )));
    }

    Ok(())
}

fn validate_server_config(server: &crate::ServerConfig) -> Result<()> {
    if server.name.is_empty() {
        return Err(ConfigError::ValidationError(
            "Server name cannot be empty".to_string(),
        ));
    }

    if server.version.is_empty() {
        return Err(ConfigError::ValidationError(
            "Server version cannot be empty".to_string(),
        ));
    }

    Version::parse(&server.version).map_err(|_| {
        ConfigError::ValidationError(format!("Invalid server version format: {}", server.version))
    })?;

    if server.port == 0 {
        return Err(ConfigError::ValidationError(
            "Server port cannot be 0".to_string(),
        ));
    }

    if let Some(timeout) = server.request_timeout {
        if timeout == 0 {
            return Err(ConfigError::ValidationError(
                "Request timeout cannot be 0".to_string(),
            ));
        }
    }

    if let Some(ref log_level) = server.log_level {
        match log_level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid log level: {}",
                    log_level
                )));
            }
        }
    }

    Ok(())
}

fn validate_transport_config(transport: &crate::TransportConfig) -> Result<()> {
    // Ensure transport settings match transport type
    match (&transport.transport_type, &transport.settings) {
        (crate::TransportType::Stdio, crate::TransportSettings::Stdio(_)) => {}
        (crate::TransportType::Http, crate::TransportSettings::Http(_)) => {}
        (crate::TransportType::WebSocket, crate::TransportSettings::WebSocket(_)) => {}
        (crate::TransportType::Grpc, crate::TransportSettings::Grpc(_)) => {}
        (transport_type, _) => {
            return Err(ConfigError::ValidationError(format!(
                "Transport settings do not match transport type: {:?}",
                transport_type
            )));
        }
    }

    match &transport.settings {
        crate::TransportSettings::Stdio(stdio) => {
            if let Some(buffer_size) = stdio.buffer_size {
                if buffer_size == 0 {
                    return Err(ConfigError::ValidationError(
                        "Stdio buffer size cannot be 0".to_string(),
                    ));
                }
            }
        }
        crate::TransportSettings::Http(http) => {
            if let Some(max_body_size) = http.max_body_size {
                if max_body_size == 0 {
                    return Err(ConfigError::ValidationError(
                        "HTTP max body size cannot be 0".to_string(),
                    ));
                }
            }
            if let Some(ref tls) = http.tls {
                validate_tls_config(tls)?;
            }
        }
        crate::TransportSettings::Grpc(grpc) => {
            if let Some(max_msg_size) = grpc.max_message_size {
                if max_msg_size == 0 {
                    return Err(ConfigError::ValidationError(
                        "gRPC max message size cannot be 0".to_string(),
                    ));
                }
            }
            if let Some(ref tls) = grpc.tls {
                validate_tls_config(tls)?;
            }
        }
        crate::TransportSettings::WebSocket(ws) => {
            if let Some(interval) = ws.ping_interval {
                if interval == 0 {
                    return Err(ConfigError::ValidationError(
                        "WebSocket ping interval cannot be 0".to_string(),
                    ));
                }
            }
            if let Some(max_frame) = ws.max_frame_size {
                if max_frame == 0 {
                    return Err(ConfigError::ValidationError(
                        "WebSocket max frame size cannot be 0".to_string(),
                    ));
                }
            }
        }
    }

    Ok(())
}

fn validate_tls_config(tls: &crate::TlsConfig) -> Result<()> {
    if !tls.cert_file.exists() {
        return Err(ConfigError::ValidationError(format!(
            "TLS certificate file not found: {}",
            tls.cert_file.display()
        )));
    }

    if !tls.key_file.exists() {
        return Err(ConfigError::ValidationError(format!(
            "TLS key file not found: {}",
            tls.key_file.display()
        )));
    }

    if let Some(ref ca_file) = tls.ca_file {
        if !ca_file.exists() {
            return Err(ConfigError::ValidationError(format!(
                "TLS CA file not found: {}",
                ca_file.display()
            )));
        }
    }

    Ok(())
}

fn validate_runtime_config(runtime: &crate::RuntimeConfig) -> Result<()> {
    if let Some(ref wasm) = runtime.wasm {
        if let Some(ref module_path) = wasm.module_path {
            if !module_path.exists() {
                return Err(ConfigError::ValidationError(format!(
                    "WASM module not found: {}",
                    module_path.display()
                )));
            }
        }

        if let Some(memory_pages) = wasm.memory_pages {
            if memory_pages == 0 {
                return Err(ConfigError::ValidationError(
                    "WASM memory pages cannot be 0".to_string(),
                ));
            }
        }
    }

    if let Some(ref limits) = runtime.limits {
        validate_resource_limits(limits)?;
    }

    Ok(())
}

fn validate_resource_limits(limits: &crate::ResourceLimits) -> Result<()> {
    if let Some(ref cpu) = limits.cpu {
        if !is_valid_cpu_limit(cpu) {
            return Err(ConfigError::ValidationError(format!(
                "Invalid CPU limit format: {}",
                cpu
            )));
        }
    }

    if let Some(ref memory) = limits.memory {
        if !is_valid_memory_limit(memory) {
            return Err(ConfigError::ValidationError(format!(
                "Invalid memory limit format: {}",
                memory
            )));
        }
    }

    if let Some(ref time) = limits.execution_time {
        if !is_valid_time_limit(time) {
            return Err(ConfigError::ValidationError(format!(
                "Invalid execution time format: {}",
                time
            )));
        }
    }

    Ok(())
}

fn validate_mcp_config(mcp: &crate::McpConfig) -> Result<()> {
    if mcp.protocol_version.is_empty() {
        return Err(ConfigError::ValidationError(
            "MCP protocol version cannot be empty".to_string(),
        ));
    }

    if let Some(ref tools) = mcp.tools {
        for tool in tools {
            if tool.name.is_empty() {
                return Err(ConfigError::ValidationError(
                    "Tool name cannot be empty".to_string(),
                ));
            }
            if tool.description.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Tool {} must have a description",
                    tool.name
                )));
            }
        }
    }

    if let Some(ref prompts) = mcp.prompts {
        for prompt in prompts {
            if prompt.name.is_empty() {
                return Err(ConfigError::ValidationError(
                    "Prompt name cannot be empty".to_string(),
                ));
            }
            if prompt.description.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Prompt {} must have a description",
                    prompt.name
                )));
            }
        }
    }

    if let Some(ref resources) = mcp.resources {
        for resource in resources {
            if resource.name.is_empty() {
                return Err(ConfigError::ValidationError(
                    "Resource name cannot be empty".to_string(),
                ));
            }
            if resource.uri.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Resource {} must have a URI",
                    resource.name
                )));
            }
        }
    }

    Ok(())
}

fn is_valid_cpu_limit(cpu: &str) -> bool {
    if let Some(stripped) = cpu.strip_suffix("m") {
        stripped.parse::<u64>().is_ok()
    } else {
        cpu.parse::<f64>().is_ok()
    }
}

fn is_valid_memory_limit(memory: &str) -> bool {
    memory.ends_with("Ki")
        || memory.ends_with("Mi")
        || memory.ends_with("Gi")
        || memory.ends_with("K")
        || memory.ends_with("M")
        || memory.ends_with("G")
        || memory.parse::<u64>().is_ok()
}

fn is_valid_time_limit(time: &str) -> bool {
    time.ends_with("ms") || time.ends_with("s") || time.ends_with("m") || time.ends_with("h")
}
