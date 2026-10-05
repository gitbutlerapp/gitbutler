//! A live connection to the hosted server, for clients other than its own web page: when what
//! it holds changes, and which of the account's machines are online.
//!
//! Holding the connection is what makes a machine online. It names itself by host name, as on
//! git requests, and signs in with the account's token, which never leaves Rust.

use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use futures::StreamExt as _;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest as _};

/// What the hosted server says, as a client acts on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostedEvent {
    /// Something was published to the project with this root commit, or, without one, to any
    /// project while disconnected.
    Published(Option<String>),
    /// The account's other machines connected right now, by host name.
    Online(Vec<String>),
    /// Another machine sent this one a branch.
    Sent(crate::watcher::WatcherHostedSentPayload),
}

/// A connection kept open in the background, reconnecting as needed, until this is dropped.
pub struct HostedListener {
    _stop: watch::Sender<()>,
}

/// The server sends a heartbeat every 25s, so this much silence means the connection is dead.
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

/// Connect to the hosted server for the whole account, outside any project, as `but mesh` does.
pub fn listen_account(on_event: impl Fn(HostedEvent) + Send + 'static) -> Result<HostedListener> {
    // `http://` becomes `ws://`, and `https://` `wss://`.
    let url = format!("{}/events", super::hosted_server()).replacen("http", "ws", 1);
    let this = super::machine_name();
    let (stop, stopped) = watch::channel(());
    std::thread::Builder::new()
        .name("hosted-listener".into())
        .spawn(move || {
            // `wss://` needs rustls to know its crypto provider; something may have set it already.
            rustls::crypto::aws_lc_rs::default_provider()
                .install_default()
                .ok();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            match runtime {
                Ok(runtime) => runtime.block_on(run(url, this, on_event, stopped)),
                Err(err) => tracing::warn!("the hosted listener can't start: {err}"),
            }
        })?;
    Ok(HostedListener { _stop: stop })
}

/// Stay connected until stopped, waiting longer after each failed attempt.
async fn run(
    url: String,
    this: Option<String>,
    on_event: impl Fn(HostedEvent),
    mut stopped: watch::Receiver<()>,
) {
    let mut delay = Duration::from_secs(1);
    loop {
        match connect(&url, this.as_deref(), &on_event, &mut stopped).await {
            Ok(Ended::Stopped) => return,
            Ok(Ended::Dropped) => delay = Duration::from_secs(1),
            Err(err) => tracing::debug!("the hosted listener lost its connection: {err:#}"),
        }
        // Jitter keeps machines that lost the same server from reconnecting in lockstep.
        let jitter = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |now| now.subsec_millis()) as f64
            / 1000.0;
        tokio::select! {
            () = tokio::time::sleep(delay.mul_f64(0.5 + jitter / 2.0)) => {}
            _ = stopped.changed() => return,
        }
        delay = (delay * 2).min(MAX_RETRY_DELAY);
    }
}

enum Ended {
    /// The listener was dropped.
    Stopped,
    /// The connection worked, then closed.
    Dropped,
}

async fn connect(
    url: &str,
    this: Option<&str>,
    on_event: &impl Fn(HostedEvent),
    stopped: &mut watch::Receiver<()>,
) -> Result<Ended> {
    let mut request = url.into_client_request()?;
    let headers = request.headers_mut();
    headers.insert("x-auth-token", super::access_token()?.parse()?);
    headers.insert("x-but-client", super::client_name().parse()?);
    if let Some(this) = this {
        headers.insert("x-but-machine", this.parse()?);
    }
    let (mut socket, _response) = tokio_tungstenite::connect_async(request)
        .await
        .context("the hosted server can't be reached")?;
    // Anything may have been published while disconnected.
    on_event(HostedEvent::Published(None));

    loop {
        let frame = tokio::select! {
            frame = tokio::time::timeout(IDLE_TIMEOUT, socket.next()) => frame,
            _ = stopped.changed() => return Ok(Ended::Stopped),
        };
        let Ok(frame) = frame else {
            bail!("the hosted server went quiet");
        };
        let Some(frame) = frame else {
            return Ok(Ended::Dropped);
        };
        let Message::Text(text) = frame? else {
            continue;
        };
        let Ok(message) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        match message["channel"].as_str() {
            Some("projectsChanged") => on_event(HostedEvent::Published(
                message["payload"]["root"].as_str().map(ToOwned::to_owned),
            )),
            Some("sent") if this.is_some() && message["payload"]["to"].as_str() == this => {
                if let Ok(sent) = serde_json::from_value(message["payload"].clone()) {
                    on_event(HostedEvent::Sent(sent));
                }
            }
            Some("presence") => {
                let online = message["payload"]["online"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|machine| machine.as_str())
                    .filter(|machine| Some(*machine) != this)
                    .map(ToOwned::to_owned)
                    .collect();
                on_event(HostedEvent::Online(online));
            }
            // Replies, heartbeats, and the web page's per-project events.
            _ => {}
        }
    }
}
