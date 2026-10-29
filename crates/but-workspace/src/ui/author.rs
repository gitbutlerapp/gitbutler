use bstr::ByteSlice;
use serde::Serialize;

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
    let gravatar_url = format!(
        "https://www.gravatar.com/avatar/{:x}?s=100&r=g&d=retro",
        md5::compute(email.to_lowercase())
    );
    url::Url::parse(gravatar_url.as_str()).expect("an MD5 as part of the URl is always valid")
}
