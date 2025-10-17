use std::sync::Mutex;

use anyhow::{Context, Result};
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

/// Persist GitHub OAuth access tokens securely.
pub fn persist_gh_access_token(login: &str, access_token: &Sensitive<String>) -> Result<()> {
    let mut map = retrieve_github_oauth_access_token_map()?;
    if let Some(entry) = map.iter_mut().find(|entry| entry.username == login) {
        entry.access_token = GitHubAccessToken::oauth(&access_token.clone().0);
    } else {
        map.push(UserToken {
            username: login.to_string(),
            access_token: GitHubAccessToken::oauth(&access_token.clone().0),
        });
    }
    persist_github_oauth_access_token_map(&map)
}

/// Delete a GitHub OAuth access token for a given username.
pub fn delete_gh_access_token(login: &str) -> Result<()> {
    let mut map = retrieve_github_oauth_access_token_map()?;
    map.retain(|entry| entry.username != login);
    persist_github_oauth_access_token_map(&map)
}

/// Retrieve a GitHub OAuth access token for a given username.
pub fn get_gh_access_token(login: &str) -> Result<Option<Sensitive<String>>> {
    let map = retrieve_github_oauth_access_token_map()?;
    Ok(map
        .into_iter()
        .find(|entry| entry.username == login)
        .map(|entry| entry.access_token.sensitive()))
}

pub fn list_known_github_usernames() -> Result<Vec<String>> {
    let map = retrieve_github_oauth_access_token_map()?;
    Ok(map.into_iter().map(|entry| entry.username).collect())
}

const GITHUB_OAUTH_ACCESS_TOKEN_MAP_KEY: &str = "github_oauth_access_token_map";

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum GitHubAccessToken {
    /// Access token obtained via OAuth device flow.
    OAuth { access_token: String },
    /// Personal access token
    Personal { access_token: String },
}

impl GitHubAccessToken {
    pub fn oauth(token: &str) -> Self {
        GitHubAccessToken::OAuth {
            access_token: token.to_string(),
        }
    }
    pub fn sensitive(&self) -> Sensitive<String> {
        match self {
            GitHubAccessToken::OAuth { access_token } => Sensitive(access_token.to_owned()),
            GitHubAccessToken::Personal { access_token } => Sensitive(access_token.to_owned()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct UserToken {
    pub username: String,
    pub access_token: GitHubAccessToken,
}

fn persist_github_oauth_access_token_map(map: &[UserToken]) -> Result<()> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    let serialized_map =
        Sensitive(serde_json::to_string(map).context("Failed to serialize access token map")?);

    secret::persist(
        GITHUB_OAUTH_ACCESS_TOKEN_MAP_KEY,
        &serialized_map,
        secret::Namespace::Global,
    )
}

fn retrieve_github_oauth_access_token_map() -> Result<Vec<UserToken>> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    if let Some(serialized_map) =
        secret::retrieve(GITHUB_OAUTH_ACCESS_TOKEN_MAP_KEY, secret::Namespace::Global)?
    {
        let map = serde_json::from_str::<Vec<UserToken>>(&serialized_map.to_string())
            .context("Failed to deserialize access token map")?;
        Ok(map)
    } else {
        Ok(vec![])
    }
}
