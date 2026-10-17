//! What the app remembers between launches, and the pane widths it keeps.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use gpui::{App, AppContext as _, Bounds, Context, DisplayId, Pixels, Window, point, px, size};
use serde::{Deserialize, Serialize};

use super::{Seen, Store};
use crate::theme::Appearance;

impl Store {
    /// Write the saved state shortly, so a burst of changes writes once. A steady stream
    /// of changes still writes every half second.
    pub(super) fn save_soon(&mut self, cx: &mut Context<Self>) {
        if self.save_queued {
            return;
        }
        self.save_queued = true;
        self.save_task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let Ok(Some(bytes)) = this.update(cx, |store, _| {
                store.save_queued = false;
                store.changed_state()
            }) else {
                return;
            };
            cx.background_spawn(async move { Saved::write(&bytes) })
                .await;
        });
    }

    /// Write the saved state now, as on quit, even if a background save of the same state
    /// hasn't finished: the process may end before it does.
    pub(super) fn save(&mut self) {
        if let Ok(bytes) = serde_json::to_vec_pretty(&self.saved()) {
            Saved::write(&bytes);
            self.last_saved = bytes;
        }
    }

    /// The state to save, unless it's what was saved last.
    fn changed_state(&mut self) -> Option<Vec<u8>> {
        let bytes = serde_json::to_vec_pretty(&self.saved()).ok()?;
        if bytes == self.last_saved {
            return None;
        }
        self.last_saved = bytes.clone();
        Some(bytes)
    }

    fn saved(&self) -> Saved {
        Saved {
            expanded: self
                .projects
                .iter()
                .filter(|entry| entry.expanded)
                .map(|entry| entry.id.to_string())
                .collect(),
            selection: self.selection.as_ref().map(|s| s.project().to_string()),
            seen: self.seen.clone(),
            widths: self.widths.clone(),
            file_tree: self.file_tree,
            window: self.window.clone(),
            prefs: self.prefs.clone(),
        }
    }

    pub fn runs_hooks(&self, project: &super::ProjectId) -> bool {
        self.prefs.run_hooks.contains(project.as_ref())
    }

    pub fn lands_directly(&self, project: &super::ProjectId) -> bool {
        self.prefs.land_directly.contains(project.as_ref())
    }

    pub fn set_prefs(&mut self, update: impl FnOnce(&mut Prefs), cx: &mut Context<Self>) {
        update(&mut self.prefs);
        self.save_soon(cx);
        cx.notify();
    }

    pub fn set_file_tree(&mut self, tree: bool, cx: &mut Context<Self>) {
        self.file_tree = tree;
        self.save_soon(cx);
        cx.notify();
    }

    pub fn width(&self, pane: Pane) -> f32 {
        self.widths
            .get(&pane)
            .copied()
            .unwrap_or(pane.default_width())
    }

    /// Remember where `window` is, for the next launch.
    pub fn save_window_place(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(display) = window.display(cx).and_then(|display| display.uuid().ok()) else {
            return;
        };
        let Bounds { origin, size } = window.window_bounds().get_bounds();
        self.window = Some(WindowPlace {
            display: display.to_string(),
            bounds: [
                origin.x.into(),
                origin.y.into(),
                size.width.into(),
                size.height.into(),
            ],
        });
        self.save_soon(cx);
    }

    /// Resize `pane`, within its limits, or put it back to its default with `None`.
    pub fn set_width(&mut self, pane: Pane, width: Option<f32>, cx: &mut Context<Self>) {
        let (min, max) = pane.limits();
        match width {
            Some(width) => self.widths.insert(pane, width.clamp(min, max)),
            None => self.widths.remove(&pane),
        };
        self.save_soon(cx);
        cx.notify();
    }
}

/// Where the window was when the app last quit, while that display is still connected and
/// the window would still show on it.
pub fn saved_window_place(cx: &App) -> Option<(DisplayId, Bounds<Pixels>)> {
    let place = Saved::load().window?;
    let display = cx.displays().into_iter().find(|display| {
        display
            .uuid()
            .is_ok_and(|uuid| uuid.to_string() == place.display)
    })?;
    let [x, y, width, height] = place.bounds;
    let bounds = Bounds::new(point(px(x), px(y)), size(px(width), px(height)));
    let screen = Bounds::new(point(px(0.), px(0.)), display.bounds().size);
    screen.intersects(&bounds).then(|| (display.id(), bounds))
}

/// A pane whose width can be dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pane {
    Navigator,
    /// The file list beside a diff.
    Files,
}

impl Pane {
    fn default_width(self) -> f32 {
        match self {
            Pane::Navigator => 340.,
            Pane::Files => 300.,
        }
    }

    fn limits(self) -> (f32, f32) {
        match self {
            Pane::Navigator => (220., 640.),
            Pane::Files => (200., 560.),
        }
    }
}

/// The part of the saved state that's only read at startup.
#[derive(Default)]
pub(super) struct Restored {
    pub(super) expanded: HashSet<String>,
    pub(super) selection: Option<String>,
}

/// The little the app remembers between launches.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Saved {
    pub(super) expanded: HashSet<String>,
    pub(super) selection: Option<String>,
    pub(super) seen: Seen,
    pub(super) widths: HashMap<Pane, f32>,
    pub(super) file_tree: bool,
    pub(super) window: Option<WindowPlace>,
    pub(super) prefs: Prefs,
}

/// Choices from Settings that only this app keeps; GitButler's shared settings live with
/// the other apps'.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub appearance: Appearance,
    /// The editor files open in, by program id. Asked for the first time it's needed.
    pub editor: Option<String>,
    /// The terminal the project opens in, by id.
    pub terminal: Option<String>,
    /// Long diff lines wrap instead of running off the edge.
    pub wrap: bool,
    /// Diffs show the old side beside the new rather than one above the other.
    pub split: bool,
    pub code_size: crate::theme::CodeSize,
    /// Projects, by id, whose Git hooks run around commits and pushes. Off unless picked,
    /// as in the desktop app.
    pub run_hooks: HashSet<String>,
    /// Projects, by id, whose branches offer Land in place of Create pull request.
    pub land_directly: HashSet<String>,
}

/// Where the window was: the display it was on, and its left, top, width and height on that
/// display. A full-screen window keeps the size it returns to.
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct WindowPlace {
    display: String,
    bounds: [f32; 4],
}

impl Saved {
    fn path() -> Option<PathBuf> {
        Some(
            crate::backend::settings::config_dir()
                .ok()?
                .join("gpui-state.json"),
        )
    }

    pub(super) fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(bytes) = std::fs::read(&path) else {
            return Self::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_else(|err| {
            // Keep what couldn't be read rather than write over it.
            tracing::warn!("couldn't read {}: {err}", path.display());
            std::fs::rename(&path, path.with_extension("json.bak")).ok();
            Self::default()
        })
    }

    /// Through a temporary file, so a crash mid-write leaves the last state rather than
    /// half a file, which would read back as nothing at all.
    fn write(bytes: &[u8]) {
        // One writer at a time: a background save can still be going when quit saves.
        static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _writing = WRITING.lock();
        let Some(path) = Self::path() else {
            return;
        };
        let temporary = path.with_extension("json.tmp");
        if std::fs::write(&temporary, bytes).is_ok() {
            std::fs::rename(&temporary, &path).ok();
        }
    }
}
