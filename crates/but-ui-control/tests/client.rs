#![cfg(unix)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use but_ui_control::{Client, Request, Response, RpcError, code};
use serde_json::json;

fn serve_once(
    path: &Path,
    answer: impl Fn(&Request) -> Response + Send + 'static,
) -> std::thread::JoinHandle<Vec<Request>> {
    let listener = UnixListener::bind(path).expect("a fresh temp dir takes a socket");
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("the test connects");
        let mut writer = stream.try_clone().expect("streams clone");
        let mut asked = Vec::new();
        for line in BufReader::new(stream).lines() {
            let request: Request = serde_json::from_str(&line.unwrap()).unwrap();
            let mut out = serde_json::to_string(&answer(&request)).unwrap();
            out.push('\n');
            writer.write_all(out.as_bytes()).unwrap();
            asked.push(request);
        }
        asked
    })
}

fn socket(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("gpui.sock")
}

#[test]
fn calls_carry_params_and_results_on_one_connection() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket(&dir);
    let server = serve_once(&path, |request| match request.method.as_str() {
        "test.echo" => Response::ok(request.id, request.params.clone()),
        _ => Response::err(request.id, RpcError::new(code::METHOD_NOT_FOUND, "no")),
    });

    let mut client = Client::connect_to(&path).unwrap();
    let first: serde_json::Value = client.call("test.echo", json!({ "step": 3 })).unwrap();
    assert_eq!(
        first,
        json!({ "step": 3 }),
        "the server's result comes back"
    );
    let second: Vec<u32> = client.call("test.echo", [1, 2]).unwrap();
    assert_eq!(second, [1, 2], "and typed as the caller asks");
    drop(client);

    let asked = server.join().unwrap();
    assert_eq!(asked.len(), 2, "both calls went over the one connection");
    assert_eq!(asked[0].id, Some(1), "ids count up from 1");
    assert_eq!(asked[1].id, Some(2), "each call gets its own id");
    assert_eq!(asked[0].jsonrpc, "2.0", "every request says its version");
}

#[test]
fn an_error_response_keeps_its_code_and_message() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket(&dir);
    let server = serve_once(&path, |request| {
        Response::err(
            request.id,
            RpcError::new(code::NOT_FOUND, "No project open in the app has /repo"),
        )
    });

    let mut client = Client::connect_to(&path).unwrap();
    let err = client
        .call::<serde_json::Value>("test.anything", json!({}))
        .unwrap_err();
    let rpc = err
        .downcast_ref::<RpcError>()
        .expect("JSON-RPC errors stay typed");
    assert_eq!(
        rpc.code,
        code::NOT_FOUND,
        "the code survives for callers to branch on"
    );
    assert_eq!(
        err.to_string(),
        "No project open in the app has /repo",
        "the message is the error"
    );
    drop(client);
    server.join().unwrap();
}

#[test]
fn nothing_listening_says_the_app_is_not_running() {
    let dir = tempfile::tempdir().unwrap();
    let path = socket(&dir);
    let err = Client::connect_to(&path)
        .err()
        .expect("nothing listens in a fresh temp dir");
    let message = err.to_string();
    assert!(
        message.starts_with("The GitButler GPUI app isn't running"),
        "the caller learns what's wrong: {message}"
    );
    assert!(
        message.contains("GITBUTLER_GPUI_SOCKET"),
        "and how to point elsewhere: {message}"
    );
}
