use anyhow::Result;
use but_secret::Sensitive;
use octorust::{Client, auth::Credentials, types::UsersGetByUsernameResponseOneOf};
use serde::Serialize;

pub struct GitHubClient {
    github: Client,
}

impl GitHubClient {
    pub fn new(access_token: &Sensitive<String>) -> Result<Self> {
        let github = Client::new(
            String::from("gb-github-integration"),
            Credentials::Token(access_token.to_string()),
        )?;

        Ok(Self { github })
    }

    pub fn new_with_host_override(access_token: &Sensitive<String>, host: &str) -> Result<Self> {
        let github = Client::new(
            String::from("gb-github-integration"),
            Credentials::Token(access_token.to_string()),
        )?
        .with_host_override(host)
        .to_owned();

        Ok(Self { github })
    }

    pub async fn get_authenticated(&self) -> Result<AuthenticatedUser, octorust::ClientError> {
        self.github
            .users()
            .get_authenticated()
            .await
            .map(|response| match response.body {
                UsersGetByUsernameResponseOneOf::PrivateUser(user) => {
                    let name = (!user.name.is_empty()).then(|| user.name.clone());
                    let email = (!user.email.is_empty()).then(|| user.email.clone());
                    let avatar_url = (!user.avatar_url.is_empty()).then(|| user.avatar_url.clone());
                    AuthenticatedUser {
                        login: user.login,
                        avatar_url,
                        name,
                        email,
                    }
                }
                UsersGetByUsernameResponseOneOf::PublicUser(user) => {
                    let name = (!user.name.is_empty()).then(|| user.name.clone());
                    let email = (!user.email.is_empty()).then(|| user.email.clone());
                    let avatar_url = (!user.avatar_url.is_empty()).then(|| user.avatar_url.clone());
                    AuthenticatedUser {
                        login: user.login,
                        avatar_url,
                        name,
                        email,
                    }
                }
            })
    }

    pub async fn list_open_pulls(&self, owner: &str, repo: &str) -> Result<Vec<PullRequest>> {
        let pulls = self
            .github
            .pulls()
            .list_all(
                owner,
                repo,
                octorust::types::IssuesListState::Open,
                "",
                "",
                octorust::types::PullsListSort::Created,
                octorust::types::Order::default(),
            )
            .await
            .map(|response| response.body)?
            .into_iter()
            .map(Into::into)
            .collect();

        Ok(pulls)
    }

    pub async fn create_pull_request(
        &self,
        params: &CreatePullRequestParams<'_>,
    ) -> Result<PullRequest> {
        let pr = self
            .github
            .pulls()
            .create(
                params.owner,
                params.repo,
                &octorust::types::PullsCreateRequest {
                    title: params.title.to_string(),
                    body: params.body.to_string(),
                    head: params.head.to_string(),
                    base: params.base.to_string(),
                    draft: Some(params.draft),
                    issue: 0,
                    maintainer_can_modify: None,
                },
            )
            .await
            .map(|response| response.body)
            .map_err(anyhow::Error::from)?;

        Ok(pr.into())
    }
}

pub struct CreatePullRequestParams<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub head: &'a str,
    pub base: &'a str,
    pub draft: bool,
    pub owner: &'a str,
    pub repo: &'a str,
}

#[derive(Debug, Serialize)]
pub struct AuthenticatedUser {
    pub login: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub is_bot: bool,
}

impl From<octorust::types::SimpleUser> for GitHubUser {
    fn from(user: octorust::types::SimpleUser) -> Self {
        GitHubUser {
            id: user.id,
            login: user.login,
            name: (!user.name.is_empty()).then_some(user.name),
            email: (!user.email.is_empty()).then_some(user.email),
            avatar_url: (!user.avatar_url.is_empty()).then_some(user.avatar_url),
            is_bot: user.type_.to_lowercase() == "bot",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GitHubPrLabel {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
}

#[derive(Debug, Serialize)]
pub struct PullRequest {
    pub html_url: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub author: Option<GitHubUser>,
    pub labels: Vec<GitHubPrLabel>,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub sha: String,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub merged_at: Option<String>,
    pub closed_at: Option<String>,
    pub repository_ssh_url: Option<String>,
    pub repository_https_url: Option<String>,
    pub repo_owner: Option<String>,
    pub requested_reviewers: Vec<GitHubUser>,
}

impl From<octorust::types::PullRequestSimple> for PullRequest {
    fn from(pr: octorust::types::PullRequestSimple) -> Self {
        let author = pr.user.map(Into::into);

        let labels = pr
            .labels
            .into_iter()
            .map(|label| GitHubPrLabel {
                id: label.id,
                name: label.name,
                description: (!label.description.is_empty()).then_some(label.description),
                color: label.color,
            })
            .collect();

        let requested_reviewers = pr.requested_reviewers.into_iter().map(Into::into).collect();

        PullRequest {
            html_url: pr.html_url,
            number: pr.number,
            title: pr.title,
            body: (!pr.body.is_empty()).then_some(pr.body),
            author,
            labels,
            draft: pr.draft,
            source_branch: pr.head.ref_,
            target_branch: pr.base.ref_,
            sha: pr.head.sha,
            created_at: pr.created_at.map(|d| d.to_string()),
            modified_at: pr.updated_at.map(|d| d.to_string()),
            merged_at: pr.merged_at.map(|d| d.to_string()),
            closed_at: pr.closed_at.map(|d| d.to_string()),
            repository_ssh_url: pr
                .base
                .repo
                .as_ref()
                .and_then(|r| (!r.ssh_url.is_empty()).then(|| r.ssh_url.to_owned())),
            repository_https_url: pr
                .head
                .repo
                .as_ref()
                .and_then(|r| (!r.clone_url.is_empty()).then(|| r.clone_url.to_owned())),
            repo_owner: pr.head.repo.and_then(|r| r.owner.map(|o| o.login)),
            requested_reviewers,
        }
    }
}

impl From<octorust::types::PullRequestData> for PullRequest {
    fn from(pr: octorust::types::PullRequestData) -> Self {
        let author = pr.user.map(Into::into);

        let labels = pr
            .labels
            .into_iter()
            .map(|label| GitHubPrLabel {
                id: label.id,
                name: label.name,
                description: (!label.description.is_empty()).then_some(label.description),
                color: label.color,
            })
            .collect();

        let requested_reviewers = pr.requested_reviewers.into_iter().map(Into::into).collect();

        PullRequest {
            html_url: pr.html_url,
            number: pr.number,
            title: pr.title,
            body: (!pr.body.is_empty()).then_some(pr.body),
            author,
            labels,
            draft: pr.draft,
            source_branch: pr.head.ref_,
            target_branch: pr.base.ref_,
            sha: pr.head.sha,
            created_at: pr.created_at.map(|d| d.to_string()),
            modified_at: pr.updated_at.map(|d| d.to_string()),
            merged_at: pr.merged_at.map(|d| d.to_string()),
            closed_at: pr.closed_at.map(|d| d.to_string()),
            repository_ssh_url: pr
                .base
                .repo
                .as_ref()
                .and_then(|r| (!r.ssh_url.is_empty()).then(|| r.ssh_url.to_owned())),
            repository_https_url: pr
                .head
                .repo
                .as_ref()
                .and_then(|r| (!r.clone_url.is_empty()).then(|| r.clone_url.to_owned())),
            repo_owner: pr.head.repo.map(|r| r.owner.login),
            requested_reviewers,
        }
    }
}
