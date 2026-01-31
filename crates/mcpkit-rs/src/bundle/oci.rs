//! OCI registry client for bundle distribution

use std::collections::HashMap;

use mcpkit_rs_config::RegistryAuth;
use reqwest::{StatusCode, header::WWW_AUTHENTICATE};
use serde::{Deserialize, Serialize};

use super::{Bundle, BundleError, compute_digest, parse_oci_uri, verify_digest};

/// OCI-specific errors
#[derive(Debug, thiserror::Error)]
pub enum OciError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Registry returned error: {status} - {message}")]
    RegistryError { status: u16, message: String },

    #[error("Invalid manifest format: {0}")]
    InvalidManifest(String),

    #[error("Layer not found: {0}")]
    LayerNotFound(String),

    #[error(
        "Authentication required. Set GITHUB_USER and GITHUB_TOKEN environment variables or use --auth flag"
    )]
    AuthenticationRequired,

    #[error("Unsupported media type: {0}")]
    UnsupportedMediaType(String),
}

/// OCI manifest structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OciManifest {
    pub schema_version: u32,
    pub media_type: String,
    pub config: OciDescriptor,
    pub layers: Vec<OciDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

/// OCI descriptor for layers and config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OciDescriptor {
    pub media_type: String,
    pub digest: String,
    pub size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

/// OCI config structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciConfig {
    pub architecture: String,
    pub os: String,
    #[serde(rename = "rootfs")]
    pub root_fs: RootFs,
    pub history: Vec<History>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootFs {
    #[serde(rename = "type")]
    pub fs_type: String,
    pub diff_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    pub created: String,
    pub created_by: String,
}

/// Media types for mcpkit bundles
pub const MEDIA_TYPE_WASM: &str = "application/vnd.mcpkit.wasm.module.v1";
pub const MEDIA_TYPE_CONFIG_YAML: &str = "application/vnd.mcpkit.config.v1+yaml";
pub const MEDIA_TYPE_OCI_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
pub const MEDIA_TYPE_OCI_CONFIG: &str = "application/vnd.oci.image.config.v1+json";

/// OCI Registry URL builder
#[derive(Debug, Clone)]
struct RegistryUrl {
    registry: String,
    repository: String,
    scheme: String,
}

impl RegistryUrl {
    fn new(registry: &str, repository: &str) -> Self {
        let scheme = if Self::is_insecure_registry(registry) {
            "http"
        } else {
            "https"
        };

        Self {
            registry: registry.to_string(),
            repository: repository.to_string(),
            scheme: scheme.to_string(),
        }
    }

    fn is_insecure_registry(registry: &str) -> bool {
        if registry.starts_with("localhost")
            || registry.starts_with("127.")
            || registry.starts_with("0.0.0.0")
            || registry.starts_with("[::1]")
        {
            return true;
        }

        if let Ok(list) = std::env::var("MCPKIT_INSECURE_REGISTRIES") {
            for item in list.split(',') {
                let item = item.trim();
                if item.is_empty() {
                    continue;
                }

                if registry.eq_ignore_ascii_case(item) {
                    return true;
                }

                let item_host = item.split(':').next().unwrap_or(item);
                let registry_host = registry.split(':').next().unwrap_or(registry);
                if !item_host.is_empty() && registry_host.eq_ignore_ascii_case(item_host) {
                    return true;
                }
            }
        }

        false
    }

    fn base_url(&self) -> String {
        format!("{}://{}", self.scheme, self.registry)
    }

    fn blob_url(&self, digest: &str) -> String {
        format!(
            "{}/v2/{}/blobs/{}",
            self.base_url(),
            self.repository,
            digest
        )
    }

    fn upload_initiation_url(&self) -> String {
        format!("{}/v2/{}/blobs/uploads/", self.base_url(), self.repository)
    }

    fn manifest_url(&self, tag: &str) -> String {
        format!(
            "{}/v2/{}/manifests/{}",
            self.base_url(),
            self.repository,
            tag
        )
    }

    fn scope(&self, actions: &str) -> String {
        format!("repository:{}:{}", self.repository, actions)
    }
}

/// Bundle client for OCI operations
pub struct BundleClient {
    http_client: reqwest::Client,
    cache: Option<super::cache::BundleCache>,
}

#[derive(Debug, Default)]
struct AuthContext {
    basic: Option<(String, String)>,
    bearer: Option<String>,
}

impl AuthContext {
    fn anonymous() -> Self {
        Self {
            basic: None,
            bearer: None,
        }
    }

    fn with_basic(username: String, password: String) -> Self {
        Self {
            basic: Some((username, password)),
            bearer: None,
        }
    }

    fn basic_credentials(&self) -> Option<(&str, &str)> {
        self.basic
            .as_ref()
            .map(|(username, password)| (username.as_str(), password.as_str()))
    }

    fn bearer_token(&self) -> Option<&str> {
        self.bearer.as_deref()
    }

    fn can_fetch_token(&self) -> bool {
        self.basic.is_some()
    }

    async fn fetch_bearer_token(
        &mut self,
        client: &reqwest::Client,
        challenge: &BearerChallenge,
        scope: Option<&str>,
    ) -> Result<(), OciError> {
        #[derive(Deserialize)]
        struct TokenResponse {
            token: Option<String>,
            access_token: Option<String>,
        }

        let mut url =
            reqwest::Url::parse(&challenge.realm).map_err(|_| OciError::AuthenticationRequired)?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(service) = challenge.service.as_deref() {
                pairs.append_pair("service", service);
            }
            if let Some(scope) = scope.or(challenge.scope.as_deref()) {
                if !scope.is_empty() {
                    pairs.append_pair("scope", scope);
                }
            }
        }

        let mut request = client.get(url);
        if let Some((username, password)) = self.basic_credentials() {
            request = request.basic_auth(username, Some(password));
        }

        let response = request.send().await.map_err(OciError::from)?;
        if !response.status().is_success() {
            return Err(OciError::RegistryError {
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let token_response: TokenResponse = response.json().await.map_err(OciError::from)?;
        let token = token_response
            .token
            .or(token_response.access_token)
            .ok_or(OciError::AuthenticationRequired)?;
        self.bearer = Some(token);
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct BearerChallenge {
    realm: String,
    service: Option<String>,
    scope: Option<String>,
}

impl BearerChallenge {
    fn parse(header: &str) -> Option<Self> {
        let header = header.trim();
        let (scheme, params) = header.split_once(' ')?;
        if !scheme.eq_ignore_ascii_case("bearer") {
            return None;
        }

        let mut realm = None;
        let mut service = None;
        let mut scope = None;

        for part in params.split(',') {
            let mut kv = part.trim().splitn(2, '=');
            let key = kv.next()?.trim();
            let value = kv.next().map(|v| v.trim().trim_matches('"')).unwrap_or("");
            match key {
                "realm" => realm = Some(value.to_string()),
                "service" => service = Some(value.to_string()),
                "scope" => scope = Some(value.to_string()),
                _ => {}
            }
        }

        realm.map(|realm| Self {
            realm,
            service,
            scope,
        })
    }
}

impl Default for BundleClient {
    fn default() -> Self {
        Self::new()
    }
}

impl BundleClient {
    /// Create a new bundle client
    pub fn new() -> Self {
        let http_client = reqwest::Client::builder()
            .user_agent("mcpkit-rs/0.14.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            http_client,
            cache: None,
        }
    }

    /// Create a new bundle client with cache
    pub fn with_cache(cache: super::cache::BundleCache) -> Self {
        let http_client = reqwest::Client::builder()
            .user_agent("mcpkit-rs/0.14.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            http_client,
            cache: Some(cache),
        }
    }

    fn build_auth_context(&self, auth: Option<&RegistryAuth>) -> Result<AuthContext, BundleError> {
        if let Some(auth) = auth {
            if let (Some(username), Some(password)) = (&auth.username, &auth.password) {
                let username = self.expand_env_var(username)?;
                let password = self.expand_env_var(password)?;
                Ok(AuthContext::with_basic(username, password))
            } else {
                Ok(AuthContext::anonymous())
            }
        } else {
            Ok(AuthContext::anonymous())
        }
    }

    async fn send_with_auth<F>(
        &self,
        mut build_request: F,
        auth: &mut AuthContext,
        scope: Option<&str>,
    ) -> Result<reqwest::Response, OciError>
    where
        F: FnMut() -> reqwest::RequestBuilder,
    {
        let mut attempts = 0;

        loop {
            attempts += 1;

            let mut request = build_request();
            if let Some(token) = auth.bearer_token() {
                request = request.bearer_auth(token);
            } else if let Some((username, password)) = auth.basic_credentials() {
                request = request.basic_auth(username, Some(password));
            }

            let response = request.send().await.map_err(OciError::from)?;

            if response.status() != StatusCode::UNAUTHORIZED {
                return Ok(response);
            }

            if !auth.can_fetch_token() || attempts >= 3 {
                return Err(OciError::AuthenticationRequired);
            }

            let header_value = response
                .headers()
                .get(WWW_AUTHENTICATE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let challenge = match header_value.and_then(|header| BearerChallenge::parse(&header)) {
                Some(challenge) => challenge,
                None => return Err(OciError::AuthenticationRequired),
            };

            let scope_override = scope.map(|s| s.to_string());
            auth.fetch_bearer_token(&self.http_client, &challenge, scope_override.as_deref())
                .await?;
        }
    }

    /// Push a bundle to OCI registry
    pub async fn push(
        &self,
        wasm: &[u8],
        config_yaml: &[u8],
        uri: &str,
        auth: Option<&RegistryAuth>,
    ) -> Result<String, BundleError> {
        let (registry, repository, tag) = parse_oci_uri(uri)?;
        let tag = tag.unwrap_or_else(|| "latest".to_string());

        let mut auth_ctx = self.build_auth_context(auth)?;

        // Create OCI config
        let oci_config = self.create_oci_config();
        let config_json =
            serde_json::to_vec(&oci_config).map_err(|e| BundleError::ConfigError(e.to_string()))?;
        let config_digest = compute_digest(&config_json);

        // Upload config blob
        self.upload_blob(
            &registry,
            &repository,
            &config_json,
            &config_digest,
            &mut auth_ctx,
        )
        .await?;

        // Upload WASM layer
        let wasm_digest = compute_digest(wasm);
        self.upload_blob(&registry, &repository, wasm, &wasm_digest, &mut auth_ctx)
            .await?;

        // Upload config.yaml layer
        let config_yaml_digest = compute_digest(config_yaml);
        self.upload_blob(
            &registry,
            &repository,
            config_yaml,
            &config_yaml_digest,
            &mut auth_ctx,
        )
        .await?;

        // Create and upload manifest
        let manifest = OciManifest {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_MANIFEST.to_string(),
            config: OciDescriptor {
                media_type: MEDIA_TYPE_OCI_CONFIG.to_string(),
                digest: config_digest.clone(),
                size: config_json.len() as i64,
                annotations: None,
            },
            layers: vec![
                OciDescriptor {
                    media_type: MEDIA_TYPE_WASM.to_string(),
                    digest: wasm_digest,
                    size: wasm.len() as i64,
                    annotations: None,
                },
                OciDescriptor {
                    media_type: MEDIA_TYPE_CONFIG_YAML.to_string(),
                    digest: config_yaml_digest,
                    size: config_yaml.len() as i64,
                    annotations: None,
                },
            ],
            annotations: Some(HashMap::from([(
                "org.mcpkit.bundle.version".to_string(),
                tag.clone(),
            )])),
        };

        let manifest_json =
            serde_json::to_vec(&manifest).map_err(|e| BundleError::ConfigError(e.to_string()))?;
        let manifest_digest = compute_digest(&manifest_json);

        self.upload_manifest(&registry, &repository, &tag, &manifest_json, &mut auth_ctx)
            .await?;

        Ok(manifest_digest)
    }

    /// Pull a bundle from OCI registry
    pub async fn pull(
        &self,
        uri: &str,
        auth: Option<&RegistryAuth>,
    ) -> Result<Bundle, BundleError> {
        let (registry, repository, tag) = parse_oci_uri(uri)?;
        let tag = tag.unwrap_or_else(|| "latest".to_string());

        // Check cache first
        if let Some(cache) = &self.cache {
            if let Ok(bundle) = cache.get(uri) {
                // Verify integrity
                if bundle.verify().is_ok() {
                    return Ok(bundle);
                }
            }
        }

        let mut auth_ctx = self.build_auth_context(auth)?;

        // Pull manifest
        let manifest = self
            .pull_manifest(&registry, &repository, &tag, &mut auth_ctx)
            .await?;

        // Find WASM and config layers
        let wasm_layer = manifest
            .layers
            .iter()
            .find(|l| l.media_type == MEDIA_TYPE_WASM)
            .ok_or_else(|| OciError::LayerNotFound("WASM module layer".to_string()))?;

        let config_layer = manifest
            .layers
            .iter()
            .find(|l| l.media_type == MEDIA_TYPE_CONFIG_YAML)
            .ok_or_else(|| OciError::LayerNotFound("Config YAML layer".to_string()))?;

        // Pull layers
        let wasm = self
            .pull_blob(&registry, &repository, &wasm_layer.digest, &mut auth_ctx)
            .await?;
        let config_yaml = self
            .pull_blob(&registry, &repository, &config_layer.digest, &mut auth_ctx)
            .await?;

        // Verify digests
        verify_digest(&wasm, &wasm_layer.digest)?;
        verify_digest(&config_yaml, &config_layer.digest)?;

        // Create bundle
        let bundle = Bundle::new(
            wasm,
            config_yaml,
            format!("{}/{}", registry, repository),
            tag.clone(),
        );

        // Cache if available
        if let Some(cache) = &self.cache {
            cache.put(uri, &bundle)?;
        }

        Ok(bundle)
    }

    /// Expand environment variable in string
    fn expand_env_var(&self, value: &str) -> Result<String, BundleError> {
        if value.starts_with("${") && value.ends_with('}') {
            let var_name = &value[2..value.len() - 1];
            std::env::var(var_name).map_err(|_| {
                BundleError::ConfigError(format!(
                    "Required environment variable '{}' is not set. Please set it before running this command.",
                    var_name
                ))
            })
        } else {
            Ok(value.to_string())
        }
    }

    /// Create OCI config for WASM modules
    fn create_oci_config(&self) -> OciConfig {
        OciConfig {
            architecture: "wasm".to_string(),
            os: "wasi".to_string(),
            root_fs: RootFs {
                fs_type: "layers".to_string(),
                diff_ids: vec![],
            },
            history: vec![History {
                created: chrono::Utc::now().to_rfc3339(),
                created_by: "mcpkit-rs".to_string(),
            }],
        }
    }

    /// Upload a blob to registry
    async fn upload_blob(
        &self,
        registry: &str,
        repository: &str,
        content: &[u8],
        digest: &str,
        auth: &mut AuthContext,
    ) -> Result<(), OciError> {
        let reg_url = RegistryUrl::new(registry, repository);
        let url = reg_url.blob_url(digest);
        let scope = reg_url.scope("push,pull");

        let head_response = self
            .send_with_auth(|| self.http_client.head(&url), auth, Some(&scope))
            .await?;
        let head_status = head_response.status();

        if head_status.is_success() {
            return Ok(());
        }

        let upload_url = reg_url.upload_initiation_url();
        let upload_response = self
            .send_with_auth(
                || {
                    self.http_client
                        .post(&upload_url)
                        .header("Content-Length", "0")
                        .header("Accept", MEDIA_TYPE_OCI_MANIFEST)
                },
                auth,
                Some(&scope),
            )
            .await?;

        if !upload_response.status().is_success() {
            let status = upload_response.status();
            let error_body = upload_response.text().await.unwrap_or_default();

            if status == StatusCode::UNAUTHORIZED {
                return Err(OciError::AuthenticationRequired);
            }
            if status == StatusCode::FORBIDDEN {
                return Err(OciError::RegistryError {
                    status: 403,
                    message:
                        "Permission denied. Check your access token has 'write:packages' scope."
                            .to_string(),
                });
            }
            if status == StatusCode::METHOD_NOT_ALLOWED {
                return Err(OciError::RegistryError {
                    status: 405,
                    message: format!(
                        "Method not allowed. Registry may not support this operation. URL: {}, Error: {}",
                        upload_url, error_body
                    ),
                });
            }
            return Err(OciError::RegistryError {
                status: status.as_u16(),
                message: error_body,
            });
        }

        let location = upload_response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| OciError::InvalidManifest("Missing upload location".to_string()))?;

        let (mut put_url, has_query) = if location.starts_with("http") {
            (location.to_string(), location.contains('?'))
        } else {
            let relative = if location.starts_with('/') {
                location.to_string()
            } else {
                format!("/{}", location)
            };
            (
                format!("{}{}", reg_url.base_url(), relative),
                relative.contains('?'),
            )
        };

        let separator = if has_query { '&' } else { '?' };
        put_url.push_str(&format!("{}digest={}", separator, digest));

        let content_len = content.len().to_string();
        let put_response = self
            .send_with_auth(
                || {
                    self.http_client
                        .put(&put_url)
                        .header("Content-Type", "application/octet-stream")
                        .header("Content-Length", content_len.clone())
                        .body(content.to_vec())
                },
                auth,
                Some(&scope),
            )
            .await?;

        if !put_response.status().is_success() {
            return Err(OciError::RegistryError {
                status: put_response.status().as_u16(),
                message: put_response.text().await.unwrap_or_default(),
            });
        }

        Ok(())
    }

    /// Upload manifest to registry
    async fn upload_manifest(
        &self,
        registry: &str,
        repository: &str,
        tag: &str,
        manifest: &[u8],
        auth: &mut AuthContext,
    ) -> Result<(), OciError> {
        let reg_url = RegistryUrl::new(registry, repository);
        let url = reg_url.manifest_url(tag);
        let scope = reg_url.scope("push,pull");
        let manifest_len = manifest.len().to_string();
        let response = self
            .send_with_auth(
                || {
                    self.http_client
                        .put(&url)
                        .header("Content-Type", MEDIA_TYPE_OCI_MANIFEST)
                        .header("Content-Length", manifest_len.clone())
                        .body(manifest.to_vec())
                },
                auth,
                Some(&scope),
            )
            .await?;

        if !response.status().is_success() {
            return Err(OciError::RegistryError {
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        Ok(())
    }

    /// Pull manifest from registry
    async fn pull_manifest(
        &self,
        registry: &str,
        repository: &str,
        tag: &str,
        auth: &mut AuthContext,
    ) -> Result<OciManifest, OciError> {
        let reg_url = RegistryUrl::new(registry, repository);
        let url = reg_url.manifest_url(tag);
        let scope = reg_url.scope("pull");
        let response = self
            .send_with_auth(
                || {
                    self.http_client
                        .get(&url)
                        .header("Accept", MEDIA_TYPE_OCI_MANIFEST)
                },
                auth,
                Some(&scope),
            )
            .await?;

        if !response.status().is_success() {
            if response.status() == StatusCode::UNAUTHORIZED {
                return Err(OciError::AuthenticationRequired);
            }
            return Err(OciError::RegistryError {
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let manifest: OciManifest = response
            .json()
            .await
            .map_err(|e| OciError::InvalidManifest(e.to_string()))?;

        Ok(manifest)
    }

    /// Pull blob from registry
    async fn pull_blob(
        &self,
        registry: &str,
        repository: &str,
        digest: &str,
        auth: &mut AuthContext,
    ) -> Result<Vec<u8>, OciError> {
        let reg_url = RegistryUrl::new(registry, repository);
        let url = reg_url.blob_url(digest);
        let scope = reg_url.scope("pull");
        let response = self
            .send_with_auth(|| self.http_client.get(&url), auth, Some(&scope))
            .await?;

        if !response.status().is_success() {
            if response.status() == StatusCode::UNAUTHORIZED {
                return Err(OciError::AuthenticationRequired);
            }
            return Err(OciError::RegistryError {
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let content = response.bytes().await?.to_vec();
        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_types() {
        assert_eq!(MEDIA_TYPE_WASM, "application/vnd.mcpkit.wasm.module.v1");
        assert_eq!(
            MEDIA_TYPE_CONFIG_YAML,
            "application/vnd.mcpkit.config.v1+yaml"
        );
    }

    #[test]
    fn test_oci_config_creation() {
        let client = BundleClient::new();
        let config = client.create_oci_config();

        assert_eq!(config.architecture, "wasm");
        assert_eq!(config.os, "wasi");
        assert_eq!(config.root_fs.fs_type, "layers");
        assert_eq!(config.history.len(), 1);
        assert_eq!(config.history[0].created_by, "mcpkit-rs");
    }

    #[test]
    fn test_env_var_expansion() {
        let client = BundleClient::new();

        unsafe {
            std::env::set_var("TEST_VAR", "test_value");
        }

        assert_eq!(client.expand_env_var("${TEST_VAR}").unwrap(), "test_value");
        assert_eq!(client.expand_env_var("plain_text").unwrap(), "plain_text");

        // Test missing env var produces clear error
        let result = client.expand_env_var("${NONEXISTENT}");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("NONEXISTENT"));
        assert!(err_msg.contains("not set"));

        unsafe {
            std::env::remove_var("TEST_VAR");
        }
    }
}
