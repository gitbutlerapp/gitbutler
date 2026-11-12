use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixListener;
use std::path::Path;

use but_ui_control::ui::{
    self, Area, CloseParams, DispatchParams, Keys, Layout, Open, OpenParams, Opened, Read,
    ReadParams, Row, State, TerminalStatus, Tile, TypeParams,
};
use but_ui_control::{Request, Response, RpcError, SOCKET_ENV, code};

use crate::utils::Sandbox;

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

fn tile(id: &str, shows: Row, left: f32, panes: &[&str]) -> Tile {
    Tile {
        id: id.into(),
        shows,
        area: Area {
            left,
            top: 0.,
            width: 0.5,
            height: 1.,
        },
        panes: panes.iter().map(|pane| (*pane).to_owned()).collect(),
    }
}

fn answer(request: &Request) -> Response {
    match request.method.as_str() {
        ui::STATE => Response::ok(
            request.id.clone(),
            State {
                navigator: true,
                detail: true,
                selection: Some(row("branch", "feature")),
                layouts: Vec::from([
                    Layout {
                        row: row("branch", "feature"),
                        open: true,
                        home: "1".into(),
                        zoomed: None,
                        tiles: Vec::from([
                            tile("1", row("branch", "feature"), 0., &["files", "diff"]),
                            tile("2", row("terminal", "Terminal"), 0.5, &[]),
                        ]),
                    },
                    Layout {
                        row: row("branch", "other"),
                        open: false,
                        home: "3".into(),
                        zoomed: None,
                        tiles: Vec::from([
                            tile("3", row("branch", "other"), 0., &[]),
                            tile("4", row("terminal", "build"), 0.5, &[]),
                        ]),
                    },
                ]),
                keys: Keys {
                    tile: Some("1".into()),
                    pane: Some("diff".into()),
                    text_field: false,
                },
                overlay: Some("palette".into()),
            },
        ),
        ui::DISPATCH => {
            let params: DispatchParams = serde_json::from_value(request.params.clone()).unwrap();
            match params.action.as_str() {
                "tiling::SplitRight" => Response::ok(request.id.clone(), ()),
                action => Response::err(
                    request.id.clone(),
                    RpcError::new(
                        code::NOT_AVAILABLE,
                        format!("{action} can't act where the keys are (1 › diff)."),
                    ),
                ),
            }
        }
        ui::TYPE => {
            let params: TypeParams = serde_json::from_value(request.params.clone()).unwrap();
            let tile = params.tile.unwrap_or_default();
            if tile == "2" {
                Response::ok(request.id.clone(), ())
            } else if tile == "9" {
                // Says what it was sent, for a test to see the order.
                Response::err(
                    request.id.clone(),
                    RpcError::new(
                        code::INVALID_PARAMS,
                        serde_json::to_string(&params.input).unwrap(),
                    ),
                )
            } else {
                Response::err(
                    request.id.clone(),
                    RpcError::new(
                        code::NOT_AVAILABLE,
                        format!("Tile {tile} shows no terminal"),
                    ),
                )
            }
        }
        ui::CLOSE => {
            let params: CloseParams = serde_json::from_value(request.params.clone()).unwrap();
            match params.tile.as_str() {
                "2" => Response::ok(request.id.clone(), ()),
                tile => Response::err(
                    request.id.clone(),
                    RpcError::new(
                        code::NOT_AVAILABLE,
                        format!("Tile {tile} is the page its layout is for"),
                    ),
                ),
            }
        }
        ui::OPEN => {
            let params: OpenParams = serde_json::from_value(request.params.clone()).unwrap();
            if params.beside != "1" {
                return Response::err(
                    request.id.clone(),
                    RpcError::new(
                        code::NOT_FOUND,
                        format!("There's no tile {}", params.beside),
                    ),
                );
            }
            let opened = match params.open {
                Open::Show(name) => Opened {
                    id: "5".into(),
                    shows: row("branch", &name),
                },
                Open::Terminal { run } => Opened {
                    id: "6".into(),
                    shows: row("terminal", run.as_deref().unwrap_or("Terminal")),
                },
            };
            Response::ok(request.id.clone(), opened)
        }
        ui::READ => {
            let params: ReadParams = serde_json::from_value(request.params.clone()).unwrap();
            if params.tile != "2" {
                return Response::err(
                    request.id.clone(),
                    RpcError::new(
                        code::NOT_AVAILABLE,
                        format!("Tile {} shows no terminal", params.tile),
                    ),
                );
            }
            let lines = ["$ cargo test", "running 3 tests", "test result: ok"];
            let skip = lines.len() - params.lines.unwrap_or(lines.len()).min(lines.len());
            Response::ok(
                request.id.clone(),
                Read {
                    title: "Terminal".into(),
                    status: TerminalStatus::Blocked {
                        reason: Some("needs approval".into()),
                    },
                    text: lines[skip..].join("\n"),
                },
            )
        }
        _ => Response::err(
            request.id.clone(),
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
Open: branch feature in repo
  1: branch feature in repo (home; panes: files, diff)
  2: terminal Terminal in repo
Not open: branch other in repo
  3: branch other in repo (home)
  4: terminal build in repo
Keys: 1 › diff
Open over the window: palette

"#]]);
}

#[test]
fn state_in_json_lists_each_rows_layout() {
    let env = Sandbox::empty();
    let socket = env.app_data_dir().join("gpui.sock");
    fake_app(&socket, 1, answer);
    // The layout not open is listed too, with its tiles' places and no panes to focus.
    env.but("--json _ui state")
        .env(SOCKET_ENV, &socket)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "navigator": true,
  "detail": true,
  "selection": {
    "project": "repo",
    "kind": "branch",
    "label": "feature"
  },
  "layouts": [
    {
      "row": {
        "project": "repo",
        "kind": "branch",
        "label": "feature"
      },
      "open": true,
      "home": "1",
      "zoomed": null,
      "tiles": [
        {
          "id": "1",
          "shows": {
            "project": "repo",
            "kind": "branch",
            "label": "feature"
          },
          "area": {
            "left": 0.0,
            "top": 0.0,
            "width": 0.5,
            "height": 1.0
          },
          "panes": [
            "files",
            "diff"
          ]
        },
        {
          "id": "2",
          "shows": {
            "project": "repo",
            "kind": "terminal",
            "label": "Terminal"
          },
          "area": {
            "left": 0.5,
            "top": 0.0,
            "width": 0.5,
            "height": 1.0
          },
          "panes": []
        }
      ]
    },
    {
      "row": {
        "project": "repo",
        "kind": "branch",
        "label": "other"
      },
      "open": false,
      "home": "3",
      "zoomed": null,
      "tiles": [
        {
          "id": "3",
          "shows": {
            "project": "repo",
            "kind": "branch",
            "label": "other"
          },
          "area": {
            "left": 0.0,
            "top": 0.0,
            "width": 0.5,
            "height": 1.0
          },
          "panes": []
        },
        {
          "id": "4",
          "shows": {
            "project": "repo",
            "kind": "terminal",
            "label": "build"
          },
          "area": {
            "left": 0.5,
            "top": 0.0,
            "width": 0.5,
            "height": 1.0
          },
          "panes": []
        }
      ]
    }
  ],
  "keys": {
    "tile": "1",
    "pane": "diff",
    "text_field": false
  },
  "overlay": "palette"
}

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
    env.but("_ui dispatch changes::MarkResolved")
        .env(SOCKET_ENV, &socket)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: changes::MarkResolved can't act where the keys are (1 › diff).
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

#[test]
fn type_goes_to_the_tile_named_or_says_why_it_cant() {
    let env = Sandbox::empty();
    let socket = env.app_data_dir().join("gpui.sock");
    fake_app(&socket, 2, answer);
    env.but("_ui type --tile 2 --text hi --key enter")
        .env(SOCKET_ENV, &socket)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Typed 2 characters and pressed 1 keys.

"#]]);
    env.but("_ui type --tile 1 --text hi")
        .env(SOCKET_ENV, &socket)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Tile 1 shows no terminal
...
"#]]);
}

#[test]
fn read_shows_a_terminals_status_and_its_last_lines() {
    let env = Sandbox::empty();
    let socket = env.app_data_dir().join("gpui.sock");
    fake_app(&socket, 3, answer);
    env.but("_ui read --tile 2")
        .env(SOCKET_ENV, &socket)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Terminal: blocked (needs approval)

$ cargo test
running 3 tests
test result: ok

"#]]);
    // `--lines` reaches the app, which sends only those.
    env.but("--json _ui read --tile 2 --lines 1")
        .env(SOCKET_ENV, &socket)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "title": "Terminal",
  "status": {
    "state": "blocked",
    "reason": "needs approval"
  },
  "text": "test result: ok"
}

"#]]);
    env.but("_ui read --tile 1")
        .env(SOCKET_ENV, &socket)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Tile 1 shows no terminal
...
"#]]);
}

#[test]
fn type_sends_text_and_keys_in_the_order_given() {
    let env = Sandbox::empty();
    let socket = env.app_data_dir().join("gpui.sock");
    fake_app(&socket, 1, answer);
    // The fake app answers tile 9 with what it was sent.
    env.but("_ui type --tile 9 --text hi --key backspace --text 'something else' --key ctrl-c")
        .env(SOCKET_ENV, &socket)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: [{"text":"hi"},{"key":"backspace"},{"text":"something else"},{"key":"ctrl-c"}]
...
"#]]);
}

#[test]
fn type_needs_something_to_send() {
    let env = Sandbox::empty();
    env.but("_ui type --tile 2")
        .env(SOCKET_ENV, env.app_data_dir().join("gpui.sock"))
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the following required arguments were not provided:
  <--text <TEXT>|--key <KEY>>
...
"#]]);
}

#[test]
fn open_puts_a_page_or_a_terminal_beside_a_tile() {
    let env = Sandbox::empty();
    let socket = env.app_data_dir().join("gpui.sock");
    fake_app(&socket, 3, answer);
    env.but("_ui open --beside 1 --show feature")
        .env(SOCKET_ENV, &socket)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Opened tile 5: branch feature in repo.

"#]]);
    // The fake app names the terminal after the command it was asked to run.
    env.but("--json _ui open --beside 1 --below --terminal --run claude")
        .env(SOCKET_ENV, &socket)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "id": "6",
  "shows": {
    "project": "repo",
    "kind": "terminal",
    "label": "claude"
  }
}

"#]]);
    env.but("_ui open --beside 8 --terminal")
        .env(SOCKET_ENV, &socket)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: There's no tile 8
...
"#]]);
}

#[test]
fn open_needs_one_thing_to_open() {
    let env = Sandbox::empty();
    let socket = env.app_data_dir().join("gpui.sock");
    env.but("_ui open --beside 1")
        .env(SOCKET_ENV, &socket)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the following required arguments were not provided:
  <--terminal|--show <NAME>>
...
"#]]);
    env.but("_ui open --beside 1 --terminal --show feature")
        .env(SOCKET_ENV, &socket)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the argument '--terminal' cannot be used with '--show <NAME>'
...
"#]]);
    env.but("_ui open --beside 1 --show feature --run claude")
        .env(SOCKET_ENV, &socket)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the argument '--show <NAME>' cannot be used with '--run <COMMAND>'
...
"#]]);
}

#[test]
fn text_and_commands_can_start_with_a_dash() {
    let env = Sandbox::empty();
    let socket = env.app_data_dir().join("gpui.sock");
    fake_app(&socket, 2, answer);
    // The fake app answers tile 9 with what it was sent.
    env.but("_ui type --tile 9 --text -y --text '- item' --key -")
        .env(SOCKET_ENV, &socket)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: [{"text":"-y"},{"text":"- item"},{"key":"-"}]
...
"#]]);
    env.but("_ui open --beside 1 --terminal --run -x")
        .env(SOCKET_ENV, &socket)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Opened tile 6: terminal -x in repo.

"#]]);
}

#[test]
fn close_closes_a_tile_or_says_why_it_cant() {
    let env = Sandbox::empty();
    let socket = env.app_data_dir().join("gpui.sock");
    fake_app(&socket, 2, answer);
    env.but("_ui close --tile 2")
        .env(SOCKET_ENV, &socket)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Closed tile 2.

"#]]);
    env.but("_ui close --tile 1")
        .env(SOCKET_ENV, &socket)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Tile 1 is the page its layout is for
...
"#]]);
}
