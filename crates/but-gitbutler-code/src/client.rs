//! The calls `but` makes on GitButler Code with an account's token.

use anyhow::{Context as _, Result};
use but_secret::Sensitive;
use reqwest::StatusCode;
use serde::Deserialize;

pub(crate) fn http() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("but/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Failed to set up an HTTP client")
}

/// A refusal from GitButler Code, with the reason it gave.
#[derive(Debug)]
pub struct HttpStatusError {
    pub status: StatusCode,
    pub message: String,
}

impl std::fmt::Display for HttpStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GitButler Code answered {}: {}",
            self.status, self.message
        )
    }
}

impl std::error::Error for HttpStatusError {}

pub(crate) fn is_unauthorized(err: &anyhow::Error) -> bool {
    err.downcast_ref::<HttpStatusError>()
        .is_some_and(|err| err.status == StatusCode::UNAUTHORIZED)
}

/// The user's project for an organization's project: their copy of it.
#[derive(Debug, Clone, Deserialize)]
pub struct Space {
    /// `<user>/<project>`.
    pub project: String,
    /// Its clone URL.
    pub url: String,
    /// The project it follows, `<org>/<project>`. `None` when the user's project of that name
    /// follows nothing.
    #[serde(default)]
    pub upstream: Option<String>,
    /// Whether this request made the copy.
    #[serde(default)]
    pub created: bool,
}

/// GitButler Code, signed in.
pub struct Client {
    base: String,
    token: Sensitive<String>,
}

impl Client {
    pub fn new(base: String, token: Sensitive<String>) -> Self {
        Self { base, token }
    }

    /// The signed-in user's username.
    pub async fn user(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct Identity {
            /// The username, under the name GitButler Code gives it on the wire.
            email: String,
        }
        let response = self.get("/.grit/user").await?;
        let identity: Identity = ok_json(response).await?;
        Ok(identity.email)
    }

    /// The signed-in user's copy of `<org>/<name>`, made on the server as a linked copy when they
    /// have none. `None` when `<org>/<name>` doesn't exist or the user can't read it.
    pub async fn space(&self, org: &str, name: &str) -> Result<Option<Space>> {
        let response = self
            .post(&format!("/.grit/space/{org}/{name}"), serde_json::json!({}))
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        ok_json(response).await.map(Some)
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response> {
        http()?
            .get(format!("{}{path}", self.base))
            .bearer_auth(&self.token.0)
            .send()
            .await
            .context("Failed to reach GitButler Code")
    }

    async fn post(&self, path: &str, body: serde_json::Value) -> Result<reqwest::Response> {
        http()?
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.token.0)
            .json(&body)
            .send()
            .await
            .context("Failed to reach GitButler Code")
    }
}

async fn ok(response: reqwest::Response) -> Result<reqwest::Response> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(status_error(response).await.into())
    }
}

async fn ok_json<T: serde::de::DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    ok(response)
        .await?
        .json()
        .await
        .context("GitButler Code answered with something unexpected")
}

/// GitButler Code's APIs answer `{"error": "..."}`; Git's endpoints answer plain text.
async fn status_error(response: reqwest::Response) -> HttpStatusError {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|json| json.get("error")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| text.trim().to_owned());
    HttpStatusError { status, message }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, BufReader, Read as _, Write as _};
    use std::net::TcpListener;

    use super::*;

    /// A server that answers one request with `status` and `body`, at the returned base URL.
    fn answer_once(status: &'static str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            reader.read_exact(&mut vec![0; length]).unwrap();
            let mut stream = reader.into_inner();
            write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        base
    }

    fn client(base: String) -> Client {
        Client::new(base, Sensitive("token".into()))
    }

    #[tokio::test]
    async fn space_of_a_missing_project_is_none() {
        let base = answer_once("404 Not Found", "project not found\n");
        let space = client(base)
            .space("gitbutler.com", "nothing")
            .await
            .unwrap();
        assert!(
            space.is_none(),
            "a 404 means the organization's project is missing or unreadable"
        );
    }

    #[tokio::test]
    async fn space_names_the_copy_and_what_it_follows() {
        let base = answer_once(
            "200 OK",
            r#"{"project":"juan/gitbutler","url":"https://code.gitbutler.com/juan/gitbutler.git","upstream":"gitbutler.com/gitbutler","can_integrate":true,"created":true,"followed":["main"]}"#,
        );
        let space = client(base)
            .space("gitbutler.com", "gitbutler")
            .await
            .unwrap()
            .expect("the copy exists");
        assert_eq!(
            (
                space.project.as_str(),
                space.upstream.as_deref(),
                space.created
            ),
            ("juan/gitbutler", Some("gitbutler.com/gitbutler"), true),
            "the reply names the copy, the project it follows, and whether it was just made"
        );
    }

    #[tokio::test]
    async fn a_revoked_token_reads_as_unauthorized() {
        let base = answer_once("401 Unauthorized", "an access token is required\n");
        let err = client(base).user().await.unwrap_err();
        assert!(
            is_unauthorized(&err),
            "a 401 must be told apart so callers ask the user to sign in again"
        );
    }
}
