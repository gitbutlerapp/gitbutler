use bstr::ByteSlice;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Represents the author of a commit.
#[derive(Serialize, Hash, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct Author {
    /// The name from the git commit signature
    pub name: String,
    /// The email from the git commit signature
    pub email: String,
    /// A URL to a gravatar image for the email from the commit signature
    #[cfg_attr(feature = "export-schema", schemars(schema_with = "but_schemars::url"))]
    pub gravatar_url: url::Url,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(Author);

impl std::fmt::Debug for Author {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} <{}>", self.name, self.email)
    }
}

impl From<gix::actor::SignatureRef<'_>> for Author {
    fn from(value: gix::actor::SignatureRef<'_>) -> Self {
        let gravatar_url = gravatar_url_from_email(&value.email.to_str_lossy());

        Author {
            name: value.name.to_string(),
            email: value.email.to_string(),
            gravatar_url,
        }
    }
}

impl From<gix::actor::Signature> for Author {
    fn from(value: gix::actor::Signature) -> Self {
        let gravatar_url = gravatar_url_from_email(&value.email.to_str_lossy());

        Author {
            name: value.name.to_string(),
            email: value.email.to_string(),
            gravatar_url,
        }
    }
}

pub fn gravatar_url_from_email(email: &str) -> url::Url {
    let email_hash = Sha256::digest(email.trim().to_lowercase().as_bytes());
    let gravatar_url = format!(
        "https://www.gravatar.com/avatar/{:x}?s=100&r=g&d=retro",
        email_hash
    );
    url::Url::parse(gravatar_url.as_str())
        .expect("a SHA-256 hash as part of the URL is always valid")
}

#[cfg(test)]
mod tests {
    use super::gravatar_url_from_email;

    #[test]
    fn gravatar_uses_sha256_and_normalized_email() {
        assert_eq!(
            gravatar_url_from_email(" Author@example.com ").as_str(),
            "https://www.gravatar.com/avatar/b0eda69977c26118feff17875d53376006568bcbcde5ca0c916d01f05c281436?s=100&r=g&d=retro",
            "gravatar hashes should match the normalized SHA-256 email used by Gravatar"
        );
    }
}
