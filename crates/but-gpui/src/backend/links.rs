//! `but://` links and requests from outside the app: how they reach it. What a link names is
//! `crate::links`'s to read, and what a request asks `crate::agents`'s to answer.
//!
//! The app listens on a socket of its own (`unix`, `windows`) for one line of JSON-RPC a
//! request, answered by one line, as [`but_path::app`] says: the CLI asks it there, for agents.
//! macOS hands the running app its links itself. Windows and Linux launch the app again with
//! the link as an argument, so that launch hands its links to the app it finds listening, as
//! notifications, and quits. Dev builds hand nothing over, so two can run side by side.

use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use but_path::AppChannel;
use but_path::app::{self, Open, Place, Response, method};
use but_path::link::OpenLink;
use futures::channel::{mpsc, oneshot};
use parking_lot::Mutex;
use serde::Serialize;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
// `self::`, as the `windows` crate has the same name.
#[cfg(windows)]
use self::windows as platform;

/// What reaches the app from outside.
#[derive(Debug)]
pub enum Incoming {
    Link(String),
    /// A request on the socket, answered through its reply. A launch without a link asks to
    /// `show` the window.
    Request(Request),
}

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub params: serde_json::Value,
    pub reply: Reply,
}

/// Where a request's answer goes, once: the connection that asked waits on it. A
/// notification's goes nowhere. A clone answers for the same request, so an event can carry it
/// to where the answer's known.
#[derive(Clone)]
pub struct Reply {
    id: Option<u64>,
    to: Option<Arc<Mutex<Option<oneshot::Sender<Response>>>>>,
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reply").field("id", &self.id).finish()
    }
}

impl Reply {
    /// A reply for request `id`, and the answer its connection waits on, none for a
    /// notification.
    pub(crate) fn new(id: Option<u64>) -> (Self, Option<oneshot::Receiver<Response>>) {
        let Some(id) = id else {
            return (Reply { id, to: None }, None);
        };
        let (sender, answer) = oneshot::channel();
        let to = Some(Arc::new(Mutex::new(Some(sender))));
        (Reply { id: Some(id), to }, Some(answer))
    }

    /// Whether something still waits on the answer.
    pub fn is_waited_on(&self) -> bool {
        (self.to.as_ref()).is_some_and(|to| to.lock().as_ref().is_some_and(|s| !s.is_canceled()))
    }

    /// Answer with `result`, unless it's been answered.
    pub fn ok(&self, result: impl Serialize) {
        match serde_json::to_value(result) {
            Ok(result) => self.send(Response::result(self.id, result)),
            Err(err) => self.fail(app::code::INTERNAL_ERROR, err.to_string()),
        }
    }

    /// Answer with an error, unless it's been answered.
    pub fn fail(&self, code: i64, message: impl Into<String>) {
        self.send(Response::error(self.id, code, message));
    }

    fn send(&self, response: Response) {
        if let Some(sender) = (self.to.as_ref()).and_then(|to| to.lock().take()) {
            sender.send(response).ok();
        }
    }
}

/// The socket this app listens at, once it does.
static LISTENING: OnceLock<PathBuf> = OnceLock::new();

/// The socket this app listens at, if it does, for its terminals to tell the programs in them.
/// Another app may have it, which they'd only reach by mistake.
pub fn listening() -> Option<&'static Path> {
    LISTENING.get().map(PathBuf::as_path)
}

/// This launch's links, and where requests come in, if it listens.
pub struct IncomingLinks {
    links: Vec<String>,
    listener: Option<platform::Listener>,
}

impl IncomingLinks {
    /// Send this launch's links to `sender`, then what the socket brings as it comes.
    pub fn receive(self, sender: mpsc::UnboundedSender<Incoming>) {
        for link in self.links {
            sender.unbounded_send(Incoming::Link(link)).ok();
        }
        if let Some(listener) = self.listener {
            if let Ok(path) = platform::socket_path() {
                LISTENING.set(path).ok();
            }
            serve(listener, sender);
        }
    }
}

/// Listen on the app's socket, and return the links this launch was given. Where an app
/// already listens, hand it this launch's links and return `None`, as this launch is then
/// done, but on macOS, which hands the app its links itself, and in dev builds.
pub fn hand_off() -> Option<IncomingLinks> {
    let links = launch_links();
    let listener = match platform::listen() {
        Ok(Some(listener)) => Some(listener),
        Ok(None) => {
            let hands_off = !cfg!(target_os = "macos") && AppChannel::new() != AppChannel::Dev;
            if hands_off {
                match send_to_running_app(&links) {
                    Ok(()) => {
                        tracing::info!("handed this launch to the app already running");
                        return None;
                    }
                    Err(err) => {
                        tracing::warn!(
                            "couldn't hand this launch to the app already running: {err}"
                        )
                    }
                }
            }
            tracing::info!("another app has the socket, so nothing asks this one");
            None
        }
        Err(err) => {
            tracing::warn!("nothing can ask this app, nor later launches hand it links: {err}");
            None
        }
    };
    Some(IncomingLinks { links, listener })
}

/// Hand `links` to the app listening, as notifications it doesn't answer: an `open` a link, or
/// a `show` for a launch without one. The only code that hands the running app links.
fn send_to_running_app(links: &[String]) -> anyhow::Result<()> {
    let mut notifications = Vec::new();
    if links.is_empty() {
        notifications.push((method::SHOW, serde_json::json!({})));
    }
    for link in links.iter().filter_map(|link| OpenLink::parse(link)) {
        let open = Open {
            path: link.path,
            new_window: link.new_window,
            at: Place(link.at),
            say: None,
        };
        notifications.push((method::OPEN, serde_json::to_value(open)?));
    }
    match app::notify_at(&platform::socket_path()?, &notifications)? {
        true => Ok(()),
        false => anyhow::bail!("no app listens"),
    }
}

/// The links this launch was given, as Windows and Linux give them. `args_os`, as `args`
/// panics on one that isn't Unicode, which is no link anyway.
fn launch_links() -> Vec<String> {
    std::env::args_os()
        .skip(1)
        .filter_map(|arg| arg.into_string().ok())
        .filter(|arg| OpenLink::parse(arg).is_some())
        .collect()
}

/// Take connections at `listener`, a thread each, until it fails.
fn serve(mut listener: platform::Listener, sender: mpsc::UnboundedSender<Incoming>) {
    let accepting = std::thread::Builder::new()
        .name("socket".into())
        .spawn(move || {
            loop {
                let connection = match platform::accept(&mut listener) {
                    Ok(connection) => connection,
                    Err(err) => {
                        tracing::warn!("the socket stopped taking connections: {err}");
                        return;
                    }
                };
                let sender = sender.clone();
                let conversing = std::thread::Builder::new()
                    .name("socket connection".into())
                    .spawn(move || converse(connection, sender));
                if let Err(err) = conversing {
                    tracing::warn!("couldn't take a connection: {err}");
                }
            }
        });
    if let Err(err) = accepting {
        tracing::warn!("nothing can ask this app, nor later launches hand it links: {err}");
    }
}

/// Pass on each request `connection` makes, writing back each answer before reading the next,
/// until it closes.
fn converse(connection: platform::Connection, sender: mpsc::UnboundedSender<Incoming>) {
    let Ok((read, mut write)) = platform::split(connection) else {
        return;
    };
    for line in BufReader::new(read).lines() {
        let Ok(line) = line else {
            return;
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: app::Request = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(err) => {
                let response =
                    Response::error(None, app::code::PARSE_ERROR, format!("Not JSON-RPC: {err}"));
                if app::send_line(&mut write, &response).is_err() {
                    return;
                }
                continue;
            }
        };
        let (reply, answer) = Reply::new(request.id);
        let incoming = Incoming::Request(Request {
            method: request.method,
            params: request.params,
            reply,
        });
        if sender.unbounded_send(incoming).is_err() {
            return;
        }
        let Some(answer) = answer else {
            continue;
        };
        let response = futures::executor::block_on(answer).unwrap_or_else(|_| {
            let message = "GitButler dropped the request before answering it";
            Response::error(request.id, app::code::INTERNAL_ERROR, message)
        });
        if app::send_line(&mut write, &response).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use but_testsupport::isolated_app_data_dir;
    use futures::StreamExt as _;
    use serde_json::{Value, json};

    use super::*;

    /// What arrives through `incoming`, as it reads in a test: a link, a launch, or a request's
    /// method and parameters.
    fn shown(incoming: &Incoming) -> Value {
        match incoming {
            Incoming::Link(link) => json!({ "link": link }),
            Incoming::Request(request) => json!({ request.method.clone(): request.params }),
        }
    }

    /// Serve the socket this test's app data has, answering every request `answer` says.
    fn app(answer: impl Fn(&Request) -> Value + Send + 'static) -> std::thread::JoinHandle<()> {
        let listener = (platform::listen())
            .expect("the socket can be listened at")
            .expect("nothing listens there yet");
        let (sender, mut incoming) = mpsc::unbounded();
        IncomingLinks {
            links: Vec::new(),
            listener: Some(listener),
        }
        .receive(sender);
        std::thread::spawn(move || {
            futures::executor::block_on(async move {
                while let Some(Incoming::Request(request)) = incoming.next().await {
                    request.reply.ok(answer(&request));
                }
            })
        })
    }

    fn socket() -> std::path::PathBuf {
        platform::socket_path().expect("an app data directory")
    }

    #[test]
    fn a_later_launch_hands_over_its_links() {
        isolated_app_data_dir(|| {
            let first = "but://open?path=%2Ffirst".to_owned();
            let listener = (platform::listen())
                .expect("the socket can be listened at")
                .expect("nothing listens there yet");
            assert!(
                platform::listen()
                    .expect("the socket can be asked")
                    .is_none(),
                "a later launch finds the app listening"
            );
            let (sender, mut received) = mpsc::unbounded();
            let incoming_links = IncomingLinks {
                links: vec![first.clone()],
                listener: Some(listener),
            };
            incoming_links.receive(sender);

            let links = [
                "but://open?path=%2Fa&to=branch&branch=refs%2Fheads%2Fx".to_owned(),
                "but://open?path=%2Fb&new_window=1".to_owned(),
            ];
            send_to_running_app(&links).expect("a launch with links hands them over");
            let mut incoming =
                futures::executor::block_on((&mut received).take(3).collect::<Vec<_>>());
            // Launches connect on their own and are read side by side, so the next waits for
            // this one's links to arrive.
            send_to_running_app(&[]).expect("a launch without one hands over itself");
            incoming.extend(futures::executor::block_on(received.next()));
            assert_eq!(
                incoming.iter().map(shown).collect::<Vec<_>>(),
                [
                    json!({ "link": first }),
                    json!({ "open": {
                        "path": "/a",
                        "new_window": false,
                        "at": { "to": "branch", "branch": "refs/heads/x" },
                    }}),
                    json!({ "open": { "path": "/b", "new_window": true, "at": {} } }),
                    json!({ "show": {} }),
                ],
                "the app gets its own links first, then a later launch's in the order it gave them"
            );
            assert!(
                incoming.iter().all(|incoming| match incoming {
                    Incoming::Request(request) => !request.reply.is_waited_on(),
                    Incoming::Link(_) => true,
                }),
                "a launch's links are notifications, which nothing waits to hear answered"
            );
        });
    }

    #[test]
    fn a_request_is_answered_on_its_connection() {
        isolated_app_data_dir(|| {
            let app = app(|request| json!({ "asked": request.method }));
            let answer: Option<Value> =
                app::call_at(&socket(), "view", &json!({}), Some(Duration::from_secs(5)))
                    .expect("the app answers");
            assert_eq!(
                answer,
                Some(json!({ "asked": "view" })),
                "the app's answer comes back to the caller"
            );
            drop(app);
        });
    }

    #[cfg(unix)]
    #[test]
    fn requests_on_one_connection_are_answered_in_turn() {
        use std::io::Write as _;

        isolated_app_data_dir(|| {
            let _app = app(|request| request.params.clone());
            let mut stream =
                std::os::unix::net::UnixStream::connect(socket()).expect("the app listens");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("a timeout");
            let mut lines = BufReader::new(stream.try_clone().expect("a second handle")).lines();
            for n in 1..=3 {
                writeln!(
                    stream,
                    r#"{{"jsonrpc":"2.0","id":{n},"method":"echo","params":{n}}}"#
                )
                .expect("the request goes");
                let line = lines.next().expect("an answer").expect("a line");
                let response: Response = serde_json::from_str(&line).expect("JSON-RPC");
                assert_eq!(
                    (response.id, response.result),
                    (Some(n), Some(json!(n))),
                    "each request's answer, with its id, before the next is read"
                );
            }
            writeln!(stream, "not json").expect("the line goes");
            let line = lines.next().expect("an answer").expect("a line");
            let response: Response = serde_json::from_str(&line).expect("JSON-RPC");
            assert_eq!(
                response.error.map(|error| error.code),
                Some(app::code::PARSE_ERROR),
                "a line that isn't JSON-RPC says so, and the connection goes on"
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn a_launch_listens_in_place_of_an_app_that_quit() {
        isolated_app_data_dir(|| {
            // Closing the socket leaves its file behind, as an app that quits does.
            drop(platform::listen().expect("the socket can be listened at"));
            assert!(
                send_to_running_app(&[]).is_err(),
                "no app listens at a socket left behind"
            );
            let listener = platform::listen().expect("the socket can be listened at");
            assert!(listener.is_some(), "a launch listens there itself");
        });
    }

    #[cfg(unix)]
    #[test]
    fn only_the_user_can_reach_the_socket() {
        use std::os::unix::fs::PermissionsExt as _;

        isolated_app_data_dir(|| {
            let _listener = platform::listen().expect("the socket can be listened at");
            let mode = (std::fs::metadata(socket()).expect("the socket is there"))
                .permissions()
                .mode();
            assert_eq!(
                mode & 0o777,
                0o600,
                "other users can't ask the app anything"
            );
        });
    }

    #[test]
    fn a_reply_answers_once_and_a_notification_answers_nobody() {
        let (reply, answer) = Reply::new(Some(7));
        let answer = answer.expect("a request is waited on");
        assert!(reply.is_waited_on(), "its connection waits");
        let clone = reply.clone();
        clone.ok(json!("first"));
        reply.fail(app::code::INTERNAL_ERROR, "second");
        let response = futures::executor::block_on(answer).expect("an answer");
        assert_eq!(
            (response.id, response.result),
            (Some(7), Some(json!("first"))),
            "the first answer, from any clone, is the one sent"
        );
        assert!(!reply.is_waited_on(), "nothing waits once it's answered");

        let (notification, answer) = Reply::new(None);
        assert!(answer.is_none(), "a notification gets no answer");
        notification.ok(json!("ignored"));
        assert!(!notification.is_waited_on(), "nothing to answer");
    }
}
