//! MCP-specific permission extension

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    core::{Action, Permission, PolicyExtension, RuntimeConfig},
    error::Result,
};

/// MCP extension for mcpkit-rs specific permissions
pub struct McpExtension;

impl PolicyExtension for McpExtension {
    fn id(&self) -> &str {
        "mcp"
    }

    fn parse(&self, value: &serde_yaml::Value) -> Result<Box<dyn Permission>> {
        let mcp_perms: McpPermissions = serde_yaml::from_value(value.clone())?;
        Ok(Box::new(mcp_perms))
    }

    fn validate(&self, permission: &dyn Permission) -> Result<()> {
        permission.validate()
    }

    fn to_runtime_config(&self, _permission: &dyn Permission) -> Result<RuntimeConfig> {
        let config = RuntimeConfig {
            runtime: "mcp".to_string(),
            config: serde_json::json!({}),
            flags: crate::core::CapabilityFlags {
                can_execute_tools: true,
                can_access_resources: true,
                ..Default::default()
            },
        };
        Ok(config)
    }
}

/// MCP-specific permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPermissions {
    /// Tool execution permissions
    #[serde(default)]
    pub tools: Option<ToolPermissions>,

    /// Prompt access permissions
    #[serde(default)]
    pub prompts: Option<PromptPermissions>,

    /// Resource access permissions
    #[serde(default)]
    pub resources: Option<ResourcePermissions>,

    /// Transport layer permissions
    #[serde(default)]
    pub transport: Option<TransportPermissions>,
}

/// Tool execution permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermissions {
    /// List of allowed tools
    #[serde(default)]
    pub allow: Vec<ToolRule>,

    /// List of denied tools
    #[serde(default)]
    pub deny: Vec<ToolRule>,
}

/// Individual tool rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRule {
    /// Tool name or pattern
    pub name: String,

    /// Rate limit for tool calls per minute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_calls_per_minute: Option<u32>,

    /// Parameter constraints for the tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

/// Prompt permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptPermissions {
    /// List of allowed prompts
    #[serde(default)]
    pub allow: Vec<PromptRule>,

    /// List of denied prompts
    #[serde(default)]
    pub deny: Vec<PromptRule>,
}

/// Individual prompt rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptRule {
    /// Prompt name or pattern
    pub name: String,

    /// Maximum prompt length in characters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
}

/// Resource access permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePermissions {
    /// List of allowed resources
    #[serde(default)]
    pub allow: Vec<ResourceRule>,

    /// List of denied resources
    #[serde(default)]
    pub deny: Vec<ResourceRule>,
}

/// Individual resource rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRule {
    /// Resource URI pattern
    pub uri: String,
    /// Allowed operations (read, write, list, etc.)
    pub operations: Vec<String>,
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportPermissions {
    /// Enable stdio transport
    #[serde(default)]
    pub stdio: bool,

    /// HTTP transport configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpTransportConfig>,

    /// Enable WebSocket transport
    #[serde(default)]
    pub websocket: bool,
}

/// HTTP transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpTransportConfig {
    /// List of allowed HTTP hosts
    pub allowed_hosts: Vec<String>,
}

/// MCP-specific action types
#[derive(Debug)]
pub struct McpAction {
    /// Type of MCP action
    pub action_type: McpActionType,
    /// Resource being accessed
    pub resource: String,
    /// Additional context for the action
    pub context: Option<HashMap<String, String>>,
}

/// Types of MCP actions that can be performed
#[derive(Debug)]
pub enum McpActionType {
    /// Tool execution action
    ToolExecute,
    /// Prompt retrieval action
    PromptGet,
    /// Resource read action
    ResourceRead,
    /// Resource write action
    ResourceWrite,
    /// Resource list action
    ResourceList,
}

impl Action for McpAction {
    fn action_type(&self) -> &str {
        match self.action_type {
            McpActionType::ToolExecute => "tool",
            McpActionType::PromptGet => "prompt",
            McpActionType::ResourceRead => "resource_read",
            McpActionType::ResourceWrite => "resource_write",
            McpActionType::ResourceList => "resource_list",
        }
    }

    fn resource(&self) -> &str {
        &self.resource
    }

    fn context(&self) -> Option<&dyn std::any::Any> {
        self.context.as_ref().map(|c| c as &dyn std::any::Any)
    }
}

#[async_trait]
impl Permission for McpPermissions {
    fn is_allowed(&self, action: &dyn Action) -> bool {
        match action.action_type() {
            "tool" => self.is_tool_allowed(action.resource()),
            "prompt" => self.is_prompt_allowed(action.resource()),
            "resource_read" | "resource_write" | "resource_list" => {
                self.is_resource_allowed(action.resource(), action.action_type())
            }
            _ => false,
        }
    }

    fn merge(&self, _other: &dyn Permission) -> Result<Box<dyn Permission>> {
        // This would merge with another McpPermissions instance
        Ok(Box::new(self.clone()))
    }

    fn to_cache_key(&self) -> String {
        format!("mcp:{:?}", self)
    }

    fn validate(&self) -> Result<()> {
        // Validate tool permissions
        if let Some(tools) = &self.tools {
            for rule in &tools.allow {
                if rule.name.is_empty() {
                    return Err(crate::error::PolicyError::ValidationError(
                        "Tool name cannot be empty".to_string(),
                    ));
                }
            }
        }

        // Validate prompt permissions
        if let Some(prompts) = &self.prompts {
            for rule in &prompts.allow {
                if rule.name.is_empty() {
                    return Err(crate::error::PolicyError::ValidationError(
                        "Prompt name cannot be empty".to_string(),
                    ));
                }
            }
        }

        // Validate resource permissions
        if let Some(resources) = &self.resources {
            for rule in &resources.allow {
                if rule.uri.is_empty() {
                    return Err(crate::error::PolicyError::ValidationError(
                        "Resource URI cannot be empty".to_string(),
                    ));
                }
                if rule.operations.is_empty() {
                    return Err(crate::error::PolicyError::ValidationError(
                        "Resource operations cannot be empty".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
}

impl McpPermissions {
    fn is_tool_allowed(&self, name: &str) -> bool {
        if let Some(tools) = &self.tools {
            // Check deny list first
            for rule in &tools.deny {
                if glob_match(&rule.name, name) {
                    return false;
                }
            }

            // Check allow list
            for rule in &tools.allow {
                if glob_match(&rule.name, name) {
                    return true;
                }
            }

            false
        } else {
            false
        }
    }

    fn is_prompt_allowed(&self, name: &str) -> bool {
        if let Some(prompts) = &self.prompts {
            // Check deny list first
            for rule in &prompts.deny {
                if rule.name == name {
                    return false;
                }
            }

            // Check allow list
            for rule in &prompts.allow {
                if rule.name == name {
                    return true;
                }
            }

            false
        } else {
            false
        }
    }

    fn is_resource_allowed(&self, uri: &str, operation: &str) -> bool {
        if let Some(resources) = &self.resources {
            // Check deny list first
            for rule in &resources.deny {
                if glob_match(&rule.uri, uri) && rule.operations.iter().any(|op| op == operation) {
                    return false;
                }
            }

            // Check allow list
            for rule in &resources.allow {
                if glob_match(&rule.uri, uri) && rule.operations.iter().any(|op| op == operation) {
                    return true;
                }
            }

            false
        } else {
            false
        }
    }
}

/// Match a glob pattern against text
pub fn glob_match(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        let mut text_pos = 0;

        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }

            if i == 0 && !text.starts_with(part) {
                return false;
            }

            if i == parts.len() - 1 && !pattern.ends_with('*') && !text.ends_with(part) {
                return false;
            }

            if let Some(pos) = text[text_pos..].find(part) {
                text_pos += pos + part.len();
            } else {
                return false;
            }
        }

        true
    } else {
        pattern == text
    }
}
