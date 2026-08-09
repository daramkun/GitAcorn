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
    use super::{ForgeEndpointPlan, ForgeProvider, parse_forge_profile, parse_forge_repositories};

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
}
