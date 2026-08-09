//! Non-secret forge provider metadata, endpoint planning, and response normalization.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForgeProvider {
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "gitlab")]
    GitLab,
    #[serde(rename = "bitbucket")]
    Bitbucket,
    #[serde(rename = "azureDevOps")]
    AzureDevOps,
}

impl ForgeProvider {
    pub const fn label(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::GitLab => "gitlab",
            Self::Bitbucket => "bitbucket",
            Self::AzureDevOps => "azureDevOps",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeAccount {
    pub id: String,
    pub provider: ForgeProvider,
    pub host: String,
    pub login: String,
    pub display_name: String,
    pub scope: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeRepository {
    pub id: String,
    pub name: String,
    pub full_name: String,
    pub clone_url: String,
    pub web_url: String,
    pub private: bool,
    pub archived: bool,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeProfile {
    pub login: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeEndpointPlan {
    pub provider: ForgeProvider,
    pub base_url: String,
    pub account_host: String,
    pub scope: Option<String>,
}

impl ForgeEndpointPlan {
    pub fn new(provider: ForgeProvider, host: &str, scope: Option<&str>) -> Result<Self, AppError> {
        let host = host
            .trim()
            .trim_start_matches("https://")
            .trim_end_matches('/');
        if host.is_empty()
            || host.len() > 255
            || host.contains(['/', '\\', '\r', '\n'])
            || !host
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.'))
        {
            return Err(AppError::InvalidRequest(
                "Forge host must be a hostname".to_owned(),
            ));
        }

        let scope = scope
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        if matches!(
            provider,
            ForgeProvider::Bitbucket | ForgeProvider::AzureDevOps
        ) && scope.is_none()
        {
            return Err(AppError::InvalidRequest(
                "This provider requires a workspace or organization".to_owned(),
            ));
        }
        if scope.as_deref().is_some_and(|value| {
            value.len() > 128
                || !value
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        }) {
            return Err(AppError::InvalidRequest(
                "Workspace or organization contains unsupported characters".to_owned(),
            ));
        }

        let (account_host, base_url) = match provider {
            ForgeProvider::GitHub
                if host.eq_ignore_ascii_case("github.com")
                    || host.eq_ignore_ascii_case("api.github.com") =>
            {
                ("github.com".to_owned(), "https://api.github.com".to_owned())
            }
            ForgeProvider::GitHub => (host.to_owned(), format!("https://{host}/api/v3")),
            ForgeProvider::GitLab => (host.to_owned(), format!("https://{host}/api/v4")),
            ForgeProvider::Bitbucket
                if host.eq_ignore_ascii_case("bitbucket.org")
                    || host.eq_ignore_ascii_case("api.bitbucket.org") =>
            {
                (
                    "bitbucket.org".to_owned(),
                    "https://api.bitbucket.org/2.0".to_owned(),
                )
            }
            ForgeProvider::Bitbucket => {
                return Err(AppError::InvalidRequest(
                    "Bitbucket Server is not supported by the Bitbucket Cloud browser".to_owned(),
                ));
            }
            ForgeProvider::AzureDevOps if host.eq_ignore_ascii_case("dev.azure.com") => (
                "dev.azure.com".to_owned(),
                "https://dev.azure.com".to_owned(),
            ),
            ForgeProvider::AzureDevOps => {
                return Err(AppError::InvalidRequest(
                    "Azure DevOps Services must use dev.azure.com".to_owned(),
                ));
            }
        };

        Ok(Self {
            provider,
            base_url,
            account_host,
            scope,
        })
    }

    pub fn profile_url(&self) -> String {
        match self.provider {
            ForgeProvider::GitHub | ForgeProvider::GitLab | ForgeProvider::Bitbucket => {
                format!("{}/user", self.base_url)
            }
            ForgeProvider::AzureDevOps => format!(
                "{}/{}/_apis/projects?api-version=7.1&$top=1",
                self.base_url,
                self.scope.as_deref().expect("Azure scope is validated")
            ),
        }
    }

    pub fn repositories_url(&self) -> String {
        match self.provider {
            ForgeProvider::GitHub => {
                format!("{}/user/repos?per_page=100&sort=updated", self.base_url)
            }
            ForgeProvider::GitLab => format!(
                "{}/projects?membership=true&simple=true&per_page=100&order_by=last_activity_at",
                self.base_url
            ),
            ForgeProvider::Bitbucket => format!(
                "{}/repositories/{}?pagelen=100&sort=-updated_on",
                self.base_url,
                self.scope.as_deref().expect("Bitbucket scope is validated")
            ),
            ForgeProvider::AzureDevOps => format!(
                "{}/{}/_apis/git/repositories?api-version=7.1",
                self.base_url,
                self.scope.as_deref().expect("Azure scope is validated")
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForgeMergeability {
    Mergeable,
    Conflicting,
    Blocked,
    Checking,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForgeReviewStatus {
    Approved,
    ChangesRequested,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForgeCiStatus {
    Success,
    Failure,
    Pending,
    Cancelled,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgePullRequest {
    pub id: String,
    pub number: u64,
    pub title: String,
    pub author: String,
    pub source_branch: String,
    pub target_branch: String,
    pub source_oid: String,
    pub source_clone_url: Option<String>,
    pub web_url: String,
    pub state: String,
    pub draft: bool,
    pub mergeability: ForgeMergeability,
    pub review_status: ForgeReviewStatus,
    pub ci_status: ForgeCiStatus,
    pub updated_at: Option<String>,
}

impl ForgeEndpointPlan {
    pub fn pull_requests_url(&self, repository: &ForgeRepository) -> Result<String, AppError> {
        let path = self.repository_api_path(repository)?;
        Ok(match self.provider {
            ForgeProvider::GitHub => {
                format!("{}/{path}/pulls?state=all&per_page=100", self.base_url)
            }
            ForgeProvider::GitLab => format!(
                "{}/{path}/merge_requests?state=all&per_page=100&order_by=updated_at&sort=desc",
                self.base_url
            ),
            ForgeProvider::Bitbucket => format!(
                "{}/{path}/pullrequests?state=OPEN&state=MERGED&state=DECLINED&state=SUPERSEDED&pagelen=50&sort=-updated_on",
                self.base_url
            ),
            ForgeProvider::AzureDevOps => format!(
                "{}/{path}/pullrequests?searchCriteria.status=all&api-version=7.1",
                self.base_url
            ),
        })
    }

    pub fn pull_request_url(
        &self,
        repository: &ForgeRepository,
        number: u64,
    ) -> Result<String, AppError> {
        let path = self.repository_api_path(repository)?;
        Ok(match self.provider {
            ForgeProvider::GitHub => format!("{}/{path}/pulls/{number}", self.base_url),
            ForgeProvider::GitLab => format!("{}/{path}/merge_requests/{number}", self.base_url),
            ForgeProvider::Bitbucket => format!("{}/{path}/pullrequests/{number}", self.base_url),
            ForgeProvider::AzureDevOps => format!(
                "{}/{path}/pullrequests/{number}?api-version=7.1",
                self.base_url
            ),
        })
    }

    pub fn pull_request_reviews_url(
        &self,
        repository: &ForgeRepository,
        number: u64,
    ) -> Result<String, AppError> {
        let detail = self.pull_request_url(repository, number)?;
        Ok(match self.provider {
            ForgeProvider::GitHub => format!("{detail}/reviews?per_page=100"),
            ForgeProvider::GitLab => format!("{detail}/approvals"),
            ForgeProvider::Bitbucket | ForgeProvider::AzureDevOps => detail,
        })
    }

    pub fn pull_request_ci_url(
        &self,
        repository: &ForgeRepository,
        number: u64,
        source_oid: &str,
    ) -> Result<String, AppError> {
        let path = self.repository_api_path(repository)?;
        Ok(match self.provider {
            ForgeProvider::GitHub => format!(
                "{}/{path}/commits/{}/check-runs?per_page=100",
                self.base_url,
                encode_path_segment(source_oid)
            ),
            ForgeProvider::GitLab => format!(
                "{}/{path}/merge_requests/{number}/pipelines?per_page=1",
                self.base_url
            ),
            ForgeProvider::Bitbucket => format!(
                "{}/{path}/pullrequests/{number}/statuses?pagelen=100",
                self.base_url
            ),
            ForgeProvider::AzureDevOps => format!(
                "{}/{path}/pullrequests/{number}/statuses?api-version=7.1",
                self.base_url
            ),
        })
    }
    pub fn pull_request_merge_url(
        &self,
        repository: &ForgeRepository,
        number: u64,
    ) -> Result<String, AppError> {
        let detail = self.pull_request_url(repository, number)?;
        Ok(match self.provider {
            ForgeProvider::GitHub => format!("{detail}/merge"),
            ForgeProvider::GitLab => format!("{detail}/merge"),
            ForgeProvider::Bitbucket => format!("{detail}/merge"),
            ForgeProvider::AzureDevOps => detail,
        })
    }

    fn repository_api_path(&self, repository: &ForgeRepository) -> Result<String, AppError> {
        if repository.id.is_empty() || repository.id.len() > 256 || repository.full_name.is_empty()
        {
            return Err(AppError::InvalidRequest(
                "Forge repository identifier is invalid".to_owned(),
            ));
        }
        Ok(match self.provider {
            ForgeProvider::GitHub => {
                let mut parts = repository.full_name.split('/');
                let owner = parts.next().unwrap_or_default();
                let name = parts.next().unwrap_or_default();
                if owner.is_empty() || name.is_empty() || parts.next().is_some() {
                    return Err(AppError::InvalidRequest(
                        "GitHub repository name is invalid".to_owned(),
                    ));
                }
                format!(
                    "repos/{}/{}",
                    encode_path_segment(owner),
                    encode_path_segment(name)
                )
            }
            ForgeProvider::GitLab => format!("projects/{}", encode_path_segment(&repository.id)),
            ForgeProvider::Bitbucket => {
                let mut parts = repository.full_name.split('/');
                let workspace = parts.next().unwrap_or_default();
                let slug = parts.next().unwrap_or_default();
                if workspace.is_empty() || slug.is_empty() || parts.next().is_some() {
                    return Err(AppError::InvalidRequest(
                        "Bitbucket repository name is invalid".to_owned(),
                    ));
                }
                format!(
                    "repositories/{}/{}",
                    encode_path_segment(workspace),
                    encode_path_segment(slug)
                )
            }
            ForgeProvider::AzureDevOps => {
                let project = repository.full_name.split('/').next().unwrap_or_default();
                if project.is_empty() {
                    return Err(AppError::InvalidRequest(
                        "Azure DevOps project is invalid".to_owned(),
                    ));
                }
                format!(
                    "{}/{}/_apis/git/repositories/{}",
                    encode_path_segment(self.scope.as_deref().expect("Azure scope is validated")),
                    encode_path_segment(project),
                    encode_path_segment(&repository.id)
                )
            }
        })
    }
}

pub fn parse_forge_pull_requests(
    provider: ForgeProvider,
    bytes: &[u8],
) -> Result<Vec<ForgePullRequest>, AppError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| AppError::InvalidGitOutput("Forge returned invalid JSON".to_owned()))?;
    let items = match provider {
        ForgeProvider::GitHub | ForgeProvider::GitLab => value.as_array(),
        ForgeProvider::Bitbucket | ForgeProvider::AzureDevOps => {
            value.get("values").and_then(Value::as_array)
        }
    }
    .ok_or_else(|| {
        AppError::InvalidGitOutput("Forge pull request response is not a list".to_owned())
    })?;
    items
        .iter()
        .map(|item| parse_pull_request(provider, item))
        .collect()
}

pub fn parse_forge_pull_request(
    provider: ForgeProvider,
    bytes: &[u8],
) -> Result<ForgePullRequest, AppError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| AppError::InvalidGitOutput("Forge returned invalid JSON".to_owned()))?;
    parse_pull_request(provider, &value)
}

fn parse_pull_request(provider: ForgeProvider, item: &Value) -> Result<ForgePullRequest, AppError> {
    let required = |pointer: &str, label: &str| {
        item.pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| AppError::InvalidGitOutput(format!("Forge pull request has no {label}")))
    };
    let number = match provider {
        ForgeProvider::GitHub => item.get("number"),
        ForgeProvider::GitLab => item.get("iid"),
        ForgeProvider::Bitbucket => item.get("id"),
        ForgeProvider::AzureDevOps => item.get("pullRequestId"),
    }
    .and_then(Value::as_u64)
    .ok_or_else(|| AppError::InvalidGitOutput("Forge pull request has no number".to_owned()))?;
    let common = |author: String,
                  source_branch: String,
                  target_branch: String,
                  source_oid: String,
                  source_clone_url: Option<String>,
                  web_url: String,
                  state: String,
                  draft: bool,
                  mergeability,
                  review_status,
                  ci_status,
                  updated_at| ForgePullRequest {
        id: item
            .get("id")
            .map(Value::to_string)
            .unwrap_or_else(|| number.to_string()),
        number,
        title: text(item, "title").unwrap_or_else(|| format!("#{number}")),
        author,
        source_branch,
        target_branch,
        source_oid,
        source_clone_url,
        web_url,
        state,
        draft,
        mergeability,
        review_status,
        ci_status,
        updated_at,
    };
    Ok(match provider {
        ForgeProvider::GitHub => common(
            required("/user/login", "author")?,
            required("/head/ref", "source branch")?,
            required("/base/ref", "target branch")?,
            required("/head/sha", "source oid")?,
            item.pointer("/head/repo/clone_url")
                .and_then(Value::as_str)
                .map(str::to_owned),
            required("/html_url", "web URL")?,
            if item.get("merged_at").is_some_and(|value| !value.is_null())
                || item.get("merged").and_then(Value::as_bool) == Some(true)
            {
                "merged".to_owned()
            } else {
                text(item, "state").unwrap_or_default()
            },
            item.get("draft").and_then(Value::as_bool).unwrap_or(false),
            match item.get("mergeable").and_then(Value::as_bool) {
                Some(true) => ForgeMergeability::Mergeable,
                Some(false) => ForgeMergeability::Conflicting,
                None => ForgeMergeability::Unknown,
            },
            ForgeReviewStatus::Unknown,
            ForgeCiStatus::Unknown,
            text(item, "updated_at"),
        ),
        ForgeProvider::GitLab => {
            let detailed = text(item, "detailed_merge_status").unwrap_or_default();
            common(
                required("/author/username", "author")?,
                required("/source_branch", "source branch")?,
                required("/target_branch", "target branch")?,
                required("/sha", "source oid")?,
                None,
                required("/web_url", "web URL")?,
                text(item, "state").unwrap_or_default(),
                item.get("draft").and_then(Value::as_bool).unwrap_or(false),
                match detailed.as_str() {
                    "mergeable" => ForgeMergeability::Mergeable,
                    "conflict" => ForgeMergeability::Conflicting,
                    "checking" | "preparing" | "unchecked" | "approvals_syncing" => {
                        ForgeMergeability::Checking
                    }
                    "" => ForgeMergeability::Unknown,
                    _ => ForgeMergeability::Blocked,
                },
                match detailed.as_str() {
                    "requested_changes" => ForgeReviewStatus::ChangesRequested,
                    "not_approved" => ForgeReviewStatus::Pending,
                    _ => ForgeReviewStatus::Unknown,
                },
                map_ci_status(
                    item.pointer("/head_pipeline/status")
                        .and_then(Value::as_str),
                ),
                text(item, "updated_at"),
            )
        }
        ForgeProvider::Bitbucket => {
            let participants = item
                .get("participants")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let review = if participants
                .iter()
                .any(|value| text(value, "state").as_deref() == Some("changes_requested"))
            {
                ForgeReviewStatus::ChangesRequested
            } else if participants
                .iter()
                .any(|value| value.get("approved").and_then(Value::as_bool) == Some(true))
            {
                ForgeReviewStatus::Approved
            } else {
                ForgeReviewStatus::Pending
            };
            common(
                required("/author/display_name", "author")?,
                required("/source/branch/name", "source branch")?,
                required("/destination/branch/name", "target branch")?,
                required("/source/commit/hash", "source oid")?,
                bitbucket_clone_url(item.pointer("/source/repository")),
                required("/links/html/href", "web URL")?,
                text(item, "state").unwrap_or_default(),
                item.get("draft").and_then(Value::as_bool).unwrap_or(false),
                ForgeMergeability::Unknown,
                review,
                ForgeCiStatus::Unknown,
                text(item, "updated_on"),
            )
        }
        ForgeProvider::AzureDevOps => {
            let reviewers = item
                .get("reviewers")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let review = if reviewers.iter().any(|value| {
                value
                    .get("vote")
                    .and_then(Value::as_i64)
                    .is_some_and(|vote| vote < 0)
            }) {
                ForgeReviewStatus::ChangesRequested
            } else if reviewers.iter().any(|value| {
                value
                    .get("vote")
                    .and_then(Value::as_i64)
                    .is_some_and(|vote| vote >= 5)
            }) {
                ForgeReviewStatus::Approved
            } else {
                ForgeReviewStatus::Pending
            };
            let statuses = item
                .get("statuses")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            common(
                required("/createdBy/displayName", "author")?,
                strip_heads(&required("/sourceRefName", "source branch")?),
                strip_heads(&required("/targetRefName", "target branch")?),
                required("/lastMergeSourceCommit/commitId", "source oid")?,
                item.pointer("/forkSource/repository/remoteUrl")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                item.pointer("/_links/web/href")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                text(item, "status").unwrap_or_default(),
                item.get("isDraft")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                match text(item, "mergeStatus").as_deref() {
                    Some("succeeded") => ForgeMergeability::Mergeable,
                    Some("conflicts") => ForgeMergeability::Conflicting,
                    Some("queued") => ForgeMergeability::Checking,
                    Some(_) => ForgeMergeability::Blocked,
                    None => ForgeMergeability::Unknown,
                },
                review,
                aggregate_ci_status(&statuses),
                text(item, "creationDate"),
            )
        }
    })
}

fn bitbucket_clone_url(repository: Option<&Value>) -> Option<String> {
    repository?
        .pointer("/links/clone")?
        .as_array()?
        .iter()
        .find(|link| text(link, "name").as_deref() == Some("https"))
        .and_then(|link| text(link, "href"))
}

fn map_ci_status(status: Option<&str>) -> ForgeCiStatus {
    match status.unwrap_or_default().to_ascii_lowercase().as_str() {
        "success" | "successful" | "succeeded" | "completed" => ForgeCiStatus::Success,
        "failed" | "failure" | "error" => ForgeCiStatus::Failure,
        "cancelled" | "canceled" | "stopped" => ForgeCiStatus::Cancelled,
        "pending" | "running" | "in_progress" | "queued" | "notset" => ForgeCiStatus::Pending,
        _ => ForgeCiStatus::Unknown,
    }
}

fn aggregate_ci_status(statuses: &[Value]) -> ForgeCiStatus {
    let mapped = statuses
        .iter()
        .map(|status| map_ci_status(status.get("state").and_then(Value::as_str)))
        .collect::<Vec<_>>();
    if mapped.contains(&ForgeCiStatus::Failure) {
        ForgeCiStatus::Failure
    } else if mapped.contains(&ForgeCiStatus::Pending) {
        ForgeCiStatus::Pending
    } else if !mapped.is_empty()
        && mapped
            .iter()
            .all(|status| *status == ForgeCiStatus::Success)
    {
        ForgeCiStatus::Success
    } else if mapped.contains(&ForgeCiStatus::Cancelled) {
        ForgeCiStatus::Cancelled
    } else {
        ForgeCiStatus::Unknown
    }
}

fn strip_heads(reference: &str) -> String {
    reference
        .strip_prefix("refs/heads/")
        .unwrap_or(reference)
        .to_owned()
}

fn encode_path_segment(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::new();
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~') {
            output.push(*byte as char);
        } else {
            output.push('%');
            output.push(HEX[(byte >> 4) as usize] as char);
            output.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    output
}
pub fn parse_forge_profile(
    provider: ForgeProvider,
    bytes: &[u8],
) -> Result<ForgeProfile, AppError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| AppError::InvalidGitOutput("Forge returned invalid JSON".to_owned()))?;
    let (login, display_name, avatar_url) = match provider {
        ForgeProvider::GitHub => (
            text(&value, "login"),
            text(&value, "name"),
            text(&value, "avatar_url"),
        ),
        ForgeProvider::GitLab => (
            text(&value, "username"),
            text(&value, "name"),
            text(&value, "avatar_url"),
        ),
        ForgeProvider::Bitbucket => (
            text(&value, "nickname"),
            text(&value, "display_name"),
            value
                .pointer("/links/avatar/href")
                .and_then(Value::as_str)
                .map(str::to_owned),
        ),
        ForgeProvider::AzureDevOps => {
            return Err(AppError::InvalidRequest(
                "Azure DevOps profiles use the configured organization".to_owned(),
            ));
        }
    };
    let login =
        login.ok_or_else(|| AppError::InvalidGitOutput("Forge profile has no login".to_owned()))?;
    Ok(ForgeProfile {
        display_name: display_name.unwrap_or_else(|| login.clone()),
        login,
        avatar_url,
    })
}

pub fn parse_forge_repositories(
    provider: ForgeProvider,
    bytes: &[u8],
) -> Result<Vec<ForgeRepository>, AppError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| AppError::InvalidGitOutput("Forge returned invalid JSON".to_owned()))?;
    let items = match provider {
        ForgeProvider::GitHub | ForgeProvider::GitLab => value.as_array(),
        ForgeProvider::Bitbucket | ForgeProvider::AzureDevOps => {
            value.get("values").and_then(Value::as_array)
        }
    }
    .ok_or_else(|| {
        AppError::InvalidGitOutput("Forge repository response is not a list".to_owned())
    })?;
    items
        .iter()
        .map(|item| parse_repository(provider, item))
        .collect()
}

fn parse_repository(provider: ForgeProvider, item: &Value) -> Result<ForgeRepository, AppError> {
    let required = |key: &str| {
        text(item, key)
            .ok_or_else(|| AppError::InvalidGitOutput(format!("Forge repository has no {key}")))
    };
    match provider {
        ForgeProvider::GitHub => Ok(ForgeRepository {
            id: item.get("id").map(Value::to_string).unwrap_or_default(),
            name: required("name")?,
            full_name: required("full_name")?,
            clone_url: required("clone_url")?,
            web_url: required("html_url")?,
            private: item
                .get("private")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            archived: item
                .get("archived")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            updated_at: text(item, "updated_at"),
        }),
        ForgeProvider::GitLab => Ok(ForgeRepository {
            id: item.get("id").map(Value::to_string).unwrap_or_default(),
            name: required("name")?,
            full_name: required("path_with_namespace")?,
            clone_url: required("http_url_to_repo")?,
            web_url: required("web_url")?,
            private: text(item, "visibility").as_deref() == Some("private"),
            archived: item
                .get("archived")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            updated_at: text(item, "last_activity_at"),
        }),
        ForgeProvider::Bitbucket => {
            let clone_url = item
                .pointer("/links/clone")
                .and_then(Value::as_array)
                .and_then(|links| {
                    links
                        .iter()
                        .find(|link| text(link, "name").as_deref() == Some("https"))
                })
                .and_then(|link| text(link, "href"))
                .ok_or_else(|| {
                    AppError::InvalidGitOutput(
                        "Bitbucket repository has no HTTPS clone URL".to_owned(),
                    )
                })?;
            Ok(ForgeRepository {
                id: required("uuid")?,
                name: required("name")?,
                full_name: required("full_name")?,
                clone_url,
                web_url: item
                    .pointer("/links/html/href")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                private: item
                    .get("is_private")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                archived: false,
                updated_at: text(item, "updated_on"),
            })
        }
        ForgeProvider::AzureDevOps => {
            let project = item
                .pointer("/project/name")
                .and_then(Value::as_str)
                .unwrap_or("Azure DevOps");
            let name = required("name")?;
            Ok(ForgeRepository {
                id: required("id")?,
                full_name: format!("{project}/{name}"),
                name,
                clone_url: required("remoteUrl")?,
                web_url: item
                    .pointer("/_links/web/href")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                private: true,
                archived: item
                    .get("isDisabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                updated_at: None,
            })
        }
    }
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{
        ForgeCiStatus, ForgeEndpointPlan, ForgeMergeability, ForgeProvider, ForgeReviewStatus,
        parse_forge_profile, parse_forge_pull_requests, parse_forge_repositories,
    };

    #[test]
    fn builds_provider_specific_repository_endpoints() {
        assert_eq!(
            ForgeEndpointPlan::new(ForgeProvider::GitHub, "github.com", None)
                .unwrap()
                .repositories_url(),
            "https://api.github.com/user/repos?per_page=100&sort=updated"
        );
        assert!(
            ForgeEndpointPlan::new(ForgeProvider::GitLab, "gitlab.example.com", None)
                .unwrap()
                .repositories_url()
                .contains("/api/v4/projects?membership=true")
        );
        assert!(
            ForgeEndpointPlan::new(ForgeProvider::Bitbucket, "bitbucket.org", Some("acorn"))
                .unwrap()
                .repositories_url()
                .contains("/repositories/acorn")
        );
        assert!(
            ForgeEndpointPlan::new(
                ForgeProvider::AzureDevOps,
                "dev.azure.com",
                Some("acorn-org")
            )
            .unwrap()
            .repositories_url()
            .contains("/acorn-org/_apis/git/repositories")
        );
    }

    #[test]
    fn rejects_scopes_that_could_change_the_endpoint_path() {
        assert!(
            ForgeEndpointPlan::new(
                ForgeProvider::AzureDevOps,
                "dev.azure.com",
                Some("../other")
            )
            .is_err()
        );
    }

    #[test]
    fn parses_profiles_and_repositories_without_secrets() {
        let profile = parse_forge_profile(
            ForgeProvider::GitHub,
            br#"{"login":"acorn","name":"Acorn User","avatar_url":"https://example/avatar"}"#,
        )
        .unwrap();
        assert_eq!(profile.login, "acorn");
        let repositories = parse_forge_repositories(ForgeProvider::GitLab, br#"[{"id":7,"name":"demo","path_with_namespace":"team/demo","http_url_to_repo":"https://gitlab.example/team/demo.git","web_url":"https://gitlab.example/team/demo","visibility":"private","archived":false}]"#).unwrap();
        assert_eq!(repositories[0].full_name, "team/demo");
        assert!(repositories[0].private);
    }

    #[test]
    fn normalizes_bitbucket_and_azure_repository_shapes() {
        let bitbucket = parse_forge_repositories(ForgeProvider::Bitbucket, br#"{"values":[{"uuid":"{one}","name":"demo","full_name":"team/demo","is_private":true,"links":{"html":{"href":"https://bitbucket.org/team/demo"},"clone":[{"name":"ssh","href":"git@bitbucket.org:team/demo.git"},{"name":"https","href":"https://bitbucket.org/team/demo.git"}]}}]}"#).unwrap();
        assert_eq!(
            bitbucket[0].clone_url,
            "https://bitbucket.org/team/demo.git"
        );
        let azure = parse_forge_repositories(ForgeProvider::AzureDevOps, br#"{"values":[{"id":"one","name":"demo","remoteUrl":"https://dev.azure.com/org/project/_git/demo","project":{"name":"project"},"_links":{"web":{"href":"https://dev.azure.com/org/project/_git/demo"}}}]}"#).unwrap();
        assert_eq!(azure[0].full_name, "project/demo");
    }
    #[test]
    fn builds_provider_specific_pull_request_endpoints() {
        let repository = super::ForgeRepository {
            id: "17".to_owned(),
            name: "demo".to_owned(),
            full_name: "team/demo".to_owned(),
            clone_url: "https://example/team/demo.git".to_owned(),
            web_url: "https://example/team/demo".to_owned(),
            private: true,
            archived: false,
            updated_at: None,
        };
        let github = ForgeEndpointPlan::new(ForgeProvider::GitHub, "github.com", None).unwrap();
        assert_eq!(
            github.pull_request_merge_url(&repository, 3).unwrap(),
            "https://api.github.com/repos/team/demo/pulls/3/merge"
        );
        let gitlab = ForgeEndpointPlan::new(ForgeProvider::GitLab, "gitlab.example", None).unwrap();
        assert!(
            gitlab
                .pull_requests_url(&repository)
                .unwrap()
                .contains("projects/17/merge_requests")
        );
        let bitbucket =
            ForgeEndpointPlan::new(ForgeProvider::Bitbucket, "bitbucket.org", Some("team"))
                .unwrap();
        assert!(
            bitbucket
                .pull_requests_url(&repository)
                .unwrap()
                .contains("repositories/team/demo/pullrequests")
        );
        let azure_repository = super::ForgeRepository {
            full_name: "Project One/demo".to_owned(),
            ..repository
        };
        let azure = ForgeEndpointPlan::new(
            ForgeProvider::AzureDevOps,
            "dev.azure.com",
            Some("acorn-org"),
        )
        .unwrap();
        assert!(
            azure
                .pull_requests_url(&azure_repository)
                .unwrap()
                .contains("acorn-org/Project%20One/_apis/git/repositories/17/pullrequests")
        );
    }

    #[test]
    fn normalizes_pull_request_review_merge_and_ci_states() {
        let github = parse_forge_pull_requests(ForgeProvider::GitHub, br#"[{"id":10,"number":7,"title":"Improve docs","user":{"login":"ada"},"head":{"ref":"docs","sha":"abc","repo":{"clone_url":"https://github.com/ada/demo.git"}},"base":{"ref":"main"},"html_url":"https://github.com/team/demo/pull/7","state":"open","draft":false,"mergeable":true}]"#).unwrap();
        assert_eq!(github[0].mergeability, ForgeMergeability::Mergeable);
        assert_eq!(github[0].review_status, ForgeReviewStatus::Unknown);
        let merged = parse_forge_pull_requests(ForgeProvider::GitHub, br#"[{"id":11,"number":8,"title":"Merged docs","user":{"login":"ada"},"head":{"ref":"docs","sha":"def"},"base":{"ref":"main"},"html_url":"https://github.com/team/demo/pull/8","state":"closed","merged_at":"2026-08-01T00:00:00Z"}]"#).unwrap();
        assert_eq!(merged[0].state, "merged");

        let gitlab = parse_forge_pull_requests(ForgeProvider::GitLab, br#"[{"id":10,"iid":8,"title":"Feature","author":{"username":"ada"},"source_branch":"feature","target_branch":"main","sha":"def","web_url":"https://gitlab.example/team/demo/-/merge_requests/8","state":"opened","draft":false,"detailed_merge_status":"not_approved","head_pipeline":{"status":"failed"}}]"#).unwrap();
        assert_eq!(gitlab[0].review_status, ForgeReviewStatus::Pending);
        assert_eq!(gitlab[0].ci_status, ForgeCiStatus::Failure);

        let bitbucket = parse_forge_pull_requests(ForgeProvider::Bitbucket, br#"{"values":[{"id":9,"title":"Fix","author":{"display_name":"Ada"},"source":{"branch":{"name":"fix"},"commit":{"hash":"123"},"repository":{"links":{"clone":[{"name":"https","href":"https://bitbucket.org/ada/demo.git"}]}}},"destination":{"branch":{"name":"main"}},"links":{"html":{"href":"https://bitbucket.org/team/demo/pull-requests/9"}},"state":"OPEN","participants":[{"approved":false,"state":"changes_requested"}]}]}"#).unwrap();
        assert_eq!(
            bitbucket[0].review_status,
            ForgeReviewStatus::ChangesRequested
        );

        let azure = parse_forge_pull_requests(ForgeProvider::AzureDevOps, br#"{"values":[{"pullRequestId":11,"title":"Update","createdBy":{"displayName":"Ada"},"sourceRefName":"refs/heads/update","targetRefName":"refs/heads/main","lastMergeSourceCommit":{"commitId":"456"},"_links":{"web":{"href":"https://dev.azure.com/org/project/_git/demo/pullrequest/11"}},"status":"active","mergeStatus":"succeeded","reviewers":[{"vote":10}],"statuses":[{"state":"succeeded"}]}]}"#).unwrap();
        assert_eq!(azure[0].source_branch, "update");
        assert_eq!(azure[0].review_status, ForgeReviewStatus::Approved);
        assert_eq!(azure[0].ci_status, ForgeCiStatus::Success);
    }
}
