//! Pre-compiled policy for O(1) runtime checks

use bloomfilter::Bloom;
use globset::{Glob, GlobSet, GlobSetBuilder};
use rustc_hash::{FxHashMap, FxHashSet};
use serde_yaml::Value as YamlValue;

use crate::{
    core::CapabilityFlags,
    error::Result,
    permissions::{NetworkRule, Policy, StorageRule},
};

/// Pre-compiled policy optimized for fast runtime checks
#[derive(Clone)]
pub struct CompiledPolicy {
    /// Set of explicitly allowed tool names
    pub tool_whitelist: FxHashSet<String>,
    /// Glob patterns for tool matching
    pub tool_patterns: GlobSet,
    /// Set of explicitly denied tool names
    pub tool_blacklist: FxHashSet<String>,

    /// Set of explicitly allowed network hosts
    pub network_whitelist: FxHashSet<String>,
    /// Set of explicitly denied network hosts
    pub network_blacklist: FxHashSet<String>,
    /// Bloom filter for fast network host checking
    pub network_bloom: Bloom<String>,
    /// Glob patterns for network host matching
    pub network_patterns: GlobSet,

    /// Glob patterns for allowed storage paths
    pub storage_allow_patterns: GlobSet,
    /// Glob patterns for denied storage paths
    pub storage_deny_patterns: GlobSet,
    /// Map of path patterns to allowed access modes
    pub storage_access_map: FxHashMap<String, FxHashSet<String>>,

    /// Set of allowed environment variable names
    pub env_whitelist: FxHashSet<String>,
    /// Set of denied environment variable names
    pub env_blacklist: FxHashSet<String>,

    /// Path trie for efficient path matching
    pub resource_trie: PathTrie,
    /// Pre-computed capability flags
    pub capabilities: CapabilityFlags,

    /// Resource usage limits
    pub resource_limits: ResourceLimits,
}

/// Compiled resource limits
#[derive(Clone, Debug)]
pub struct ResourceLimits {
    /// CPU limit in millicores (1000 = 1 CPU core)
    pub cpu_millicores: Option<u64>,
    /// Memory limit in bytes
    pub memory_bytes: Option<u64>,
    /// Execution time limit in milliseconds
    pub execution_time_ms: Option<u64>,
    /// WebAssembly fuel limit
    pub fuel: Option<u64>,
}

/// Path trie for efficient path matching
#[derive(Clone)]
pub struct PathTrie {
    root: TrieNode,
}

#[derive(Clone)]
struct TrieNode {
    children: FxHashMap<String, TrieNode>,
    is_allowed: bool,
    is_denied: bool,
}

impl std::fmt::Debug for CompiledPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledPolicy")
            .field("tool_whitelist", &self.tool_whitelist)
            .field("tool_blacklist", &self.tool_blacklist)
            .field("network_whitelist", &self.network_whitelist)
            .field("network_blacklist", &self.network_blacklist)
            .field("env_whitelist", &self.env_whitelist)
            .field("env_blacklist", &self.env_blacklist)
            .field("capabilities", &self.capabilities)
            .field("resource_limits", &self.resource_limits)
            .finish()
    }
}

#[derive(Default)]
struct SimpleToolPolicy {
    allow: Vec<String>,
    deny: Vec<String>,
}

fn add_tool_allow_rule(
    compiled: &mut CompiledPolicy,
    pattern_builder: &mut GlobSetBuilder,
    name: &str,
) -> Result<()> {
    compiled.capabilities.can_execute_tools = true;
    if name.contains('*') {
        let glob = Glob::new(name).map_err(|e| {
            crate::error::PolicyError::GlobError(format!("Invalid glob pattern '{}': {}", name, e))
        })?;
        pattern_builder.add(glob);
    } else {
        compiled.tool_whitelist.insert(name.to_string());
    }
    Ok(())
}

fn parse_simple_tool_policy(value: &YamlValue) -> Result<SimpleToolPolicy> {
    match value {
        YamlValue::Null => Ok(SimpleToolPolicy::default()),
        YamlValue::Sequence(seq) => {
            let allow = seq
                .iter()
                .map(parse_tool_name)
                .collect::<Result<Vec<_>>>()?;
            Ok(SimpleToolPolicy {
                allow,
                deny: vec![],
            })
        }
        YamlValue::Mapping(map) => {
            let allow_key = YamlValue::String("allow".to_string());
            let deny_key = YamlValue::String("deny".to_string());
            let allow = map
                .get(&allow_key)
                .map(parse_tool_list)
                .transpose()?
                .unwrap_or_default();
            let deny = map
                .get(&deny_key)
                .map(parse_tool_list)
                .transpose()?
                .unwrap_or_default();
            Ok(SimpleToolPolicy { allow, deny })
        }
        _ => Err(crate::error::PolicyError::InvalidFormat(
            "Top-level 'tools' entry must be a mapping or sequence".to_string(),
        )),
    }
}

fn parse_tool_list(value: &YamlValue) -> Result<Vec<String>> {
    match value {
        YamlValue::Null => Ok(vec![]),
        YamlValue::Sequence(seq) => seq.iter().map(parse_tool_name).collect(),
        _ => Err(crate::error::PolicyError::InvalidFormat(
            "Tool lists must be sequences".to_string(),
        )),
    }
}

fn parse_tool_name(value: &YamlValue) -> Result<String> {
    match value {
        YamlValue::String(name) => Ok(name.clone()),
        YamlValue::Mapping(map) => {
            let name_key = YamlValue::String("name".to_string());
            map.get(&name_key)
                .and_then(YamlValue::as_str)
                .map(|name| name.to_string())
                .ok_or_else(|| {
                    crate::error::PolicyError::InvalidFormat(
                        "Tool entries must be strings or mappings containing a 'name' field"
                            .to_string(),
                    )
                })
        }
        _ => Err(crate::error::PolicyError::InvalidFormat(
            "Tool entries must be strings or mappings".to_string(),
        )),
    }
}

impl CompiledPolicy {
    /// Compile a policy into optimized runtime structures
    pub fn compile(policy: &Policy) -> Result<Self> {
        let mut compiled = CompiledPolicy {
            tool_whitelist: FxHashSet::default(),
            tool_patterns: GlobSetBuilder::new().build().unwrap(),
            tool_blacklist: FxHashSet::default(),
            network_whitelist: FxHashSet::default(),
            network_blacklist: FxHashSet::default(),
            network_bloom: Bloom::new_for_fp_rate(1000, 0.01),
            network_patterns: GlobSetBuilder::new().build().unwrap(),
            storage_allow_patterns: GlobSetBuilder::new().build().unwrap(),
            storage_deny_patterns: GlobSetBuilder::new().build().unwrap(),
            storage_access_map: FxHashMap::default(),
            env_whitelist: FxHashSet::default(),
            env_blacklist: FxHashSet::default(),
            resource_trie: PathTrie::new(),
            capabilities: CapabilityFlags::default(),
            resource_limits: ResourceLimits {
                cpu_millicores: None,
                memory_bytes: None,
                execution_time_ms: None,
                fuel: None,
            },
        };

        // Compile network permissions
        if let Some(network) = &policy.core.network {
            compiled.compile_network_permissions(network)?;
        }

        // Compile storage permissions
        if let Some(storage) = &policy.core.storage {
            compiled.compile_storage_permissions(storage)?;
        }

        // Compile environment permissions
        if let Some(env) = &policy.core.environment {
            for rule in &env.allow {
                compiled.env_whitelist.insert(rule.key.clone());
            }
            for rule in &env.deny {
                compiled.env_blacklist.insert(rule.key.clone());
            }
            if !env.allow.is_empty() {
                compiled.capabilities.can_read_environment = true;
            }
        }

        // Compile resource limits
        if let Some(resources) = &policy.core.resources {
            compiled.compile_resource_limits(&resources.limits)?;
        }

        let mut tool_pattern_builder = GlobSetBuilder::new();

        // Compile MCP extension permissions
        // The extensions field is flattened, so "extensions" becomes a key in the HashMap
        // We need to check for both nested and direct access patterns
        let mcp_value = policy
            .extensions
            .get("extensions")
            .and_then(|ext| ext.as_mapping())
            .and_then(|ext_map| ext_map.get("mcp"))
            .or_else(|| policy.extensions.get("mcp"));

        if let Some(mcp_value) = mcp_value {
            // Parse MCP extension
            if let Ok(mcp) =
                serde_yaml::from_value::<crate::extensions::mcp::McpPermissions>(mcp_value.clone())
            {
                // Compile tool permissions
                if let Some(tools) = &mcp.tools {
                    // Process allow rules
                    for rule in &tools.allow {
                        add_tool_allow_rule(&mut compiled, &mut tool_pattern_builder, &rule.name)?;
                    }

                    // Process deny rules
                    for rule in &tools.deny {
                        compiled.tool_blacklist.insert(rule.name.clone());
                    }
                }
            }
        }

        if let Some(tools_value) = policy.extensions.get("tools") {
            let simple_policy = parse_simple_tool_policy(tools_value)?;
            for name in &simple_policy.allow {
                add_tool_allow_rule(&mut compiled, &mut tool_pattern_builder, name)?;
            }
            for name in simple_policy.deny {
                compiled.tool_blacklist.insert(name);
            }
        }

        compiled.tool_patterns = tool_pattern_builder.build()?;

        // Set capability flags based on what's allowed
        compiled.update_capability_flags();

        Ok(compiled)
    }

    fn compile_network_permissions(
        &mut self,
        network: &crate::permissions::NetworkPermissions,
    ) -> Result<()> {
        let mut allow_builder = GlobSetBuilder::new();
        let mut deny_builder = GlobSetBuilder::new();

        for rule in &network.allow {
            match rule {
                NetworkRule::Host { host } => {
                    if host.contains('*') {
                        allow_builder.add(Glob::new(host).map_err(|e| {
                            crate::error::PolicyError::GlobError(format!(
                                "Invalid glob pattern '{}': {}",
                                host, e
                            ))
                        })?);
                    } else {
                        self.network_whitelist.insert(host.clone());
                        self.network_bloom.set(&host.clone());
                    }
                }
                NetworkRule::Cidr { cidr: _ } => {}
            }
        }

        for rule in &network.deny {
            match rule {
                NetworkRule::Host { host } => {
                    if host.contains('*') {
                        deny_builder.add(Glob::new(host)?);
                    } else {
                        self.network_blacklist.insert(host.clone());
                    }
                }
                NetworkRule::Cidr { cidr: _ } => {}
            }
        }

        self.network_patterns = allow_builder.build()?;

        let _deny_patterns = deny_builder.build()?;

        Ok(())
    }

    fn compile_storage_permissions(
        &mut self,
        storage: &crate::permissions::StoragePermissions,
    ) -> Result<()> {
        let mut allow_builder = GlobSetBuilder::new();
        let mut deny_builder = GlobSetBuilder::new();

        for rule in &storage.allow {
            self.compile_storage_rule(rule, &mut allow_builder, true)?;
        }

        for rule in &storage.deny {
            self.compile_storage_rule(rule, &mut deny_builder, false)?;
        }

        self.storage_allow_patterns = allow_builder.build()?;
        self.storage_deny_patterns = deny_builder.build()?;
        Ok(())
    }

    fn compile_storage_rule(
        &mut self,
        rule: &StorageRule,
        builder: &mut GlobSetBuilder,
        is_allow: bool,
    ) -> Result<()> {
        let pattern = if rule.uri.starts_with("fs://") {
            &rule.uri[5..]
        } else {
            &rule.uri
        };

        builder.add(Glob::new(pattern)?);

        if is_allow {
            let access_set: FxHashSet<String> = rule.access.iter().cloned().collect();
            self.storage_access_map
                .insert(pattern.to_string(), access_set);
        }

        self.resource_trie.insert(pattern, is_allow, !is_allow);

        Ok(())
    }

    fn compile_resource_limits(
        &mut self,
        limits: &crate::permissions::ResourceLimitValues,
    ) -> Result<()> {
        if let Some(cpu) = &limits.cpu {
            self.resource_limits.cpu_millicores = Some(parse_cpu_limit(cpu)?);
        }

        if let Some(memory) = &limits.memory {
            self.resource_limits.memory_bytes = Some(parse_memory_limit(memory)?);
        }

        if let Some(time) = &limits.execution_time {
            self.resource_limits.execution_time_ms = Some(parse_time_limit(time)?);
        }

        if let Some(fuel) = limits.fuel {
            self.resource_limits.fuel = Some(fuel);
        }

        Ok(())
    }

    fn update_capability_flags(&mut self) {
        self.capabilities.can_access_network = !self.network_whitelist.is_empty();
        self.capabilities.can_access_filesystem = !self.storage_access_map.is_empty();
        self.capabilities.can_read_environment = !self.env_whitelist.is_empty();
    }

    /// Check if a tool is allowed
    #[inline(always)]
    pub fn is_tool_allowed(&self, name: &str) -> bool {
        if self.tool_blacklist.contains(name) {
            return false;
        }

        if self.tool_whitelist.contains(name) {
            return true;
        }

        self.tool_patterns.is_match(name)
    }

    /// Check if network access is allowed
    #[inline(always)]
    pub fn is_network_allowed(&self, host: &str) -> bool {
        if self.network_blacklist.contains(host) {
            return false;
        }

        if self.network_whitelist.contains(host) {
            return true;
        }

        self.network_patterns.is_match(host)
    }

    /// Check if storage access is allowed
    #[inline(always)]
    pub fn is_storage_allowed(&self, path: &str, operation: &str) -> bool {
        let normalized_path = if let Some(stripped) = path.strip_prefix("fs://") {
            stripped
        } else {
            path
        };

        if self.storage_deny_patterns.is_match(normalized_path) {
            return false;
        }

        if self.storage_allow_patterns.is_match(normalized_path) {
            for (pattern, ops) in &self.storage_access_map {
                if glob_match(pattern, normalized_path) {
                    return ops.contains(operation);
                }
            }
        }

        false
    }

    /// Check if environment variable access is allowed
    #[inline(always)]
    pub fn is_env_allowed(&self, key: &str) -> bool {
        if self.env_blacklist.contains(key) {
            return false;
        }

        self.env_whitelist.contains(key)
    }
}

impl PathTrie {
    fn new() -> Self {
        PathTrie {
            root: TrieNode {
                children: FxHashMap::default(),
                is_allowed: false,
                is_denied: false,
            },
        }
    }

    fn insert(&mut self, path: &str, allow: bool, deny: bool) {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = &mut self.root;

        for segment in segments {
            current = current
                .children
                .entry(segment.to_string())
                .or_insert_with(|| TrieNode {
                    children: FxHashMap::default(),
                    is_allowed: false,
                    is_denied: false,
                });
        }

        if allow {
            current.is_allowed = true;
        }
        if deny {
            current.is_denied = true;
        }
    }

    /// Check if a path is allowed or denied
    pub fn check(&self, path: &str) -> Option<bool> {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = &self.root;

        for segment in segments {
            if let Some(node) = current.children.get(segment) {
                current = node;
            } else {
                return None;
            }
        }

        if current.is_denied {
            Some(false)
        } else if current.is_allowed {
            Some(true)
        } else {
            None
        }
    }
}

fn parse_cpu_limit(cpu: &str) -> Result<u64> {
    if let Some(stripped) = cpu.strip_suffix('m') {
        let value = stripped.parse::<u64>().map_err(|_| {
            crate::error::PolicyError::ParseError(format!("Invalid CPU limit: {}", cpu))
        })?;
        Ok(value)
    } else {
        let cores = cpu.parse::<f64>().map_err(|_| {
            crate::error::PolicyError::ParseError(format!("Invalid CPU limit: {}", cpu))
        })?;
        Ok((cores * 1000.0) as u64)
    }
}

fn parse_memory_limit(memory: &str) -> Result<u64> {
    let (value_str, unit) = if let Some(stripped) = memory.strip_suffix("Ki") {
        (stripped, 1024u64)
    } else if let Some(stripped) = memory.strip_suffix("Mi") {
        (stripped, 1024u64 * 1024)
    } else if let Some(stripped) = memory.strip_suffix("Gi") {
        (stripped, 1024u64 * 1024 * 1024)
    } else {
        return Err(crate::error::PolicyError::ParseError(format!(
            "Invalid memory limit: {}",
            memory
        )));
    };

    let value = value_str.parse::<u64>().map_err(|_| {
        crate::error::PolicyError::ParseError(format!("Invalid memory limit: {}", memory))
    })?;
    Ok(value * unit)
}

fn parse_time_limit(time: &str) -> Result<u64> {
    let (value_str, multiplier) = if let Some(stripped) = time.strip_suffix("ms") {
        (stripped, 1u64)
    } else if let Some(stripped) = time.strip_suffix('s') {
        (stripped, 1000u64)
    } else if let Some(stripped) = time.strip_suffix('m') {
        (stripped, 60000u64)
    } else {
        return Err(crate::error::PolicyError::ParseError(format!(
            "Invalid time limit: {}",
            time
        )));
    };

    let value = value_str.parse::<u64>().map_err(|_| {
        crate::error::PolicyError::ParseError(format!("Invalid time limit: {}", time))
    })?;
    Ok(value * multiplier)
}

fn glob_match(pattern: &str, text: &str) -> bool {
    Glob::new(pattern)
        .map(|glob| glob.compile_matcher().is_match(text))
        .unwrap_or(false)
}
