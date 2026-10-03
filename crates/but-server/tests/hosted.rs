//! The hosted server's `/events` WebSocket: who may open it, and which publishes reach which socket.

use std::{io::Write as _, net::SocketAddr, path::Path, process::Stdio, time::Duration};

use but_server::hosted::{HostedConfig, router};
use futures_util::{SinkExt as _, StreamExt as _};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest as _},
};

const TOKEN: &str = "test-token";

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[tokio::test(flavor = "multi_thread")]
async fn a_ticket_needs_the_bearer_token() {
    let server = Server::start().await;
    assert_eq!(
        server.ticket_response(None).await.status(),
        401,
        "the ticket route sits behind `require_token`"
    );
    assert_eq!(
        server.ticket_response(Some("wrong")).await.status(),
        401,
        "only the configured token gets a ticket"
    );
    let ticket = server.ticket().await;
    assert!(
        server.connect_with(&ticket).await.is_ok(),
        "a ticket from the right token opens `/events`"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn events_refuse_unknown_and_reused_tickets() {
    let server = Server::start().await;
    assert!(
        server.connect_with("never-issued").await.is_err(),
        "only issued tickets open a socket"
    );
    let ticket = server.ticket().await;
    let _socket = server
        .connect_with(&ticket)
        .await
        .expect("a fresh ticket opens a socket");
    assert!(
        server.connect_with(&ticket).await.is_err(),
        "a ticket opens one socket only"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_ticket_is_never_cached() {
    let server = Server::start().await;
    let response = server.ticket_response(Some(TOKEN)).await;
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .map(|value| value.as_bytes()),
        Some(b"no-store".as_slice()),
        "a ticket is a credential until it's redeemed"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_browser_socket_must_be_same_origin() {
    let server = Server::start().await;
    assert!(
        server
            .connect_from("http://elsewhere.example")
            .await
            .is_err(),
        "a page on another origin can't open the socket, even with a ticket"
    );
    assert!(
        server
            .connect_from(&format!("http://{}", server.addr))
            .await
            .is_ok(),
        "the pages this server serves can"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_publish_reaches_only_sockets_subscribed_to_it() {
    let server = Server::start().await;
    let (repo, _tmp) = but_testsupport::writable_scenario("published-checkout");
    let workdir = repo.workdir().expect("the fixture has a worktree");
    let alpha = server.publish(workdir, "alpha").await;
    server.publish(workdir, "beta").await;

    let mut socket = server.connect().await;
    assert_eq!(
        subscribe(&mut socket, &alpha).await,
        "subscribed",
        "a published checkout can be subscribed to"
    );
    server.publish(workdir, "beta").await;
    server.publish(workdir, "alpha").await;

    let event = next_event(&mut socket).await;
    // The first event is alpha's, so beta's publish sent nothing here. It is the watcher
    // event Lite's desktop host would emit, on the channel the web transport listens to.
    snapbox::assert_data_eq!(
        serde_json::to_string_pretty(&event).expect("serializable"),
        snapbox::str![[r#"
{
  "channel": "watcher:[..]/laptop/alpha",
  "payload": {
    "name": "project://[..]/laptop/alpha/git/activity",
    "payload": {
      "type": "gitActivity",
      "subject": {
        "headSha": "[..]"
      }
    }
  }
}
"#]]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn unsubscribes_and_bad_messages_leave_the_socket_working() {
    let server = Server::start().await;
    let (repo, _tmp) = but_testsupport::writable_scenario("published-checkout");
    let workdir = repo.workdir().expect("the fixture has a worktree");
    let alpha = server.publish(workdir, "alpha").await;
    let beta = server.publish(workdir, "beta").await;

    let mut socket = server.connect().await;
    subscribe(&mut socket, &alpha).await;
    assert_eq!(
        subscribe(
            &mut socket,
            "0000000000000000000000000000000000000000/laptop/never-published",
        )
        .await,
        "rejected",
        "a checkout that was never published can't be subscribed to, and the page is told so"
    );
    send(&mut socket, "not json").await;
    unsubscribe(&mut socket, &alpha).await;
    subscribe(&mut socket, &beta).await;
    server.publish(workdir, "alpha").await;
    server.publish(workdir, "beta").await;

    assert_eq!(
        project_of(&next_event(&mut socket).await),
        beta,
        "alpha was unsubscribed, and the bad messages were ignored without closing the socket"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn subscriptions_belong_to_their_socket() {
    let server = Server::start().await;
    let (repo, _tmp) = but_testsupport::writable_scenario("published-checkout");
    let workdir = repo.workdir().expect("the fixture has a worktree");
    let alpha = server.publish(workdir, "alpha").await;
    let beta = server.publish(workdir, "beta").await;

    let mut on_alpha = server.connect().await;
    let mut on_beta = server.connect().await;
    subscribe(&mut on_alpha, &alpha).await;
    subscribe(&mut on_beta, &beta).await;
    server.publish(workdir, "alpha").await;
    server.publish(workdir, "beta").await;
    server.publish(workdir, "alpha").await;

    let alpha_events = [
        project_of(&next_event(&mut on_alpha).await),
        project_of(&next_event(&mut on_alpha).await),
    ];
    assert_eq!(
        alpha_events,
        [alpha.clone(), alpha],
        "beta's publish in between never reached the socket on alpha"
    );
    assert_eq!(
        project_of(&next_event(&mut on_beta).await),
        beta,
        "the socket on beta got beta's publish and not alpha's"
    );

    on_alpha.close(None).await.expect("closes cleanly");
    server.publish(workdir, "beta").await;
    assert_eq!(
        project_of(&next_event(&mut on_beta).await),
        beta,
        "closing one socket leaves the other's subscriptions alone"
    );
}

/// A hosted server on a free port, with its own data directory.
struct Server {
    addr: SocketAddr,
    _data: tempfile::TempDir,
}

impl Server {
    async fn start() -> Self {
        let data = tempfile::tempdir().expect("a temporary data directory");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a free port");
        let addr = listener.local_addr().expect("a bound address");
        let app = router(HostedConfig {
            port: addr.port(),
            bind_addr: addr.ip().to_string(),
            data_dir: data.path().to_owned(),
            web_dir: None,
            token: TOKEN.into(),
        });
        tokio::spawn(async move { axum::serve(listener, app).await });
        Server { addr, _data: data }
    }

    /// Asks for a ticket as the web transport does, with or without a bearer token.
    async fn ticket_response(&self, token: Option<&str>) -> reqwest::Response {
        let mut request = reqwest::Client::new()
            .post(format!("http://{}/events/ticket", self.addr))
            .header("content-type", "application/json")
            .body("{}");
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request.send().await.expect("the server answers")
    }

    async fn ticket(&self) -> String {
        let body = self
            .ticket_response(Some(TOKEN))
            .await
            .text()
            .await
            .expect("a body");
        let body: serde_json::Value = serde_json::from_str(&body).expect("JSON");
        body["subject"]["ticket"]
            .as_str()
            .expect("a ticket in the success subject")
            .to_owned()
    }

    async fn connect_with(
        &self,
        ticket: &str,
    ) -> Result<Socket, tokio_tungstenite::tungstenite::Error> {
        connect_async(format!("ws://{}/events?ticket={ticket}", self.addr))
            .await
            .map(|(socket, _response)| socket)
    }

    /// Opens a socket with a fresh ticket, as a browser on `origin` would.
    async fn connect_from(
        &self,
        origin: &str,
    ) -> Result<Socket, tokio_tungstenite::tungstenite::Error> {
        let ticket = self.ticket().await;
        let mut request = format!("ws://{}/events?ticket={ticket}", self.addr)
            .into_client_request()
            .expect("a valid request");
        request
            .headers_mut()
            .insert("origin", origin.parse().expect("a valid header"));
        connect_async(request)
            .await
            .map(|(socket, _response)| socket)
    }

    async fn connect(&self) -> Socket {
        let ticket = self.ticket().await;
        self.connect_with(&ticket)
            .await
            .expect("a fresh ticket opens a socket")
    }

    /// Publishes `repo` as `laptop/<checkout>`, returning its hosted project ID.
    async fn publish(&self, repo: &Path, checkout: &str) -> String {
        let (repo, git_url, checkout) = (
            repo.to_owned(),
            format!("http://{}/git", self.addr),
            checkout.to_owned(),
        );
        tokio::task::spawn_blocking(move || publish(&repo, &git_url, &checkout))
            .await
            .expect("the publish ran")
    }
}

/// Pushes the way `but _publish` does (`crates/but/src/command/legacy/publish.rs`): the
/// branches and a `published.json` announcement on `refs/gitbutler/meta`, after a new commit
/// so that every publish changes refs.
fn publish(repo: &Path, git_url: &str, checkout: &str) -> String {
    git(
        repo,
        &["commit", "--allow-empty", "-q", "-m", "publish"],
        None,
    );
    let root = git(repo, &["rev-list", "--max-parents=0", "HEAD"], None);
    let head = git(repo, &["symbolic-ref", "HEAD"], None);
    let announcement =
        format!(r#"{{"version":1,"head":"{head}","remotes":{{}},"includesUncommitted":false}}"#);
    let blob = git(repo, &["hash-object", "-w", "--stdin"], Some(&announcement));
    let tree = git(
        repo,
        &["mktree"],
        Some(&format!("100644 blob {blob}\tpublished.json\n")),
    );
    let project_id = format!("{root}/laptop/{checkout}");
    git(
        repo,
        &[
            "-c",
            &format!("http.extraHeader=Authorization: Bearer {TOKEN}"),
            "push",
            "-q",
            "--atomic",
            &format!("{git_url}/{project_id}"),
            "+refs/heads/*:refs/heads/*",
            &format!("+{tree}:refs/gitbutler/meta"),
        ],
        None,
    );
    project_id
}

/// Runs git in `dir`, isolated from the user's git configuration.
fn git(dir: &Path, args: &[&str], stdin: Option<&str>) -> String {
    let mut child = but_testsupport::git_at_dir(dir)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("git runs");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(stdin.unwrap_or_default().as_bytes())
        .expect("git reads stdin");
    let out = child.wait_with_output().expect("git finishes");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("git prints UTF-8 here")
        .trim()
        .to_owned()
}

async fn send(socket: &mut Socket, text: &str) {
    socket
        .send(Message::text(text))
        .await
        .expect("the socket is open");
}

/// Subscribes and waits for the server's answer, `subscribed` or `rejected`. Once it's
/// `subscribed`, every later publish of the project reaches the socket.
async fn subscribe(socket: &mut Socket, project_id: &str) -> String {
    send(
        socket,
        &format!(r#"{{"type":"subscribe","projectId":"{project_id}"}}"#),
    )
    .await;
    loop {
        let reply = next_json(socket).await;
        if reply["projectId"] == project_id {
            return reply["type"]
                .as_str()
                .expect("replies are typed")
                .to_owned();
        }
    }
}

async fn unsubscribe(socket: &mut Socket, project_id: &str) {
    send(
        socket,
        &format!(r#"{{"type":"unsubscribe","projectId":"{project_id}"}}"#),
    )
    .await;
}

/// The next event, as JSON, skipping replies and heartbeats. Asserting on the first event
/// after several publishes shows which of them reached the socket, without waiting for ones
/// that shouldn't arrive.
async fn next_event(socket: &mut Socket) -> serde_json::Value {
    loop {
        let message = next_json(socket).await;
        if message.get("channel").is_some() {
            return message;
        }
    }
}

async fn next_json(socket: &mut Socket) -> serde_json::Value {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(30), socket.next())
            .await
            .expect("a message within the timeout")
            .expect("the socket is open")
            .expect("a valid frame");
        if let Message::Text(text) = message {
            return serde_json::from_str(&text).expect("messages are JSON");
        }
    }
}

fn project_of(event: &serde_json::Value) -> String {
    event["channel"]
        .as_str()
        .and_then(|channel| channel.strip_prefix("watcher:"))
        .expect("events arrive on `watcher:<projectId>`")
        .to_owned()
}
