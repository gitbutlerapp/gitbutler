//! This code is a fork of [`gitbutler_branch_actions::author`] to avoid depending on the `gitbutler_branch_actions` crate.
use anyhow::Result;
use bstr::ByteSlice;
use serde::Serialize;

/// Represents the author of a commit.
#[derive(Debug, Serialize, Hash, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    /// The name from the git commit signature
    pub name: String,
    /// The email from the git commit signature
    pub email: String,
    /// A URL to a gravatar image for the email from the commit signature
    pub gravatar_url: url::Url,
}

impl From<git2::Signature<'_>> for Author {
    fn from(value: git2::Signature<'_>) -> Self {
        let name = value.name().unwrap_or_default().to_string();
        let email = value.email().unwrap_or_default().to_string();

        let gravatar_url = gravatar_url_from_email(email.as_str()).unwrap();

        Author {
            name,
            email,
            gravatar_url,
        }
    }
}

impl From<gix::actor::SignatureRef<'_>> for Author {
    fn from(value: gix::actor::SignatureRef<'_>) -> Self {
        let gravatar_url = gravatar_url_from_email(&value.email.to_str_lossy()).unwrap();

        Author {
            name: value.name.to_owned().to_string(),
            email: value.email.to_owned().to_string(),
            gravatar_url,
        }
    }
}

pub fn gravatar_url_from_email(email: &str) -> Result<url::Url> {
    let gravatar_url = format!(
        "https://www.gravatar.com/avatar/{:x}?s=100&r=g&d=retro",
        md5::compute(email.to_lowercase())
    );
    url::Url::parse(gravatar_url.as_str()).map_err(Into::into)
}
