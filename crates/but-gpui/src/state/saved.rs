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
            // Detached, so a newer save replacing this task can't cancel the write.
            cx.background_spawn(async move { Saved::write(&bytes) })
                .detach();
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
            file_sort: self.file_sort,
            window: self.window.clone(),
            prefs: self.prefs.clone(),
            kept_snapshots: {
                let kept = self.kept_snapshots.lock().expect("not poisoned");
                let mut ids: Vec<String> = kept.iter().map(|id| id.0.to_string()).collect();
                ids.sort();
                ids
            },
        }
    }

    pub fn runs_hooks(&self, project: &super::ProjectId) -> bool {
        self.prefs.run_hooks.contains(project.as_ref())
    }

    pub fn lands_directly(&self, project: &super::ProjectId) -> bool {
        self.prefs.land_directly.contains(project.as_ref())
    }

    pub fn project_icon(&self, project: &super::ProjectId) -> Option<&str> {
        self.prefs
            .project_icons
            .get(project.as_ref())
            .map(String::as_str)
    }

    pub fn set_prefs(&mut self, update: impl FnOnce(&mut Prefs), cx: &mut Context<Self>) {
        update(&mut self.prefs);
        self.save_soon(cx);
        cx.notify();
    }

    /// Change what only the diff shows. The detail redraws its diff itself, and telling the
    /// whole window would redraw the navigator and the rest with it, past a frame's budget.
    pub fn set_diff_prefs(&mut self, update: impl FnOnce(&mut Prefs), cx: &mut Context<Self>) {
        update(&mut self.prefs);
        self.save_soon(cx);
    }

    pub fn set_file_tree(&mut self, tree: bool, cx: &mut Context<Self>) {
        self.file_tree = tree;
        self.save_soon(cx);
        cx.notify();
    }

    pub fn set_file_sort(&mut self, sort: FileSort, cx: &mut Context<Self>) {
        self.file_sort = sort;
        self.save_soon(cx);
        cx.notify();
    }

    /// `pane`'s width in a window `window` wide, beside the navigator and the file list as
    /// they're shown: as set, unless that leaves the diff too narrow to read. Then the file
    /// list gives way first, down to its limit, then the navigator, down to what its header
    /// needs.
    pub fn fitted_width(&self, pane: Pane, window: f32, navigator: bool, files: bool) -> f32 {
        const MIN_DIFF: f32 = 400.;
        const MIN_NAVIGATOR: f32 = 300.;
        let shown = |pane: Pane, on: bool| if on { self.width(pane) } else { 0. };
        let (nav, list) = (shown(Pane::Navigator, navigator), shown(Pane::Files, files));
        let over = (nav + list + MIN_DIFF - window).max(0.);
        let list_fit = (list - over).max(Pane::Files.limits().0.min(list));
        match pane {
            Pane::Files => list_fit,
            Pane::Navigator => (nav - (over - (list - list_fit))).max(MIN_NAVIGATOR.min(nav)),
        }
    }

    pub fn width(&self, pane: Pane) -> f32 {
        let (min, max) = pane.limits();
        (self.widths.get(&pane)).map_or(pane.default_width(), |width| width.clamp(min, max))
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
            // Wide enough for the composer's Cancel, Amend and Commit.
            Pane::Files => (240., 560.),
        }
    }
}

/// The part of the saved state that's only read at startup.
#[derive(Default)]
pub(super) struct Restored {
    pub(super) expanded: Vec<String>,
    pub(super) selection: Option<String>,
}

/// The order file lists show files in, as but.dev's do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileSort {
    /// By path.
    #[default]
    Name,
    /// The last written first.
    Modified,
}

/// The order the navigator's watched projects go in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchedOrder {
    /// As the user put them, by dragging their cards or Move up and Move down.
    #[default]
    Manual,
    Name,
    /// The one something last changed in first, as an agent at work.
    Activity,
}

/// How many unchanged lines the rest of a hunk's function or type may add for a diff to show
/// it whole.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScopeLines {
    Short,
    #[default]
    Medium,
    Long,
}

impl ScopeLines {
    pub fn lines(self) -> usize {
        match self {
            ScopeLines::Short => 20,
            ScopeLines::Medium => 50,
            ScopeLines::Long => 200,
        }
    }
}

/// The little the app remembers between launches.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Saved {
    /// The watched projects, in the order the navigator shows them.
    pub(super) expanded: Vec<String>,
    pub(super) selection: Option<String>,
    pub(super) seen: Seen,
    pub(super) widths: HashMap<Pane, f32>,
    pub(super) file_tree: bool,
    pub(super) file_sort: FileSort,
    pub(super) window: Option<WindowPlace>,
    pub(super) prefs: Prefs,
    /// The oplog entries ⌘Z mustn't take back, as the oplog can't restore all they changed,
    /// which stays so after a restart.
    pub(super) kept_snapshots: Vec<String>,
}

/// Choices from Settings, and how the navigator lists projects, that only this app keeps;
/// GitButler's shared settings live with the other apps'.
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
    /// Changed lines go without their green and red ground, keeping the bar beside them.
    pub plain_lines: bool,
    pub code_size: crate::theme::CodeSize,
    /// Diffs leave out what deltoids reads from the code around each hunk, and skip reading
    /// it: the function or type the hunk changes, and the rest of that when it's short.
    pub no_semantic_context: bool,
    pub whole_scopes: ScopeLines,
    /// Projects, by id, whose Git hooks run around commits and pushes. Off unless picked,
    /// as in the desktop app.
    pub run_hooks: HashSet<String>,
    /// Projects, by id, whose branches offer Land in place of Create pull request.
    pub land_directly: HashSet<String>,
    /// The emoji each project shows before its name in the navigator, by id, as but.dev
    /// lets a repository have one.
    pub project_icons: HashMap<String, String>,
    /// The navigator's other projects go by when they were last active rather than by name.
    pub projects_by_activity: bool,
    /// The navigator's other projects leave out those not active in the last 7 days.
    pub active_projects_only: bool,
    /// The navigator's other projects are folded away under their heading.
    pub projects_folded: bool,
    /// Watched projects, by id, whose cards are folded to their row; they're still loaded and
    /// kept current.
    pub folded_projects: HashSet<String>,
    /// Watched projects, by id, whose work on other machines shows under its heading; it
    /// starts folded.
    pub shown_mesh: HashSet<String>,
    /// The order the watched projects go in.
    pub watched_order: WatchedOrder,
    /// The Branches page lists branches with no commits of their own too, as Lite's can.
    pub branches_with_empty: bool,
    /// The Branches page leaves out the branches only a remote has.
    pub branches_local_only: bool,
    /// The navigator's branches start with their commits listed, rather than folded to a
    /// row each.
    pub branches_start_open: bool,
}

impl Prefs {
    /// The most unchanged lines a hunk's scope adds around it, `None` with semantic context
    /// off.
    pub fn scope_lines(&self) -> Option<usize> {
        (!self.no_semantic_context).then(|| self.whole_scopes.lines())
    }
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
