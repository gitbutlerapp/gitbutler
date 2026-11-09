//! `but _ui` against a stand-in for the GPUI app's control socket.

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixListener;
use std::path::Path;

use but_ui_control::ui::{self, Area, DispatchParams, Keys, Overlay, Row, State, Tile};
use but_ui_control::{Request, Response, RpcError, SOCKET_ENV, code};

use crate::utils::Sandbox;

/// Stand in for the app at `path`: answer each connection's requests with `answer`, for
/// `connections` connections, one per command.
fn fake_app(path: &Path, connections: usize, answer: fn(&Request) -> Response) {
    let listener = UnixListener::bind(path).expect("the sandbox has room for a socket");
    std::thread::spawn(move || {
        for stream in listener.incoming().take(connections) {
            let stream = stream.unwrap();
            let mut writer = stream.try_clone().unwrap();
            for line in BufReader::new(stream).lines() {
                let request: Request = serde_json::from_str(&line.unwrap()).unwrap();
                let mut out = serde_json::to_string(&answer(&request)).unwrap();
                out.push('\n');
                writer.write_all(out.as_bytes()).unwrap();
            }
        }
    });
}

fn row(kind: &str, label: &str) -> Row {
    Row {
        project: Some("repo".into()),
        kind: kind.into(),
        label: label.into(),
    }
}

fn answer(request: &Request) -> Response {
    match request.method.as_str() {
        ui::STATE => Response::ok(
            request.id,
            State {
                navigator: true,
                detail: true,
                selection: Some(row("branch", "feature")),
                tiles: vec![
                    Tile {
                        id: ui::HOME.into(),
                        shows: row("branch", "feature"),
                        area: Area {
                            left: 0.,
                            top: 0.,
                            width: 0.5,
                            height: 1.,
                        },
                        panes: vec!["files".into(), "diff".into()],
                    },
                    Tile {
                        id: "Terminal".into(),
                        shows: row("terminal", "Terminal"),
                        area: Area {
                            left: 0.5,
                            top: 0.,
                            width: 0.5,
                            height: 1.,
                        },
                        panes: vec![],
                    },
                ],
                keys: Keys {
                    tile: Some(ui::HOME.into()),
                    pane: Some("diff".into()),
                    text_field: false,
                },
                overlay: Some(Overlay::Palette),
            },
        ),
        ui::DISPATCH => {
            let params: DispatchParams = serde_json::from_value(request.params.clone()).unwrap();
            match params.action.as_str() {
                "tiling::SplitRight" => Response::ok(request.id, ()),
                action => Response::err(
                    request.id,
                    RpcError::new(
                        code::NOT_AVAILABLE,
                        format!("{action} can't act where the keys are (home › diff)."),
                    ),
                ),
            }
        }
        _ => Response::err(
            request.id,
            RpcError::new(code::METHOD_NOT_FOUND, "no such method"),
        ),
    }
}

#[test]
fn state_says_what_shows_and_where_the_keys_are() {
    let env = Sandbox::empty();
    let socket = env.app_data_dir().join("gpui.sock");
    fake_app(&socket, 1, answer);
    env.but("_ui state")
        .env(SOCKET_ENV, &socket)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Navigator shown, detail shown.
Selected: branch feature in repo
Tiles:
  home: branch feature in repo (panes: files, diff)
  Terminal: terminal Terminal in repo
Keys: home › diff
Open over the window: Palette

"#]]);
}

#[test]
fn dispatch_runs_an_action_or_says_why_it_cant() {
    let env = Sandbox::empty();
    let socket = env.app_data_dir().join("gpui.sock");
    fake_app(&socket, 2, answer);
    env.but("_ui dispatch tiling::SplitRight")
        .env(SOCKET_ENV, &socket)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Ran tiling::SplitRight.

"#]]);
    // An action that can't act where the keys are is refused, saying where they are.
    env.but("_ui dispatch changes::MarkResolved")
        .env(SOCKET_ENV, &socket)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: changes::MarkResolved can't act where the keys are (home › diff).
...
"#]]);
}

#[test]
fn args_that_arent_json_are_refused_before_reaching_the_app() {
    let env = Sandbox::empty();
    env.but("_ui dispatch some::Action --args '{oops'")
        .env(SOCKET_ENV, env.app_data_dir().join("gpui.sock"))
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for 'JSON'
...
"#]]);
}
