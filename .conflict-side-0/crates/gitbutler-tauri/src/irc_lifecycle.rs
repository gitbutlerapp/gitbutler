//! IRC connection lifecycle management for the Tauri desktop app.
//!
//! Provides a [`TauriEmitter`] that implements [`but_irc::lifecycle::EventEmitter`],
//! bridging the shared lifecycle logic to Tauri's event system.

use but_irc::lifecycle::EventEmitter;
use tauri::{AppHandle, Emitter, Manager};

/// Event emitter that forwards IRC events via Tauri's `AppHandle::emit`.
#[derive(Clone)]
pub struct TauriEmitter {
    app_handle: AppHandle,
}

impl TauriEmitter {
    pub fn new(app_handle: &AppHandle) -> Self {
        Self {
            app_handle: app_handle.clone(),
        }
    }
}

impl EventEmitter for TauriEmitter {
    fn emit(&self, name: &str, payload: serde_json::Value) {
        let _ = self.app_handle.emit(name, payload);
    }
}

/// Auto-connect IRC connections based on current settings.
pub fn auto_connect_on_startup(
    app_handle: &AppHandle,
    irc_settings: &but_settings::app_settings::IrcSettings,
) {
    let manager = app_handle.state::<but_irc::IrcManager>().inner().clone();
    let emitter = TauriEmitter::new(app_handle);
    but_irc::lifecycle::auto_connect_on_startup(&manager, &emitter, irc_settings);
}

/// React to IRC settings changes.
pub fn on_settings_changed(
    app_handle: &AppHandle,
    old: &but_settings::app_settings::IrcSettings,
    new: &but_settings::app_settings::IrcSettings,
) {
    let manager = app_handle.state::<but_irc::IrcManager>().inner().clone();
    let emitter = TauriEmitter::new(app_handle);
    but_irc::lifecycle::on_settings_changed(&manager, &emitter, old, new);
}
