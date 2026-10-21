//! The window's root: the project navigator on the left, details on the right, the keys,
//! and the actions that work anywhere in the window. `overlays.rs` draws what opens above.

use gpui::{
    AnyView, App, ClipboardItem, Context, Entity, FocusHandle, Focusable, KeyBinding, PromptLevel,
    Role, StyleRefinement, Subscription, Window, actions, div, prelude::*, px,
};

use crate::backend::{self, short_ref};
use crate::detail::{Detail, DiffPane};
use crate::navigator::Navigator;
use crate::overlays::{ContextMenu, Prompt};
use crate::palette::Palette;
use crate::projects::pick_folder;
use crate::state::{Cut, MeshPick, Pane, ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::transfer::{Place, Transfer};
use crate::ui::{HEADER_HEIGHT, resize_pane};

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
        FocusDiff,
        StartCommit,
        ToggleNavigator,
        CommandPalette,
    ]
);

/// Keys follow GitButler Lite where it has one, and Zed's agent review for discarding a hunk.
pub fn bind_keys(cx: &mut App) {
    let nav = Some("Navigator && !TextInput");
    let workspace = Some("Workspace && !TextInput");
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("cmd-o", AddProject, None),
        KeyBinding::new("alt-shift-f", Fetch, None),
        KeyBinding::new("alt-shift-r", UpdateFromTarget, None),
        KeyBinding::new("cmd-n", NewBranch, None),
        // Zed's key works too; ⌘K is the one shown, as the last binding wins.
        KeyBinding::new("cmd-shift-p", CommandPalette, None),
        KeyBinding::new("cmd-k", CommandPalette, None),
        KeyBinding::new("cmd-1", FocusNavigator, None),
        KeyBinding::new("cmd-2", FocusChanges, None),
        KeyBinding::new("cmd-3", FocusDiff, None),
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
        KeyBinding::new("shift-p", Push, workspace),
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
    /// The emoji picker open on a project's icon.
    pub icon_picker: Option<crate::projects::IconPicker>,
    pub navigator_open: bool,
    pub palette: Option<Palette>,
    /// The modal on screen, if any, what takes its keys, and the subscription that closes it.
    pub modal: Option<(AnyView, FocusHandle, Subscription)>,
    /// Palette commands by label, most recently run first.
    pub recent_commands: Vec<&'static str>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    /// Where the diff starts beside the detail's file list, when the selection has a diff.
    fn diff_left(&self, window: &Window, cx: &App) -> Option<f32> {
        let store = self.store.read(cx);
        let detail = self.detail.read(cx);
        let has_diff = store.selection.is_some() && !detail.showing_review(cx);
        has_diff.then(|| match detail.files_shown(cx) {
            false => 0.,
            true => detail.files_width(window, cx),
        })
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = cx.new(Store::new);
        let prefs = &store.read(cx).prefs;
        let theme = Theme::for_appearance(prefs.appearance, window.appearance(), prefs.code_size);
        cx.set_global(theme);
        let app = cx.entity().downgrade();
        let detail = cx.new(|cx| Detail::new(store.clone(), app.clone(), window, cx));
        let diff = cx.new(|cx| DiffPane::new(&detail, window, cx));
        let navigator = cx.new(|cx| Navigator::new(app, store.clone(), detail.clone(), window, cx));
        window.focus(&navigator.focus_handle(cx), cx);
        let subscriptions = vec![
            // A menu's items and the palette's commands act on the selection and the picks, so
            // when an agent's change moves them off what they opened on, they go rather than
            // act on something else.
            cx.observe_in(&store, window, |this, store, window, cx| {
                let now = store.read(cx).acting_on();
                this.menu.take_if(|menu| menu.acting_on != now);
                if (this.palette.as_ref()).is_some_and(|palette| palette.acting_on != now) {
                    this.close_palette(window, cx);
                }
                let on_branch = this.prompt.as_ref().and_then(|p| p.on_branch.as_ref());
                if on_branch.is_some_and(|(project, from)| {
                    (store.read(cx).workspace(project))
                        .is_some_and(|ws| ws.find_branch(from).is_none())
                }) {
                    this.cancel_prompt(window, cx);
                }
                cx.notify()
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                this.apply_appearance(window, cx)
            }),
            cx.observe_window_bounds(window, |this, window, cx| {
                this.store
                    .update(cx, |store, cx| store.save_window_place(window, cx));
            }),
            // The CLI and GitButler's other apps add and remove projects too, and folders move.
            cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() {
                    show_cursor_again(window);
                    this.store.update(cx, |store, cx| {
                        store.load_projects(cx);
                        store.recheck_folders(cx);
                    });
                }
            }),
            // Keys go to what's focused, so when it leaves the screen, as the Branches page's
            // list does once Apply selects the branch, they go to the navigator instead; to an
            // open modal, as Settings, rather than what's under it.
            cx.on_focus_lost(window, |this, window, cx| match &this.modal {
                Some((_, focus, _)) => window.focus(focus, cx),
                None => this.focus_main(window, cx),
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
            icon_picker: None,
            navigator_open: true,
            palette: None,
            modal: None,
            recent_commands: Vec::new(),
            _subscriptions: subscriptions,
        }
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
                "Those running may stop partway, and the rest won't run.",
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

    /// Undo the selected project's last operation, or with `redo` what was undone.
    fn undo(&mut self, redo: bool, cx: &mut Context<Self>) {
        if let Some(project) = self.selected_project(cx) {
            self.store
                .update(cx, |store, cx| store.undo(&project, redo, cx));
        }
    }

    /// Fetch the selected project, and ask its forge about pull requests and CI again: what
    /// the header's Refresh does.
    fn fetch(&mut self, _: &Fetch, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.selected_project(cx) {
            self.store.update(cx, |store, cx| {
                store.fetch(&project, cx);
                store.refresh_forge(&project);
            });
        }
    }

    fn new_branch(&mut self, _: &NewBranch, window: &mut Window, cx: &mut Context<Self>) {
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
        self.prompt_new_branch(project, "New branch", true, window, cx, |path, _, name| {
            backend::create_branch(path, Some(name))
        });
    }

    /// Install the version the desktop app's update feed offers: start the desktop app's web
    /// client once, which installs it and starts again, and quit.
    pub fn restart_to_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(version) = store.newer_version.clone() else {
            return;
        };
        if store.unfinished() > 0 {
            self.store.update(cx, |store, cx| {
                store.snack("Wait for the running operations to finish", cx)
            });
            return;
        }
        let detail = "GitButler opens its web app to install it, then starts in this app if \
                      Settings › Experimental › Open this app at start is on.";
        self.confirm_then(
            format!("Restart to update to GitButler {version}?"),
            detail,
            "Restart",
            window,
            cx,
            |app, cx| {
                let restarted = std::env::current_exe().and_then(|exe| {
                    std::process::Command::new(exe)
                        .env(crate::WEB_APP_ONCE, "1")
                        .spawn()
                });
                match restarted {
                    Ok(_) => cx.quit(),
                    Err(err) => app.store.update(cx, |store, cx| {
                        store.error("Couldn't restart GitButler", err.to_string(), cx)
                    }),
                }
            },
        );
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

    fn cut_selection(&mut self, _: &CutSelection, window: &mut Window, cx: &mut Context<Self>) {
        let in_diff = self.detail.read(cx).is_focused(window, cx);
        let cut = match self.selection(cx) {
            // From a commit's diff, its lines, hunk or file under the cursor.
            Some(Selection::Commit(..)) if in_diff => self.detail.read(cx).cursor_cut(cx),
            // The picked commits with it, when it's one of them.
            Some(Selection::Commit(project, id)) => {
                let ids = self.store.read(cx).with_picks(&project, id);
                Some(Cut::Commit(project, ids))
            }
            Some(Selection::Branch(project, ref_name)) if !ref_name.is_empty() => {
                Some(Cut::Branch(project, ref_name))
            }
            // From the diff, the lines, hunk or file under the cursor; from the navigator or
            // the composer, what's checked.
            Some(Selection::Uncommitted(_) | Selection::Worktree(..))
                if in_diff && !self.detail.read(cx).composing() =>
            {
                self.detail.read(cx).cursor_cut(cx)
            }
            Some(Selection::Uncommitted(_) | Selection::Worktree(..)) => {
                let detail = self.detail.read(cx);
                let files = detail.selected_changes(cx);
                (detail.selected_checkout(cx))
                    .filter(|_| !files.is_empty())
                    .map(|(project, worktree)| Cut::Files {
                        project,
                        from: None,
                        worktree,
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
        let place = Place::from_keys(window.modifiers(), &cut, &target);
        self.transfer(&cut, &target, place, window, cx);
    }

    pub fn cut(&mut self, cut: Cut, cx: &mut Context<Self>) {
        let message = format!("{} {}, ready to paste with ⌘V", cut.verb(), cut.describe());
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
        place: Place,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(transfer) = Transfer::resolve(payload, target, place, self.store.read(cx)) else {
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
        // Let go of a drag without dropping it, as macOS does. Left to GPUI, the drag stays
        // up and the next release drops nothing, as a key press stops it seeing the pointer.
        if cx.stop_active_drag(window) {
            return;
        }
        if self.modal.is_some() {
            return self.close_modal(window, cx);
        }
        if self.menu.take().is_some() {
            cx.notify();
            return;
        }
        if self.detail.read(cx).is_focused(window, cx) {
            // Undo the nearest thing first: a text selection, then the file filter.
            let handled = self.detail.update(cx, |detail, cx| {
                detail.clear_text_selection(cx)
                    || detail.clear_line_picks(cx)
                    || detail.clear_file_picks(cx)
                    || detail.close_open_filter(window, cx)
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
        self.detail.update(cx, |detail, cx| {
            detail.clear_file_picks(cx);
            detail.clear_line_picks(cx);
        });
    }

    fn focus_navigator(&mut self, _: &FocusNavigator, window: &mut Window, cx: &mut Context<Self>) {
        // Asking for the navigator brings it back.
        if !self.navigator_open {
            self.set_navigator(true, window, cx);
        }
        window.focus(&self.navigator.focus_handle(cx), cx);
    }

    /// Focus the navigator, or the changes while the navigator isn't on screen, so keys
    /// always reach something visible.
    pub(crate) fn focus_main(&self, window: &mut Window, cx: &mut App) {
        if self.navigator_open {
            window.focus(&self.navigator.focus_handle(cx), cx);
        } else {
            self.detail
                .update(cx, |detail, cx| detail.focus(window, cx));
        }
    }

    /// The file list, or the diff while the list is hidden; a Branches, Operations history or
    /// project page's list.
    fn focus_changes(&mut self, _: &FocusChanges, window: &mut Window, cx: &mut Context<Self>) {
        self.detail
            .update(cx, |detail, cx| detail.focus_files(window, cx));
    }

    fn focus_diff(&mut self, _: &FocusDiff, window: &mut Window, cx: &mut Context<Self>) {
        self.detail
            .update(cx, |detail, cx| detail.focus(window, cx));
    }

    fn start_commit(&mut self, _: &StartCommit, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.selection(cx) else {
            return;
        };
        // Only what's on screen gets committed: from another page, ⌘↩ shows the changes
        // first, a linked worktree's from its branches and commits.
        let project = selection.project().clone();
        let checkout = match (&selection, self.store.read(cx).worktree_of(&selection)) {
            (Selection::Uncommitted(_) | Selection::Worktree(..), _) => selection.clone(),
            (_, Some(worktree)) => Selection::Worktree(project, worktree.name.clone()),
            (_, None) => Selection::Uncommitted(project),
        };
        let shown = selection == checkout;
        self.store
            .update(cx, |store, cx| store.select(Some(checkout), cx));
        self.detail.update(cx, |detail, cx| match shown {
            true => detail.commit_or_write(window, cx),
            false => detail.focus_message(window, cx),
        });
    }

    /// Open the pull request of the selected branch, or of the branch the selected commit
    /// is on, in the browser. Without one, start one where that's possible.
    fn open_pull_request(
        &mut self,
        _: &OpenPullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let url = self.selection(cx).and_then(|selection| {
            let branch = self.store.read(cx).branch_for(&selection)?;
            branch.review.as_ref()?.url.clone()
        });
        let create = crate::pr::CreatePullRequest;
        match url {
            Some(url) => cx.open_url(&url),
            None if window.is_action_available(&create, cx) => {
                self.create_pull_request(&create, window, cx)
            }
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
                let worktree = workspace.find_worktree(&name)?;
                Some(worktree.path.display().to_string())
            }
            Selection::Mesh(project, id, MeshPick::Checkout) => Some(
                self.store
                    .read(cx)
                    .mesh_checkout(&project, &id)?
                    .path
                    .clone(),
            ),
            Selection::Uncommitted(_)
            | Selection::Mesh(..)
            | Selection::Branches(_)
            | Selection::History(_)
            | Selection::Stack(..) => None,
        });
        if let Some(text) = text {
            self.copy_text(text, cx);
        }
    }

    /// What right-clicking `target`'s row in the navigator offers, opened at `position`.
    pub fn open_row_menu(
        &mut self,
        target: &Selection,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let items = self.navigator.read(cx).menu_items(target, cx);
        self.open_menu(position, items, cx);
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
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self.selected_project(cx) {
            self.update_from_target(project, cx);
        }
    }

    /// Rebase onto the target. When that would leave commits conflicted that aren't now, the
    /// card saying so offers to go ahead. Asking again while one waits or runs does nothing.
    pub fn update_from_target(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(target) = store.workspace(&project).and_then(|ws| ws.target.as_ref()) else {
            return;
        };
        if !target.needs_update {
            let message = format!("Already up to date with {}", target.name);
            self.store.update(cx, |store, cx| store.snack(message, cx));
            return;
        }
        self.store
            .update(cx, |store, cx| store.update_from_target(&project, None, cx));
    }

    fn toggle_navigator(
        &mut self,
        _: &ToggleNavigator,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_navigator(!self.navigator_open, window, cx);
    }

    fn command_palette(&mut self, _: &CommandPalette, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.is_some() {
            self.close_palette(window, cx);
        } else {
            self.open_palette(window, cx);
        }
    }

    /// Show or hide the navigator. The details pane draws its own header, so it's told
    /// what's around it. When the navigator goes, focus moves off it, and it comes back with
    /// it, as hiding it took it away.
    fn set_navigator(&mut self, navigator_open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.navigator_open = navigator_open;
        self.detail.update(cx, |detail, cx| {
            detail.beside_traffic_lights = !navigator_open;
            cx.notify();
        });
        if navigator_open {
            window.focus(&self.navigator.focus_handle(cx), cx);
        } else if self.navigator.focus_handle(cx).contains_focused(window, cx) {
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
        let covered = self.menu.is_some()
            || self.prompt.is_some()
            || self.palette.is_some()
            || self.modal.is_some();
        crate::ui::Covered::note(covered, window, cx);
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
            // Here rather than around the navigator, so the details pane's buttons reach them.
            .map(|el| self.pr_actions(el, cx))
            .map(|el| self.resolve_actions(el, cx))
            .on_action(cx.listener(Self::go_to_branch))
            .on_action(cx.listener(Self::open_selected_in_terminal))
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
            .on_action(cx.listener(Self::focus_diff))
            .on_action(cx.listener(Self::start_commit))
            .on_action(cx.listener(Self::update_selected_project))
            .on_action(cx.listener(Self::toggle_navigator))
            .on_action(cx.listener(Self::command_palette))
            // The diff's own keys, pressed with the focus elsewhere, like the navigator.
            .on_action(cx.listener(|this, _: &crate::detail::ToggleSplit, _, cx| {
                this.detail.update(cx, |detail, cx| detail.toggle_split(cx))
            }))
            .on_action(cx.listener(|this, _: &crate::detail::ToggleWrap, _, cx| {
                this.detail.update(cx, |detail, cx| detail.toggle_wrap(cx))
            }))
            .on_action(
                cx.listener(|this, _: &crate::detail::FindInDiff, window, cx| {
                    this.detail
                        .update(cx, |detail, cx| detail.open_find(window, cx))
                }),
            )
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
                    // What the navigator's menus and keys, and the commit header's menu, do to
                    // the selection.
                    .map(|el| self.edit_actions(el, cx))
                    .on_drag_move(resize_pane(Pane::Navigator, self.store.clone()))
                    .when(self.navigator_open, |el| {
                        let files = self.detail.read(cx).files_shown(cx);
                        let width = self.store.read(cx).fitted_width(
                            Pane::Navigator,
                            window.viewport_size().width.into(),
                            true,
                            files,
                        );
                        el.child(
                            div()
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
                            .when_some(self.diff_left(window, cx), |el, left| {
                                el.child(
                                    div()
                                        .absolute()
                                        .top(px(HEADER_HEIGHT
                                            + self.detail.read(cx).view_header_height(cx)))
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
            .children(self.render_icon_picker(window, cx))
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

/// Have AppKit show the window's cursor again. GPUI tells it only when the cursor it wants
/// changes, so one left by another app or by the window's edge stays on screen when the window
/// becomes key with the pointer already over it, and its own cursor wanted there is the same.
fn show_cursor_again(window: &Window) {
    use objc2::{msg_send, runtime::AnyObject};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    let view = handle.ns_view.as_ptr().cast::<AnyObject>();
    // SAFETY: `view` is the window's live NSView, used on the main thread, and
    // `invalidateCursorRectsForView:` only asks AppKit to set its cursor rects again.
    unsafe {
        let ns_window: *mut AnyObject = msg_send![view, window];
        if !ns_window.is_null() {
            let _: () = msg_send![ns_window, invalidateCursorRectsForView: view];
        }
    }
}
