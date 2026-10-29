use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
/// Supported git forge types
pub enum ForgeName {
    GitHub,
    GitLab,
    Bitbucket,
    Azure,
}

impl ForgeName {
    pub(crate) fn public_host(&self) -> Option<&'static str> {
        match self {
            Self::GitHub => Some("github.com"),
            Self::GitLab => Some("gitlab.com"),
            Self::Bitbucket => Some("bitbucket.org"),
            Self::Azure => Some("dev.azure.com"),
        }
    }

    /// Parse a project's stored `forge_override` string (the value the Settings
    /// forge picker saves) into a `ForgeName`.
    ///
    /// Returns `None` for `"default"` or any unrecognized value, meaning "no
    /// override". The accepted spellings match this enum's `lowercase` serde
    /// representation.
    pub fn from_override_str(value: &str) -> Option<Self> {
        // Case- and whitespace-insensitive so any legacy spelling of a saved
        // preference still resolves; unknown values (incl. "default") mean "no
        // override".
        match value.trim().to_ascii_lowercase().as_str() {
            "github" => Some(ForgeName::GitHub),
            "gitlab" => Some(ForgeName::GitLab),
            "bitbucket" => Some(ForgeName::Bitbucket),
            "azure" => Some(ForgeName::Azure),
            _ => None,
        }
    }
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeName);

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// Remote repository coordinates. Compare explicitly with [`Self::same_repository`];
/// SSH's unknown API port makes repository compatibility non-transitive.
pub struct ForgeRepoInfo {
    pub forge: ForgeName,
    /// Repository host from the remote URL, not an account's API endpoint.
    /// Live API calls must select credentials for this host.
    pub host: String,
    /// Explicit remote port. SSH ports are transport-only, not API ports.
    pub port: Option<u16>,
    pub owner: String,
    pub repo: String,
    pub protocol: String,
}

impl ForgeRepoInfo {
    /// Compare provider and canonical repository host without considering ports.
    fn same_host(&self, other: &Self) -> bool {
        self.forge == other.forge
            && crate::repository_hosts_match(
                crate::canonical_repository_host(&self.forge, &self.host),
                crate::canonical_repository_host(&other.forge, &other.host),
            )
    }

    /// Whether this host (including known SSH aliases) identifies the public
    /// forge instance. Ports are checked separately.
    pub(crate) fn is_public_host(&self) -> bool {
        self.forge.public_host().is_some_and(|public| {
            crate::repository_hosts_match(
                crate::canonical_repository_host(&self.forge, &self.host),
                public,
            )
        })
    }

    /// Whether these remotes can identify the same forge instance. This is a
    /// compatibility check, not equality: SSH remotes have unknown API ports.
    pub(crate) fn same_server(&self, other: &Self) -> bool {
        self.same_host(other)
            && match (self.identity_port(), other.identity_port()) {
                (Some(left), Some(right)) => left == right,
                _ => true,
            }
    }

    /// Whether these remotes can identify the same repository for remote reuse
    /// and fork detection. Provider, canonical host, owner, and repo must match;
    /// known API ports must agree, while private SSH's API port is unknown.
    ///
    /// This is symmetric but not transitive: SSH can match HTTPS on either of
    /// two ports that do not match each other. It does not authorize credentials.
    pub fn same_repository(&self, other: &Self) -> bool {
        self.same_server(other) && self.owner == other.owner && self.repo == other.repo
    }

    /// Known API port for server and account checks. Private SSH remotes remain
    /// unknown because their transport port does not identify an HTTP instance.
    pub(crate) fn identity_port(&self) -> Option<u16> {
        match self.protocol.as_str() {
            "http" => Some(self.port.unwrap_or(80)),
            "https" => Some(self.port.unwrap_or(443)),
            // Non-HTTP remotes (usually SSH) expose a transport port, not an API
            // port, so ignore self.port. Public forge hosts and their known SSH
            // aliases use HTTPS APIs on 443. Private hosts remain None: their
            // SSH URL cannot tell us which HTTP(S) port serves the API.
            _ => self.is_public_host().then_some(443),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "provider", rename_all = "lowercase", content = "details")]
pub enum ForgeUser {
    GitHub(but_github::GithubAccountIdentifier),
    GitLab(but_gitlab::GitlabAccountIdentifier),
    Bitbucket(but_bitbucket::BitbucketAccountIdentifier),
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeUser);

impl ForgeUser {
    pub fn github(&self) -> Option<&but_github::GithubAccountIdentifier> {
        match self {
            ForgeUser::GitHub(id) => Some(id),
            _ => None,
        }
    }
    pub fn gitlab(&self) -> Option<&but_gitlab::GitlabAccountIdentifier> {
        match self {
            ForgeUser::GitLab(id) => Some(id),
            _ => None,
        }
    }
    pub fn bitbucket(&self) -> Option<&but_bitbucket::BitbucketAccountIdentifier> {
        match self {
            ForgeUser::Bitbucket(id) => Some(id),
            _ => None,
        }
    }
    pub fn forge_name(&self) -> ForgeName {
        match self {
            ForgeUser::GitHub(_) => ForgeName::GitHub,
            ForgeUser::GitLab(_) => ForgeName::GitLab,
            ForgeUser::Bitbucket(_) => ForgeName::Bitbucket,
        }
    }
    /// The enterprise/self-hosted instance host, when the account has one.
    pub fn custom_host(&self) -> Option<String> {
        match self {
            ForgeUser::GitHub(id) => id.custom_host(),
            ForgeUser::GitLab(id) => id.custom_host(),
            ForgeUser::Bitbucket(id) => id.custom_host(),
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
