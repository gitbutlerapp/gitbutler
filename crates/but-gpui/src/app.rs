//! The window's root: the project navigator on the left, details on the right, and the
//! overlays (menus, prompts, toasts) above both.

use std::path::Path;
use std::rc::Rc;

use anyhow::Result;
use gpui::{
    Action, AnyElement, AnyView, App, ClickEvent, ClipboardItem, Context, DismissEvent, Entity,
    FocusHandle, Focusable, KeyBinding, ManagedView, MouseButton, PathPromptOptions, Pixels, Point,
    PromptLevel, Role, SharedString, StyleRefinement, Subscription, Window, actions, anchored,
    deferred, div, prelude::*, px, relative,
};

use crate::backend::{self, CommitId};
use crate::detail::{Detail, DiffPane};
use crate::navigator::Navigator;
use crate::palette::Palette;
use crate::state::{Cut, Notice, Overlay, Pane, ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::transfer::{Transfer, short_ref};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, STATUS_BAR_HEIGHT, button, h_flex, icon, keycap, list_row,
    popover, resize_pane, shortcut, v_flex,
};

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

/// A right-click menu.
pub struct ContextMenu {
    pub position: Point<Pixels>,
    pub items: Vec<MenuItem>,
}

type MenuHandler = Rc<dyn Fn(&mut AppView, &mut Window, &mut Context<AppView>)>;
type SubmitHandler = Box<dyn FnOnce(&mut AppView, String, &mut Window, &mut Context<AppView>)>;
type CancelHandler = Box<dyn FnOnce(&mut AppView)>;

#[derive(Clone)]
pub struct MenuItem {
    pub label: SharedString,
    /// The keys that do the same, shown at the end of the row.
    pub key: Option<SharedString>,
    pub danger: bool,
    /// Starts a new group, with a line above it.
    pub separated: bool,
    pub handler: MenuHandler,
}

impl MenuItem {
    pub fn new(
        label: impl Into<SharedString>,
        handler: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            key: None,
            danger: false,
            separated: false,
            handler: Rc::new(handler),
        }
    }

    /// An item that dispatches `action` where focus is, showing the keys bound to it.
    pub fn action(label: impl Into<SharedString>, action: &dyn Action, cx: &App) -> Self {
        let key = shortcut(action, cx);
        let action = action.boxed_clone();
        Self::new(label, move |_, window, cx| {
            window.dispatch_action(action.boxed_clone(), cx)
        })
        .key(key)
    }

    pub fn key(mut self, key: Option<SharedString>) -> Self {
        self.key = key;
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn separated(mut self) -> Self {
        self.separated = true;
        self
    }
}

/// A small modal asking for one line of text: a branch name or a credential.
struct Prompt {
    title: SharedString,
    detail: Option<SharedString>,
    confirm: SharedString,
    input: Entity<TextInput>,
    on_submit: SubmitHandler,
    on_cancel: Option<CancelHandler>,
    _subscription: Subscription,
}

pub struct AppView {
    pub store: Entity<Store>,
    pub detail: Entity<Detail>,
    diff: Entity<DiffPane>,
    navigator: Entity<Navigator>,
    pub menu: Option<ContextMenu>,
    prompt: Option<Prompt>,
    pub navigator_open: bool,
    /// The diff takes the whole window.
    pub zoomed: bool,
    pub palette: Option<Palette>,
    /// The modal on screen, if any, and the subscription that closes it.
    modal: Option<(AnyView, Subscription)>,
    /// Palette commands by label, most recently run first.
    pub recent_commands: Vec<&'static str>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    /// Where the diff starts beside the detail's file list, when the selection has a diff.
    fn diff_left(&self, cx: &App) -> Option<f32> {
        let store = self.store.read(cx);
        let has_diff = store.selection.is_some();
        has_diff.then(|| match self.detail.read(cx).zoomed {
            true => 0.,
            false => store.width(Pane::Files),
        })
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = cx.new(Store::new);
        let appearance = store.read(cx).prefs.appearance;
        cx.set_global(Theme::for_appearance(appearance, window.appearance()));
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
            _subscriptions: subscriptions,
        }
    }

    /// Use the theme the appearance setting and the system call for.
    pub fn apply_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let choice = self.store.read(cx).prefs.appearance;
        cx.set_global(Theme::for_appearance(choice, window.appearance()));
        cx.refresh_windows();
    }

    pub fn select(&mut self, selection: Selection, window: &mut Window, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.select(Some(selection), cx));
        self.focus_main(window, cx);
    }

    fn selection(&self, cx: &App) -> Option<Selection> {
        self.store.read(cx).selection.clone()
    }

    fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
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
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add project".into()),
        });
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.into_iter().next()
            {
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

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.selection(cx).map(|s| s.project().clone()) {
            self.store
                .update(cx, |store, cx| store.undo(&project, false, cx));
        }
    }

    fn fetch(&mut self, _: &Fetch, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.selection(cx).map(|s| s.project().clone()) {
            self.store.update(cx, |store, cx| store.fetch(&project, cx));
        }
    }

    pub fn new_branch(&mut self, _: &NewBranch, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.selection(cx).map(|s| s.project().clone()).or_else(|| {
            self.store
                .read(cx)
                .projects
                .iter()
                .find(|p| p.expanded)
                .map(|p| p.id.clone())
        }) else {
            return;
        };
        self.prompt_branch(
            project,
            "New branch",
            "Create",
            "",
            window,
            cx,
            |path, name| backend::create_branch(path, Some(name)),
        );
    }

    /// The selected branch, or the branch of the selected commit, by project and ref.
    fn selected_branch(&self, cx: &App) -> Option<(ProjectId, String)> {
        let selection = self.selection(cx)?;
        let branch = self.store.read(cx).branch_for(&selection)?;
        Some((selection.project().clone(), branch.ref_name.clone()))
    }

    fn push_selected(&mut self, _: &Push, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((project, ref_name)) = self.selected_branch(cx) {
            self.store
                .update(cx, |store, cx| store.push(&project, ref_name, cx));
        }
    }

    fn delete_selection(
        &mut self,
        _: &DeleteSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.selection(cx) {
            Some(Selection::Commit(project, id)) => self.discard_commit(project, id, window, cx),
            Some(Selection::Branch(project, ref_name)) => {
                self.remove_branch(project, ref_name, window, cx)
            }
            Some(Selection::Uncommitted(project)) => {
                let changes = self.detail.read(cx).selected_changes(cx);
                self.discard(project, changes, window, cx);
            }
            _ => {}
        }
    }

    fn rename_selection(
        &mut self,
        _: &RenameSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.selection(cx) {
            Some(Selection::Branch(project, ref_name)) => {
                let current = short_ref(&ref_name).to_owned();
                self.prompt_branch(
                    project,
                    "Rename branch",
                    "Rename",
                    &current,
                    window,
                    cx,
                    move |path, name| backend::rename_branch(path, &ref_name, name),
                )
            }
            Some(Selection::Commit(..)) => self
                .detail
                .update(cx, |detail, cx| detail.start_reword(window, cx)),
            _ => {}
        }
    }

    fn new_branch_on_top(
        &mut self,
        _: &NewBranchOnTop,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((project, below)) = self
            .selected_branch(cx)
            .filter(|(_, ref_name)| !ref_name.is_empty())
        {
            self.prompt_branch(
                project,
                "New branch on top",
                "Create",
                "",
                window,
                cx,
                move |path, name| backend::create_dependent_branch(path, &below, name),
            );
        }
    }

    fn uncommit_selection(
        &mut self,
        _: &UncommitSelection,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(Selection::Commit(project, id)) = self.selection(cx) {
            self.store
                .update(cx, |store, cx| store.uncommit(&project, id, cx));
        }
    }

    fn remove_branch(
        &mut self,
        project: ProjectId,
        ref_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = short_ref(&ref_name).to_owned();
        self.confirm_then(
            format!("Delete branch {name}?"),
            "Its commits go too. You can undo this with ⌘Z.",
            "Delete",
            window,
            cx,
            move |this, cx| {
                let project = project.clone();
                let ref_name = ref_name.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Delete branch",
                        Overlay::default(),
                        move |path, _| backend::remove_branch(path, &ref_name),
                        |store, (), cx| store.snack_undoable("Branch deleted", cx),
                        cx,
                    )
                });
            },
        );
    }

    fn discard_commit(
        &mut self,
        project: ProjectId,
        id: CommitId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_then(
            format!("Discard commit {}?", id.short()),
            "Its changes are dropped. You can undo this with ⌘Z.",
            "Discard",
            window,
            cx,
            move |this, cx| {
                let project = project.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Discard commit",
                        Overlay::commits(store.key_of(&project, id)),
                        move |path, remap| backend::discard_commit(path, remap.get(id)?),
                        |store, (), cx| store.snack_undoable("Commit discarded", cx),
                        cx,
                    )
                });
            },
        );
    }

    /// Throw away uncommitted changes. One file or hunk goes at once, with Undo on the
    /// snackbar as in Zed's review; several files ask first.
    pub fn discard(
        &mut self,
        project: ProjectId,
        changes: Vec<backend::Selected>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let run = move |this: &mut AppView,
                        changes: Vec<backend::Selected>,
                        cx: &mut Context<AppView>| {
            let project = project.clone();
            this.store.update(cx, |store, cx| {
                store.run(
                    &project,
                    "Discard",
                    Overlay::files(&changes),
                    move |path, _| backend::discard(path, &changes),
                    |store, (), cx| store.snack_undoable("Changes discarded", cx),
                    cx,
                )
            });
        };
        match changes.len() {
            0 => {}
            1 => run(self, changes, cx),
            n => self.confirm_then(
                format!("Discard changes to {n} files?"),
                "You can undo this with ⌘Z.",
                "Discard",
                window,
                cx,
                move |this, cx| run(this, changes, cx),
            ),
        }
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

    /// Ask for a branch name, then run `op` with it and select the branch it returns.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prompt_branch(
        &mut self,
        project: ProjectId,
        title: &str,
        confirm: &str,
        initial: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        op: impl FnOnce(&Path, &str) -> Result<String> + Send + 'static,
    ) {
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Branch name", false, cx);
            input.set_text(initial, cx);
            input.select_all_text(cx);
            input
        });
        let label = format!("{confirm} branch");
        self.open_prompt(
            title,
            None,
            confirm,
            input,
            None,
            window,
            cx,
            move |this, name, _, cx| {
                let name = name.trim().to_owned();
                if name.is_empty() {
                    return;
                }
                let selected = project.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        label,
                        Overlay::default(),
                        move |path, _| op(path, &name),
                        move |store, ref_name, cx| {
                            store.select(Some(Selection::Branch(selected, ref_name)), cx)
                        },
                        cx,
                    )
                });
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn open_prompt(
        &mut self,
        title: &str,
        detail: Option<String>,
        confirm: &str,
        input: Entity<TextInput>,
        on_cancel: Option<CancelHandler>,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_submit: impl FnOnce(&mut AppView, String, &mut Window, &mut Context<AppView>) + 'static,
    ) {
        // A prompt Git is waiting on, for credentials, stays until answered: replacing it
        // would fail the push that asked. A second one is cancelled, so its Git process
        // fails rather than waits forever.
        if self
            .prompt
            .as_ref()
            .is_some_and(|prompt| prompt.on_cancel.is_some())
        {
            if let Some(on_cancel) = on_cancel {
                on_cancel(self);
            }
            return;
        }
        let subscription =
            cx.subscribe_in(
                &input,
                window,
                |this, input, event, window, cx| match event {
                    InputEvent::Submit => {
                        let text = input.read(cx).text().to_owned();
                        if let Some(prompt) = this.prompt.take() {
                            (prompt.on_submit)(this, text, window, cx);
                        }
                        this.focus_main(window, cx);
                        cx.notify();
                    }
                    InputEvent::Cancel => {
                        this.cancel_prompt(window, cx);
                    }
                    InputEvent::Changed => {}
                },
            );
        window.focus(&input.focus_handle(cx), cx);
        self.prompt = Some(Prompt {
            title: title.to_owned().into(),
            detail: detail.map(Into::into),
            confirm: confirm.to_owned().into(),
            input,
            on_submit: Box::new(on_submit),
            on_cancel,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn cancel_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(prompt) = self.prompt.take()
            && let Some(on_cancel) = prompt.on_cancel
        {
            on_cancel(self);
        }
        self.focus_main(window, cx);
        cx.notify();
    }

    /// Git asks for passphrases and passwords through GitButler; show those as a prompt.
    fn listen_for_credential_prompts(window: &mut Window, cx: &mut Context<Self>) {
        let (sender, mut prompts) = futures::channel::mpsc::unbounded();
        backend::askpass::init(move |prompt| {
            sender.unbounded_send(prompt).ok();
        });
        cx.spawn_in(window, async move |this, cx| {
            use futures::StreamExt as _;
            while let Some(prompt) = prompts.next().await {
                let id = prompt.id;
                let result = this.update_in(cx, |this, window, cx| {
                    let input =
                        cx.new(|cx| TextInput::new("Passphrase or password", false, cx).masked());
                    this.open_prompt(
                        "Git needs your credentials",
                        Some(prompt.text),
                        "Continue",
                        input,
                        Some(Box::new(move |_| backend::askpass::respond(id, None))),
                        window,
                        cx,
                        move |_, secret, _, _| backend::askpass::respond(id, Some(secret)),
                    );
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn cut_selection(&mut self, _: &CutSelection, _: &mut Window, cx: &mut Context<Self>) {
        let cut = match self.selection(cx) {
            Some(Selection::Commit(project, id)) => Some(Cut::Commit(project, id)),
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
        self.transfer(&cut, &target, window.modifiers().alt, window, cx);
    }

    pub fn cut(&mut self, cut: Cut, cx: &mut Context<Self>) {
        let verb = if matches!(cut, Cut::Pick(..)) {
            "Copied"
        } else {
            "Cut"
        };
        let message = format!("{verb} {}. Pick a spot and press ⌘V.", cut.describe());
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
        alternate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(transfer) = Transfer::resolve(payload, target, alternate, self.store.read(cx))
        else {
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

    /// Show `view` over the window until it emits `DismissEvent` or the backdrop is clicked.
    pub fn open_modal<V: ManagedView>(
        &mut self,
        view: Entity<V>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subscription =
            cx.subscribe_in(&view, window, |this, _, _: &DismissEvent, window, cx| {
                this.close_modal(window, cx)
            });
        window.focus(&view.focus_handle(cx), cx);
        self.modal = Some((view.into(), subscription));
        cx.notify();
    }

    fn close_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal.take().is_some() {
            self.focus_main(window, cx);
            cx.notify();
        }
    }

    fn render_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (view, _) = self.modal.as_ref()?;
        Some(
            div()
                .id("modal-backdrop")
                .absolute()
                .inset_0()
                .bg(gpui::black().opacity(0.25))
                .flex()
                .justify_center()
                .items_center()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_modal(window, cx)),
                )
                .child(
                    div()
                        .flex()
                        .max_h(relative(0.88))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(view.clone()),
                )
                .into_any_element(),
        )
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
            self.zoomed = false;
            self.sync_chrome(cx);
            return;
        }
        if self.detail.read(cx).is_focused(window, cx) {
            if !self
                .detail
                .update(cx, |detail, cx| detail.clear_text_selection(cx))
            {
                self.focus_main(window, cx);
            }
            return;
        }
        if self.store.read(cx).cut.is_some() {
            self.store.update(cx, |store, cx| store.set_cut(None, cx));
        }
    }

    fn focus_navigator(&mut self, _: &FocusNavigator, window: &mut Window, cx: &mut Context<Self>) {
        // Asking for the navigator brings it back.
        if !self.navigator_open || self.zoomed {
            self.navigator_open = true;
            self.zoomed = false;
            self.sync_chrome(cx);
        }
        window.focus(&self.navigator.focus_handle(cx), cx);
    }

    /// Focus the navigator, or the changes while the navigator isn't on screen, so keys
    /// always reach something visible.
    fn focus_main(&self, window: &mut Window, cx: &mut App) {
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
        if let Some(project) = self.selection(cx).map(|s| s.project().clone()) {
            self.store.update(cx, |store, cx| {
                store.select(Some(Selection::Uncommitted(project)), cx)
            });
            self.detail
                .update(cx, |detail, cx| detail.focus_message(window, cx));
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.selection(cx).map(|s| s.project().clone()) {
            self.store
                .update(cx, |store, cx| store.undo(&project, true, cx));
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
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self.selection(cx).map(|s| s.project().clone()) {
            self.store
                .update(cx, |store, cx| store.update_from_target(&project, None, cx));
        }
    }

    pub fn toggle_navigator(
        &mut self,
        _: &ToggleNavigator,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigator_open = !self.navigator_open;
        self.sync_chrome(cx);
        if self.navigator.focus_handle(cx).contains_focused(window, cx) {
            self.focus_main(window, cx);
        }
    }

    fn command_palette(&mut self, _: &CommandPalette, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.is_some() {
            self.close_palette(window, cx);
        } else {
            self.open_palette(window, cx);
        }
    }

    fn toggle_zoom(&mut self, _: &ToggleZoom, window: &mut Window, cx: &mut Context<Self>) {
        self.zoomed = !self.zoomed;
        self.sync_chrome(cx);
        if self.navigator.focus_handle(cx).contains_focused(window, cx) {
            self.focus_main(window, cx);
        }
    }

    /// The details pane draws its own header, so it needs to know what's around it.
    fn sync_chrome(&mut self, cx: &mut Context<Self>) {
        let (zoomed, beside_traffic_lights) = (self.zoomed, self.zoomed || !self.navigator_open);
        self.detail.update(cx, |detail, cx| {
            detail.zoomed = zoomed;
            detail.beside_traffic_lights = beside_traffic_lights;
            cx.notify();
        });
        cx.notify();
    }

    pub fn open_menu(
        &mut self,
        position: Point<Pixels>,
        items: Vec<MenuItem>,
        cx: &mut Context<Self>,
    ) {
        self.menu = Some(ContextMenu { position, items });
        cx.notify();
    }

    fn render_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.menu.as_ref()?;
        let theme = Theme::get(cx);
        let items = menu.items.iter().enumerate().flat_map(|(ix, item)| {
            let handler = item.handler.clone();
            let line = (item.separated && ix > 0).then(|| {
                div()
                    .h(px(1.))
                    .mx(px(10.))
                    .my(px(4.))
                    .bg(theme.border_3)
                    .into_any_element()
            });
            let row = list_row(("menu-item", ix), false, theme)
                .mx(px(4.))
                .gap(px(16.))
                .text_size(px(13.))
                .text_color(if item.danger {
                    theme.danger
                } else {
                    theme.text_1
                })
                .child(div().flex_1().child(item.label.clone()))
                .when_some(item.key.clone(), |el, key| el.child(keycap(&key, theme)))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.menu = None;
                    handler(this, window, cx);
                    cx.notify();
                }))
                .into_any_element();
            line.into_iter().chain([row])
        });
        Some(
            deferred(
                anchored()
                    .position(menu.position)
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        popover(theme)
                            .id("context-menu")
                            // Clicks on an item stop here, not on the row under it.
                            .occlude()
                            .min_w(px(180.))
                            .py(px(4.))
                            .children(items)
                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                this.menu = None;
                                cx.notify();
                            })),
                    ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    fn render_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let prompt = self.prompt.as_ref()?;
        let theme = Theme::get(cx);
        let input = prompt.input.clone();
        Some(
            div()
                .id("prompt-backdrop")
                .absolute()
                .inset_0()
                .bg(gpui::black().opacity(0.2))
                .flex()
                .justify_center()
                .items_start()
                .pt(px(120.))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.cancel_prompt(window, cx);
                    }),
                )
                .child(
                    popover(theme)
                        .id("prompt")
                        .w(px(380.))
                        .h_auto()
                        .p(px(16.))
                        .gap(px(12.))
                        .rounded(px(10.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(prompt.title.clone()),
                        )
                        .when_some(prompt.detail.clone(), |el, detail| {
                            el.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.text_2)
                                    .child(detail),
                            )
                        })
                        .child(
                            div()
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(px(4.))
                                .border_1()
                                .border_color(theme.pop)
                                .font_family(theme.mono_font.clone())
                                .text_size(px(13.))
                                .child(prompt.input.clone()),
                        )
                        .child(
                            h_flex()
                                .justify_end()
                                .gap(px(8.))
                                .child(
                                    button("prompt-cancel", "Cancel")
                                        .style(ButtonStyle::Ghost)
                                        .on_click({
                                            let input = input.clone();
                                            move |_, window, cx| {
                                                input.update(cx, |_, cx| {
                                                    cx.emit(InputEvent::Cancel)
                                                });
                                                window.refresh();
                                            }
                                        }),
                                )
                                .child(
                                    button("prompt-confirm", prompt.confirm.clone())
                                        .style(ButtonStyle::Gray)
                                        .on_click(move |_, _, cx| {
                                            input.update(cx, |_, cx| cx.emit(InputEvent::Submit))
                                        }),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    /// Snackbars at the bottom centre, error cards at the bottom right.
    fn render_toasts(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let store = self.store.read(cx);
        let mut errors = Vec::new();
        let mut snackbar = None;
        for toast in &store.toasts {
            let id = toast.id;
            let dismiss = cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.store
                    .update(cx, |store, cx| store.dismiss_toast(id, cx))
            });
            match &toast.notice {
                Notice::Snackbar { message, undo } => {
                    snackbar = Some(
                        h_flex()
                            .id(("snackbar", id))
                            .role(Role::Alert)
                            .aria_label(message.clone())
                            .h(px(32.))
                            .pl(px(12.))
                            .pr(px(if undo.is_some() { 4. } else { 12. }))
                            .gap(px(8.))
                            .rounded(px(8.))
                            .bg(theme.snackbar_bg)
                            .text_color(theme.snackbar_text)
                            .text_size(px(12.))
                            .shadow_lg()
                            .child(message.clone())
                            .when_some(undo.clone(), |el, (project, snapshot)| {
                                el.child(
                                    div()
                                        .w(px(1.))
                                        .h(px(16.))
                                        .bg(theme.snackbar_text.opacity(0.2)),
                                )
                                .child(
                                    h_flex()
                                        .id(("undo", id))
                                        .h(px(24.))
                                        .px(px(8.))
                                        .rounded(px(5.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .hover(|el| el.bg(theme.snackbar_text.opacity(0.12)))
                                        .child("Undo")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.store.update(cx, |store, cx| {
                                                store.dismiss_toast(id, cx);
                                                store.undo_snapshot(&project, snapshot, cx);
                                            })
                                        })),
                                )
                            })
                            .on_click(dismiss),
                    );
                }
                Notice::Error { title, detail } => {
                    let copied = detail.to_string();
                    errors.push(
                        popover(theme)
                            .id(("error", id))
                            .role(Role::Alert)
                            .aria_label(title.clone())
                            .aria_description(detail.clone())
                            .w(px(320.))
                            .p(px(14.))
                            .gap(px(8.))
                            .child(
                                h_flex()
                                    .gap(px(8.))
                                    .child(icon(Icon::Warning).text_color(theme.danger))
                                    .child(
                                        div()
                                            .flex_1()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(title.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.text_2)
                                    .line_clamp(4)
                                    .child(detail.clone()),
                            )
                            .child(
                                h_flex()
                                    .gap(px(6.))
                                    .justify_end()
                                    .child(
                                        button(("copy-error", id), "Copy error")
                                            .small()
                                            .style(ButtonStyle::Ghost)
                                            .on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(
                                                    gpui::ClipboardItem::new_string(copied.clone()),
                                                )
                                            }),
                                    )
                                    .child(
                                        button(("dismiss", id), "Dismiss")
                                            .small()
                                            .on_click(dismiss),
                                    ),
                            ),
                    );
                }
            }
        }
        div()
            .absolute()
            .inset_0()
            .child(
                v_flex()
                    .absolute()
                    .bottom(px(STATUS_BAR_HEIGHT + 12.))
                    .right(px(16.))
                    .gap(px(8.))
                    .children(errors),
            )
            .children(snackbar.map(|snackbar| {
                h_flex()
                    .absolute()
                    .bottom(px(STATUS_BAR_HEIGHT + 16.))
                    .left_0()
                    .right_0()
                    .justify_center()
                    .child(snackbar)
            }))
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
            .on_action(cx.listener(Self::create_pull_request))
            .on_action(cx.listener(Self::edit_pull_request))
            .on_action(cx.listener(Self::toggle_draft))
            .on_action(cx.listener(Self::merge_pull_request))
            .on_action(cx.listener(Self::close_pull_request))
            .on_action(cx.listener(Self::go_to_branch))
            .on_action(cx.listener(Self::resolve_conflicts))
            .on_action(cx.listener(Self::open_selected_in_terminal))
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::undo))
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
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::update_selected_project))
            .on_action(cx.listener(Self::toggle_navigator))
            .on_action(cx.listener(Self::toggle_zoom))
            .on_action(cx.listener(Self::command_palette))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.bg_1)
            .text_color(theme.text_1)
            .font_family(theme.ui_font.clone())
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
                                // Navigator keys that open prompts, which the window owns.
                                .map(|el| self.edit_actions(el, cx))
                                .on_action(cx.listener(Self::delete_selection))
                                .on_action(cx.listener(Self::rename_selection))
                                .on_action(cx.listener(Self::new_branch_on_top))
                                .on_action(cx.listener(Self::uncommit_selection))
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
            .children(self.render_prompt(cx))
            .children(self.render_modal(cx))
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
