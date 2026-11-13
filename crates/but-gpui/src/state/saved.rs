//! What the app remembers between launches, and the pane widths it keeps.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use gpui::{
    App, AppContext as _, Bounds, Context, DisplayId, Pixels, Window, WindowBounds, point, size,
};
use serde::{Deserialize, Serialize};

use super::{Seen, Selection, Store};
use crate::backend::saved::Database;
use crate::clock;
use crate::theme::Appearance;
use crate::ui::fold::Glide;

/// How long a pane takes to slide back to its default width.
const PUT_BACK: Duration = Duration::from_millis(110);

impl Store {
    /// Write the saved state shortly, so a burst of changes writes once. A steady stream
    /// of changes still writes every half second.
    pub(super) fn save_soon(&mut self, cx: &mut Context<Self>) {
        if self.save_queued {
            return;
        }
        self.save_queued = true;
        let db = self.saved_db.clone();
        self.save_task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let Ok((version, rows)) = this.update(cx, |store, _| {
                store.save_queued = false;
                store.next_save()
            }) else {
                return;
            };
            // Detached, so a newer save replacing this task can't cancel the write.
            cx.background_spawn(async move { db.write(version, rows) })
                .detach();
        });
    }

    /// Write the saved state now, as on quit: the process may end before a background save
    /// would.
    pub(super) fn save(&mut self) {
        let (version, rows) = self.next_save();
        self.saved_db.write(version, rows);
    }

    /// The saved state's rows, numbered in the order they're taken, so a background write
    /// that comes late can't undo a newer one.
    fn next_save(&mut self) -> (u64, Vec<(String, String)>) {
        self.saves += 1;
        (self.saves, self.saved().rows())
    }

    fn saved(&self) -> Saved {
        // In one order, so a save of the same state writes the same bytes.
        let mut activity: Vec<SavedActivity> = (self.activity.iter())
            .map(|((project, worktree), activity)| SavedActivity {
                project: project.to_string(),
                worktree: worktree.linked().map(|name| name.to_string()),
                activity: activity.clone(),
            })
            .collect();
        activity.sort_by(|a, b| (&a.project, &a.worktree).cmp(&(&b.project, &b.worktree)));
        Saved {
            // Those saved last time too until the projects are listed, which can fail.
            expanded: (self.projects.iter().filter(|entry| entry.expanded))
                .map(|entry| entry.id.to_string())
                .chain(self.restored.expanded.iter().cloned())
                .collect(),
            selection: (self.selection.as_ref().map(|s| s.project().to_string()))
                .or_else(|| self.restored.selection.clone()),
            seen: self.seen.clone(),
            widths: self.widths.clone(),
            window: self.window.clone(),
            prefs: self.prefs.clone(),
            kept_by_project: (self.kept_snapshots.iter())
                .map(|(project, kept)| {
                    let mut ids: Vec<String> = kept.iter().map(|id| id.0.to_string()).collect();
                    ids.sort();
                    (project.to_string(), ids)
                })
                .collect(),
            activity,
        }
    }

    /// Forget what's kept for the projects not in the listing, as one removed, and the
    /// activity of checkouts quiet for longer than it's kept, as a deleted worktree's, so it
    /// isn't saved on and on.
    pub(super) fn forget_unlisted(&mut self) {
        let listed: HashSet<&str> = self.projects.iter().map(|entry| &*entry.id).collect();
        let listed = |id: &str| listed.contains(id);
        // What ⌘Z and ⇧⌘Z would do in one, which can't be done without it.
        self.undo_steps.retain(|step| listed(&step.project));
        self.redo_steps.retain(|(id, _)| listed(id));
        self.ran.retain(|id, _| listed(id));
        // None listed is more likely a list that couldn't be read than every one removed.
        if self.projects.is_empty() {
            return;
        }
        self.seen.commits.retain(|id, _| listed(id));
        self.kept_snapshots.retain(|id, _| listed(id));
        let since = crate::ui::now() - super::activity::KEPT_FOR;
        (self.activity).retain(|(id, _), activity| {
            let latest = activity.last.max(activity.commits.first().map(|c| c.time));
            listed(id) && latest.is_some_and(|at| at > since)
        });
        let prefs = &mut self.prefs;
        let sets = [
            &mut prefs.run_hooks,
            &mut prefs.folded_projects,
            &mut prefs.shown_mesh,
            &mut prefs.shown_inactive,
        ];
        for ids in sets {
            ids.retain(|id| listed(id));
        }
        prefs.project_icons.retain(|id, _| listed(id));
        prefs.merge_methods.retain(|id, _| listed(id));
        prefs.worktree_order.retain(|id, _| listed(id));
    }

    pub fn runs_hooks(&self, project: &super::ProjectId) -> bool {
        self.prefs.run_hooks.contains(project.as_ref())
    }

    /// How pull requests in `project` merge, if one was picked there.
    pub fn picked_merge_method(
        &self,
        project: &super::ProjectId,
    ) -> Option<crate::backend::reviews::MergeMethod> {
        self.prefs.merge_methods.get(project.as_ref()).copied()
    }

    pub fn project_icon(&self, project: &super::ProjectId) -> Option<&str> {
        self.prefs
            .project_icons
            .get(project.as_ref())
            .map(String::as_str)
    }

    /// Give `project` the `icon`, as but.dev stores it, or none, here and in the mesh.
    pub fn set_project_icon(
        &mut self,
        project: &super::ProjectId,
        icon: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let id = project.to_string();
        self.set_prefs(
            |prefs| match &icon {
                Some(icon) => _ = prefs.project_icons.insert(id, icon.clone()),
                None => _ = prefs.project_icons.remove(&id),
            },
            cx,
        );
        self.mesh_send_icon(project, icon.unwrap_or_default(), cx);
    }

    pub fn set_prefs(&mut self, update: impl FnOnce(&mut Prefs), cx: &mut Context<Self>) {
        update(&mut self.prefs);
        self.save_soon(cx);
        cx.notify();
    }

    /// Count worktrees as inactive `after`, which can empty the heading selected.
    pub fn set_inactive_after(
        &mut self,
        after: crate::backend::InactiveAfter,
        cx: &mut Context<Self>,
    ) {
        self.set_prefs(|prefs| prefs.inactive_after = after, cx);
        if let Some(project) = self.selection.as_ref().map(|s| s.project().clone()) {
            self.validate_selection(&project, cx);
        }
    }

    /// Change what only the diff shows. The detail redraws its diff itself, and telling the
    /// whole window would redraw the navigator and the rest with it, past a frame's budget.
    pub fn set_diff_prefs(&mut self, update: impl FnOnce(&mut Prefs), cx: &mut Context<Self>) {
        update(&mut self.prefs);
        self.save_soon(cx);
    }

    /// `pane`'s width in a window `window` wide, beside the navigator and the page's column,
    /// `list` (the file list, or the graph), as much as each shows, from 0 hidden to 1, as they
    /// slide: as set, unless that leaves the diff too narrow to read. Then the column gives way
    /// first, down to its limit, then the navigator, down to what its header needs.
    pub fn fitted_width(
        &self,
        pane: Pane,
        list: Pane,
        window: f32,
        navigator: f32,
        files: f32,
    ) -> f32 {
        const MIN_DIFF: f32 = 400.;
        const MIN_NAVIGATOR: f32 = 300.;
        let shown = |pane: Pane, share: f32| self.width(pane) * share;
        let (nav, list_width) = (shown(Pane::Navigator, navigator), shown(list, files));
        let over = (nav + list_width + MIN_DIFF - window).max(0.);
        let list_fit = (list_width - over).max(list.limits().0.min(list_width));
        match pane {
            Pane::Navigator => (nav - (over - (list_width - list_fit))).max(MIN_NAVIGATOR.min(nav)),
            pane if pane == list => list_fit,
            // Fitted to what the list page leaves it, by `Detail::beside_files_width`.
            Pane::Files | Pane::BesideFiles | Pane::Graph | Pane::Terminals => self.width(pane),
        }
    }

    /// The width `pane` was dragged to, if it was, as the terminal overview fits its rows
    /// until it is.
    pub fn dragged_width(&self, pane: Pane) -> Option<f32> {
        let (min, max) = pane.limits();
        (self.widths.get(&pane)).map(|width| width.clamp(min, max))
    }

    pub fn width(&self, pane: Pane) -> f32 {
        let (min, max) = pane.limits();
        let width =
            (self.widths.get(&pane)).map_or(pane.default_width(), |width| width.clamp(min, max));
        match self.width_glide(pane) {
            Some(glide) => glide.at(width, clock::now()),
            None => width,
        }
    }

    /// How `pane` slides back to its default width, while it does.
    pub fn width_glide(&self, pane: Pane) -> Option<Glide> {
        (self.width_glide)
            .filter(|(gliding, glide)| *gliding == pane && glide.moving(clock::now()))
            .map(|(_, glide)| glide)
    }

    /// Whether a pane is sliding back to its default width.
    pub fn widths_gliding(&self) -> bool {
        (self.width_glide).is_some_and(|(_, glide)| glide.moving(clock::now()))
    }

    /// Put `pane` back to its default width, sliding there from `from`, as wide as it shows
    /// now, as the lines between tiles do.
    pub fn put_back_width(&mut self, pane: Pane, from: f32, cx: &mut Context<Self>) {
        let mut glide = Glide::lasting(PUT_BACK);
        glide.start(from, cx);
        self.set_width(pane, None, cx);
        self.width_glide = Some((pane, glide));
    }

    /// Remember where `window` is, for the next launch.
    pub fn save_window_place(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(display) = window.display(cx).and_then(|display| display.uuid().ok()) else {
            return;
        };
        let bounds = window.window_bounds();
        let Bounds { origin, size } = bounds.get_bounds();
        self.window = Some(WindowPlace {
            display: display.to_string(),
            bounds: [
                origin.x.into(),
                origin.y.into(),
                size.width.into(),
                size.height.into(),
            ],
            fills: match bounds {
                WindowBounds::Windowed(_) => None,
                WindowBounds::Maximized(_) => Some(Fill::Maximized),
                WindowBounds::Fullscreen(_) => Some(Fill::FullScreen),
            },
        });
        self.save_soon(cx);
    }

    /// Where the window was when the app last quit, while that display is still connected and
    /// the window would still show on it.
    pub fn window_place(&self, cx: &App) -> Option<(DisplayId, WindowBounds)> {
        let place = self.window.as_ref()?;
        let display = cx.displays().into_iter().find(|display| {
            display
                .uuid()
                .is_ok_and(|uuid| uuid.to_string() == place.display)
        })?;
        let [x, y, width, height] = place.bounds;
        let bounds = Bounds::new(
            point(Pixels::from(x), Pixels::from(y)),
            size(Pixels::from(width), Pixels::from(height)),
        );
        let screen = Bounds::new(point(Pixels::ZERO, Pixels::ZERO), display.bounds().size);
        if !screen.intersects(&bounds) {
            return None;
        }
        let bounds = match place.fills {
            None => WindowBounds::Windowed(bounds),
            Some(Fill::Maximized) => WindowBounds::Maximized(bounds),
            Some(Fill::FullScreen) => WindowBounds::Fullscreen(bounds),
        };
        Some((display.id(), bounds))
    }

    /// Resize `pane`, within its limits, or put it back to its default with `None`.
    pub fn set_width(&mut self, pane: Pane, width: Option<f32>, cx: &mut Context<Self>) {
        let (min, max) = pane.limits();
        match width {
            Some(width) => self.widths.insert(pane, width.clamp(min, max)),
            None => self.widths.remove(&pane),
        };
        self.width_glide = None;
        self.save_soon(cx);
        cx.notify();
    }
}

/// A pane whose width can be dragged, or its height, the one under another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pane {
    Navigator,
    /// The file list beside a diff.
    Files,
    /// The files between a list page's list and the diff.
    BesideFiles,
    /// The graph beside the diff of the commit pinned on it.
    Graph,
    /// The terminal overview under the navigator, by its height.
    Terminals,
}

impl Pane {
    fn default_width(self) -> f32 {
        match self {
            Pane::Navigator => 340.,
            Pane::Files => 300.,
            Pane::BesideFiles => 260.,
            Pane::Graph => 560.,
            // Unless dragged, it fits its rows: see `dragged_width`.
            Pane::Terminals => 240.,
        }
    }

    pub(crate) fn limits(self) -> (f32, f32) {
        match self {
            // As wide as the window leaves it beside a readable diff: see `fitted_width`.
            Pane::Navigator => (220., 2000.),
            // Wide enough for the composer's Cancel and Commit.
            Pane::Files => (240., 560.),
            Pane::BesideFiles => (200., 560.),
            // A lane's commits, at 100%.
            Pane::Graph => (320., 2000.),
            // Its bar and a row. The column's height limits it too.
            Pane::Terminals => (72., 2000.),
        }
    }
}

/// The part of the saved state that's only read at startup.
#[derive(Default, Clone)]
pub(super) struct Restored {
    pub(super) expanded: Vec<String>,
    pub(super) selection: Option<String>,
    /// The row the build this one took over from had selected, which goes before `selection`.
    pub(super) handed: Option<Selection>,
    /// Nothing was saved before, as on the first launch.
    pub(super) fresh: bool,
}

/// How file lists show their files, as but.dev's can.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileView {
    /// As a tree of their folders.
    #[default]
    Tree,
    /// Flat, each with its folder beside its name.
    List,
}

/// The order file lists show files in, as but.dev's do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileSort {
    /// By path.
    #[default]
    Name,
    /// Names in capitals first, as `README.md`, then the rest by name whatever their case,
    /// folders and files mixed.
    DocsFirst,
    /// The last written first.
    Modified,
}

/// What the terminal overview lists its terminals under.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalGrouping {
    /// A heading for each status, what wants the user first.
    #[default]
    Status,
    /// A heading for each project.
    Project,
    /// Those that want the user under one heading first, then the rest under their project's.
    NeedsYouThenProject,
    /// No headings.
    Ungrouped,
}

/// What the app's own terminals start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalRuns {
    /// The user's login shell, at its prompt.
    #[default]
    LoginShell,
    /// [`Prefs::terminal_command`], through the login shell.
    Command,
}

/// The order the navigator's watched projects go in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchedOrder {
    /// As the user put them, by dragging their cards.
    #[default]
    Manual,
    Name,
    /// The one something last changed in first, as an agent at work.
    Activity,
}

/// Where the navigator goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavigatorPlace {
    /// A column beside the panes.
    #[default]
    Side,
    /// A strip of tabs across the window's top, which gives its width back to the panes.
    Top,
}

/// Which branches the Branches page lists by their pull request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchStatus {
    /// Those under way: with an open or draft pull request, or none.
    NotMerged,
    Open,
    Draft,
    Closed,
    NoPullRequest,
    /// Merged and closed ones too.
    #[default]
    Any,
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

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Saved {
    /// The watched projects, in the order the navigator shows them.
    pub(super) expanded: Vec<String>,
    pub(super) selection: Option<String>,
    pub(super) seen: Seen,
    pub(super) widths: HashMap<Pane, f32>,
    pub(super) window: Option<WindowPlace>,
    pub(super) prefs: Prefs,
    /// The oplog entries ⌘Z mustn't take back, by project, as the oplog can't restore all
    /// they changed, which stays so after a restart.
    pub(super) kept_by_project: BTreeMap<String, Vec<String>>,
    /// How busy each checkout was over the last week, so a restart doesn't start it over.
    pub(super) activity: Vec<SavedActivity>,
}

/// A checkout's [`super::Activity`], as saved: a project's main checkout, or its linked
/// worktree with that name.
#[derive(Serialize, Deserialize)]
pub(super) struct SavedActivity {
    pub(super) project: String,
    pub(super) worktree: Option<String>,
    pub(super) activity: super::Activity,
}

/// Choices from Settings, and how the navigator lists projects, that only this app keeps;
/// GitButler's shared settings live with the other apps'.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub appearance: Appearance,
    pub color_scheme: crate::theme::ColorScheme,
    /// The colour GitButler's own colours pick things out in.
    pub accent: crate::theme::Accent,
    /// A custom accent, as typed, kept while another one is picked.
    pub accent_color: String,
    /// The gray GitButler's own colours are built on.
    pub ground: crate::theme::Ground,
    pub corners: crate::theme::Corners,
    /// How tall controls are.
    pub density: crate::theme::Density,
    /// The editor files open in, by program id. Asked for the first time it's needed.
    pub editor: Option<String>,
    /// The terminal the project opens in, by id.
    pub terminal: Option<String>,
    pub file_view: FileView,
    pub file_sort: FileSort,
    /// A file tree's folders always show the lines their files add and remove, rather than
    /// only under the pointer.
    pub folder_line_counts: bool,
    /// The file list's keys stop on a file tree's folders too, rather than going from file to
    /// file.
    pub folder_stops: bool,
    /// Long diff lines wrap instead of running off the edge.
    pub wrap: bool,
    /// Diffs show the old side beside the new rather than one above the other.
    pub split: bool,
    /// Changed lines go without their green and red ground, keeping the bar beside them.
    pub plain_lines: bool,
    pub code_size: crate::theme::CodeSize,
    /// The family code in diffs shows in, or `None` for the app's own.
    pub diff_font: Option<String>,
    /// The size text shows in the app's own terminals, or `None` for Ghostty's.
    pub terminal_text: Option<u8>,
    /// The family the app's own terminals show in, or `None` for Ghostty's.
    pub terminal_font: Option<String>,
    pub terminal_runs: TerminalRuns,
    /// The command they run with [`TerminalRuns::Command`], kept while the login shell is
    /// picked.
    pub terminal_command: String,
    /// The commands "New terminal with command" offers, the stock ones if never changed.
    pub terminal_launchers: Option<Vec<String>>,
    /// Closing a terminal by hand asks first, as what runs in it ends with it. Off unless
    /// picked. Quitting asks either way.
    pub confirm_terminal_close: bool,
    /// How large the whole interface draws, as ⌘+ and ⌘− set it.
    pub ui_scale: crate::theme::UiScale,
    pub navigator_place: NavigatorPlace,
    /// The bar along the window's bottom is left out.
    pub hide_status_bar: bool,
    /// The terminals open on every project aren't listed under the navigator.
    pub hide_terminal_overview: bool,
    /// What the terminal overview lists its terminals under.
    pub terminal_grouping: TerminalGrouping,
    /// Commit and branch pages list the messages first among their files, in place of their
    /// headers, and the uncommitted changes go without theirs.
    pub messages_in_diff: bool,
    /// Commit pages start with their whole message open under the title, rather than only
    /// the title, which a button opens.
    pub full_messages: bool,
    /// The bar over the diff and the file headers' counts and buttons are left out, as keys
    /// and menus have what they do.
    pub hide_diff_controls: bool,
    /// The keys the app binds, as Settings › Keyboard picks them.
    pub keymap: crate::keymap::Keymap,
    /// The commands checked as audited in the command editor, by `command_editor`'s key for
    /// each. Kept here as it's a person's progress, not something the keymaps say.
    pub audited_commands: HashSet<String>,
    /// Code shows the font's ligatures, `->` as one arrow. Off unless picked, as in the
    /// desktop app.
    pub ligatures: bool,
    /// Diffs leave out what deltoids reads from the code around each hunk, and skip reading
    /// it: the function or type the hunk changes, and the rest of that when it's short.
    pub no_semantic_context: bool,
    pub whole_scopes: ScopeLines,
    /// Projects, by id, whose Git hooks run around commits and pushes. Off unless picked,
    /// as in the desktop app.
    pub run_hooks: HashSet<String>,
    /// The icon each project shows before its name in the navigator, by id, as but.dev
    /// stores a repository's: an emoji, or `glyph:<colour>`. The mesh's, once read, wins.
    pub project_icons: HashMap<String, String>,
    /// How each project's pull requests merge, by id, as last picked beside Merge.
    pub merge_methods: HashMap<String, crate::backend::reviews::MergeMethod>,
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
    /// Watched projects, by id, whose inactive worktrees show under their heading; it starts
    /// folded.
    pub shown_inactive: HashSet<String>,
    pub inactive_after: crate::backend::InactiveAfter,
    pub merged_worktrees: crate::backend::MergedWorktrees,
    pub watched_order: WatchedOrder,
    /// Each project's linked worktrees by name, archived ones too, by project id, in the
    /// order the navigator lists them: as the user moved them, with new ones last.
    pub worktree_order: HashMap<String, Vec<String>>,
    /// The Branches page lists local branches instead of the complete remote inventory.
    pub branches_local_only: bool,
    /// The Branches page leaves out branches without commits of their own, unless searched
    /// for by name. Off by default so remote branches sharing a tip are listed too.
    pub branches_hide_empty: bool,
    /// The common notes a click leaves on a brief, the stock ones if never changed.
    pub brief_shortcuts: Option<Vec<String>>,
    pub branches_status: BranchStatus,
    /// The navigator's branches start with their commits listed, rather than folded to a
    /// row each.
    pub branches_start_open: bool,
    /// The navigator's open branches list all their commits, rather than the first few and
    /// how many more.
    pub all_branch_commits: bool,
    /// A click on a navigator branch only selects it, rather than listing its commits too and
    /// folding the ones a click listed before, which with several branches keeps moving the
    /// rows. Off unless picked.
    pub click_only_selects_branch: bool,
    /// The commit composer opens with every file left out, for what goes in to be checked,
    /// rather than with them all in.
    pub composer_starts_unchecked: bool,
    /// Screen readers can read the app. Off unless picked: with it on, closing a window
    /// crashes the app on a Mac with a Touch Bar (zed-industries/zed#64819). Read once, as
    /// the app starts, by [`screen_reader_on`].
    pub screen_reader: bool,
    /// The window's top right corner shows the frames drawn in the last second. Only Settings'
    /// Developer page, in a build of its own, turns it on.
    pub show_fps: bool,
    /// The app draws its own menu bar on macOS too, as it does on Windows and Linux, to try it
    /// there. Only Settings' Developer page, in a build of its own, turns it on.
    pub menu_bar_everywhere: bool,
}

/// A part of the window a setting in Settings › Appearance can leave out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The bar along the window's bottom, with what's under way and the keys that go with it.
    StatusBar,
    /// The header across a branch's or a stack's page, with its name, commits and push state,
    /// which the navigator's row has too. Where it's left out, a branch's messages list first
    /// among its files instead, as do those of the commits only a remote has, and the header
    /// of the branch opened on the Branches page.
    BranchHeader,
    /// The header across a commit's page, with its message, author, ID and push state. Where
    /// it's left out, the commit's message lists first among its files instead. Also a
    /// commit's only a remote has, with which remote has it, and the message over a list
    /// page's diff.
    CommitHeader,
    /// The header across a worktree's uncommitted changes, with their title and the worktree,
    /// which the navigator's row has too.
    ChangesHeader,
    /// The diff's wrap, backgrounds and split controls, which keys switch too.
    DiffTools,
    /// A file header's changed line counts and its buttons, which its right-click menu and
    /// keys have too.
    FileHeaderControls,
    /// The terminals open on every project, under the navigator beside the panes, which lists
    /// them too, but for those living in a tile.
    TerminalOverview,
}

impl Prefs {
    pub fn shows(&self, part: Part) -> bool {
        match part {
            Part::StatusBar => !self.hide_status_bar,
            Part::BranchHeader | Part::CommitHeader | Part::ChangesHeader => !self.messages_in_diff,
            Part::DiffTools | Part::FileHeaderControls => !self.hide_diff_controls,
            Part::TerminalOverview => !self.hide_terminal_overview,
        }
    }

    /// What a new terminal runs in place of the login shell's prompt: the command, while it's
    /// picked and there is one.
    pub fn terminal_run(&self) -> Option<&str> {
        match self.terminal_runs {
            TerminalRuns::LoginShell => None,
            TerminalRuns::Command => Some(self.terminal_command.trim()).filter(|c| !c.is_empty()),
        }
    }

    /// The commands "New terminal with command" offers.
    pub fn terminal_launchers(&self) -> Vec<String> {
        (self.terminal_launchers.clone())
            .unwrap_or_else(|| vec!["claude".to_owned(), "codex".to_owned()])
    }

    /// The most unchanged lines a hunk's scope adds around it, `None` with semantic context
    /// off.
    pub fn scope_lines(&self) -> Option<usize> {
        (!self.no_semantic_context).then(|| self.whole_scopes.lines())
    }
}

/// `value`, an object of `T`'s fields, without those `T` can't read.
fn readable<T: serde::de::DeserializeOwned>(value: &mut serde_json::Value) {
    if let Some(fields) = value.as_object_mut() {
        fields.retain(|key, field| {
            let alone = serde_json::json!({ key.as_str(): field.clone() });
            serde_json::from_value::<T>(alone).is_ok()
        });
    }
}

/// Where the window was: the display it was on, and its left, top, width and height on that
/// display. A maximized or full-screen window keeps the size it returns to, and opens filling
/// the display again.
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct WindowPlace {
    display: String,
    bounds: [f32; 4],
    #[serde(default)]
    fills: Option<Fill>,
}

/// How a window filled its display.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub(super) enum Fill {
    Maximized,
    FullScreen,
}

/// Whether Settings turned screen reader support on. GPUI fixes it as it builds the app,
/// before the store loads, so this reads the saved state on its own and a change takes
/// effect at the next launch.
pub fn screen_reader_on() -> bool {
    Saved::load(&Database::open()).0.prefs.screen_reader
}

impl Saved {
    /// What `db` holds, a row per field. It's the channel's, as each knows the projects by ids
    /// of its own and forgets what's kept for those its list doesn't have. And whether it held
    /// nothing, as on the first launch.
    pub(super) fn load(db: &Database) -> (Self, bool) {
        let rows = db.read();
        let fresh = rows.is_empty();
        let fields = (rows.into_iter())
            .filter_map(|(key, value)| Some((key, serde_json::from_str(&value).ok()?)))
            .collect();
        let saved = Self::parse(serde_json::Value::Object(fields));
        // As this build writes them, so only what changes from here on is written.
        db.loaded(saved.rows());
        (saved, fresh)
    }

    /// What `value` saved, but for what this build can't read: a choice a newer build saved
    /// that this one doesn't know, as a new size, goes back to its default alone.
    fn parse(mut value: serde_json::Value) -> Self {
        Self::deserialize(&value).unwrap_or_else(|err| {
            tracing::warn!("reading only some of the saved state: {err}");
            if let Some(prefs) = value.get_mut("prefs") {
                readable::<Prefs>(prefs);
            }
            readable::<Self>(&mut value);
            Self::deserialize(&value).unwrap_or_default()
        })
    }

    /// A row per field, its value as JSON.
    fn rows(&self) -> Vec<(String, String)> {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::Object(fields)) => (fields.into_iter())
                .map(|(key, value)| (key, value.to_string()))
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Saved;

    #[test]
    fn branches_without_unique_commits_show_by_default() {
        let saved = Saved::parse(
            serde_json::from_str(r#"{ "prefs": { "wrap": true } }"#).expect("it's JSON"),
        );
        assert!(
            !saved.prefs.branches_hide_empty,
            "existing settings include branches without unique commits"
        );
        assert!(saved.prefs.wrap, "the existing preferences are kept");
        assert_eq!(
            saved.prefs.branches_status,
            super::BranchStatus::Any,
            "an unconfigured status filter includes every remote branch"
        );
        assert!(
            !saved.prefs.branches_local_only,
            "remote branches are the default scope"
        );
        assert!(
            !Saved::default().prefs.branches_hide_empty,
            "new installations include branches without unique commits too"
        );
    }

    #[test]
    fn hiding_branches_without_unique_commits_survives_a_restart() {
        let mut saved = Saved::default();
        saved.prefs.branches_hide_empty = true;
        let saved = Saved::parse(serde_json::to_value(saved).expect("saved state is JSON"));
        assert!(
            saved.prefs.branches_hide_empty,
            "the explicitly chosen filter is kept between launches"
        );
    }

    #[test]
    fn closing_a_terminal_asks_only_once_picked() {
        let saved = Saved::parse(serde_json::from_str(r#"{ "prefs": {} }"#).expect("it's JSON"));
        assert!(
            !saved.prefs.confirm_terminal_close,
            "what was saved before the setting closes at once"
        );
    }

    #[test]
    fn a_choice_this_build_doesnt_know_loses_only_itself() {
        let newer = r#"{ "expanded": ["a"], "prefs": { "code_size": "Huge", "wrap": true } }"#;
        let saved = Saved::parse(serde_json::from_str(newer).expect("it's JSON"));
        assert_eq!(saved.expanded, ["a"], "the watched projects are kept");
        assert!(saved.prefs.wrap, "the other choices are kept");
        assert_eq!(
            saved.prefs.code_size,
            Default::default(),
            "the unknown one is the default"
        );
    }
}
