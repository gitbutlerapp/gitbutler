use serde::Serialize;

/// Struct for exposing remote information to the front end.
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRemote {
    pub name: Option<String>,
    pub url: Option<String>,
}

impl GitRemote {
    pub fn from_gix(name: String, remote: &gix::Remote<'_>) -> Self {
        GitRemote {
            name: Some(name),
            url: remote
                .url(gix::remote::Direction::Push)
                .or_else(|| remote.url(gix::remote::Direction::Fetch))
                .map(|url| url.to_bstring().to_string()),
        }
    }
}
