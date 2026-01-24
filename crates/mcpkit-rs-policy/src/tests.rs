#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compiled::CompiledPolicy,
        extensions::mcp::{McpPermissions, ToolPermissions, ToolRule},
        permissions::{NetworkRule, Policy, StorageRule},
    };

    #[test]
    fn test_policy_parsing_yaml() {
        let yaml = r#"
version: "1.0"
description: "Test policy"
core:
  network:
    allow:
      - host: "api.example.com"
      - host: "*.internal.com"
    deny:
      - host: "malicious.com"
  storage:
    allow:
      - uri: "fs://tmp/**"
        access: ["read", "write"]
      - uri: "fs://config/*.json"
        access: ["read"]
"#;

        let policy = Policy::from_yaml(yaml).unwrap();
        assert_eq!(policy.version, "1.0");
        assert_eq!(policy.description, Some("Test policy".to_string()));

        let network = policy.core.network.unwrap();
        assert_eq!(network.allow.len(), 2);
        assert_eq!(network.deny.len(), 1);

        let storage = policy.core.storage.unwrap();
        assert_eq!(storage.allow.len(), 2);
    }

    #[test]
    fn test_policy_validation() {
        let mut policy = Policy {
            version: "".to_string(),
            description: None,
            core: Default::default(),
            extensions: Default::default(),
        };

        assert!(policy.validate().is_err());

        policy.version = "1.0".to_string();
        assert!(policy.validate().is_ok());

        policy.version = "2.0".to_string();
        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_compiled_policy_tool_check() {
        let policy = Policy {
            version: "1.0".to_string(),
            description: None,
            core: Default::default(),
            extensions: Default::default(),
        };

        let compiled = CompiledPolicy::compile(&policy).unwrap();

        // With empty policy, nothing should be allowed
        assert!(!compiled.is_tool_allowed("calculator"));
    }

    #[test]
    fn test_top_level_tools_shorthand() {
        let yaml = r#"
version: "1.0"
tools:
  allow:
    - fetch_todos
    - search_*
  deny:
    - search_todos
"#;

        let policy = Policy::from_yaml(yaml).unwrap();
        let compiled = CompiledPolicy::compile(&policy).unwrap();

        assert!(compiled.is_tool_allowed("fetch_todos"));
        assert!(compiled.is_tool_allowed("search_items"));
        assert!(!compiled.is_tool_allowed("search_todos"));
        assert!(!compiled.is_tool_allowed("unknown_tool"));
    }

    #[test]
    fn test_compiled_policy_network_check() {
        let mut policy = Policy {
            version: "1.0".to_string(),
            description: None,
            core: Default::default(),
            extensions: Default::default(),
        };

        policy.core.network = Some(crate::permissions::NetworkPermissions {
            allow: vec![
                NetworkRule::Host {
                    host: "api.example.com".to_string(),
                },
                NetworkRule::Host {
                    host: "*.internal.com".to_string(),
                },
            ],
            deny: vec![NetworkRule::Host {
                host: "malicious.com".to_string(),
            }],
        });

        let compiled = CompiledPolicy::compile(&policy).unwrap();

        // Debug output
        eprintln!("Network whitelist: {:?}", compiled.network_whitelist);
        eprintln!("Network blacklist: {:?}", compiled.network_blacklist);
        eprintln!(
            "Pattern matches test.internal.com: {}",
            compiled.network_patterns.is_match("test.internal.com")
        );
        eprintln!(
            "Testing api.example.com: {}",
            compiled.is_network_allowed("api.example.com")
        );
        eprintln!(
            "Testing test.internal.com: {}",
            compiled.is_network_allowed("test.internal.com")
        );

        assert!(compiled.is_network_allowed("api.example.com"));
        assert!(compiled.is_network_allowed("test.internal.com"));
        assert!(!compiled.is_network_allowed("malicious.com"));
        assert!(!compiled.is_network_allowed("unknown.com"));
    }

    #[test]
    fn test_compiled_policy_storage_check() {
        let mut policy = Policy {
            version: "1.0".to_string(),
            description: None,
            core: Default::default(),
            extensions: Default::default(),
        };

        policy.core.storage = Some(crate::permissions::StoragePermissions {
            allow: vec![
                StorageRule {
                    uri: "fs:///tmp/**".to_string(),
                    access: vec!["read".to_string(), "write".to_string()],
                },
                StorageRule {
                    uri: "fs:///config/*.json".to_string(),
                    access: vec!["read".to_string()],
                },
            ],
            deny: vec![StorageRule {
                uri: "fs:///etc/**".to_string(),
                access: vec!["write".to_string()],
            }],
        });

        let compiled = CompiledPolicy::compile(&policy).unwrap();

        eprintln!(
            "Storage allow patterns match /tmp/test.txt: {}",
            compiled.storage_allow_patterns.is_match("/tmp/test.txt")
        );
        eprintln!(
            "Storage allow patterns match tmp/test.txt: {}",
            compiled.storage_allow_patterns.is_match("tmp/test.txt")
        );
        eprintln!("Storage access map: {:?}", compiled.storage_access_map);

        assert!(compiled.is_storage_allowed("/tmp/test.txt", "read"));
        assert!(compiled.is_storage_allowed("/tmp/test.txt", "write"));
        assert!(compiled.is_storage_allowed("/config/app.json", "read"));
        assert!(!compiled.is_storage_allowed("/config/app.json", "write"));
        assert!(!compiled.is_storage_allowed("/etc/passwd", "write"));
    }

    #[test]
    fn test_policy_merge() {
        let mut policy1 = Policy {
            version: "1.0".to_string(),
            description: Some("Policy 1".to_string()),
            core: Default::default(),
            extensions: Default::default(),
        };

        policy1.core.network = Some(crate::permissions::NetworkPermissions {
            allow: vec![NetworkRule::Host {
                host: "api1.com".to_string(),
            }],
            deny: vec![],
        });

        let mut policy2 = Policy {
            version: "1.0".to_string(),
            description: Some("Policy 2".to_string()),
            core: Default::default(),
            extensions: Default::default(),
        };

        policy2.core.network = Some(crate::permissions::NetworkPermissions {
            allow: vec![NetworkRule::Host {
                host: "api2.com".to_string(),
            }],
            deny: vec![],
        });

        policy1.merge(policy2).unwrap();

        let network = policy1.core.network.unwrap();
        assert_eq!(network.allow.len(), 2);
    }

    #[test]
    fn test_mcp_extension() {
        use crate::{
            core::{Action, Permission, PolicyExtension},
            extensions::mcp::{McpAction, McpActionType, McpExtension, McpPermissions},
        };

        let yaml = serde_yaml::to_value(McpPermissions {
            tools: Some(ToolPermissions {
                allow: vec![ToolRule {
                    name: "calculator/*".to_string(),
                    max_calls_per_minute: Some(100),
                    parameters: None,
                }],
                deny: vec![ToolRule {
                    name: "system_*".to_string(),
                    max_calls_per_minute: None,
                    parameters: None,
                }],
            }),
            prompts: None,
            resources: None,
            transport: None,
        })
        .unwrap();

        let ext = McpExtension;
        let permission = ext.parse(&yaml).unwrap();

        let action = McpAction {
            action_type: McpActionType::ToolExecute,
            resource: "calculator/add".to_string(),
            context: None,
        };

        assert!(permission.is_allowed(&action));

        let denied_action = McpAction {
            action_type: McpActionType::ToolExecute,
            resource: "system_exec".to_string(),
            context: None,
        };

        assert!(!permission.is_allowed(&denied_action));
    }

    #[test]
    fn test_resource_limits_parsing() {
        let yaml = r#"
version: "1.0"
core:
  resources:
    limits:
      cpu: "500m"
      memory: "512Mi"
      execution_time: "30s"
"#;

        let policy = Policy::from_yaml(yaml).unwrap();
        let compiled = CompiledPolicy::compile(&policy).unwrap();

        assert_eq!(compiled.resource_limits.cpu_millicores, Some(500));
        assert_eq!(
            compiled.resource_limits.memory_bytes,
            Some(512 * 1024 * 1024)
        );
        assert_eq!(compiled.resource_limits.execution_time_ms, Some(30000));
    }

    #[test]
    fn test_capability_flags() {
        use crate::core::CapabilityFlags;

        let flags1 = CapabilityFlags {
            can_execute_tools: true,
            can_access_network: true,
            can_access_filesystem: false,
            can_read_environment: false,
            can_access_resources: false,
            custom_flags: 0b0101,
        };

        let flags2 = CapabilityFlags {
            can_execute_tools: true,
            can_access_network: false,
            can_access_filesystem: false,
            can_read_environment: false,
            can_access_resources: false,
            custom_flags: 0b0001,
        };

        assert!(flags1.has_all(flags2));
        assert!(flags1.has_any(flags2));

        let flags3 = CapabilityFlags {
            can_execute_tools: false,
            can_access_network: false,
            can_access_filesystem: true,
            can_read_environment: false,
            can_access_resources: false,
            custom_flags: 0,
        };

        assert!(!flags1.has_all(flags3));
        assert!(!flags1.has_any(flags3));
    }

    #[test]
    fn test_cache_functionality() {
        use crate::cache::{AccessMode, ActionHash, PermissionCache};

        let mut cache = PermissionCache::new(10);

        let action = ActionHash::Tool("calculator".to_string());
        assert!(cache.check(&action).is_none());

        cache.insert(action.clone(), true);
        assert_eq!(cache.check(&action), Some(true));

        // Test specialized caches
        assert_eq!(cache.check_tool("calculator"), Some(true));
        assert!(cache.check_tool("unknown").is_none());

        cache.insert(ActionHash::Network("api.example.com".to_string()), true);
        assert_eq!(cache.check_network("api.example.com"), Some(true));

        let stats = cache.stats();
        assert_eq!(stats.hits, 3); // We have 3 cache hits total
        assert!(stats.misses > 0);
    }

    #[test]
    fn test_glob_matching() {
        use crate::extensions::mcp::glob_match;

        assert!(glob_match("*", "anything"));
        assert!(glob_match("calculator/*", "calculator/add"));
        assert!(glob_match("*.json", "config.json"));
        assert!(glob_match("*/test/*", "foo/test/bar"));
        assert!(!glob_match("calculator/*", "system/exec"));
        assert!(!glob_match("*.json", "config.yaml"));
    }
}
