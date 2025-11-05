use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(rename_all = "lowercase")]
/// Supported git forge types
pub enum ForgeName {
    GitHub,
    GitLab,
    Bitbucket,
    Azure,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ForgeRepoInfo {
    pub forge: ForgeName,
    pub owner: String,
    pub repo: String,
    pub protocol: String,
}

impl PartialEq for ForgeRepoInfo {
    fn eq(&self, other: &Self) -> bool {
        self.forge == other.forge && self.owner == other.owner && self.repo == other.repo
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "provider", rename_all = "lowercase", content = "details")]
pub enum ForgeUser {
    GitHub(but_github::GithubAccountIdentifier),
}

impl ForgeUser {
    pub fn github(&self) -> Option<&but_github::GithubAccountIdentifier> {
        match self {
            ForgeUser::GitHub(id) => Some(id),
        }
    }
}

// Custom deserializer for Option<ForgeUser> that accepts either a string or ForgeUser
pub fn deserialize_preferred_forge_user_opt<'de, D>(
    deserializer: D,
) -> Result<Option<ForgeUser>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    Ok(
        match Option::<serde_json::Value>::deserialize(deserializer)? {
            // Handle the deprecated string case
            Some(serde_json::Value::String(s)) => Some(ForgeUser::GitHub(
                but_github::GithubAccountIdentifier::OAuthUsername { username: s },
            )),
            Some(other) => Some(ForgeUser::deserialize(other).map_err(D::Error::custom)?),
            None => None,
        },
    )
}
