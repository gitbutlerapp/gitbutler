//! The socket the GUI listens at while it runs, so the CLI, and agents through it, can talk to
//! it: one line of JSON-RPC 2.0 a request, answered by one line. A request without an `id` is a
//! notification and gets no answer, as when a second launch hands the running app its links.
//!
//! The app listens at [`socket_path`]. Its terminals set [`SOCKET_ENV`] to it, so an agent
//! running in one reaches that app, whichever channel's CLI it runs.
//!
//! What the app is asked, and answers, is the [`method`]s' types here, which the app and the CLI
//! both build with.

use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, bail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AppChannel;

/// Set in the app's terminals to the socket of the app they run in.
pub const SOCKET_ENV: &str = "GITBUTLER_APP_SOCKET";

/// Where `channel`'s app listens: a Unix socket in its data directory, or on Windows a named
/// pipe named after it, as pipe names are machine-wide.
pub fn socket_path(channel: AppChannel) -> anyhow::Result<PathBuf> {
    let dir = crate::app_data_dir_for_channel(channel)?;
    if cfg!(windows) {
        let id = (dir.to_string_lossy())
            .replace(['\\', '/', ':'], "-")
            .to_lowercase();
        return Ok(PathBuf::from(format!(r"\\.\pipe\gitbutler-app-{id}")));
    }
    Ok(dir.join("gpui.sock"))
}

/// Where a client finds the app: [`SOCKET_ENV`] in the app's terminals, else `channel`'s socket.
fn address(channel: AppChannel) -> anyhow::Result<PathBuf> {
    match std::env::var_os(SOCKET_ENV).filter(|value| !value.is_empty()) {
        Some(path) => Ok(PathBuf::from(path)),
        None => socket_path(channel),
    }
}

/// The methods the app answers, with the types of what they take and give.
pub mod method {
    /// Go to a place, [`super::Open`], answering [`super::Opened`] once it's there or isn't.
    pub const OPEN: &str = "open";
    /// What the user is looking at, [`super::View`]. Takes nothing.
    pub const VIEW: &str = "view";
    /// The next of what the user tells agents, [`super::Wait`], answering [`super::Waited`].
    pub const WAIT: &str = "wait";
    /// Bring the window forward, as a launch without a link does. Takes nothing.
    pub const SHOW: &str = "show";
}

/// A place in a project, by a link's own parameters past its path, as `to=branch` and
/// `branch=refs/heads/x`, in order. It goes over the socket as a JSON object of strings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Place(pub Vec<(String, String)>);

impl Serialize for Place {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Place {
    /// In the object's order, which `serde_json::Map` would sort. A number, as `"line": 4`,
    /// reads as its digits, as an agent may well write one.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Pairs;

        impl<'de> serde::de::Visitor<'de> for Pairs {
            type Value = Place;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object of strings")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Place, A::Error> {
                let mut pairs = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    let value = match value {
                        Value::String(value) => value,
                        Value::Number(number) => number.to_string(),
                        other => {
                            return Err(serde::de::Error::custom(format!(
                                "`{key}` is {other}, where a place takes strings"
                            )));
                        }
                    };
                    pairs.push((key, value));
                }
                Ok(Place(pairs))
            }
        }

        deserializer.deserialize_map(Pairs)
    }
}

/// What [`method::OPEN`] takes: the project at `path`, and the place `at` in it, with `say`
/// shown beside it for the user to read, as a step of a walkthrough.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Open {
    pub path: PathBuf,
    #[serde(default)]
    pub new_window: bool,
    #[serde(default)]
    pub at: Place,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub say: Option<String>,
}

/// What [`method::OPEN`] answers: whether the window went where it was asked, and if not,
/// why, as the app told the user, with what it shows now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opened {
    pub landed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    pub view: View,
}

/// What the user is looking at: the project's directory, the place in it as a link names it,
/// and how the window titles it. Nothing when no project is open.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub at: Place,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// What [`method::WAIT`] takes: the events after `after`, or with none, after the last one
/// so far, waiting for one up to `timeout_secs`, or for as long as it takes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wait {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

/// What [`method::WAIT`] answers: the events after the one asked for, none if it timed out,
/// and the last event's number, to wait after next.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waited {
    pub events: Vec<Event>,
    pub last: u64,
}

/// Something the user told agents in the app, numbered in order from the app's start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    #[serde(flatten)]
    pub kind: EventKind,
    /// Where the user was, as [`View`] says.
    pub view: View,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    /// The user asked agents about what they're looking at.
    Asked { note: String },
    /// The user is ready for the next step of a walkthrough.
    Next,
    /// The user ended a walkthrough.
    Done,
}

/// A request or a notification, as a line on the socket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    /// `None` for a notification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// The answer to a request, as a line on the socket: its `result`, or an `error`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: i64,
    pub message: String,
}

/// JSON-RPC's codes for what the app can't take: a line that isn't JSON, a method it doesn't
/// have, parameters that don't fit it, and anything else that failed.
pub mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
}

impl Response {
    pub fn result(id: Option<u64>, result: Value) -> Self {
        Response {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<u64>, code: i64, message: impl Into<String>) -> Self {
        Response {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(ResponseError {
                code,
                message: message.into(),
            }),
        }
    }
}

/// Ask `channel`'s running app `method` with `params`, waiting up to `timeout` for its answer
/// (for ever with none), or `None` when no app listens.
pub fn call<P: Serialize, R: DeserializeOwned>(
    channel: AppChannel,
    method: &str,
    params: &P,
    timeout: Option<Duration>,
) -> anyhow::Result<Option<R>> {
    call_at(&address(channel)?, method, params, timeout)
}

/// [`call`] the app listening at `address`.
pub fn call_at<P: Serialize, R: DeserializeOwned>(
    address: &Path,
    method: &str,
    params: &P,
    timeout: Option<Duration>,
) -> anyhow::Result<Option<R>> {
    let Some(mut stream) = transport::connect(address)? else {
        return Ok(None);
    };
    transport::set_read_timeout(&stream, timeout)?;
    let request = Request {
        jsonrpc: "2.0".into(),
        id: Some(1),
        method: method.into(),
        params: serde_json::to_value(params)?,
    };
    send_line(&mut stream, &request)?;
    let mut line = String::new();
    BufReader::new(&mut stream)
        .read_line(&mut line)
        .with_context(|| format!("GitButler didn't answer `{method}`"))?;
    if line.is_empty() {
        bail!("GitButler closed the connection before answering `{method}`");
    }
    let response: Response = serde_json::from_str(&line)
        .with_context(|| format!("GitButler's answer to `{method}` isn't JSON-RPC"))?;
    if let Some(error) = response.error {
        bail!("{}", error.message);
    }
    let result = response.result.unwrap_or(Value::Null);
    serde_json::from_value(result)
        .map(Some)
        .with_context(|| format!("GitButler's answer to `{method}` isn't what was expected"))
}

/// Tell `channel`'s running app `method` with `params`, a notification it doesn't answer, or
/// say `false` when no app listens.
pub fn notify<P: Serialize>(channel: AppChannel, method: &str, params: &P) -> anyhow::Result<bool> {
    notify_at(&socket_path(channel)?, method, params)
}

/// [`notify`] the app listening at `address`.
pub fn notify_at<P: Serialize>(address: &Path, method: &str, params: &P) -> anyhow::Result<bool> {
    let Some(mut stream) = transport::connect(address)? else {
        return Ok(false);
    };
    let request = Request {
        jsonrpc: "2.0".into(),
        id: None,
        method: method.into(),
        params: serde_json::to_value(params)?,
    };
    send_line(&mut stream, &request)?;
    Ok(true)
}

/// Write `value` as one line of JSON.
pub fn send_line(writer: &mut impl std::io::Write, value: &impl Serialize) -> anyhow::Result<()> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    writer.write_all(&line)?;
    writer.flush()?;
    Ok(())
}

#[cfg(unix)]
mod transport {
    use std::io;
    use std::os::unix::net::UnixStream;
    use std::path::Path;
    use std::time::Duration;

    pub type Stream = UnixStream;

    /// The app's socket, or `None` where none listens: no file, or one an app that quit left.
    pub fn connect(address: &Path) -> anyhow::Result<Option<Stream>> {
        match UnixStream::connect(address) {
            Ok(stream) => Ok(Some(stream)),
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) =>
            {
                Ok(None)
            }
            Err(err) => Err(err.into()),
        }
    }

    pub fn set_read_timeout(stream: &Stream, timeout: Option<Duration>) -> io::Result<()> {
        stream.set_read_timeout(timeout)
    }
}

#[cfg(windows)]
mod transport {
    use std::fs::File;
    use std::io;
    use std::path::Path;
    use std::time::{Duration, Instant};

    pub type Stream = File;

    /// `ERROR_PIPE_BUSY`: every instance of the pipe is taken, until the app makes another.
    const PIPE_BUSY: i32 = 231;

    /// The app's pipe, or `None` where none listens. A busy pipe is tried again for a while.
    pub fn connect(address: &Path) -> anyhow::Result<Option<Stream>> {
        let started = Instant::now();
        loop {
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(address)
            {
                Ok(pipe) => return Ok(Some(pipe)),
                Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(err)
                    if err.raw_os_error() == Some(PIPE_BUSY)
                        && started.elapsed() < Duration::from_secs(2) =>
                {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(err) => return Err(err.into()),
            }
        }
    }

    /// A pipe opened as a file has no timeout to set: its reads wait for the app.
    pub fn set_read_timeout(_stream: &Stream, _timeout: Option<Duration>) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_place_goes_as_an_object_and_keeps_its_order() -> anyhow::Result<()> {
        let place = Place(vec![
            ("to".into(), "branch".into()),
            ("branch".into(), "refs/heads/a & b".into()),
            ("file".into(), "src/a.rs".into()),
        ]);
        let line = serde_json::to_string(&place)?;
        assert_eq!(
            line, r#"{"to":"branch","branch":"refs/heads/a & b","file":"src/a.rs"}"#,
            "an agent reads and writes it as an object, in the place's order"
        );
        assert_eq!(
            serde_json::from_str::<Place>(&line)?,
            place,
            "it reads back as written, in order, as it comes off the socket"
        );
        Ok(())
    }

    #[test]
    fn a_place_reads_a_number_as_its_digits_and_refuses_other_values() {
        let place: Place = serde_json::from_value(json!({"line": 4})).expect("a number is fine");
        assert_eq!(
            place,
            Place(vec![("line".into(), "4".into())]),
            "an agent may write a line as a number"
        );
        assert!(
            serde_json::from_value::<Place>(json!({"line": [4]})).is_err(),
            "a list is no value of a place"
        );
    }

    #[test]
    fn an_event_says_its_kind_beside_its_number() -> anyhow::Result<()> {
        let event = Event {
            seq: 3,
            kind: EventKind::Asked {
                note: "why is this here?".into(),
            },
            view: View {
                path: Some("/repo".into()),
                at: Place(vec![("to".into(), "commit".into())]),
                title: None,
            },
        };
        let value = serde_json::to_value(&event)?;
        assert_eq!(
            value,
            json!({
                "seq": 3,
                "kind": "asked",
                "note": "why is this here?",
                "view": {"path": "/repo", "at": {"to": "commit"}},
            }),
            "flat, as an agent reads it"
        );
        assert_eq!(serde_json::from_value::<Event>(value)?, event);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_call_gets_the_apps_answer_and_none_without_an_app() -> anyhow::Result<()> {
        use std::os::unix::net::UnixListener;

        let dir = tempfile::tempdir()?;
        let socket = dir.path().join("app.sock");
        let no_app: Option<View> = call_at(&socket, method::VIEW, &json!({}))?;
        assert_eq!(no_app, None, "nothing listens there");

        let listener = UnixListener::bind(&socket)?;
        let app = std::thread::spawn(move || -> anyhow::Result<Request> {
            let (stream, _) = listener.accept()?;
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line)?;
            let request: Request = serde_json::from_str(&line)?;
            let view = View {
                path: Some("/repo".into()),
                ..View::default()
            };
            send_line(
                &mut &stream,
                &Response::result(request.id, serde_json::to_value(view)?),
            )?;
            Ok(request)
        });
        let view: Option<View> = call_at(&socket, method::VIEW, &json!({}))?;
        let request = app.join().expect("the app's thread")?;
        assert_eq!(request.method, "view", "the app is asked by name");
        assert_eq!(
            view.and_then(|view| view.path),
            Some(PathBuf::from("/repo")),
            "the answer reads as the method's type"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn an_error_from_the_app_is_the_calls_error() -> anyhow::Result<()> {
        use std::os::unix::net::UnixListener;

        let dir = tempfile::tempdir()?;
        let socket = dir.path().join("app.sock");
        let listener = UnixListener::bind(&socket)?;
        let app = std::thread::spawn(move || -> anyhow::Result<()> {
            let (stream, _) = listener.accept()?;
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line)?;
            let request: Request = serde_json::from_str(&line)?;
            send_line(
                &mut &stream,
                &Response::error(request.id, code::METHOD_NOT_FOUND, "No method `nope`"),
            )
        });
        let answer = call_at::<_, Value>(&socket, "nope", &json!({}));
        app.join().expect("the app's thread")?;
        assert_eq!(
            answer.map_err(|err| err.to_string()),
            Err("No method `nope`".to_owned()),
            "the app's message is what the CLI says"
        );
        Ok(())
    }

    /// [`super::call_at`] with a timeout, so a test that goes wrong fails rather than hangs.
    #[cfg(unix)]
    fn call_at<P: Serialize, R: DeserializeOwned>(
        address: &Path,
        method: &str,
        params: &P,
    ) -> anyhow::Result<Option<R>> {
        super::call_at(address, method, params, Some(Duration::from_secs(5)))
    }
}
