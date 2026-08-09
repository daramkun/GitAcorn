use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use app_core::{
    AppError, ForgeAccount, ForgeEndpointPlan, ForgeProfile, ForgeProvider, ForgeRepository,
    parse_forge_profile, parse_forge_repositories,
};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::Deserialize;
use uuid::Uuid;

const USER_AGENT: &str = "GitAcorn/0.1";
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeConnectRequest {
    pub provider: ForgeProvider,
    pub host: String,
    pub auth_username: String,
    pub token: String,
    pub scope: Option<String>,
}

#[derive(Clone)]
pub struct ForgeService {
    client: Client,
}

impl Default for ForgeService {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .timeout(Duration::from_secs(30))
                .build()
                .expect("forge HTTP client configuration is valid"),
        }
    }
}

impl ForgeService {
    pub async fn connect(
        &self,
        request: &ForgeConnectRequest,
    ) -> Result<(ForgeAccount, String), AppError> {
        validate_secret(&request.token)?;
        let plan =
            ForgeEndpointPlan::new(request.provider, &request.host, request.scope.as_deref())?;
        let auth_username = authentication_username(request)?;
        let profile = self.profile(&plan, &auth_username, &request.token).await?;
        let account = ForgeAccount {
            id: account_id(&plan, &profile.login),
            provider: plan.provider,
            host: plan.account_host.clone(),
            login: profile.login,
            display_name: profile.display_name,
            scope: plan.scope.clone(),
            avatar_url: profile.avatar_url,
        };
        credential_approve(&plan.account_host, &auth_username, &request.token)?;
        Ok((account, auth_username))
    }

    pub async fn repositories(
        &self,
        account: &ForgeAccount,
        auth_username: &str,
    ) -> Result<Vec<ForgeRepository>, AppError> {
        let plan =
            ForgeEndpointPlan::new(account.provider, &account.host, account.scope.as_deref())?;
        let token = credential_fill(&plan.account_host, auth_username)?;
        let response = self
            .authenticated_get(
                plan.provider,
                &plan.repositories_url(),
                auth_username,
                &token,
            )
            .send()
            .await
            .map_err(map_network_error)?;
        let bytes = checked_body(response).await?;
        parse_forge_repositories(plan.provider, &bytes)
    }

    pub fn forget(&self, account: &ForgeAccount, auth_username: &str) -> Result<(), AppError> {
        credential_reject(&account.host, auth_username)
    }

    async fn profile(
        &self,
        plan: &ForgeEndpointPlan,
        auth_username: &str,
        token: &str,
    ) -> Result<ForgeProfile, AppError> {
        let response = self
            .authenticated_get(plan.provider, &plan.profile_url(), auth_username, token)
            .send()
            .await
            .map_err(map_network_error)?;
        let bytes = checked_body(response).await?;
        if plan.provider == ForgeProvider::AzureDevOps {
            let scope = plan.scope.clone().expect("Azure scope is validated");
            return Ok(ForgeProfile {
                login: auth_username.to_owned(),
                display_name: scope,
                avatar_url: None,
            });
        }
        parse_forge_profile(plan.provider, &bytes)
    }

    fn authenticated_get(
        &self,
        provider: ForgeProvider,
        url: &str,
        auth_username: &str,
        token: &str,
    ) -> RequestBuilder {
        let request = self.client.get(url).header("Accept", "application/json");
        match provider {
            ForgeProvider::GitHub => request
                .bearer_auth(token)
                .header("X-GitHub-Api-Version", "2022-11-28"),
            ForgeProvider::GitLab => request.header("PRIVATE-TOKEN", token),
            ForgeProvider::Bitbucket | ForgeProvider::AzureDevOps => {
                request.basic_auth(auth_username, Some(token))
            }
        }
    }
}

fn validate_secret(token: &str) -> Result<(), AppError> {
    if token.is_empty() || token.len() > 16 * 1024 || token.contains(['\r', '\n', '\0']) {
        return Err(AppError::InvalidRequest(
            "Access token is empty or contains unsupported characters".to_owned(),
        ));
    }
    Ok(())
}

fn validate_credential_field(value: &str, label: &str) -> Result<(), AppError> {
    if value.is_empty() || value.len() > 512 || value.contains(['\r', '\n', '\0']) {
        return Err(AppError::InvalidRequest(format!(
            "{label} is empty or contains unsupported characters"
        )));
    }
    Ok(())
}

fn authentication_username(request: &ForgeConnectRequest) -> Result<String, AppError> {
    let username = request.auth_username.trim();
    if matches!(
        request.provider,
        ForgeProvider::Bitbucket | ForgeProvider::AzureDevOps
    ) && username.is_empty()
    {
        return Err(AppError::InvalidRequest(
            "This provider requires an authentication username".to_owned(),
        ));
    }
    let username = if username.is_empty() {
        "oauth2"
    } else {
        username
    };
    validate_credential_field(username, "Authentication username")?;
    Ok(username.to_owned())
}

fn account_id(plan: &ForgeEndpointPlan, login: &str) -> String {
    let key = format!(
        "{}\n{}\n{}\n{}",
        plan.provider.label(),
        plan.account_host.to_ascii_lowercase(),
        plan.scope
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase(),
        login.to_ascii_lowercase()
    );
    Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes()).to_string()
}

async fn checked_body(response: reqwest::Response) -> Result<Vec<u8>, AppError> {
    let status = response.status();
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        return Err(AppError::AuthenticationFailed);
    }
    if !status.is_success() {
        return Err(AppError::InvalidRequest(format!(
            "Forge request failed with HTTP {}",
            status.as_u16()
        )));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(AppError::InvalidGitOutput(
            "Forge response is too large".to_owned(),
        ));
    }
    let bytes = response.bytes().await.map_err(map_network_error)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(AppError::InvalidGitOutput(
            "Forge response is too large".to_owned(),
        ));
    }
    Ok(bytes.to_vec())
}

fn map_network_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() || error.is_connect() {
        AppError::Offline
    } else {
        AppError::InvalidRequest(format!("Forge request failed: {error}"))
    }
}

fn credential_approve(host: &str, username: &str, password: &str) -> Result<(), AppError> {
    credential_command("approve", host, username, Some(password)).map(|_| ())
}

fn credential_fill(host: &str, username: &str) -> Result<String, AppError> {
    let output = credential_command("fill", host, username, None)?;
    output
        .lines()
        .find_map(|line| line.strip_prefix("password="))
        .filter(|password| !password.is_empty())
        .map(str::to_owned)
        .ok_or(AppError::AuthenticationFailed)
}

fn credential_reject(host: &str, username: &str) -> Result<(), AppError> {
    credential_command("reject", host, username, None).map(|_| ())
}

fn credential_command(
    action: &str,
    host: &str,
    username: &str,
    password: Option<&str>,
) -> Result<String, AppError> {
    validate_credential_field(host, "Credential host")?;
    validate_credential_field(username, "Authentication username")?;
    let mut child = Command::new("git")
        .args(["credential", action])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => AppError::GitNotFound,
            _ => AppError::AuthenticationFailed,
        })?;
    let mut input = format!("protocol=https\nhost={host}\nusername={username}\n");
    if let Some(password) = password {
        input.push_str("password=");
        input.push_str(password);
        input.push('\n');
    }
    input.push('\n');
    child
        .stdin
        .take()
        .ok_or(AppError::AuthenticationFailed)?
        .write_all(input.as_bytes())
        .map_err(|_| AppError::AuthenticationFailed)?;
    let output = child
        .wait_with_output()
        .map_err(|_| AppError::AuthenticationFailed)?;
    if !output.status.success() {
        return Err(AppError::AuthenticationFailed);
    }
    String::from_utf8(output.stdout).map_err(|_| AppError::AuthenticationFailed)
}

#[cfg(test)]
mod tests {
    use super::{
        ForgeConnectRequest, account_id, authentication_username, validate_credential_field,
        validate_secret,
    };
    use app_core::{ForgeEndpointPlan, ForgeProvider};

    #[test]
    fn rejects_values_that_can_inject_credential_protocol_fields() {
        assert!(validate_secret("secret\npassword=other").is_err());
        assert!(validate_secret("").is_err());
        assert!(validate_credential_field("user\npassword=other", "username").is_err());
    }

    #[test]
    fn provider_username_requirements_are_explicit() {
        let request = ForgeConnectRequest {
            provider: ForgeProvider::Bitbucket,
            host: "bitbucket.org".to_owned(),
            auth_username: String::new(),
            token: "token".to_owned(),
            scope: Some("workspace".to_owned()),
        };
        assert!(authentication_username(&request).is_err());
    }

    #[test]
    fn account_ids_are_stable_and_scope_sensitive() {
        let first =
            ForgeEndpointPlan::new(ForgeProvider::AzureDevOps, "dev.azure.com", Some("one"))
                .unwrap();
        let second =
            ForgeEndpointPlan::new(ForgeProvider::AzureDevOps, "dev.azure.com", Some("two"))
                .unwrap();
        assert_eq!(account_id(&first, "Ada"), account_id(&first, "ada"));
        assert_ne!(account_id(&first, "ada"), account_id(&second, "ada"));
    }
}
