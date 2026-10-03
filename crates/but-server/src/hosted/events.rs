//! The `/events` WebSocket: browsers follow publishes with the same watcher events a desktop
//! host gets from its file watcher.

use std::{collections::HashSet, sync::Arc, time::Duration};

use axum::{
    extract::{
        Extension, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use but_api::watcher::{WatcherGitActivityPayload, WatcherPayload};
use tokio::sync::broadcast;

use super::{HostedConfig, UserId, existing_store};

/// What reaches every `/events` socket, for each to pick out its own.
#[derive(Clone)]
pub(super) enum ProjectEvent {
    /// For `user`'s sockets: about one project, for those subscribed to it, or, without a
    /// project, about the user's projects as a whole, for all of them.
    Changed {
        user: UserId,
        project_id: Option<String>,
        /// Serialized once, for every socket.
        message: String,
    },
    /// A browser signed out: the sockets it opened close.
    SignedOut { session: String },
}

/// The channel of the event saying a user's set of projects may have changed.
const PROJECTS_CHANNEL: &str = "projectsChanged";

/// An event on the `/events` socket: the watcher event Lite's desktop host emits, on the channel
/// the web transport listens to.
#[derive(serde::Serialize)]
struct EventMessage {
    channel: String,
    payload: Option<WatcherEvent>,
}

/// The shape of `but-napi`'s `WatcherEvent`, which isn't available here.
#[derive(serde::Serialize)]
struct WatcherEvent {
    name: String,
    payload: WatcherPayload,
}

/// What a browser sends over `/events`: the projects whose events it wants, as Lite's desktop
/// host takes `watcherSubscribe` and `watcherUnsubscribe`.
#[derive(serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum EventsRequest {
    Subscribe { project_id: String },
    Unsubscribe { project_id: String },
}

/// What the server sends over `/events` besides events.
#[derive(serde::Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum EventsReply<'a> {
    /// The subscription is live: whatever is published from now on arrives as an event.
    Subscribed { project_id: &'a str },
    /// Not a published project, or the socket holds too many subscriptions.
    Rejected { project_id: &'a str },
    /// Sent on a timer, so both ends can tell a quiet socket from a dead one.
    Heartbeat,
}

/// Below the 60s idle timeout common to proxies, so they keep the socket open.
const EVENTS_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(25);
const MAX_SUBSCRIPTIONS_PER_SOCKET: usize = 64;
/// Publishes a socket may fall behind on before it's closed to resynchronize.
pub(super) const EVENTS_BUFFER: usize = 64;
/// Browsers only send small subscription messages.
const MAX_EVENTS_MESSAGE_SIZE: usize = 16 * 1024;

/// Whether a browser request comes from a page this server served; other clients send no
/// `Origin`.
pub(super) fn same_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let host = headers
        .get(header::HOST)
        .and_then(|host| host.to_str().ok());
    origin
        .to_str()
        .ok()
        .and_then(|origin| origin.parse::<Uri>().ok())
        .zip(host)
        .is_some_and(|(origin, host)| {
            origin
                .authority()
                .is_some_and(|authority| authority.as_str().eq_ignore_ascii_case(host))
        })
}

/// Signed in like any other route: a browser's handshake carries its session cookie.
pub(super) async fn events(
    State(config): State<Arc<HostedConfig>>,
    State(events): State<broadcast::Sender<ProjectEvent>>,
    Extension(user): Extension<UserId>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    // WebSockets aren't bound by CORS, so another site's page could open one with the
    // browser's cookie, were it not `SameSite=Strict`; this checks it too.
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let session = super::session_id(&headers).map(ToOwned::to_owned);
    let receiver = events.subscribe();
    ws.max_message_size(MAX_EVENTS_MESSAGE_SIZE)
        .on_upgrade(move |socket| forward_events(socket, receiver, config, user, session))
}

/// Subscriptions belong to the socket, so they end with it, as a desktop window's do. A socket
/// only ever gets its user's events, and closes when the session it came from signs out.
async fn forward_events(
    mut socket: WebSocket,
    mut events: broadcast::Receiver<ProjectEvent>,
    config: Arc<HostedConfig>,
    user: UserId,
    // The browser session it was opened in, whose end closes it; clients with a token have none.
    session: Option<String>,
) {
    let mut subscriptions = HashSet::new();
    let mut heartbeat = tokio::time::interval(EVENTS_HEARTBEAT_INTERVAL);
    // An interval ticks at once; the first heartbeat is due one interval after connecting.
    heartbeat.reset();
    loop {
        let message = tokio::select! {
            event = events.recv() => match event {
                Ok(ProjectEvent::Changed { user: of, project_id, message })
                    if of == user
                        && project_id.as_ref().is_none_or(|id| subscriptions.contains(id)) =>
                {
                    message
                }
                Ok(ProjectEvent::SignedOut { session: ended }) if session.as_ref() == Some(&ended) => {
                    return;
                }
                Ok(_) => continue,
                // Some events were dropped, and whose is unknown. Closing makes the page
                // reconnect, and each resubscription refreshes what it shows.
                Err(broadcast::error::RecvError::Lagged(_) | broadcast::error::RecvError::Closed) => {
                    return;
                }
            },
            _ = heartbeat.tick() => reply(EventsReply::Heartbeat),
            message = socket.recv() => match message {
                Some(Ok(Message::Text(text))) => match serde_json::from_str(&text) {
                    Ok(EventsRequest::Subscribe { project_id }) => {
                        let has_room = subscriptions.len() < MAX_SUBSCRIPTIONS_PER_SOCKET
                            || subscriptions.contains(&project_id);
                        let published = existing_store(&user.dir(&config), &project_id)
                            .inspect_err(|err| tracing::debug!("rejected a subscription: {err:#}"))
                            .is_ok();
                        if has_room && published {
                            subscriptions.insert(project_id.clone());
                            reply(EventsReply::Subscribed { project_id: &project_id })
                        } else {
                            reply(EventsReply::Rejected { project_id: &project_id })
                        }
                    }
                    Ok(EventsRequest::Unsubscribe { project_id }) => {
                        subscriptions.remove(&project_id);
                        continue;
                    }
                    Err(err) => {
                        tracing::debug!("ignored an /events message: {err:#}");
                        continue;
                    }
                },
                // The socket answers pings itself, and after a close the next `recv` ends.
                Some(Ok(Message::Binary(_) | Message::Ping(_) | Message::Pong(_) | Message::Close(_))) => {
                    continue;
                }
                Some(Err(_)) | None => return,
            },
        };
        if socket.send(Message::Text(message.into())).await.is_err() {
            return;
        }
    }
}

fn reply(reply: EventsReply<'_>) -> String {
    serde_json::to_string(&reply).expect("replies serialize")
}

/// Tell `user`'s browsers that `project` changed, as its file watcher would have, and that their
/// project list may have; `head_sha` is the published branch's tip.
pub(super) fn announce_publish(
    events: &broadcast::Sender<ProjectEvent>,
    user: UserId,
    project: &str,
    head_sha: String,
) {
    let watcher = EventMessage {
        channel: format!("watcher:{project}"),
        payload: Some(WatcherEvent {
            name: format!("project://{project}/git/activity"),
            payload: WatcherPayload::GitActivity(WatcherGitActivityPayload { head_sha }),
        }),
    };
    let projects = EventMessage {
        channel: PROJECTS_CHANNEL.to_owned(),
        payload: None,
    };
    for (project_id, message) in [(Some(project.to_owned()), watcher), (None, projects)] {
        // No receivers means no browser is open, which is fine.
        events
            .send(ProjectEvent::Changed {
                user,
                project_id,
                message: serde_json::to_string(&message).expect("events serialize"),
            })
            .ok();
    }
}
