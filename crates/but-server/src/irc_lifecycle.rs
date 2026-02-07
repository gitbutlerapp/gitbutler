//! IRC connection lifecycle management for but-server.
//!
//! Provides a [`BroadcasterEmitter`] that implements [`but_irc::lifecycle::EventEmitter`],
//! bridging the shared lifecycle logic to the WebSocket broadcaster.

use but_claude::Broadcaster;
use but_claude::broadcaster::FrontendEvent;
use but_irc::IrcManager;
use but_irc::lifecycle::EventEmitter;
use but_settings::app_settings::IrcSettings;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Event emitter that forwards IRC events via the WebSocket [`Broadcaster`].
#[derive(Clone)]
pub struct BroadcasterEmitter {
    broadcaster: Arc<Mutex<Broadcaster>>,
}

impl BroadcasterEmitter {
    pub fn new(broadcaster: &Arc<Mutex<Broadcaster>>) -> Self {
        Self {
            broadcaster: broadcaster.clone(),
        }
    }
}

impl EventEmitter for BroadcasterEmitter {
    fn emit(&self, name: &str, payload: serde_json::Value) {
        let event = FrontendEvent {
            name: name.to_string(),
            payload,
        };
        // Fire-and-forget: events may arrive out of order under high concurrency.
        let broadcaster = self.broadcaster.clone();
        tokio::spawn(async move {
            broadcaster.lock().await.send(event);
        });
    }
}

/// Auto-connect IRC connections based on current settings.
pub fn auto_connect_on_startup(
    irc_manager: &IrcManager,
    broadcaster: &Arc<Mutex<Broadcaster>>,
    irc_settings: &IrcSettings,
) {
    let emitter = BroadcasterEmitter::new(broadcaster);
    but_irc::lifecycle::auto_connect_on_startup(irc_manager, &emitter, irc_settings);
}

/// React to IRC settings changes.
pub fn on_settings_changed(
    irc_manager: &IrcManager,
    broadcaster: &Arc<Mutex<Broadcaster>>,
    old: &IrcSettings,
    new: &IrcSettings,
) {
    let emitter = BroadcasterEmitter::new(broadcaster);
    but_irc::lifecycle::on_settings_changed(irc_manager, &emitter, old, new);
}
