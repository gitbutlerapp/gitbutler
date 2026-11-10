//! The GitButler GPUI app's control socket: JSON-RPC 2.0, one message per line.

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub mod ui;

pub const SOCKET_ENV: &str = "GITBUTLER_GPUI_SOCKET";

pub fn socket_path() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os(SOCKET_ENV).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    Ok(but_path::app_data_dir()?.join("gpui.sock"))
}

pub const VERSION: &str = "2.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    pub fn ok(id: Option<u64>, result: impl Serialize) -> Response {
        Response {
            jsonrpc: VERSION.into(),
            id,
            result: Some(serde_json::to_value(result).unwrap_or(serde_json::Value::Null)),
            error: None,
        }
    }

    pub fn err(id: Option<u64>, error: RpcError) -> Response {
        Response {
            jsonrpc: VERSION.into(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
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

/// JSON-RPC's error codes, then the app's.
pub mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
    pub const NOT_FOUND: i64 = -32001;
    pub const NOT_AVAILABLE: i64 = -32003;
}

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
    pub fn connect() -> anyhow::Result<Client> {
        Client::connect_to(&socket_path()?)
    }

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

    #[cfg(not(unix))]
    pub fn connect_to(_path: &Path) -> anyhow::Result<Client> {
        bail!("Controlling the GitButler GPUI app is only supported on macOS and Linux")
    }

    /// A JSON-RPC error comes back as an [`RpcError`], which callers can downcast.
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
