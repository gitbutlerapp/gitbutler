use anyhow::{Context as _, Result};
use but_secret::Sensitive;

mod client;
mod token;

pub use client::{
    CreatePullRequestParams, GiteeBranch, GiteeClient, GiteeCommitStatus, GiteeLabel,
    GiteePullRequest, GiteePullRequestBranch, GiteeRepo, GiteeUser, HttpStatusError,
};
pub use token::{
    AuthenticatedUser, GiteeAccountIdentifier, get_gitee_access_token as token_access_token_raw,
    list_known_gitee_accounts,
};

/// Store a PAT access token and fetch the associated user data.
/// Gitee answered 401: the token was rejected.
pub(crate) const GITEE_UNAUTHORIZED: but_error::Context = but_error::Context::new_static(
    but_error::Code::GiteeUnauthorized,
    "Gitee did not accept the token.",
);

/// Gitee answered 403.
pub(crate) const GITEE_FORBIDDEN: but_error::Context = but_error::Context::new_static(
    but_error::Code::GiteeForbidden,
    "Gitee refused access for the token.",
);

pub async fn store_pat(
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<AuthStatusResponse> {
    let user = fetch_and_persist_pat_user_data(access_token, storage).await?;
    Ok(AuthStatusResponse {
        access_token: access_token.clone(),
        username: user.login,
        name: user.name,
        email: user.email,
        host: None,
    })
}

/// Store a self-hosted (enterprise) Gitee access token and fetch the associated user data.
pub async fn store_selfhosted_pat(
    host: &str,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<AuthStatusResponse> {
    let base_url = format!("https://{host}/api/v5");
    let gl = GiteeClient::with_base_url(access_token, &base_url)
        .context("Failed to create Gitee client")?;
    let user = gl
        .get_authenticated()
        .await
        .context("Failed to get authenticated user")?;
    let account_id = token::GiteeAccountIdentifier::selfhosted(&user.login, host);
    token::persist_gitee_access_token(&account_id, access_token, storage)
        .context("Failed to persist access token")?;
    Ok(AuthStatusResponse {
        access_token: access_token.clone(),
        username: user.login,
        name: user.name,
        email: user.email,
        host: Some(host.to_owned()),
    })
}

async fn fetch_and_persist_pat_user_data(
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<client::GiteeUser, anyhow::Error> {
    let gitee = client::GiteeClient::new(access_token).context("Failed to create Gitee client")?;
    let user = gitee
        .get_authenticated()
        .await
        .map_err(classify_pat_validation_error)
        .context("Failed to get authenticated user")?;
    let account_id = token::GiteeAccountIdentifier::pat(&user.login);
    token::persist_gitee_access_token(&account_id, access_token, storage)
        .context("Failed to persist access token")?;
    Ok(user)
}

/// Public accessor for an account's stored access token (used by but-forge).
pub fn token_access_token(
    account: &GiteeAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<but_secret::Sensitive<String>>> {
    token::get_gitee_access_token(account, storage)
}

fn classify_pat_validation_error(err: anyhow::Error) -> anyhow::Error {
    let Some(http_err) = err.downcast_ref::<client::HttpStatusError>() else {
        return err;
    };
    let context = match http_err.status {
        reqwest::StatusCode::UNAUTHORIZED => GITEE_UNAUTHORIZED,
        reqwest::StatusCode::FORBIDDEN => GITEE_FORBIDDEN,
        _ => return err,
    };
    err.context(context)
}

pub fn forget_gitee_access_token(
    account: &GiteeAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    token::delete_gitee_access_token(account, storage).context("Failed to delete access token")
}

pub async fn get_gitee_user(
    account: &GiteeAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<AuthenticatedUser>> {
    if let Some(access_token) = token::get_gitee_access_token(account, storage)? {
        let base = match account {
            GiteeAccountIdentifier::SelfHosted { host, .. } => format!("https://{host}/api/v5"),
            _ => client::api_base_url().to_string(),
        };
        let gl = GiteeClient::with_base_url(&access_token, &base)
            .context("Failed to create Gitee client")?;
        match gl.get_authenticated().await {
            Ok(user) => Ok(Some(AuthenticatedUser {
                access_token,
                username: user.login,
                name: user.name,
                email: user.email,
                avatar_url: user.avatar_url,
            })),
            Err(_) => Ok(Some(AuthenticatedUser {
                access_token,
                username: account.username().to_owned(),
                name: None,
                email: None,
                avatar_url: None,
            })),
        }
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct AuthStatusResponse {
    /// The access token.
    /// This is only shared with the FrontEnd temporarily as we undergo the migration to having all API calls
    /// made to the forges from the Rustend.
    pub access_token: Sensitive<String>,
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub host: Option<String>,
}

/// JSON-facing API response types.
pub mod json {
    use crate::{AuthStatusResponse, AuthenticatedUser};
    use serde::Serialize;

    /// Serializable version of [`AuthStatusResponse`], without the access token.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct GiteeAuthStatusResponse {
        pub username: String,
        pub name: Option<String>,
        pub email: Option<String>,
        /// The enterprise/self-hosted host, when there is one.
        pub host: Option<String>,
    }

    impl From<AuthStatusResponse> for GiteeAuthStatusResponse {
        fn from(
            AuthStatusResponse {
                username,
                name,
                email,
                host,
                ..
            }: AuthStatusResponse,
        ) -> Self {
            GiteeAuthStatusResponse {
                username,
                name,
                email,
                host,
            }
        }
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(GiteeAuthStatusResponse);

    /// Serializable version of [`AuthenticatedUser`] with exposed access token.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct GiteeAuthenticatedUserSensitive {
        pub access_token: String,
        pub username: String,
        pub name: Option<String>,
        pub email: Option<String>,
        pub avatar_url: Option<String>,
    }

    impl From<AuthenticatedUser> for GiteeAuthenticatedUserSensitive {
        fn from(
            AuthenticatedUser {
                access_token,
                username,
                name,
                email,
                avatar_url,
            }: AuthenticatedUser,
        ) -> Self {
            GiteeAuthenticatedUserSensitive {
                access_token: access_token.0.clone(),
                username,
                name,
                email,
                avatar_url,
            }
        }
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(GiteeAuthenticatedUserSensitive);
}

/// Result of validating stored credentials for an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialCheckResult {
    Valid,
    Invalid,
    NoCredentials,
}

/// Resolve the account + token and build a client, mirroring the other forges.
async fn client_for_account(
    preferred_account: Option<&GiteeAccountIdentifier>,
    storage: &but_forge_storage::Controller,
) -> Result<Option<(GiteeClient, GiteeAccountIdentifier)>> {
    let account = preferred_account
        .cloned()
        .or_else(|| {
            token::list_known_gitee_accounts(storage)
                .ok()
                .and_then(|mut accounts| accounts.pop())
        })
        .ok_or_else(|| anyhow::anyhow!("No Gitee account is signed in."))?;
    let Some(access_token) = token::get_gitee_access_token(&account, storage)? else {
        return Ok(None);
    };
    let client = match &account {
        GiteeAccountIdentifier::SelfHosted { host, .. } => {
            GiteeClient::with_base_url(&access_token, &format!("https://{host}/api/v5"))?
        }
        _ => GiteeClient::new(&access_token)?,
    };
    Ok(Some((client, account)))
}

/// List the PRs whose head branch is `branch`, for the preferred (or only) account.
pub async fn list_prs_for_branch(
    preferred_account: Option<&GiteeAccountIdentifier>,
    owner: &str,
    repo: &str,
    branch: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<GiteePullRequest>> {
    let Some((client, _)) = client_for_account(preferred_account, storage).await? else {
        return Ok(vec![]);
    };
    client
        .list_pull_requests_for_branch(owner, repo, branch)
        .await
}

/// List all PRs (open + closed) for the preferred (or only) account.
pub async fn list_all_prs(
    preferred_account: Option<&GiteeAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<GiteePullRequest>> {
    let Some((client, _)) = client_for_account(preferred_account, storage).await? else {
        return Ok(vec![]);
    };
    client.list_pull_requests(owner, repo, "all").await
}

/// Create a pull request. `head` should be `owner:branch` when opening from a fork.
pub async fn create_pr(
    preferred_account: Option<&GiteeAccountIdentifier>,
    owner: &str,
    repo: &str,
    head: &str,
    base: &str,
    title: &str,
    body: &str,
    storage: &but_forge_storage::Controller,
) -> Result<GiteePullRequest> {
    let Some((client, _)) = client_for_account(preferred_account, storage).await? else {
        anyhow::bail!(
            "No Gitee access token found. Add a Gitee account in the integrations settings."
        );
    };
    client
        .create_pull_request(
            owner,
            repo,
            &CreatePullRequestParams {
                title: title.to_string(),
                head: head.to_string(),
                base: base.to_string(),
                body: (!body.is_empty()).then(|| body.to_string()),
            },
        )
        .await
}

/// Validate the stored credentials for the preferred (or only) Gitee account.
pub async fn check_credentials(
    preferred_account: Option<&GiteeAccountIdentifier>,
    storage: &but_forge_storage::Controller,
) -> Result<CredentialCheckResult> {
    let Some((client, _)) = client_for_account(preferred_account, storage).await? else {
        return Ok(CredentialCheckResult::NoCredentials);
    };
    match client.get_authenticated().await {
        Ok(_) => Ok(CredentialCheckResult::Valid),
        Err(_) => Ok(CredentialCheckResult::Invalid),
    }
}
