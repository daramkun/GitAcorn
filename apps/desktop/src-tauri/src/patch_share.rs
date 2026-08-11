use std::time::Duration;

use app_core::AppError;
use reqwest::{Client, RequestBuilder, StatusCode, Url};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

const MAX_PATCH_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = MAX_PATCH_BYTES + 256 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSharePublishRequest {
    pub endpoint: String,
    pub token: Option<String>,
    pub title: String,
    pub description: String,
    pub repository: String,
    pub base_revision: String,
    pub patch: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchShareFetchRequest {
    pub endpoint: String,
    pub token: Option<String>,
    pub patch_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchShareDeleteRequest {
    pub endpoint: String,
    pub token: Option<String>,
    pub patch_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchShareReceipt {
    pub schema_version: u16,
    pub patch_id: String,
    pub web_url: Option<String>,
    pub sha256: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedPatch {
    pub schema_version: u16,
    pub patch_id: String,
    pub title: String,
    pub description: String,
    pub repository: String,
    pub base_revision: String,
    pub patch: String,
    pub sha256: String,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
    pub web_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishPayload<'a> {
    schema_version: u16,
    title: &'a str,
    description: &'a str,
    repository: &'a str,
    base_revision: &'a str,
    patch: &'a str,
    sha256: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishResponse {
    schema_version: u16,
    patch_id: String,
    web_url: Option<String>,
    sha256: String,
    expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedPatchResponse {
    schema_version: u16,
    patch_id: String,
    title: String,
    #[serde(default)]
    description: String,
    repository: String,
    base_revision: String,
    patch: String,
    sha256: String,
    created_at: Option<String>,
    expires_at: Option<String>,
    web_url: Option<String>,
}

#[derive(Clone)]
pub struct PatchShareService {
    client: Client,
}

impl Default for PatchShareService {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .user_agent("GitAcorn/0.1")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("patch share HTTP client configuration is valid"),
        }
    }
}

impl PatchShareService {
    pub async fn publish(
        &self,
        request: &PatchSharePublishRequest,
    ) -> Result<PatchShareReceipt, AppError> {
        validate_text(&request.title, "title", 200, false)?;
        validate_text(&request.description, "description", 4_000, true)?;
        validate_text(&request.repository, "repository", 512, false)?;
        validate_text(&request.base_revision, "base revision", 1_024, false)?;
        validate_patch(&request.patch)?;
        let endpoint = endpoint_url(&request.endpoint)?;
        let url = endpoint
            .join("v1/patches")
            .map_err(|_| invalid_endpoint())?;
        let checksum = patch_checksum(request.patch.as_bytes());
        let payload = PublishPayload {
            schema_version: 1,
            title: request.title.trim(),
            description: request.description.trim(),
            repository: request.repository.trim(),
            base_revision: request.base_revision.trim(),
            patch: &request.patch,
            sha256: &checksum,
        };
        let response: PublishResponse = checked_json(
            authenticate(
                self.client.post(url).json(&payload),
                request.token.as_deref(),
            )?
            .send()
            .await
            .map_err(map_network_error)?,
        )
        .await?;
        if response.schema_version != 1 {
            return Err(AppError::InvalidGitOutput(
                "Patch share response uses an unsupported schema".to_owned(),
            ));
        }
        validate_patch_id(&response.patch_id)?;
        validate_optional_web_url(response.web_url.as_deref())?;
        if response.sha256 != checksum {
            return Err(AppError::InvalidGitOutput(
                "Patch share service returned a different checksum".to_owned(),
            ));
        }
        Ok(PatchShareReceipt {
            schema_version: 1,
            patch_id: response.patch_id,
            web_url: response.web_url,
            sha256: checksum,
            expires_at: response.expires_at,
        })
    }

    pub async fn fetch(&self, request: &PatchShareFetchRequest) -> Result<SharedPatch, AppError> {
        validate_patch_id(&request.patch_id)?;
        let endpoint = endpoint_url(&request.endpoint)?;
        let url = endpoint
            .join(&format!("v1/patches/{}", request.patch_id))
            .map_err(|_| invalid_endpoint())?;
        let response: SharedPatchResponse = checked_json(
            authenticate(self.client.get(url), request.token.as_deref())?
                .send()
                .await
                .map_err(map_network_error)?,
        )
        .await?;
        normalize_shared_patch(response)
    }

    pub async fn delete(&self, request: &PatchShareDeleteRequest) -> Result<(), AppError> {
        validate_patch_id(&request.patch_id)?;
        let endpoint = endpoint_url(&request.endpoint)?;
        let url = endpoint
            .join(&format!("v1/patches/{}", request.patch_id))
            .map_err(|_| invalid_endpoint())?;
        let response = authenticate(self.client.delete(url), request.token.as_deref())?
            .send()
            .await
            .map_err(map_network_error)?;
        checked_status(response).await.map(|_| ())
    }
}

fn normalize_shared_patch(response: SharedPatchResponse) -> Result<SharedPatch, AppError> {
    if response.schema_version != 1 {
        return Err(AppError::InvalidGitOutput(
            "Shared patch uses an unsupported schema".to_owned(),
        ));
    }
    validate_patch_id(&response.patch_id)?;
    validate_text(&response.title, "title", 200, false)?;
    validate_text(&response.description, "description", 4_000, true)?;
    validate_text(&response.repository, "repository", 512, false)?;
    validate_text(&response.base_revision, "base revision", 1_024, false)?;
    validate_patch(&response.patch)?;
    validate_optional_web_url(response.web_url.as_deref())?;
    let checksum = patch_checksum(response.patch.as_bytes());
    if checksum != response.sha256 {
        return Err(AppError::InvalidGitOutput(
            "Shared patch checksum does not match its content".to_owned(),
        ));
    }
    Ok(SharedPatch {
        schema_version: 1,
        patch_id: response.patch_id,
        title: response.title,
        description: response.description,
        repository: response.repository,
        base_revision: response.base_revision,
        patch: response.patch,
        sha256: checksum,
        created_at: response.created_at,
        expires_at: response.expires_at,
        web_url: response.web_url,
    })
}

fn endpoint_url(value: &str) -> Result<Url, AppError> {
    let mut url = Url::parse(value.trim()).map_err(|_| invalid_endpoint())?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if (url.scheme() != "https" && !(url.scheme() == "http" && local))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid_endpoint());
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

fn validate_optional_web_url(value: Option<&str>) -> Result<(), AppError> {
    let Some(value) = value else {
        return Ok(());
    };
    endpoint_url(value).map(|_| ())
}

fn validate_patch_id(value: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::InvalidRequest(
            "Patch share identifier is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_text(
    value: &str,
    label: &str,
    max_len: usize,
    allow_empty: bool,
) -> Result<(), AppError> {
    let value = value.trim();
    if (!allow_empty && value.is_empty()) || value.len() > max_len || value.contains('\0') {
        return Err(AppError::InvalidRequest(format!(
            "Patch share {label} is invalid"
        )));
    }
    Ok(())
}

fn validate_patch(value: &str) -> Result<(), AppError> {
    if value.is_empty() || value.len() > MAX_PATCH_BYTES || value.contains('\0') {
        return Err(AppError::InvalidRequest(
            "Shared patch is empty or exceeds the 8 MiB limit".to_owned(),
        ));
    }
    Ok(())
}

fn authenticate(builder: RequestBuilder, token: Option<&str>) -> Result<RequestBuilder, AppError> {
    let Some(token) = token.map(str::trim).filter(|token| !token.is_empty()) else {
        return Ok(builder);
    };
    if token.len() > 16 * 1024 || token.contains(['\r', '\n', '\0']) {
        return Err(AppError::InvalidRequest(
            "Patch share token contains unsupported characters".to_owned(),
        ));
    }
    Ok(builder.bearer_auth(token))
}

async fn checked_json<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, AppError> {
    let bytes = checked_status(response).await?;
    serde_json::from_slice(&bytes)
        .map_err(|_| AppError::InvalidGitOutput("Patch share returned invalid JSON".to_owned()))
}

async fn checked_status(response: reqwest::Response) -> Result<Vec<u8>, AppError> {
    let status = response.status();
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        return Err(AppError::AuthenticationFailed);
    }
    if !status.is_success() {
        return Err(AppError::InvalidRequest(format!(
            "Patch share request failed with HTTP {}",
            status.as_u16()
        )));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(AppError::InvalidGitOutput(
            "Patch share response is too large".to_owned(),
        ));
    }
    let bytes = response.bytes().await.map_err(map_network_error)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(AppError::InvalidGitOutput(
            "Patch share response is too large".to_owned(),
        ));
    }
    Ok(bytes.to_vec())
}

fn patch_checksum(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn invalid_endpoint() -> AppError {
    AppError::InvalidRequest(
        "Patch share endpoint must use HTTPS (HTTP is allowed only for localhost) and contain no credentials, query, or fragment"
            .to_owned(),
    )
}

fn map_network_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() || error.is_connect() {
        AppError::Offline
    } else {
        AppError::InvalidRequest(format!("Patch share request failed: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SharedPatchResponse, endpoint_url, normalize_shared_patch, patch_checksum,
        validate_patch_id,
    };

    #[test]
    fn accepts_https_and_local_http_endpoints_without_url_credentials() {
        assert_eq!(
            endpoint_url("https://patch.example/api").unwrap().as_str(),
            "https://patch.example/api/"
        );
        assert!(endpoint_url("http://localhost:8080/").is_ok());
        assert!(endpoint_url("http://patch.example/").is_err());
        assert!(endpoint_url("https://token@patch.example/").is_err());
        assert!(endpoint_url("https://patch.example/?token=secret").is_err());
    }

    #[test]
    fn validates_identifiers_and_shared_patch_integrity() {
        assert!(validate_patch_id("patch_01-demo").is_ok());
        assert!(validate_patch_id("../patch").is_err());
        let patch =
            "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n";
        let response = SharedPatchResponse {
            schema_version: 1,
            patch_id: "patch-1".to_owned(),
            title: "Update a.txt".to_owned(),
            description: String::new(),
            repository: "team/demo".to_owned(),
            base_revision: "main".to_owned(),
            patch: patch.to_owned(),
            sha256: patch_checksum(patch.as_bytes()),
            created_at: None,
            expires_at: None,
            web_url: Some("https://patch.example/p/patch-1".to_owned()),
        };
        assert!(normalize_shared_patch(response).is_ok());
    }

    #[test]
    fn rejects_tampered_shared_patch_content() {
        let response = SharedPatchResponse {
            schema_version: 1,
            patch_id: "patch-1".to_owned(),
            title: "Tampered".to_owned(),
            description: String::new(),
            repository: "team/demo".to_owned(),
            base_revision: "main".to_owned(),
            patch: "diff --git a/a b/a\n".to_owned(),
            sha256: "0".repeat(64),
            created_at: None,
            expires_at: None,
            web_url: None,
        };
        assert!(normalize_shared_patch(response).is_err());
    }
}
