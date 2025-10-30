use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ForgeSettings {
    /// GitHub-specific settings.
    pub github: GitHubSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSettings {
    /// GitHub-specific settings.
    pub known_accounts: Vec<GitHubAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GitHubAccount {
    OAuth {
        // Username associated with the OAuth account.
        username: String,
        // Key to retrieve the access token from secure storage.
        access_token_key: String,
    },
    Pat {
        // Username associated with the PAT account.
        username: String,
        // Key to retrieve the access token from secure storage.
        access_token_key: String,
    },
}

impl GitHubAccount {
    pub fn access_token_key(&self) -> &str {
        match self {
            GitHubAccount::OAuth {
                access_token_key, ..
            } => access_token_key,
            GitHubAccount::Pat {
                access_token_key, ..
            } => access_token_key,
        }
    }

    pub fn username(&self) -> &str {
        match self {
            GitHubAccount::OAuth { username, .. } => username,
            GitHubAccount::Pat { username, .. } => username,
        }
    }
}

impl PartialEq for GitHubAccount {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                GitHubAccount::OAuth {
                    username: u1,
                    access_token_key: k1,
                },
                GitHubAccount::OAuth {
                    username: u2,
                    access_token_key: k2,
                },
            ) => u1 == u2 && k1 == k2,
            (
                GitHubAccount::Pat {
                    username: u1,
                    access_token_key: k1,
                },
                GitHubAccount::Pat {
                    username: u2,
                    access_token_key: k2,
                },
            ) => u1 == u2 && k1 == k2,
            (GitHubAccount::OAuth { .. }, GitHubAccount::Pat { .. }) => false,
            (GitHubAccount::Pat { .. }, GitHubAccount::OAuth { .. }) => false,
        }
    }
}
