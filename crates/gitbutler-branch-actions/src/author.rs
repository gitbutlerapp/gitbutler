use bstr::ByteSlice;
use serde::Serialize;

use crate::gravatar::gravatar_url_from_email;

#[derive(Debug, Serialize, Hash, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub name: String,
    pub email: String,
    pub gravatar_url: url::Url,
}

impl From<git2::Signature<'_>> for Author {
    fn from(value: git2::Signature) -> Self {
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
