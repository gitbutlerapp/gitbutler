//! The window's root: the project navigator on the left, details on the right, the keys,
//! and the actions that work anywhere in the window. `overlays.rs` draws what opens above.

use std::collections::HashSet;
use std::path::Path;

use gpui::{
    AnyView, App, ClipboardItem, Context, Entity, FocusHandle, Focusable, KeyBinding, Modifiers,
    PromptLevel, Role, StyleRefinement, Subscription, Window, actions, div, prelude::*, px,
};

use crate::backend::{self, short_ref};
use crate::detail::{Detail, DiffPane};
use crate::navigator::Navigator;
use crate::overlays::{ContextMenu, Prompt};
use crate::palette::Palette;
use crate::projects::pick_folder;
use crate::state::{Cut, Pane, ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::transfer::{Transfer, conflicted_file};
use crate::ui::{HEADER_HEIGHT, plural, resize_pane};

actions!(
    gitbutler,
    [
        Quit,
        Hide,
        HideOthers,
        ShowAll,
        Minimize,
        Zoom,
        AddProject,
        CopySelection,
        OpenPullRequest,
        ToggleFileTree,
        Refresh,
        Undo,
        Redo,
        Fetch,
        UpdateFromTarget,
        NewBranch,
        NewBranchOnTop,
        SelectPrevious,
        SelectNext,
        Expand,
        Collapse,
        Confirm,
        CutSelection,
        PasteHere,
        Cancel,
        DeleteSelection,
        UncommitSelection,
        RenameSelection,
        Push,
        FocusNavigator,
        FocusChanges,
        StartCommit,
        ToggleNavigator,
        ToggleZoom,
        CommandPalette,
        ToggleAppearance,
    ]
);

/// Keys follow GitButler Lite where it has one, and Zed for reviewing changes.
pub fn bind_keys(cx: &mut App) {
    let nav = Some("Navigator && !TextInput");
    let workspace = Some("Workspace && !TextInput");
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("cmd-o", AddProject, None),
        KeyBinding::new("cmd-r", Refresh, None),
        KeyBinding::new("alt-shift-f", Fetch, None),
        KeyBinding::new("alt-shift-r", UpdateFromTarget, None),
        KeyBinding::new("cmd-n", NewBranch, None),
        // Zed's key works too; ⌘K is the one shown, as the last binding wins.
        KeyBinding::new("cmd-shift-p", CommandPalette, None),
        KeyBinding::new("cmd-k", CommandPalette, None),
        KeyBinding::new("cmd-1", FocusNavigator, None),
        KeyBinding::new("cmd-2", FocusChanges, None),
        KeyBinding::new("cmd-enter", StartCommit, workspace),
        KeyBinding::new("cmd-z", Undo, workspace),
        KeyBinding::new("cmd-shift-z", Redo, workspace),
        KeyBinding::new("cmd-x", CutSelection, workspace),
        // ⌥ turns moving a commit onto another into squashing it, as when dragging. Bound
        // first, so ⌘V is the key shown.
        KeyBinding::new("alt-cmd-v", PasteHere, workspace),
        KeyBinding::new("shift-cmd-v", PasteHere, workspace),
        KeyBinding::new("cmd-v", PasteHere, workspace),
        KeyBinding::new("cmd-c", CopySelection, workspace),
        KeyBinding::new("escape", Cancel, workspace),
        KeyBinding::new(".", ToggleNavigator, workspace),
        KeyBinding::new("shift-escape", ToggleZoom, None),
        KeyBinding::new("up", SelectPrevious, nav),
        KeyBinding::new("k", SelectPrevious, nav),
        KeyBinding::new("down", SelectNext, nav),
        KeyBinding::new("j", SelectNext, nav),
        // The arrows come last, so menus show them.
        KeyBinding::new("l", Expand, nav),
        KeyBinding::new("right", Expand, nav),
        KeyBinding::new("h", Collapse, nav),
        KeyBinding::new("left", Collapse, nav),
        KeyBinding::new("enter", Confirm, nav),
        KeyBinding::new("space", Confirm, nav),
        KeyBinding::new("r", RenameSelection, nav),
        KeyBinding::new("b", NewBranchOnTop, nav),
        KeyBinding::new("shift-p", Push, nav),
        KeyBinding::new("o", OpenPullRequest, nav),
        KeyBinding::new("cmd-backspace", DeleteSelection, nav),
        KeyBinding::new("cmd-alt-backspace", UncommitSelection, nav),
    ]);
}

pub struct AppView {
    pub store: Entity<Store>,
    pub detail: Entity<Detail>,
    diff: Entity<DiffPane>,
    navigator: Entity<Navigator>,
    pub menu: Option<ContextMenu>,
    pub prompt: Option<Prompt>,
    pub navigator_open: bool,
    /// The diff takes the whole window.
    pub zoomed: bool,
    pub palette: Option<Palette>,
    /// The modal on screen, if any, and the subscription that closes it.
    pub modal: Option<(AnyView, Subscription)>,
    /// Palette commands by label, most recently run first.
    pub recent_commands: Vec<&'static str>,
    /// Projects where a dry run is checking what an operation would conflict, before
    /// it's queued.
    pub checking: HashSet<ProjectId>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    /// Where the diff starts beside the detail's file list, when the selection has a diff.
    fn diff_left(&self, cx: &App) -> Option<f32> {
        let store = self.store.read(cx);
        let has_diff = store.selection.is_some();
        let detail = self.detail.read(cx);
        has_diff.then(|| match detail.zoomed || detail.files_hidden {
            true => 0.,
            false => store.width(Pane::Files),
        })
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = cx.new(Store::new);
        let prefs = &store.read(cx).prefs;
        let theme = Theme::for_appearance(prefs.appearance, window.appearance(), prefs.code_size);
        cx.set_global(theme);
        let app = cx.entity().downgrade();
        let detail = cx.new(|cx| Detail::new(store.clone(), app.clone(), window, cx));
        let diff = cx.new(|cx| DiffPane::new(&detail, cx));
        let navigator = cx.new(|cx| Navigator::new(app, store.clone(), detail.clone(), cx));
        window.focus(&navigator.focus_handle(cx), cx);
        let subscriptions = vec![
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe_window_appearance(window, |this, window, cx| {
                this.apply_appearance(window, cx)
            }),
            cx.observe_window_bounds(window, |this, window, cx| {
                this.store
                    .update(cx, |store, cx| store.save_window_place(window, cx));
            }),
        ];
        // Closing the window quits, so it asks the same as ⌘Q.
        let this = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            this.update(cx, |this, cx| this.can_quit(window, cx))
                .unwrap_or(true)
        });
        Self::listen_for_credential_prompts(window, cx);
        Self {
            store,
            detail,
            diff,
            navigator,
            menu: None,
            prompt: None,
            navigator_open: true,
            zoomed: false,
            palette: None,
            modal: None,
            recent_commands: Vec::new(),
            checking: HashSet::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Switch to light from dark, or to dark from light, whichever is showing.
    fn toggle_appearance(
        &mut self,
        _: &ToggleAppearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::theme::Appearance;
        let next = if Theme::get(cx).dark {
            Appearance::Light
        } else {
            Appearance::Dark
        };
        self.store.update(cx, |store, cx| {
            store.set_prefs(|prefs| prefs.appearance = next, cx)
        });
        self.apply_appearance(window, cx);
    }

    /// Use the theme the appearance setting and the system call for.
    pub fn apply_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prefs = &self.store.read(cx).prefs;
        let theme = Theme::for_appearance(prefs.appearance, window.appearance(), prefs.code_size);
        cx.set_global(theme);
        cx.refresh_windows();
    }

    pub fn select(&mut self, selection: Selection, window: &mut Window, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.select(Some(selection), cx));
        self.focus_main(window, cx);
    }

    pub(crate) fn selection(&self, cx: &App) -> Option<Selection> {
        self.store.read(cx).selection.clone()
    }

    pub(crate) fn selected_project(&self, cx: &App) -> Option<ProjectId> {
        Some(self.store.read(cx).selection.as_ref()?.project().clone())
    }

    pub fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        if self.can_quit(window, cx) {
            cx.quit();
        }
    }

    /// Whether to quit right away. While an operation hasn't finished, ask instead, and quit
    /// on a yes: quitting would stop it partway and drop the ones waiting.
    fn can_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let (message, detail) = match self.store.read(cx).unfinished() {
            0 => return true,
            1 => (
                "Quit before the operation finishes?".to_owned(),
                "It may stop partway.",
            ),
            n => (
                format!("Quit before {n} operations finish?"),
                "The running one may stop partway, and the rest won't run.",
            ),
        };
        self.confirm_then(message, detail, "Quit", window, cx, |_, cx| cx.quit());
        false
    }

    pub fn add_project(&mut self, _: &AddProject, _: &mut Window, cx: &mut Context<Self>) {
        let folder = pick_folder("Add project", cx);
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            if let Some(path) = folder.await {
                store.update(cx, |store, cx| store.add_project(path, cx));
            }
        })
        .detach();
    }

    pub fn refresh(&mut self, _: &Refresh, _: &mut Window, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.load_projects(cx);
            store.reload_all(cx);
        });
    }

    /// Undo the selected project's last operation, or with `redo` what was undone.
    fn undo(&mut self, redo: bool, cx: &mut Context<Self>) {
        if let Some(project) = self.selected_project(cx) {
            self.store
                .update(cx, |store, cx| store.undo(&project, redo, cx));
        }
    }

    fn fetch(&mut self, _: &Fetch, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.selected_project(cx) {
            self.store.update(cx, |store, cx| store.fetch(&project, cx));
        }
    }

    pub fn new_branch(&mut self, _: &NewBranch, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.selected_project(cx).or_else(|| {
            self.store
                .read(cx)
                .projects
                .iter()
                .find(|p| p.expanded)
                .map(|p| p.id.clone())
        }) else {
            return;
        };
        self.prompt_branch(project, "New branch", "", window, cx, |path, _, name| {
            backend::create_branch(path, Some(name))
        });
    }

    /// Ask with a native dialog before doing something destructive.
    pub fn confirm_then(
        &mut self,
        message: String,
        detail: &str,
        confirm: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut AppView, &mut Context<AppView>) + 'static,
    ) {
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            Some(detail),
            &[confirm, "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                this.update(cx, |this, cx| then(this, cx)).ok();
            }
        })
        .detach();
    }

    fn cut_selection(&mut self, _: &CutSelection, _: &mut Window, cx: &mut Context<Self>) {
        let cut = match self.selection(cx) {
            Some(Selection::Commit(..)) => {
                (self.single_commit(cx)).map(|(project, id)| Cut::Commit(project, id))
            }
            Some(Selection::Branch(project, ref_name)) if !ref_name.is_empty() => {
                Some(Cut::Branch(project, ref_name))
            }
            Some(Selection::Uncommitted(project)) => {
                let files = self.detail.read(cx).selected_changes(cx);
                (!files.is_empty()).then_some(Cut::Files {
                    project,
                    from: None,
                    files: files.into(),
                })
            }
            _ => None,
        };
        if let Some(cut) = cut {
            self.cut(cut, cx);
        }
    }

    fn paste_here(&mut self, _: &PasteHere, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(cut), Some(target)) = (self.store.read(cx).cut.clone(), self.selection(cx))
        else {
            return;
        };
        self.transfer(&cut, &target, window.modifiers(), window, cx);
    }

    pub fn cut(&mut self, cut: Cut, cx: &mut Context<Self>) {
        let message = format!(
            "{} {}. Pick a spot and press ⌘V.",
            cut.verb(),
            cut.describe()
        );
        self.store.update(cx, |store, cx| {
            store.set_cut(Some(cut), cx);
            store.snack(message, cx);
        });
    }

    /// Land `payload` on `target`, from a drop or a paste.
    pub fn transfer(
        &mut self,
        payload: &Cut,
        target: &Selection,
        held: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(path) = conflicted_file(payload, self.store.read(cx)) {
            let message = format!("Mark {path} resolved first");
            self.store.update(cx, |store, cx| store.snack(message, cx));
            return;
        }
        let Some(transfer) = Transfer::resolve(payload, target, held, self.store.read(cx)) else {
            self.store
                .update(cx, |store, cx| store.snack("That can't go there", cx));
            return;
        };
        let project = target.project().clone();
        let staged = self.store.update(cx, |store, cx| {
            store.set_cut(None, cx);
            transfer.perform(&project, store, cx)
        });
        if let Some((branch, files)) = staged {
            self.store.update(cx, |store, cx| {
                store.select(Some(Selection::Uncommitted(project.clone())), cx)
            });
            self.detail
                .update(cx, |detail, cx| detail.stage(branch, &files, window, cx));
        }
    }

    fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_some() {
            return self.close_modal(window, cx);
        }
        if self.menu.take().is_some() {
            cx.notify();
            return;
        }
        if self.zoomed {
            return self.set_chrome(self.navigator_open, false, window, cx);
        }
        if self.detail.read(cx).is_focused(window, cx) {
            // Undo the nearest thing first: a text selection, then the file filter.
            let handled = self.detail.update(cx, |detail, cx| {
                detail.clear_text_selection(cx) || detail.close_open_filter(window, cx)
            });
            if !handled {
                self.focus_main(window, cx);
            }
            return;
        }
        if self.store.read(cx).cut.is_some() {
            self.store.update(cx, |store, cx| store.set_cut(None, cx));
        }
        if !self.store.read(cx).picked.is_empty() {
            self.store.update(cx, |store, cx| {
                store.picked.clear();
                cx.notify()
            });
        }
    }

    fn focus_navigator(&mut self, _: &FocusNavigator, window: &mut Window, cx: &mut Context<Self>) {
        // Asking for the navigator brings it back.
        if !self.navigator_open || self.zoomed {
            self.set_chrome(true, false, window, cx);
        }
        window.focus(&self.navigator.focus_handle(cx), cx);
    }

    /// Focus the navigator, or the changes while the navigator isn't on screen, so keys
    /// always reach something visible.
    pub(crate) fn focus_main(&self, window: &mut Window, cx: &mut App) {
        if self.navigator_open && !self.zoomed {
            window.focus(&self.navigator.focus_handle(cx), cx);
        } else {
            self.detail
                .update(cx, |detail, cx| detail.focus(window, cx));
        }
    }

    fn focus_changes(&mut self, _: &FocusChanges, window: &mut Window, cx: &mut Context<Self>) {
        self.detail
            .update(cx, |detail, cx| detail.focus(window, cx));
    }

    fn start_commit(&mut self, _: &StartCommit, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.selected_project(cx) {
            self.store.update(cx, |store, cx| {
                store.select(Some(Selection::Uncommitted(project)), cx)
            });
            self.detail
                .update(cx, |detail, cx| detail.focus_message(window, cx));
        }
    }

    /// Open the pull request of the selected branch, or of the branch the selected commit
    /// is on, in the browser.
    fn open_pull_request(&mut self, _: &OpenPullRequest, _: &mut Window, cx: &mut Context<Self>) {
        let url = self.selection(cx).and_then(|selection| {
            let branch = self.store.read(cx).branch_for(&selection)?;
            branch.review.as_ref()?.url.clone()
        });
        match url {
            Some(url) => cx.open_url(&url),
            None => self
                .store
                .update(cx, |store, cx| store.snack("No pull request here", cx)),
        }
    }

    /// Copy what's selected in the form other tools take: a file's path, a commit's id,
    /// a branch's name or a project's directory.
    fn copy_selection(&mut self, _: &CopySelection, window: &mut Window, cx: &mut Context<Self>) {
        let detail = self.detail.read(cx);
        let file = detail
            .is_focused(window, cx)
            .then(|| detail.selected_text().or_else(|| detail.cursor_path()))
            .flatten();
        let text = file.or_else(|| match self.selection(cx)? {
            Selection::Commit(_, id) => Some(id.0.to_string()),
            Selection::Branch(_, ref_name) => Some(short_ref(&ref_name).to_owned()),
            Selection::Project(project) => {
                Some(self.store.read(cx).path(&project)?.display().to_string())
            }
            Selection::Worktree(project, name) => {
                let workspace = self.store.read(cx).workspace(&project)?;
                let worktree = workspace.worktrees.iter().find(|w| w.name == name)?;
                Some(worktree.path.display().to_string())
            }
            Selection::Uncommitted(_) | Selection::Branches(_) | Selection::History(_) => None,
        });
        if let Some(text) = text {
            self.copy_text(text, cx);
        }
    }

    /// Put `text` on the clipboard and say so.
    pub fn copy_text(&mut self, text: String, cx: &mut Context<Self>) {
        let message = copied(&text);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.store.update(cx, |store, cx| store.snack(message, cx));
    }

    fn update_selected_project(
        &mut self,
        _: &UpdateFromTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self.selected_project(cx) {
            self.update_from_target(project, None, window, cx);
        }
    }

    /// Rebase onto the target, or with `branch` just its stack, after asking when a dry run
    /// says that would leave commits conflicted that aren't now.
    pub fn update_from_target(
        &mut self,
        project: ProjectId,
        branch: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let checked = branch.clone();
        let check = move |path: &Path| {
            let found = backend::update_conflicts(path, checked.as_deref())?;
            let mut detail = Vec::new();
            if !found.branches.is_empty() {
                detail.push(format!(
                    "Rebasing leaves conflicted commits in {}.",
                    commit_counts(&found.branches)
                ));
            }
            if !found.files.is_empty() {
                detail.push(format!(
                    "The target changed the same lines as your uncommitted changes to {}, \
                     which get conflict markers.",
                    found.files.join(", ")
                ));
            }
            Ok((!detail.is_empty()).then(|| {
                detail.push("You can resolve them afterwards, or undo the update.".into());
                detail.join(" ")
            }))
        };
        let updated = project.clone();
        let update = move |this: &mut AppView, cx: &mut Context<AppView>| {
            this.store.update(cx, |store, cx| {
                store.update_from_target(&updated, branch, cx)
            })
        };
        self.check_then(
            &project,
            check,
            "Update with conflicts?",
            "Update anyway",
            window,
            cx,
            update,
        );
    }

    /// Run `check` on the project's folder off the UI thread, then `then` straight away
    /// when it has nothing to warn about, or after asking with what it says. A failed check
    /// leaves it to `then` to report what's wrong.
    #[allow(clippy::too_many_arguments)]
    pub fn check_then(
        &mut self,
        project: &ProjectId,
        check: impl FnOnce(&Path) -> anyhow::Result<Option<String>> + Send + 'static,
        title: &'static str,
        confirm: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut AppView, &mut Context<AppView>) + 'static,
    ) {
        let Some(path) = self.store.read(cx).path(project) else {
            return;
        };
        // Asking again while it checks would queue the operation twice.
        if !self.checking.insert(project.clone()) {
            return;
        }
        cx.notify();
        let project = project.clone();
        cx.spawn_in(window, async move |this, cx| {
            let warning = backend::run(move || check(&path)).await.ok().flatten();
            this.update_in(cx, |this, window, cx| {
                this.checking.remove(&project);
                cx.notify();
                match warning {
                    None => then(this, cx),
                    Some(detail) => {
                        this.confirm_then(title.into(), &detail, confirm, window, cx, then)
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle_navigator(
        &mut self,
        _: &ToggleNavigator,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_chrome(!self.navigator_open, self.zoomed, window, cx);
    }

    fn command_palette(&mut self, _: &CommandPalette, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.is_some() {
            self.close_palette(window, cx);
        } else {
            self.open_palette(window, cx);
        }
    }

    fn toggle_zoom(&mut self, _: &ToggleZoom, window: &mut Window, cx: &mut Context<Self>) {
        self.set_chrome(self.navigator_open, !self.zoomed, window, cx);
    }

    /// Show or hide the navigator, and zoom the diff to the whole window or not. The
    /// details pane draws its own header, so it's told what's around it. When the navigator
    /// goes, focus moves off it.
    fn set_chrome(
        &mut self,
        navigator_open: bool,
        zoomed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigator_open = navigator_open;
        self.zoomed = zoomed;
        self.detail.update(cx, |detail, cx| {
            detail.zoomed = zoomed;
            detail.beside_traffic_lights = zoomed || !navigator_open;
            cx.notify();
        });
        if self.navigator.focus_handle(cx).contains_focused(window, cx) {
            self.focus_main(window, cx);
        }
        cx.notify();
    }
}

impl Focusable for AppView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.navigator.focus_handle(cx)
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        div()
            .id("root")
            .role(Role::Application)
            .aria_label("GitButler")
            .key_context("Workspace")
            // Drop targets read ⌥ to tell moving from squashing.
            .on_modifiers_changed(cx.listener(|this, _, _, cx| {
                if cx.has_active_drag() || this.store.read(cx).cut.is_some() {
                    this.navigator.update(cx, |_, cx| cx.notify());
                }
            }))
            .on_action(cx.listener(Self::quit))
            .on_action(|_: &Minimize, window, _| window.minimize_window())
            .on_action(|_: &Zoom, window, _| window.zoom_window())
            .on_action(cx.listener(Self::add_project))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::clone_repository))
            .on_action(cx.listener(Self::toggle_appearance))
            // Here rather than around the navigator, so the details pane's buttons reach them.
            .map(|el| self.pr_actions(el, cx))
            .map(|el| self.resolve_actions(el, cx))
            .on_action(cx.listener(Self::go_to_branch))
            .on_action(cx.listener(Self::open_selected_in_terminal))
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.undo(false, cx)))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.undo(true, cx)))
            .on_action(cx.listener(Self::fetch))
            .on_action(cx.listener(Self::new_branch))
            .on_action(cx.listener(Self::push_selected))
            .on_action(cx.listener(Self::cut_selection))
            .on_action(cx.listener(Self::paste_here))
            .on_action(cx.listener(Self::copy_selection))
            .on_action(cx.listener(Self::open_pull_request))
            .on_action(cx.listener(|this, _: &ToggleFileTree, _, cx| {
                this.store
                    .update(cx, |store, cx| store.set_file_tree(!store.file_tree, cx))
            }))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::focus_navigator))
            .on_action(cx.listener(Self::focus_changes))
            .on_action(cx.listener(Self::start_commit))
            .on_action(cx.listener(Self::update_selected_project))
            .on_action(cx.listener(Self::toggle_navigator))
            .on_action(cx.listener(Self::toggle_zoom))
            .on_action(cx.listener(Self::command_palette))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.bg_1)
            .text_color(theme.text_1)
            .font_family(theme.ui_font)
            .text_size(px(13.))
            .line_height(px(18.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.))
                    .on_drag_move(resize_pane(Pane::Navigator, self.store.clone()))
                    .when(self.navigator_open && !self.zoomed, |el| {
                        let width = self.store.read(cx).width(Pane::Navigator);
                        el.child(
                            div()
                                // What the navigator's menus and keys do to the selection.
                                .map(|el| self.edit_actions(el, cx))
                                .w(px(width))
                                .flex_none()
                                .h_full()
                                // Drawn again only when it changes, not when the diff does.
                                .child(
                                    self.navigator
                                        .clone()
                                        .cached(StyleRefinement::default().size_full()),
                                ),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .h_full()
                            .relative()
                            .child(
                                self.detail
                                    .clone()
                                    .cached(StyleRefinement::default().size_full()),
                            )
                            .when_some(self.diff_left(cx), |el, left| {
                                el.child(
                                    div()
                                        .absolute()
                                        .top(px(HEADER_HEIGHT))
                                        .bottom_0()
                                        .right_0()
                                        .left(px(left))
                                        .child(
                                            self.diff
                                                .clone()
                                                .cached(StyleRefinement::default().size_full()),
                                        ),
                                )
                            }),
                    ),
            )
            .child(self.render_status_bar(window, cx))
            .child(self.render_toasts(cx))
            .children(self.render_modal(cx))
            // Above a modal: Git can ask for credentials while one is open.
            .children(self.render_prompt(cx))
            .children(self.render_palette(cx))
            .children(self.render_menu(cx))
    }
}

/// What the snackbar says after copying `text`: the text itself when it's short.
fn copied(text: &str) -> String {
    match text.lines().count() {
        0 | 1 if text.chars().count() <= 64 => format!("Copied {text}"),
        0 | 1 => "Copied the selection".to_owned(),
        lines => format!("Copied {lines} lines"),
    }
}

/// Branches with how many of their commits: "feature/a (2 commits), feature/b (1 commit)".
pub fn commit_counts(branches: &[(String, usize)]) -> String {
    let counts: Vec<String> = (branches.iter())
        .map(|(name, count)| format!("{name} ({})", plural(*count, "commit")))
        .collect();
    counts.join(", ")
}
