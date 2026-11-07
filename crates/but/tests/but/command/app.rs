//! `but _view`, `but _wait` and `but _open --gui` against a stand-in for the GUI, listening at a
//! socket of the test's own that `GITBUTLER_APP_SOCKET` points at, as the GUI's terminals point
//! the programs in them at the app.

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread::JoinHandle;

use serde_json::{Value, json};
use snapbox::str;

use crate::utils::Sandbox;

/// A stand-in for the GUI, answering what it's asked, and what it was asked.
pub(crate) struct App {
    /// Where it listens, in a folder of its own: short, as a Unix socket's path has to be.
    socket: PathBuf,
    _dir: tempfile::TempDir,
    asked: JoinHandle<Vec<Value>>,
}

impl App {
    /// Answer the next `n` connections' one request each with what `answer` makes of its method
    /// and parameters.
    pub(crate) fn answering(
        n: usize,
        answer: impl Fn(&str, &Value) -> Value + Send + 'static,
    ) -> App {
        let dir = tempfile::Builder::new()
            .prefix("app")
            .tempdir_in("/tmp")
            .expect("a temporary folder");
        let socket = dir.path().join("s");
        let listener = UnixListener::bind(&socket).expect("the socket binds");
        let asked = std::thread::spawn(move || {
            let mut asked = Vec::new();
            for stream in listener.incoming().take(n) {
                let stream = stream.expect("a connection");
                let mut line = String::new();
                BufReader::new(&stream)
                    .read_line(&mut line)
                    .expect("a request");
                let request: Value = serde_json::from_str(&line).expect("JSON-RPC");
                let method = request["method"].as_str().expect("a method").to_owned();
                let result = answer(&method, &request["params"]);
                let response = json!({"jsonrpc": "2.0", "id": request["id"], "result": result});
                writeln!(&stream, "{response}").expect("the answer goes");
                asked.push(json!({ method: request["params"] }));
            }
            asked
        });
        App {
            socket,
            _dir: dir,
            asked,
        }
    }

    /// `but` with `args`, run as in the app's terminal.
    pub(crate) fn but(&self, env: &Sandbox, args: &str) -> snapbox::cmd::Command {
        env.but(args).env(but_path::app::SOCKET_ENV, &self.socket)
    }

    /// What it was asked, once it's answered all it was to.
    pub(crate) fn asked(self) -> Vec<Value> {
        self.asked.join().expect("the app's thread")
    }
}

/// What the stand-in shows: a branch of `shop`, at a line of a file with a space in its name.
pub(crate) fn shown() -> Value {
    json!({
        "path": "/work/shop",
        "at": {"to": "branch", "branch": "refs/heads/feature", "file": "src/a b.rs", "line": "4"},
        "title": "shop",
    })
}

#[test]
fn view_and_wait_say_when_the_gui_isnt_running() {
    let env = Sandbox::empty();
    for args in ["_view", "_wait --timeout 1"] {
        env.but(args).assert().failure().stderr_eq(str![[r#"
Error: GitButler isn't running: start it with `but gui`

"#]]);
    }
}

#[test]
fn view_says_what_the_gui_shows_as_open_takes_a_place() {
    let env = Sandbox::empty();
    let app = App::answering(2, |_, _| shown());
    app.but(&env, "_view")
        .assert()
        .success()
        .stdout_eq(str![[r#"
GitButler shows shop: to=branch branch=refs/heads/feature 'file=src/a b.rs' line=4 in /work/shop

"#]]);
    app.but(&env, "--json _view")
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "path": "/work/shop",
  "at": {
    "to": "branch",
    "branch": "refs/heads/feature",
    "file": "src/a b.rs",
    "line": "4"
  },
  "title": "shop"
}

"#]]);
    assert_eq!(
        app.asked(),
        [json!({"view": {}}), json!({"view": {}})],
        "the app was asked what it shows, with nothing else"
    );
}

#[test]
fn wait_prints_what_the_user_told_agents_and_where_to_wait_next() {
    let env = Sandbox::empty();
    let app = App::answering(2, |_, params| match params["after"].as_u64() {
        Some(_) => json!({"events": [], "last": 7}),
        None => json!({
            "events": [
                {"seq": 6, "kind": "asked", "note": "why is this here?", "view": shown()},
                {"seq": 7, "kind": "next", "view": shown()},
            ],
            "last": 7,
        }),
    });
    app.but(&env, "_wait")
        .assert()
        .success()
        .stdout_eq(str![[r#"
#6 The user asks: why is this here?
  about shop: to=branch branch=refs/heads/feature 'file=src/a b.rs' line=4 in /work/shop
#7 The user is ready for the next step
Wait for more with: but _wait --after 7

"#]]);
    app.but(&env, "_wait --after 7 --timeout 30")
        .assert()
        .success()
        .stdout_eq(str![[r#"
The user said nothing in time
Wait for more with: but _wait --after 7

"#]]);
    assert_eq!(
        app.asked(),
        [
            json!({"wait": {}}),
            json!({"wait": {"after": 7, "timeout_secs": 30}}),
        ],
        "the app is asked to wait after what was printed, for as long as asked"
    );
}
