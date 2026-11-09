//! Controlling a running GitButler GPUI app from another process, as a coding agent's.
//!
//! The app listens on a Unix socket only its user can reach. A client finds it at
//! [`SOCKET_ENV`] when that's set, as it is in the app's own terminals, and otherwise at
//! `gpui.sock` in the app data directory of the channel the client was built for
//! ([`socket_path`]).
//!
//! Messages are JSON-RPC 2.0, one per line ([`Request`], [`Response`]). A connection carries
//! any number of requests, each answered in turn. Methods are named by area, as `area.method`,
//! and each area's names, parameters and results live in a module of its own.
#![deny(missing_docs)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// The environment variable naming the socket, which wins over the default place.
pub const SOCKET_ENV: &str = "GITBUTLER_GPUI_SOCKET";

/// Where the app listens: [`SOCKET_ENV`] when it's set, else `gpui.sock` in the app data
/// directory of this build's channel.
pub fn socket_path() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os(SOCKET_ENV).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    Ok(but_path::app_data_dir()?.join("gpui.sock"))
}

/// The JSON-RPC version every message names.
pub const VERSION: &str = "2.0";

/// A call, or with no `id` a notification, which gets no response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// Always [`VERSION`].
    pub jsonrpc: String,
    /// What the response carries back; `None` for a notification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// The method, as `walkthrough.show`.
    pub method: String,
    /// The method's parameters.
    #[serde(default)]
    pub params: serde_json::Value,
}

/// The answer to the [`Request`] with the same `id`: its result or an error, never both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// Always [`VERSION`].
    pub jsonrpc: String,
    /// The request's id, or `None` when the request couldn't be read far enough to have one.
    pub id: Option<u64>,
    /// What the method returned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Why the method failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    /// A response carrying `result`.
    pub fn ok(id: Option<u64>, result: impl Serialize) -> Response {
        Response {
            jsonrpc: VERSION.into(),
            id,
            result: Some(serde_json::to_value(result).unwrap_or(serde_json::Value::Null)),
            error: None,
        }
    }

    /// A response carrying `error`.
    pub fn err(id: Option<u64>, error: RpcError) -> Response {
        Response {
            jsonrpc: VERSION.into(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// A JSON-RPC error object. `message` is a sentence the caller can act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    /// One of [`code`]'s.
    pub code: i64,
    /// What went wrong, and when it helps, what to do.
    pub message: String,
}

impl RpcError {
    /// An error with `code` and `message`.
    pub fn new(code: i64, message: impl Into<String>) -> RpcError {
        RpcError {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RpcError {}

/// The error codes responses use: JSON-RPC's own, then the app's.
pub mod code {
    /// The line wasn't JSON.
    pub const PARSE_ERROR: i64 = -32700;
    /// The JSON wasn't a request.
    pub const INVALID_REQUEST: i64 = -32600;
    /// No such method.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// The parameters don't fit the method.
    pub const INVALID_PARAMS: i64 = -32602;
    /// The app failed at something it should have managed, as reading a diff.
    pub const INTERNAL_ERROR: i64 = -32603;
    /// What the request names isn't there, as a repository the app hasn't open.
    pub const NOT_FOUND: i64 = -32001;
}

/// A connection to the app, which sends requests one at a time and waits for each answer.
pub struct Client {
    reader: BufReader<Stream>,
    writer: Stream,
    next_id: u64,
}

#[cfg(unix)]
type Stream = std::os::unix::net::UnixStream;
#[cfg(not(unix))]
type Stream = std::fs::File;

impl Client {
    /// Connect to the app at [`socket_path`].
    pub fn connect() -> anyhow::Result<Client> {
        Client::connect_to(&socket_path()?)
    }

    /// Connect to the app listening at `path`. Fails saying the app isn't running when
    /// nothing listens there.
    #[cfg(unix)]
    pub fn connect_to(path: &Path) -> anyhow::Result<Client> {
        let stream = Stream::connect(path).map_err(|err| {
            use std::io::ErrorKind;
            match err.kind() {
                ErrorKind::NotFound | ErrorKind::ConnectionRefused => anyhow::anyhow!(
                    "The GitButler GPUI app isn't running: nothing listens at {}. Start it, or set {SOCKET_ENV} to where it listens.",
                    path.display()
                ),
                _ => anyhow::Error::from(err)
                    .context(format!("Couldn't reach the GitButler GPUI app at {}", path.display())),
            }
        })?;
        let writer = stream
            .try_clone()
            .context("Couldn't share the connection")?;
        Ok(Client {
            reader: BufReader::new(stream),
            writer,
            next_id: 1,
        })
    }

    /// Controlling the app isn't supported here yet.
    #[cfg(not(unix))]
    pub fn connect_to(_path: &Path) -> anyhow::Result<Client> {
        bail!("Controlling the GitButler GPUI app is only supported on macOS and Linux")
    }

    /// Call `method` with `params` and wait for its result. A JSON-RPC error comes back as an
    /// [`RpcError`] in the returned error, so callers can read its code.
    pub fn call<R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: impl Serialize,
    ) -> anyhow::Result<R> {
        let id = self.next_id;
        self.next_id += 1;
        let request = Request {
            jsonrpc: VERSION.into(),
            id: Some(id),
            method: method.into(),
            params: serde_json::to_value(params)?,
        };
        let mut line = serde_json::to_string(&request)?;
        line.push('\n');
        self.writer
            .write_all(line.as_bytes())
            .context("Couldn't send to the GitButler GPUI app")?;
        let mut answer = String::new();
        let read = self
            .reader
            .read_line(&mut answer)
            .context("Couldn't read the GitButler GPUI app's answer")?;
        if read == 0 {
            bail!("The GitButler GPUI app closed the connection before answering");
        }
        let response: Response = serde_json::from_str(&answer)
            .context("The GitButler GPUI app answered with something that isn't JSON-RPC")?;
        if response.id != Some(id) {
            bail!("The GitButler GPUI app answered another request");
        }
        if let Some(error) = response.error {
            return Err(error.into());
        }
        Ok(serde_json::from_value(
            response.result.unwrap_or(serde_json::Value::Null),
        )?)
    }
}
