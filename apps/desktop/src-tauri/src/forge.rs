use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use app_core::{
    AppError, ForgeAccount, ForgeCiStatus, ForgeEndpointPlan, ForgeIssue, ForgeProfile,
    ForgeProvider, ForgePullRequest, ForgeRepository, ForgeReviewStatus, parse_forge_issues,
    parse_forge_profile, parse_forge_pull_request, parse_forge_pull_requests,
    parse_forge_repositories,
};
use reqwest::{Client, Method, RequestBuilder, StatusCode, Url, header::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgePullRequestCreateRequest {
    pub title: String,
    pub description: String,
    pub source_branch: String,
    pub target_branch: String,
    pub draft: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgePullRequestMergeRequest {
    pub expected_source_oid: String,
    pub squash: bool,
    pub delete_source_branch: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AzureCompletionOptions {
    delete_source_branch: bool,
    squash_merge: bool,
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
        let mut repositories = Vec::new();
        let mut next_url = Some(plan.repositories_url());
        for _ in 0..10 {
            let Some(url) = next_url.take() else {
                break;
            };
            if !url.starts_with(&format!("{}/", plan.base_url)) {
                return Err(AppError::InvalidGitOutput(
                    "Forge pagination URL changed hosts".to_owned(),
                ));
            }
            let response = self
                .authenticated_get(plan.provider, &url, auth_username, &token)
                .send()
                .await
                .map_err(map_network_error)?;
            let headers = response.headers().clone();
            let bytes = checked_body(response).await?;
            next_url = next_repository_url(plan.provider, &url, &headers, &bytes)?;
            repositories.extend(parse_forge_repositories(plan.provider, &bytes)?);
        }
        repositories.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        repositories.dedup_by(|left, right| left.id == right.id);
        Ok(repositories)
    }

    pub async fn pull_requests(
        &self,
        account: &ForgeAccount,
        auth_username: &str,
        repository: &ForgeRepository,
    ) -> Result<Vec<ForgePullRequest>, AppError> {
        let plan =
            ForgeEndpointPlan::new(account.provider, &account.host, account.scope.as_deref())?;
        let token = credential_fill(&plan.account_host, auth_username)?;
        let mut pull_requests = self
            .pull_request_summaries_with_auth(&plan, auth_username, &token, repository)
            .await?;
        for pull_request in &mut pull_requests {
            if !matches!(
                pull_request.state.to_ascii_lowercase().as_str(),
                "open" | "opened" | "active"
            ) {
                continue;
            }
            let detail_url = plan.pull_request_url(repository, pull_request.number)?;
            if let Ok(bytes) = self
                .optional_authenticated_body(plan.provider, &detail_url, auth_username, &token)
                .await
                && let Ok(detail) = parse_forge_pull_request(plan.provider, &bytes)
            {
                *pull_request = detail;
            }
            let review_url = plan.pull_request_reviews_url(repository, pull_request.number)?;
            if let Ok(bytes) = self
                .optional_authenticated_body(plan.provider, &review_url, auth_username, &token)
                .await
            {
                apply_review_status(plan.provider, pull_request, &bytes);
            }
            let ci_url = plan.pull_request_ci_url(
                repository,
                pull_request.number,
                &pull_request.source_oid,
            )?;
            if let Ok(bytes) = self
                .optional_authenticated_body(plan.provider, &ci_url, auth_username, &token)
                .await
            {
                apply_ci_status(plan.provider, pull_request, &bytes);
            }
        }
        Ok(pull_requests)
    }

    pub async fn dashboard_repository(
        &self,
        account: &ForgeAccount,
        auth_username: &str,
        repository: &ForgeRepository,
    ) -> Result<
        (
            Result<Vec<ForgePullRequest>, AppError>,
            Result<Vec<ForgeIssue>, AppError>,
        ),
        AppError,
    > {
        let plan =
            ForgeEndpointPlan::new(account.provider, &account.host, account.scope.as_deref())?;
        let token = credential_fill(&plan.account_host, auth_username)?;
        Ok(tokio::join!(
            self.pull_request_summaries_with_auth(&plan, auth_username, &token, repository),
            self.issues_with_auth(&plan, auth_username, &token, repository),
        ))
    }

    async fn pull_request_summaries_with_auth(
        &self,
        plan: &ForgeEndpointPlan,
        auth_username: &str,
        token: &str,
        repository: &ForgeRepository,
    ) -> Result<Vec<ForgePullRequest>, AppError> {
        let response = self
            .authenticated_request(
                Method::GET,
                plan.provider,
                &plan.pull_requests_url(repository)?,
                auth_username,
                token,
            )
            .send()
            .await
            .map_err(map_network_error)?;
        parse_forge_pull_requests(plan.provider, &checked_body(response).await?)
    }

    async fn issues_with_auth(
        &self,
        plan: &ForgeEndpointPlan,
        auth_username: &str,
        token: &str,
        repository: &ForgeRepository,
    ) -> Result<Vec<ForgeIssue>, AppError> {
        if plan.provider == ForgeProvider::AzureDevOps {
            let query = serde_json::json!({
                "query": "SELECT [System.Id] FROM WorkItems WHERE [System.TeamProject] = @project ORDER BY [System.ChangedDate] DESC"
            });
            let response = self
                .authenticated_request(
                    Method::POST,
                    plan.provider,
                    &plan.azure_work_item_query_url(repository)?,
                    auth_username,
                    token,
                )
                .json(&query)
                .send()
                .await
                .map_err(map_network_error)?;
            let query_body = checked_body(response).await?;
            let ids = azure_work_item_ids(&query_body)?;
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            let response = self
                .authenticated_get(
                    plan.provider,
                    &plan.azure_work_items_url(repository, &ids)?,
                    auth_username,
                    token,
                )
                .send()
                .await
                .map_err(map_network_error)?;
            return parse_forge_issues(plan.provider, &checked_body(response).await?);
        }
        let response = self
            .authenticated_get(
                plan.provider,
                &plan.issues_url(repository)?,
                auth_username,
                token,
            )
            .send()
            .await
            .map_err(map_network_error)?;
        parse_forge_issues(plan.provider, &checked_body(response).await?)
    }

    pub async fn create_pull_request(
        &self,
        account: &ForgeAccount,
        auth_username: &str,
        repository: &ForgeRepository,
        request: &ForgePullRequestCreateRequest,
    ) -> Result<ForgePullRequest, AppError> {
        validate_pull_request_text(&request.title, "title", false)?;
        validate_pull_request_text(&request.description, "description", true)?;
        validate_branch(&request.source_branch)?;
        validate_branch(&request.target_branch)?;
        let plan =
            ForgeEndpointPlan::new(account.provider, &account.host, account.scope.as_deref())?;
        let token = credential_fill(&plan.account_host, auth_username)?;
        let title = if plan.provider == ForgeProvider::GitLab && request.draft {
            format!("Draft: {}", request.title.trim())
        } else {
            request.title.trim().to_owned()
        };
        let body = match plan.provider {
            ForgeProvider::GitHub => {
                serde_json::json!({"title": title, "body": request.description, "head": request.source_branch, "base": request.target_branch, "draft": request.draft})
            }
            ForgeProvider::GitLab => {
                serde_json::json!({"title": title, "description": request.description, "source_branch": request.source_branch, "target_branch": request.target_branch})
            }
            ForgeProvider::Bitbucket => {
                serde_json::json!({"title": title, "description": request.description, "source": {"branch": {"name": request.source_branch}}, "destination": {"branch": {"name": request.target_branch}}, "draft": request.draft})
            }
            ForgeProvider::AzureDevOps => {
                serde_json::json!({"title": title, "description": request.description, "sourceRefName": format!("refs/heads/{}", request.source_branch), "targetRefName": format!("refs/heads/{}", request.target_branch), "isDraft": request.draft})
            }
        };
        let response = self
            .authenticated_request(
                Method::POST,
                plan.provider,
                &plan.pull_requests_url(repository)?,
                auth_username,
                &token,
            )
            .json(&body)
            .send()
            .await
            .map_err(map_network_error)?;
        parse_forge_pull_request(plan.provider, &checked_body(response).await?)
    }

    pub async fn merge_pull_request(
        &self,
        account: &ForgeAccount,
        auth_username: &str,
        repository: &ForgeRepository,
        number: u64,
        request: &ForgePullRequestMergeRequest,
    ) -> Result<(), AppError> {
        validate_oid(&request.expected_source_oid)?;
        let plan =
            ForgeEndpointPlan::new(account.provider, &account.host, account.scope.as_deref())?;
        let token = credential_fill(&plan.account_host, auth_username)?;
        let (method, body) = match plan.provider {
            ForgeProvider::GitHub => (
                Method::PUT,
                serde_json::json!({"sha": request.expected_source_oid, "merge_method": if request.squash { "squash" } else { "merge" }}),
            ),
            ForgeProvider::GitLab => (
                Method::PUT,
                serde_json::json!({"sha": request.expected_source_oid, "squash": request.squash, "should_remove_source_branch": request.delete_source_branch}),
            ),
            ForgeProvider::Bitbucket => (
                Method::POST,
                serde_json::json!({"type": "pullrequest", "close_source_branch": request.delete_source_branch, "merge_strategy": if request.squash { "squash" } else { "merge_commit" }}),
            ),
            ForgeProvider::AzureDevOps => (
                Method::PATCH,
                serde_json::json!({"status": "completed", "lastMergeSourceCommit": {"commitId": request.expected_source_oid}, "completionOptions": AzureCompletionOptions { delete_source_branch: request.delete_source_branch, squash_merge: request.squash }}),
            ),
        };
        let response = self
            .authenticated_request(
                method,
                plan.provider,
                &plan.pull_request_merge_url(repository, number)?,
                auth_username,
                &token,
            )
            .json(&body)
            .send()
            .await
            .map_err(map_network_error)?;
        checked_body(response).await.map(|_| ())
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

    async fn optional_authenticated_body(
        &self,
        provider: ForgeProvider,
        url: &str,
        auth_username: &str,
        token: &str,
    ) -> Result<Vec<u8>, AppError> {
        let response = self
            .authenticated_request(Method::GET, provider, url, auth_username, token)
            .send()
            .await
            .map_err(map_network_error)?;
        checked_body(response).await
    }
    fn authenticated_get(
        &self,
        provider: ForgeProvider,
        url: &str,
        auth_username: &str,
        token: &str,
    ) -> RequestBuilder {
        self.authenticated_request(Method::GET, provider, url, auth_username, token)
    }

    fn authenticated_request(
        &self,
        method: Method,
        provider: ForgeProvider,
        url: &str,
        auth_username: &str,
        token: &str,
    ) -> RequestBuilder {
        let request = self
            .client
            .request(method, url)
            .header("Accept", "application/json");
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

fn next_repository_url(
    provider: ForgeProvider,
    current_url: &str,
    headers: &HeaderMap,
    bytes: &[u8],
) -> Result<Option<String>, AppError> {
    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    Ok(match provider {
        ForgeProvider::GitHub => header("link").and_then(|links| {
            links.split(',').find_map(|part| {
                let part = part.trim();
                let url = part.strip_prefix('<')?.split('>').next()?;
                part.contains("rel=\"next\"").then(|| url.to_owned())
            })
        }),
        ForgeProvider::GitLab => header("x-next-page")
            .filter(|page| !page.is_empty())
            .map(|page| with_query_value(current_url, "page", page))
            .transpose()?,
        ForgeProvider::Bitbucket => serde_json::from_slice::<Value>(bytes)
            .ok()
            .and_then(|value| value.get("next").and_then(Value::as_str).map(str::to_owned)),
        ForgeProvider::AzureDevOps => header("x-ms-continuationtoken")
            .filter(|token| !token.is_empty())
            .map(|token| with_query_value(current_url, "continuationToken", token))
            .transpose()?,
    })
}

fn azure_work_item_ids(bytes: &[u8]) -> Result<Vec<u64>, AppError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| AppError::InvalidGitOutput("Azure WIQL returned invalid JSON".to_owned()))?;
    let items = value
        .get("workItems")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AppError::InvalidGitOutput("Azure WIQL response has no work items".to_owned())
        })?;
    Ok(items
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_u64))
        .take(100)
        .collect())
}

fn with_query_value(url: &str, key: &str, value: &str) -> Result<String, AppError> {
    let mut url = Url::parse(url).map_err(|_| {
        AppError::InvalidGitOutput("Forge returned an invalid pagination URL".to_owned())
    })?;
    let mut pairs = url
        .query_pairs()
        .filter(|(candidate, _)| candidate != key)
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    pairs.push((key.to_owned(), value.to_owned()));
    url.set_query(None);
    url.query_pairs_mut().extend_pairs(pairs);
    Ok(url.to_string())
}
fn apply_review_status(provider: ForgeProvider, pull_request: &mut ForgePullRequest, bytes: &[u8]) {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return;
    };
    pull_request.review_status = match provider {
        ForgeProvider::GitHub => review_votes(
            value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|review| review.get("state").and_then(Value::as_str)),
        ),
        ForgeProvider::GitLab => {
            if value.get("approved").and_then(Value::as_bool) == Some(true) {
                ForgeReviewStatus::Approved
            } else if value
                .get("approvals_left")
                .and_then(Value::as_u64)
                .is_some()
            {
                ForgeReviewStatus::Pending
            } else {
                ForgeReviewStatus::Unknown
            }
        }
        ForgeProvider::Bitbucket => review_votes(
            value
                .get("participants")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|participant| {
                    if participant.get("approved").and_then(Value::as_bool) == Some(true) {
                        Some("APPROVED")
                    } else {
                        participant.get("state").and_then(Value::as_str)
                    }
                }),
        ),
        ForgeProvider::AzureDevOps => {
            let votes = value
                .get("reviewers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|reviewer| reviewer.get("vote").and_then(Value::as_i64));
            let mut approved = false;
            for vote in votes {
                if vote < 0 {
                    return pull_request.review_status = ForgeReviewStatus::ChangesRequested;
                }
                approved |= vote >= 5;
            }
            if approved {
                ForgeReviewStatus::Approved
            } else {
                ForgeReviewStatus::Pending
            }
        }
    };
}

fn review_votes<'a>(states: impl Iterator<Item = &'a str>) -> ForgeReviewStatus {
    let mut approved = false;
    let mut seen = false;
    for state in states {
        seen = true;
        let state = state.to_ascii_uppercase();
        if state.contains("CHANGES_REQUESTED") || state.contains("REQUEST_CHANGES") {
            return ForgeReviewStatus::ChangesRequested;
        }
        approved |= state.contains("APPROV");
    }
    if approved {
        ForgeReviewStatus::Approved
    } else if seen {
        ForgeReviewStatus::Pending
    } else {
        ForgeReviewStatus::Unknown
    }
}

fn apply_ci_status(provider: ForgeProvider, pull_request: &mut ForgePullRequest, bytes: &[u8]) {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return;
    };
    let items = match provider {
        ForgeProvider::GitHub => value.get("check_runs").and_then(Value::as_array),
        ForgeProvider::GitLab => value.as_array(),
        ForgeProvider::Bitbucket | ForgeProvider::AzureDevOps => {
            value.get("values").and_then(Value::as_array)
        }
    };
    let Some(items) = items else {
        return;
    };
    if items.is_empty() {
        pull_request.ci_status = ForgeCiStatus::Unknown;
        return;
    }
    let mut status = ForgeCiStatus::Success;
    for item in items {
        let raw = item
            .get("conclusion")
            .or_else(|| item.get("state"))
            .or_else(|| item.get("status"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let current = if raw.contains("fail") || raw.contains("error") || raw.contains("stop") {
            ForgeCiStatus::Failure
        } else if raw.contains("cancel") || raw.contains("skip") {
            ForgeCiStatus::Cancelled
        } else if raw.contains("success") || raw.contains("succeed") || raw.contains("complete") {
            ForgeCiStatus::Success
        } else {
            ForgeCiStatus::Pending
        };
        status = match (status, current) {
            (_, ForgeCiStatus::Failure) => ForgeCiStatus::Failure,
            (ForgeCiStatus::Failure, _) => ForgeCiStatus::Failure,
            (_, ForgeCiStatus::Pending) => ForgeCiStatus::Pending,
            (ForgeCiStatus::Pending, _) => ForgeCiStatus::Pending,
            (_, ForgeCiStatus::Cancelled) => ForgeCiStatus::Cancelled,
            (ForgeCiStatus::Cancelled, _) => ForgeCiStatus::Cancelled,
            _ => ForgeCiStatus::Success,
        };
    }
    pull_request.ci_status = status;
}
fn validate_pull_request_text(value: &str, label: &str, allow_empty: bool) -> Result<(), AppError> {
    let value = value.trim();
    if (!allow_empty && value.is_empty()) || value.len() > 65_536 || value.contains('\0') {
        return Err(AppError::InvalidRequest(format!(
            "Pull request {label} is invalid"
        )));
    }
    Ok(())
}

fn validate_branch(value: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value.len() > 1024
        || value.starts_with('-')
        || value.contains(['\r', '\n', '\0'])
    {
        return Err(AppError::InvalidRequest(
            "Pull request branch is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_oid(value: &str) -> Result<(), AppError> {
    if !(7..=64).contains(&value.len()) || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(AppError::InvalidRequest(
            "Pull request source commit is invalid".to_owned(),
        ));
    }
    Ok(())
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
        ForgeConnectRequest, account_id, apply_ci_status, apply_review_status,
        authentication_username, azure_work_item_ids, next_repository_url,
        validate_credential_field, validate_secret,
    };
    use reqwest::header::{HeaderMap, HeaderValue};

    use app_core::{
        ForgeCiStatus, ForgeEndpointPlan, ForgeProvider, ForgeReviewStatus,
        parse_forge_pull_requests,
    };

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
    #[test]
    fn follows_only_explicit_repository_pagination_markers() {
        let mut github = HeaderMap::new();
        github.insert(
            "link",
            HeaderValue::from_static(
                "<https://api.github.com/user/repos?per_page=100&page=2>; rel=\"next\"",
            ),
        );
        assert_eq!(
            next_repository_url(
                ForgeProvider::GitHub,
                "https://api.github.com/user/repos?per_page=100",
                &github,
                b"[]",
            )
            .unwrap()
            .as_deref(),
            Some("https://api.github.com/user/repos?per_page=100&page=2")
        );

        let mut gitlab = HeaderMap::new();
        gitlab.insert("x-next-page", HeaderValue::from_static("3"));
        let next = next_repository_url(
            ForgeProvider::GitLab,
            "https://gitlab.example/api/v4/projects?membership=true&page=2",
            &gitlab,
            b"[]",
        )
        .unwrap()
        .unwrap();
        assert!(next.contains("page=3"));
        assert!(!next.contains("page=2"));
    }
    #[test]
    fn normalizes_review_and_ci_detail_responses() {
        let mut pull_request = parse_forge_pull_requests(
            ForgeProvider::GitHub,
            br#"[{"id":10,"number":7,"title":"Improve docs","user":{"login":"ada"},"head":{"ref":"docs","sha":"abcdef0123456789abcdef0123456789abcdef01","repo":{"clone_url":"https://github.com/ada/demo.git"}},"base":{"ref":"main"},"html_url":"https://github.com/team/demo/pull/7","state":"open","draft":false}]"#,
        )
        .unwrap()
        .remove(0);
        apply_review_status(
            ForgeProvider::GitHub,
            &mut pull_request,
            br#"[{"state":"APPROVED"}]"#,
        );
        apply_ci_status(
            ForgeProvider::GitHub,
            &mut pull_request,
            br#"{"check_runs":[{"status":"completed","conclusion":"success"}]}"#,
        );
        assert_eq!(pull_request.review_status, ForgeReviewStatus::Approved);
        assert_eq!(pull_request.ci_status, ForgeCiStatus::Success);

        apply_review_status(
            ForgeProvider::GitHub,
            &mut pull_request,
            br#"[{"state":"CHANGES_REQUESTED"}]"#,
        );
        apply_ci_status(
            ForgeProvider::GitHub,
            &mut pull_request,
            br#"{"check_runs":[{"status":"completed","conclusion":"failure"}]}"#,
        );
        assert_eq!(
            pull_request.review_status,
            ForgeReviewStatus::ChangesRequested
        );
        assert_eq!(pull_request.ci_status, ForgeCiStatus::Failure);
    }

    #[test]
    fn limits_azure_work_item_batches() {
        let values = (1..=120)
            .map(|id| serde_json::json!({ "id": id }))
            .collect::<Vec<_>>();
        let bytes = serde_json::to_vec(&serde_json::json!({ "workItems": values })).unwrap();
        let ids = azure_work_item_ids(&bytes).unwrap();
        assert_eq!(ids.len(), 100);
        assert_eq!(ids[0], 1);
        assert_eq!(ids[99], 100);
    }
}
