//! The `/events` WebSocket: browsers follow publishes with the same watcher events a desktop
//! host gets from its file watcher.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use but_api::watcher::{WatcherGitActivityPayload, WatcherPayload};
use tokio::sync::broadcast;

use super::{HostedConfig, existing_store};
use crate::cmd_result_to_json;

/// Single-use tickets for opening `/events`. In memory: a restart only makes clients ask again,
/// but each server instance only knows the tickets it issued.
pub(super) type EventTickets = Arc<Mutex<HashMap<String, Instant>>>;

const EVENT_TICKET_LIFETIME: Duration = Duration::from_secs(30);

/// A watcher event for one published project, for the `/events` sockets subscribed to it.
#[derive(Clone)]
pub(super) struct ProjectEvent {
    project_id: String,
    /// Serialized once, for every socket.
    message: String,
}

/// An event on the `/events` socket: the watcher event Lite's desktop host emits, on the channel
/// the web transport listens to.
#[derive(serde::Serialize)]
struct EventMessage {
    channel: String,
    payload: WatcherEvent,
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
/// Unredeemed tickets are swept when they expire, and capped until then.
const MAX_OUTSTANDING_TICKETS: usize = 1024;

pub(super) async fn events_ticket(State(tickets): State<EventTickets>) -> Response {
    let ticket = issue_ticket(
        &mut tickets.lock().unwrap_or_else(PoisonError::into_inner),
        Instant::now(),
    );
    let Some(ticket) = ticket else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    // A ticket is a credential until it's redeemed.
    (
        [(header::CACHE_CONTROL, "no-store")],
        cmd_result_to_json(Ok(serde_json::json!({ "ticket": ticket }))),
    )
        .into_response()
}

/// A new ticket valid until `now + EVENT_TICKET_LIFETIME`, sweeping the expired ones first,
/// or `None` while `MAX_OUTSTANDING_TICKETS` are still unredeemed.
fn issue_ticket(tickets: &mut HashMap<String, Instant>, now: Instant) -> Option<String> {
    tickets.retain(|_, expires| *expires > now);
    if tickets.len() >= MAX_OUTSTANDING_TICKETS {
        return None;
    }
    let ticket = uuid::Uuid::new_v4().to_string();
    tickets.insert(ticket.clone(), now + EVENT_TICKET_LIFETIME);
    Some(ticket)
}

/// Whether `ticket` was issued and is unexpired at `now`. Redeeming spends it either way.
fn redeem_ticket(tickets: &mut HashMap<String, Instant>, ticket: &str, now: Instant) -> bool {
    tickets.remove(ticket).is_some_and(|expires| expires > now)
}

#[derive(serde::Deserialize)]
pub(super) struct EventsQuery {
    ticket: String,
}

pub(super) async fn events(
    State(config): State<Arc<HostedConfig>>,
    State(tickets): State<EventTickets>,
    State(events): State<broadcast::Sender<ProjectEvent>>,
    Query(query): Query<EventsQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    // Pages come from this server, so a browser's socket is same-origin; other clients send
    // no `Origin`. This backs up the ticket, it doesn't replace it.
    if let Some(origin) = headers.get(header::ORIGIN) {
        let host = headers
            .get(header::HOST)
            .and_then(|host| host.to_str().ok());
        let same_origin = origin
            .to_str()
            .ok()
            .and_then(|origin| origin.parse::<Uri>().ok())
            .zip(host)
            .is_some_and(|(origin, host)| {
                origin
                    .authority()
                    .is_some_and(|authority| authority.as_str().eq_ignore_ascii_case(host))
            });
        if !same_origin {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let redeemed = redeem_ticket(
        &mut tickets.lock().unwrap_or_else(PoisonError::into_inner),
        &query.ticket,
        Instant::now(),
    );
    if !redeemed {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let receiver = events.subscribe();
    ws.max_message_size(MAX_EVENTS_MESSAGE_SIZE)
        .on_upgrade(move |socket| forward_events(socket, receiver, config))
}

/// Subscriptions belong to the socket, so they end with it, as a desktop window's do.
async fn forward_events(
    mut socket: WebSocket,
    mut events: broadcast::Receiver<ProjectEvent>,
    config: Arc<HostedConfig>,
) {
    let mut subscriptions = HashSet::new();
    let mut heartbeat = tokio::time::interval(EVENTS_HEARTBEAT_INTERVAL);
    // An interval ticks at once; the first heartbeat is due one interval after connecting.
    heartbeat.reset();
    loop {
        let message = tokio::select! {
            event = events.recv() => match event {
                Ok(event) if subscriptions.contains(&event.project_id) => event.message,
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
                        let published = existing_store(&config, &project_id)
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

/// Tell browsers that `project` changed, as its file watcher would have; `head_sha` is the
/// published branch's tip.
pub(super) fn announce_publish(
    events: &broadcast::Sender<ProjectEvent>,
    project: &str,
    head_sha: String,
) {
    let project_id = project.to_owned();
    let message = EventMessage {
        channel: format!("watcher:{project_id}"),
        payload: WatcherEvent {
            name: format!("project://{project_id}/git/activity"),
            payload: WatcherPayload::GitActivity(WatcherGitActivityPayload { head_sha }),
        },
    };
    // No receivers means no browser is open, which is fine.
    events
        .send(ProjectEvent {
            project_id,
            message: serde_json::to_string(&message).expect("events serialize"),
        })
        .ok();
}
