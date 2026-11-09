#![cfg_attr(
    not(unix),
    allow(dead_code, reason = "only the unix socket hands requests over")
)]
//! The control socket: other processes on the machine, as a coding agent, control the app
//! through it. It speaks JSON-RPC 2.0 a line at a time, as `but_ui_control` describes, at
//! `GITBUTLER_GPUI_SOCKET` or `gpui.sock` in the app data dir. Only the user can reach it.
//!
//! Each connection has a thread of its own, which reads a request, hands it to the UI thread
//! and writes the answer once it's given, so a request that waits holds up only its own
//! connection. Nothing here touches the app's state off the UI thread. Each area of methods is
//! routed in [`dispatch`], as `ui.*` to [`crate::app::ui_control`].

use std::path::PathBuf;

use but_ui_control::{Request, Response, RpcError, code};
use gpui::{App, Global, Window};
use serde::Serialize;

/// Where the app listens, once it does: what its terminals are told.
pub struct ControlSocket(pub PathBuf);

impl Global for ControlSocket {}

/// The answer to one request, given once from the UI thread, now or later.
pub struct Reply {
    id: Option<u64>,
    answer: futures::channel::oneshot::Sender<Response>,
}

impl Reply {
    /// A reply to the request `id`, and where its answer arrives.
    pub fn new(id: Option<u64>) -> (Reply, futures::channel::oneshot::Receiver<Response>) {
        let (answer, answered) = futures::channel::oneshot::channel();
        (Reply { id, answer }, answered)
    }

    pub fn ok(self, result: impl Serialize) {
        self.answer.send(Response::ok(self.id, result)).ok();
    }

    pub fn err(self, error: RpcError) {
        self.answer.send(Response::err(self.id, error)).ok();
    }
}

/// What the socket's threads hand the UI thread.
enum Incoming {
    /// The socket is bound, there.
    Listening(PathBuf),
    Request(Request, Reply),
}

/// Take requests on the control socket for as long as the app runs, for `window`. Where
/// another app already listens there, this one doesn't.
#[cfg(unix)]
pub fn listen(window: gpui::AnyWindowHandle, cx: &mut App) {
    use futures::StreamExt as _;
    use gpui::AppContext as _;

    let path = match but_ui_control::socket_path() {
        Ok(path) => path,
        Err(err) => return tracing::warn!("no control socket: {err:#}"),
    };
    let (send, mut incoming) = futures::channel::mpsc::unbounded::<Incoming>();
    // Binding touches the disk, so it's done off the UI thread too.
    std::thread::spawn(move || {
        let listener = match crate::backend::control::bind(&path) {
            Ok(listener) => listener,
            Err(err) => return tracing::warn!("no control socket at {}: {err:#}", path.display()),
        };
        tracing::info!("control socket at {}", path.display());
        if send.unbounded_send(Incoming::Listening(path)).is_err() {
            return;
        }
        for stream in listener.incoming() {
            let Ok(stream) = stream else {
                continue;
            };
            let send = send.clone();
            std::thread::spawn(move || serve(stream, send));
        }
    });
    cx.spawn(async move |cx| {
        while let Some(incoming) = incoming.next().await {
            cx.update_window(window, |_, window, cx| dispatch(incoming, window, cx))
                .ok();
        }
    })
    .detach();
}

/// Answer the requests on one connection in turn, until it closes.
#[cfg(unix)]
fn serve(
    stream: std::os::unix::net::UnixStream,
    send: futures::channel::mpsc::UnboundedSender<Incoming>,
) {
    use std::io::{BufRead as _, BufReader, Write as _};

    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut write = |response: &Response| -> bool {
        let Ok(mut line) = serde_json::to_string(response) else {
            return false;
        };
        line.push('\n');
        writer.write_all(line.as_bytes()).is_ok()
    };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        let request = match parse(&line) {
            Ok(request) => request,
            Err(response) => {
                if !write(&response) {
                    break;
                }
                continue;
            }
        };
        let (reply, answered) = Reply::new(request.id);
        if send
            .unbounded_send(Incoming::Request(request.clone(), reply))
            .is_err()
        {
            break;
        }
        // A request the app dropped unanswered, as one asked while it quits.
        let response = futures::executor::block_on(answered).unwrap_or_else(|_| {
            Response::err(
                request.id,
                RpcError::new(code::INVALID_REQUEST, "The app stopped before answering"),
            )
        });
        // A notification gets no answer.
        if request.id.is_none() {
            continue;
        }
        if !write(&response) {
            break;
        }
    }
}

/// `line` as a request, or the error response saying why it isn't one.
fn parse(line: &str) -> Result<Request, Response> {
    let value: serde_json::Value = serde_json::from_str(line).map_err(|err| {
        Response::err(
            None,
            RpcError::new(code::PARSE_ERROR, format!("Not JSON: {err}")),
        )
    })?;
    let id = value.get("id").and_then(serde_json::Value::as_u64);
    let invalid = |why: String| Response::err(id, RpcError::new(code::INVALID_REQUEST, why));
    let request: Request =
        serde_json::from_value(value).map_err(|err| invalid(format!("Not a request: {err}")))?;
    if request.jsonrpc != but_ui_control::VERSION {
        return Err(invalid(format!(
            "Only JSON-RPC {} is spoken here",
            but_ui_control::VERSION
        )));
    }
    Ok(request)
}

/// Act on what a connection handed over, on the UI thread.
fn dispatch(incoming: Incoming, window: &mut Window, cx: &mut App) {
    match incoming {
        Incoming::Listening(path) => cx.set_global(ControlSocket(path)),
        Incoming::Request(request, reply) if request.method.starts_with("ui.") => {
            crate::app::ui_control::handle(request, reply, window, cx)
        }
        // The window the user looks at, which an agent can lay out as asked in a dev build.
        #[cfg(feature = "dev-script")]
        Incoming::Request(request, reply) if request.method == "dev.layout" => {
            crate::dev::layout(request, reply, window, cx)
        }
        Incoming::Request(request, reply) => reply.err(RpcError::new(
            code::METHOD_NOT_FOUND,
            format!("The app has no method {}", request.method),
        )),
    }
}

#[cfg(test)]
mod tests {
    use but_ui_control::code;

    use super::parse;

    #[test]
    fn a_line_that_isnt_a_request_gets_an_error_with_its_id() {
        let not_json = parse("{oops").unwrap_err();
        assert_eq!(
            not_json.error.unwrap().code,
            code::PARSE_ERROR,
            "a broken line isn't JSON"
        );
        assert_eq!(not_json.id, None, "and has no id to answer");

        let no_method = parse(r#"{"jsonrpc":"2.0","id":7}"#).unwrap_err();
        assert_eq!(
            no_method.error.unwrap().code,
            code::INVALID_REQUEST,
            "a request names a method"
        );
        assert_eq!(
            no_method.id,
            Some(7),
            "the answer carries the id it could read"
        );

        let old = parse(r#"{"jsonrpc":"1.0","id":3,"method":"app.ping"}"#).unwrap_err();
        assert_eq!(
            old.error.unwrap().code,
            code::INVALID_REQUEST,
            "only 2.0 is spoken"
        );

        let fine = parse(r#"{"jsonrpc":"2.0","id":1,"method":"app.ping","params":{}}"#);
        assert_eq!(fine.unwrap().method, "app.ping", "a request reads as one");
    }
}
