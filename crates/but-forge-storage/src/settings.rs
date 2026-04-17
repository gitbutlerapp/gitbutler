use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ForgeSettings {
    /// GitHub-specific settings.
    pub github: GitHubSettings,
    /// GitLab-specific settings.
    #[serde(default)]
    pub gitlab: GitLabSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSettings {
    /// GitHub-specific settings.
    #[serde(default, deserialize_with = "deserialize_lenient_vec")]
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
    Enterprise {
        // Hostname of the GitHub Enterprise instance.
        host: String,
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
            GitHubAccount::Enterprise {
                access_token_key, ..
            } => access_token_key,
        }
    }

    pub fn username(&self) -> &str {
        match self {
            GitHubAccount::OAuth { username, .. } => username,
            GitHubAccount::Pat { username, .. } => username,
            GitHubAccount::Enterprise { host, .. } => host,
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
            (
                GitHubAccount::Enterprise {
                    host: h1,
                    username: u1,
                    access_token_key: k1,
                },
                GitHubAccount::Enterprise {
                    host: h2,
                    username: u2,
                    access_token_key: k2,
                },
            ) => h1 == h2 && u1 == u2 && k1 == k2,
            (GitHubAccount::OAuth { .. }, GitHubAccount::Pat { .. }) => false,
            (GitHubAccount::Pat { .. }, GitHubAccount::OAuth { .. }) => false,
            (GitHubAccount::Enterprise { .. }, GitHubAccount::OAuth { .. }) => false,
            (GitHubAccount::OAuth { .. }, GitHubAccount::Enterprise { .. }) => false,
            (GitHubAccount::Pat { .. }, GitHubAccount::Enterprise { .. }) => false,
            (GitHubAccount::Enterprise { .. }, GitHubAccount::Pat { .. }) => false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GitLabSettings {
    /// GitLab-specific settings.
    #[serde(default, deserialize_with = "deserialize_lenient_vec")]
    pub known_accounts: Vec<GitLabAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GitLabAccount {
    Pat {
        // Username associated with the PAT account.
        username: String,
        // Key to retrieve the access token from secure storage.
        access_token_key: String,
    },
    SelfHosted {
        // Hostname of the self-hosted GitLab instance.
        host: String,
        // Username associated with the PAT account.
        username: String,
        // Key to retrieve the access token from secure storage.
        access_token_key: String,
    },
}

impl GitLabAccount {
    pub fn access_token_key(&self) -> &str {
        match self {
            GitLabAccount::Pat {
                access_token_key, ..
            } => access_token_key,
            GitLabAccount::SelfHosted {
                access_token_key, ..
            } => access_token_key,
        }
    }

    pub fn username(&self) -> &str {
        match self {
            GitLabAccount::Pat { username, .. } => username,
            GitLabAccount::SelfHosted { host, .. } => host,
        }
    }
}

impl PartialEq for GitLabAccount {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                GitLabAccount::Pat {
                    username: u1,
                    access_token_key: k1,
                },
                GitLabAccount::Pat {
                    username: u2,
                    access_token_key: k2,
                },
            ) => u1 == u2 && k1 == k2,
            (
                GitLabAccount::SelfHosted {
                    host: h1,
                    username: u1,
                    access_token_key: k1,
                },
                GitLabAccount::SelfHosted {
                    host: h2,
                    username: u2,
                    access_token_key: k2,
                },
            ) => h1 == h2 && u1 == u2 && k1 == k2,
            (GitLabAccount::Pat { .. }, GitLabAccount::SelfHosted { .. }) => false,
            (GitLabAccount::SelfHosted { .. }, GitLabAccount::Pat { .. }) => false,
        }
    }
}

/// Deserialize a list of values, silently discarding entries that cannot be
/// deserialized (e.g. legacy bare-string usernames from an older storage format).
fn deserialize_lenient_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let raw: Vec<serde_json::Value> = Vec::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .filter_map(|v| match serde_json::from_value::<T>(v.clone()) {
            Ok(account) => Some(account),
            Err(_) if v.is_string() => None, // known legacy bare-string format
            Err(err) => {
                eprintln!("warning: discarding unrecognised account entry: {err}");
                None
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_string_accounts_are_discarded() {
        let json = r#"{
            "github": { "knownAccounts": ["someuser"] },
            "gitlab": { "knownAccounts": ["johnsonjs2", "anotheruser"] }
        }"#;
        let settings: ForgeSettings = serde_json::from_str(json).unwrap();
        assert!(settings.github.known_accounts.is_empty());
        assert!(settings.gitlab.known_accounts.is_empty());
    }

    #[test]
    fn mixed_legacy_and_valid_accounts() {
        let json = r#"{
            "github": { "knownAccounts": [] },
            "gitlab": {
                "knownAccounts": [
                    "legacyuser",
                    { "type": "Pat", "username": "validuser", "access_token_key": "gitlab_pat_validuser" },
                    "anotherlegacy"
                ]
            }
        }"#;
        let settings: ForgeSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.gitlab.known_accounts.len(), 1);
        assert_eq!(settings.gitlab.known_accounts[0].username(), "validuser");
    }

    #[test]
    fn roundtrip_serialization_preserves_accounts() {
        let settings = ForgeSettings {
            github: GitHubSettings {
                known_accounts: vec![GitHubAccount::Pat {
                    username: "testuser".into(),
                    access_token_key: "github_pat_testuser".into(),
                }],
            },
            gitlab: GitLabSettings {
                known_accounts: vec![GitLabAccount::Pat {
                    username: "gltest".into(),
                    access_token_key: "gitlab_pat_gltest".into(),
                }],
            },
        };
        let json = serde_json::to_string(&settings).unwrap();
        let roundtripped: ForgeSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped.github.known_accounts.len(), 1);
        assert_eq!(roundtripped.gitlab.known_accounts.len(), 1);
    }
}
