#![cfg_attr(
    not(unix),
    allow(dead_code, reason = "only the unix socket hands requests over")
)]
//! Each connection has its own thread, so a request that waits holds up only its connection.

use std::path::PathBuf;

use but_ui_control::{Request, Response, RpcError, code};
use gpui::{App, Global, Window};
use serde::Serialize;

pub struct ControlSocket(pub PathBuf);

impl Global for ControlSocket {}

pub struct Reply {
    id: Option<u64>,
    answer: futures::channel::oneshot::Sender<Response>,
}

impl Reply {
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

enum Incoming {
    Listening(PathBuf),
    Request(Request, Reply),
}

#[cfg(unix)]
pub fn listen(window: gpui::AnyWindowHandle, cx: &mut App) {
    use futures::StreamExt as _;
    use gpui::AppContext as _;

    let path = match but_ui_control::socket_path() {
        Ok(path) => path,
        Err(err) => return tracing::warn!("no control socket: {err:#}"),
    };
    let (send, mut incoming) = futures::channel::mpsc::unbounded::<Incoming>();
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
        let response = futures::executor::block_on(answered).unwrap_or_else(|_| {
            Response::err(
                request.id,
                RpcError::new(code::INVALID_REQUEST, "The app stopped before answering"),
            )
        });
        if request.id.is_none() {
            continue;
        }
        if !write(&response) {
            break;
        }
    }
}

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

fn dispatch(incoming: Incoming, window: &mut Window, cx: &mut App) {
    match incoming {
        Incoming::Listening(path) => cx.set_global(ControlSocket(path)),
        Incoming::Request(request, reply) if request.method.starts_with("ui.") => {
            crate::app::ui_control::handle(request, reply, window, cx)
        }
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
