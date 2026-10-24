//! The window's root: the project navigator on the left, details on the right, the keys,
//! and the actions that work anywhere in the window. `overlays.rs` draws what opens above.

use gpui::{
    AnyView, App, ClipboardItem, Context, Entity, FocusHandle, Focusable, FontFeatures, KeyBinding,
    PromptLevel, Role, StyleRefinement, Subscription, Window, actions, div, prelude::*,
};

use crate::backend::{self, short_ref};
use crate::detail::{Detail, DiffPane};
use crate::navigator::Navigator;
use crate::overlays::{ContextMenu, Prompt};
use crate::palette::Palette;
use crate::projects::pick_folder;
use crate::state::{Cut, MeshPick, Pane, ProjectId, Selection, Store, snack};
use crate::theme::{Theme, UiScale};
use crate::transfer::{Place, Transfer};
use crate::ui::{HEADER_HEIGHT, dp, resize_pane};

actions!(
    gitbutler,
    [
        Quit,
        Hide,
        HideOthers,
        ShowAll,
        Minimize,
        Zoom,
        ZoomIn,
        ZoomOut,
        ActualSize,
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
        CheckForUpdates,
    ]
);

/// Keys follow GitButler Lite where it has one, and Zed's agent review for discarding a hunk.
pub fn bind_keys(cx: &mut App) {
    let nav = Some("Navigator && !TextInput");
    let workspace = Some("Workspace && !TextInput");
    cx.bind_keys([
        KeyBinding::new("secondary-q", Quit, None),
        // As a browser zooms, and Lite and the desktop app with it.
        KeyBinding::new("secondary-=", ZoomIn, None),
        KeyBinding::new("secondary-+", ZoomIn, None),
        KeyBinding::new("secondary--", ZoomOut, None),
        KeyBinding::new("secondary-0", ActualSize, None),
        KeyBinding::new("secondary-o", AddProject, None),
        // Not while typing, where they're ‰ and Ï.
        KeyBinding::new("alt-shift-f", Fetch, Some("!TextInput")),
        KeyBinding::new("alt-shift-r", UpdateFromTarget, Some("!TextInput")),
        KeyBinding::new("secondary-n", NewBranch, None),
        // Zed's key works too; ⌘K is the one shown, as the last binding wins.
        KeyBinding::new("secondary-shift-p", CommandPalette, None),
        KeyBinding::new("secondary-k", CommandPalette, None),
        KeyBinding::new("secondary-1", FocusNavigator, None),
        KeyBinding::new("secondary-2", FocusChanges, None),
        KeyBinding::new("secondary-3", FocusDiff, None),
        KeyBinding::new("secondary-enter", StartCommit, workspace),
        KeyBinding::new("secondary-z", Undo, workspace),
        KeyBinding::new("secondary-shift-z", Redo, workspace),
        KeyBinding::new("secondary-x", CutSelection, workspace),
        // ⌥ turns moving a commit onto another into squashing it, as when dragging. Bound
        // first, so ⌘V is the key shown.
        KeyBinding::new("alt-secondary-v", PasteHere, workspace),
        KeyBinding::new("shift-secondary-v", PasteHere, workspace),
        KeyBinding::new("secondary-v", PasteHere, workspace),
        KeyBinding::new("secondary-c", CopySelection, workspace),
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
        KeyBinding::new("secondary-backspace", DeleteSelection, nav),
        KeyBinding::new("secondary-alt-backspace", UncommitSelection, nav),
    ]);
    // Windows has these on the title bar instead.
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-m", Minimize, None),
    ]);
}

pub struct AppView {
    pub store: Entity<Store>,
    pub detail: Entity<Detail>,
    diff: Entity<DiffPane>,
    pub(crate) navigator: Entity<Navigator>,
    pub menu: Option<ContextMenu>,
    pub prompt: Option<Prompt>,
    /// The emoji picker open on a project's icon.
    pub icon_picker: Option<crate::projects::IconPicker>,
    pub navigator_open: bool,
    pub palette: Option<Palette>,
    /// The modal on screen, if any, what takes its keys, the subscription that closes it,
    /// and what had the keys before it opened, to give them back.
    pub modal: Option<(AnyView, FocusHandle, Subscription, Option<FocusHandle>)>,
    /// Palette commands by label, most recently run first.
    pub recent_commands: Vec<&'static str>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    /// Where the diff starts beside the detail's file list, when the selection has a diff.
    fn diff_left(&self, window: &Window, cx: &App) -> Option<f32> {
        let store = self.store.read(cx);
        let detail = self.detail.read(cx);
        let has_diff = store.selection.is_some() && !detail.diff_hidden(cx);
        has_diff.then(|| match detail.files_shown(window, cx) {
            false => 0.,
            true => detail.files_width(window, cx) + detail.beside_offset(cx),
        })
    }

    pub fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.set_global(theme_for(store.read(cx), window));
        #[cfg(target_os = "macos")]
        place_traffic_lights(window, cx);
        let app = cx.entity().downgrade();
        let detail = cx.new(|cx| Detail::new(store.clone(), app.clone(), window, cx));
        let diff = cx.new(|cx| DiffPane::new(&detail, window, cx));
        let navigator = cx.new(|cx| Navigator::new(app, store.clone(), detail.clone(), window, cx));
        window.focus(&navigator.focus_handle(cx), cx);
        let subscriptions = vec![
            // A menu's items and the palette's commands act on the selection and the picks, so
            // when an agent's change moves them off what they opened on, they go rather than
            // act on something else; a prompt on a branch goes with the branch. Menu items can
            // hold the commit they opened on, so a menu goes when an agent only rewrote it too,
            // as does M's list of them; the palette's commands read the selection when run.
            cx.observe_in(&store, window, |this, store, window, cx| {
                let store = store.read(cx);
                let menu_gone =
                    (this.menu.as_ref()).is_some_and(|m| m.acting_on != store.acting_on());
                let palette_gone = (this.palette.as_ref()).is_some_and(|p| match p.holds_ids {
                    true => p.acting_on != store.acting_on(),
                    false => !store.still_acting_on(&p.acting_on),
                });
                let prompt_gone = (this.prompt.as_ref())
                    .and_then(|prompt| prompt.on_branch.as_ref())
                    .filter(|(project, from)| {
                        (store.workspace(project)).is_some_and(|ws| ws.find_branch(from).is_none())
                    })
                    .map(|(_, from)| {
                        format!(
                            "{} was renamed or left the workspace meanwhile",
                            short_ref(from)
                        )
                    });
                if menu_gone {
                    this.menu = None;
                }
                if palette_gone {
                    this.close_palette(window, cx);
                }
                // Saying why, as it went from under what was being typed.
                if let Some(why) = prompt_gone {
                    this.cancel_prompt(window, cx);
                    snack(&this.store, why, cx);
                }
                cx.notify()
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                this.apply_appearance(window, cx)
            }),
            #[cfg(target_os = "macos")]
            cx.observe_global_in::<Theme>(window, |_, window, cx| place_traffic_lights(window, cx)),
            cx.observe_window_bounds(window, |this, window, cx| {
                this.store
                    .update(cx, |store, cx| store.save_window_place(window, cx));
            }),
            cx.observe_window_visibility(window, |this, visibility, _, cx| {
                if !scripted() {
                    this.store.update(cx, |store, cx| {
                        store.set_visible(visibility.is_visible(), cx)
                    });
                }
            }),
            // The CLI and GitButler's other apps add and remove projects too, and folders move.
            cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() {
                    #[cfg(target_os = "macos")]
                    show_cursor_again(window);
                    this.store.update(cx, |store, cx| {
                        store.load_projects(cx);
                        store.recheck_folders(cx);
                    });
                }
            }),
            // Keys go to what's focused, so when it leaves the screen, as the Branches page's
            // list does once Apply selects the branch, they go to the navigator instead, or to
            // the diff while a message is being written.
            cx.on_focus_lost(window, |this, window, cx| this.restore_focus(window, cx)),
        ];
        // Closing the window quits, so it asks the same as ⌘Q.
        let this = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            this.update(cx, |this, cx| this.can_quit(window, cx))
                .unwrap_or(true)
        });
        Self::listen_for_credential_prompts(window, cx);
        // Visibility arrives only as it changes, so a window that opens hidden says so here.
        if !scripted() {
            let visible = window.is_visible();
            store.update(cx, |store, cx| store.set_visible(visible, cx));
        }
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
        cx.set_global(theme_for(self.store.read(cx), window));
        cx.refresh_windows();
    }

    /// Select `selection`. Asked for again, as a crumb naming the page shown is clicked, the
    /// page goes back to how it opens, as the navigator's row does.
    pub fn select(&mut self, selection: Selection, window: &mut Window, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.reopen(selection, cx));
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
        self.confirm_then(None, message, detail, "Quit", window, cx, |_, cx| cx.quit());
        false
    }

    /// Draw the whole interface at `scale`.
    pub fn set_ui_scale(&mut self, scale: UiScale, cx: &mut Context<Self>) {
        (self.store).update(cx, |store, cx| {
            store.set_prefs(|prefs| prefs.ui_scale = scale, cx)
        });
        cx.set_global(Theme::get(cx).clone().with_scale(scale));
        // The lists keep the heights their rows had, which only the width changing drops.
        self.detail.update(cx, |detail, cx| detail.remeasure(cx));
        self.navigator
            .update(cx, |navigator, _| navigator.list.remeasure());
        cx.refresh_windows();
    }

    /// ⌘+, ⌘− and ⌘0: the scale `step` gives, said in a snackbar.
    fn scale_ui(&mut self, step: fn(UiScale) -> UiScale, cx: &mut Context<Self>) {
        let scale = step(self.store.read(cx).prefs.ui_scale);
        self.set_ui_scale(scale, cx);
        let message = format!("Interface at {}%", scale.percent());
        snack(&self.store, message, cx);
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

    /// Fetch the selected project, and ask its forge about pull requests and CI again.
    fn fetch(&mut self, _: &Fetch, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.selected_project(cx) {
            self.store.update(cx, |store, cx| {
                store.fetch(&project, cx);
                store.refresh_forge(&project);
            });
        }
    }

    fn new_branch(&mut self, _: &NewBranch, window: &mut Window, cx: &mut Context<Self>) {
        // Not while a prompt or picker is open: the branch would be made out of sight behind it.
        if self.covered() {
            return;
        }
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
        self.new_branch_at(project, true, None, window, cx, |path, _| {
            backend::create_branch(path, None)
        });
    }

    /// Ask the desktop app's update feed now: offer the newer version it has, or say this one
    /// is the latest.
    fn check_for_updates(
        &mut self,
        _: &CheckForUpdates,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(feed) = cx
            .try_global::<crate::Embedded>()
            .and_then(|e| e.update_feed.clone())
        else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let newer = backend::run(move || backend::settings::newer_version(&feed)).await;
            this.update_in(cx, |app, window, cx| match newer {
                Ok(Some(version)) => {
                    app.store.update(cx, |store, cx| {
                        store.newer_version = Some(version);
                        cx.notify();
                    });
                    app.restart_to_update(window, cx);
                }
                Ok(None) => snack(&app.store, "GitButler is up to date", cx),
                Err(err) => app.store.update(cx, |store, cx| {
                    store.failed("Couldn't check for updates", &err, cx)
                }),
            })
            .ok();
        })
        .detach();
    }

    /// Install the version the desktop app's update feed offers: start the desktop app's web
    /// client once, which installs it and starts again, and quit.
    pub fn restart_to_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(version) = store.newer_version.clone() else {
            return;
        };
        if store.unfinished() > 0 {
            snack(&self.store, "Wait for the running operations to finish", cx);
            return;
        }
        let detail = "GitButler opens its web app to install it, then starts in this app if \
                      Settings › Experimental › Open this app at start is on.";
        self.confirm_then(
            None,
            format!("Restart to update to GitButler {version}?"),
            detail,
            "Restart",
            window,
            cx,
            |app, cx| {
                // Off the UI thread: on Windows, starting a process can run the window's message
                // loop, which would enter the app again while this holds it.
                let store = app.store.clone();
                cx.spawn(async move |_, cx| {
                    let restarted = backend::run(|| {
                        let exe = std::env::current_exe()?;
                        std::process::Command::new(exe)
                            .env(crate::WEB_APP_ONCE, "1")
                            .spawn()?;
                        Ok(())
                    })
                    .await;
                    match restarted {
                        Ok(()) => cx.update(|cx| cx.quit()),
                        Err(err) => store.update(cx, |store, cx| {
                            store.error("Couldn't restart GitButler", err.to_string(), cx)
                        }),
                    }
                })
                .detach();
            },
        );
    }

    /// Ask with a native dialog before doing something destructive. Acting on something in a
    /// project, `at`, the detail starts with where, as [`Store::place`] says.
    #[expect(clippy::too_many_arguments)]
    pub fn confirm_then(
        &mut self,
        at: Option<Selection>,
        message: String,
        detail: &str,
        confirm: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut AppView, &mut Context<AppView>) + 'static,
    ) {
        let detail = match at {
            Some(at) => format!("{}\n{detail}", self.store.read(cx).place(&at)),
            None => detail.to_owned(),
        };
        // Info rather than Warning: macOS draws the two alike, and Windows would title every
        // confirmation "Warning", with a warning sign, as if something had gone wrong.
        let answer = window.prompt(
            PromptLevel::Info,
            &message,
            Some(&detail),
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
        // Pasting lets go of the cut, before it lands: letting go clears snackbars, and landing
        // can show one. A drop leaves the cut alone.
        if Transfer::resolve(&cut, &target, place, self.store.read(cx)).is_some() {
            self.store.update(cx, |store, cx| store.set_cut(None, cx));
        }
        self.transfer(&cut, &target, place, window, cx);
    }

    pub fn cut(&mut self, cut: Cut, cx: &mut Context<Self>) {
        let message = format!(
            "{} {}, ready to paste with {}",
            cut.verb(),
            cut.describe(self.store.read(cx)),
            crate::ui::keys_text("⌘V")
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
        place: Place,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(transfer) = Transfer::resolve(payload, target, place, self.store.read(cx)) else {
            snack(&self.store, "That can't go there", cx);
            return;
        };
        let project = target.project().clone();
        let staged = (self.store).update(cx, |store, cx| transfer.perform(&project, store, cx));
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
        // The menu first, as it can be open over a modal.
        if self.menu.take().is_some() {
            cx.notify();
            return;
        }
        if self.modal.is_some() {
            return self.close_modal(window, cx);
        }
        if self.detail.read(cx).is_focused(window, cx) {
            // Undo the nearest thing first: a text selection, then the file filter.
            // Then the branch opened on the Branches page, for its table.
            let handled = self.detail.update(cx, |detail, cx| {
                detail.clear_text_selection(cx)
                    || detail.clear_line_picks(cx)
                    || detail.clear_file_picks(cx)
                    || detail.close_open_filter(window, cx)
                    || detail.close_opened_branch(cx)
                    || detail.close_opened_target(cx)
                    || detail.close_changes(cx)
            });
            if handled {
                return cx.notify();
            }
            // What's cut or picked goes next, below, as the status bar's esc says.
            let store = self.store.read(cx);
            if store.cut.is_none() && store.picked.is_empty() {
                return self.focus_main(window, cx);
            }
        }
        // With nothing cut or picked, the branch opened on the Branches page goes back to its
        // table.
        let store = self.store.read(cx);
        if store.cut.is_none()
            && store.picked.is_empty()
            && self.detail.update(cx, |detail, cx| {
                detail.close_opened_branch(cx)
                    || detail.close_opened_target(cx)
                    || detail.close_changes(cx)
            })
        {
            return cx.notify();
        }
        let store = self.store.read(cx);
        let mut let_go = store.cut.is_some() || !store.picked.is_empty();
        if store.cut.is_some() {
            self.store.update(cx, |store, cx| store.set_cut(None, cx));
        }
        if !self.store.read(cx).picked.is_empty() {
            self.store.update(cx, |store, cx| {
                store.picked.clear();
                cx.notify()
            });
        }
        let_go |= self.detail.update(cx, |detail, cx| {
            detail.clear_file_picks(cx) | detail.clear_line_picks(cx)
        });
        // With nothing cut or picked, the newest card, as its Dismiss does.
        let store = self.store.read(cx);
        let card = (store.toasts.iter().rev())
            .find(|toast| matches!(toast.notice, crate::state::Notice::Error { .. }));
        if let Some(id) = card.map(|toast| toast.id).filter(|_| !let_go) {
            self.store
                .update(cx, |store, cx| store.dismiss_toast(id, cx));
        }
    }

    pub(crate) fn focus_navigator(
        &mut self,
        _: &FocusNavigator,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Not from under what's open over it, where the keys would go on to rows out of sight.
        if self.covered() {
            return;
        }
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

    /// Focus what's left on top once something above the window goes: the open modal, as
    /// Settings, rather than what's under it.
    pub(crate) fn restore_focus(&self, window: &mut Window, cx: &mut App) {
        match &self.modal {
            Some((_, focus, ..)) => window.focus(focus, cx),
            // A message being written whose box left the screen, as when an operation selects
            // something else, hands its keys to the diff rather than to the navigator, where
            // single letters rename and start branches. ⇧P still pushes from either.
            None if self.detail.read(cx).composing() => self
                .detail
                .update(cx, |detail, cx| detail.focus(window, cx)),
            None => self.focus_main(window, cx),
        }
    }

    /// The file list, or the diff while the list is hidden; a Branches, Operations history or
    /// project page's list.
    fn focus_changes(&mut self, _: &FocusChanges, window: &mut Window, cx: &mut Context<Self>) {
        if self.covered() {
            return;
        }
        self.detail
            .update(cx, |detail, cx| detail.focus_files(window, cx));
    }

    fn focus_diff(&mut self, _: &FocusDiff, window: &mut Window, cx: &mut Context<Self>) {
        if self.covered() {
            return;
        }
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
        if selection == checkout {
            return self
                .detail
                .update(cx, |detail, cx| detail.commit_or_write(window, cx));
        }
        self.store
            .update(cx, |store, cx| store.select(Some(checkout), cx));
        // Once the detail has taken the new selection's checks, which it does as this update
        // ends: opened before, the composer would be of the checkout left, and close.
        let detail = self.detail.clone();
        window.defer(cx, move |window, cx| {
            detail.update(cx, |detail, cx| detail.focus_message(window, cx))
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
            None => snack(&self.store, "No pull request here", cx),
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
            Selection::Commit(_, id) | Selection::Upstream(_, _, id) => Some(id.0.to_string()),
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

    /// Do `act` in the composer, if it's open, once this update ends: it opens its pickers
    /// and prompts through this view.
    fn in_composer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        act: impl FnOnce(&mut crate::detail::Detail, &mut Window, &mut Context<crate::detail::Detail>)
        + 'static,
    ) {
        // Not from what's open over it.
        if self.covered() {
            return;
        }
        let detail = self.detail.clone();
        window.defer(cx, move |window, cx| {
            detail.update(cx, |detail, cx| {
                if detail.composing() {
                    act(detail, window, cx)
                }
            })
        });
    }

    /// Whether a menu, prompt, picker or modal is open over the window.
    fn covered(&self) -> bool {
        self.menu.is_some()
            || self.prompt.is_some()
            || self.palette.is_some()
            || self.modal.is_some()
            || self.icon_picker.is_some()
    }

    /// Put `text` on the clipboard and say so.
    pub fn copy_text(&mut self, text: String, cx: &mut Context<Self>) {
        let message = copied(&text);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        snack(&self.store, message, cx);
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
            snack(&self.store, message, cx);
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
        // Every `dp` size is in rems, so this scales the whole window.
        window.set_rem_size(Theme::get(cx).scaled(crate::ui::BASE_REM));
        crate::ui::Covered::note(self.covered(), window, cx);
        let theme = Theme::get(cx).clone();
        let ligatures = self.store.read(cx).prefs.ligatures;
        let has_update_feed = cx
            .try_global::<crate::Embedded>()
            .is_some_and(|e| e.update_feed.is_some());
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
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.scale_ui(UiScale::larger, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.scale_ui(UiScale::smaller, cx)))
            .on_action(
                cx.listener(|this, _: &ActualSize, _, cx| {
                    this.scale_ui(|_| UiScale::default(), cx)
                }),
            )
            .on_action(cx.listener(Self::add_project))
            // Only with a feed to ask, so the palette offers it only then, as the menu does.
            .when(has_update_feed, |el| {
                el.on_action(cx.listener(Self::check_for_updates))
            })
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::clone_repository))
            // Here rather than around the navigator, so the details pane's buttons reach them.
            .map(|el| self.pr_actions(el, cx))
            .map(|el| self.resolve_actions(el, cx))
            .on_action(cx.listener(Self::go_to_branch))
            .on_action(cx.listener(Self::show_actions))
            .on_action(cx.listener(Self::open_selected_in_terminal))
            // The selection doesn't decide: an operation that finished in another project is
            // still the last thing done.
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.store.update(cx, Store::undo_last)))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.store.update(cx, Store::redo_last)))
            .on_action(cx.listener(Self::fetch))
            .on_action(cx.listener(Self::new_branch))
            .on_action(cx.listener(Self::push_selected))
            .on_action(cx.listener(Self::cut_selection))
            // Only while something is cut, so ⌘K doesn't offer it with nothing to paste.
            .when(self.store.read(cx).cut.is_some(), |el| {
                el.on_action(cx.listener(Self::paste_here))
            })
            .on_action(cx.listener(Self::copy_selection))
            .on_action(cx.listener(Self::open_pull_request))
            .on_action(cx.listener(|this, _: &ToggleFileTree, _, cx| {
                (this.store).update(cx, |store, cx| {
                    store.set_prefs(|prefs| prefs.flat_files = !prefs.flat_files, cx)
                })
            }))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::focus_navigator))
            .on_action(cx.listener(Self::focus_changes))
            .on_action(cx.listener(Self::focus_diff))
            .on_action(cx.listener(Self::start_commit))
            .on_action(cx.listener(Self::update_selected_project))
            .on_action(cx.listener(Self::toggle_navigator))
            .on_action(cx.listener(Self::command_palette))
            // The composer's keys, pressed with the focus elsewhere, like the navigator.
            .on_action(
                cx.listener(|this, _: &crate::detail::AmendTop, window, cx| {
                    this.in_composer(window, cx, Detail::amend_top)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::detail::PickCommitTarget, window, cx| {
                    this.in_composer(window, cx, Detail::pick_commit_target)
                }),
            )
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
            // Only the code font has ligatures (`liga`), so this leaves the UI font as it is.
            .when(!ligatures, |el| {
                el.font_features(FontFeatures(std::sync::Arc::new(vec![("liga".into(), 0)])))
            })
            .text_size(dp(13.))
            .line_height(dp(18.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    // What the navigator's menus and keys, and the commit header's menu, do to
                    // the selection.
                    .map(|el| self.edit_actions(el, cx))
                    .on_drag_move(resize_pane(Pane::Navigator, self.store.clone()))
                    .when(self.navigator_open, |el| {
                        let files = self.detail.read(cx).files_shown(window, cx);
                        let width = self.store.read(cx).fitted_width(
                            Pane::Navigator,
                            f32::from(window.viewport_size().width) / theme.scale,
                            true,
                            files,
                        );
                        el.child(
                            div()
                                .w(dp(width))
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
                            .min_w_0()
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
                                        .top(dp(HEADER_HEIGHT
                                            + self.detail.read(cx).view_header_height(cx)))
                                        .bottom_0()
                                        .right_0()
                                        .left(dp(left))
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
            .children(self.render_modal(cx))
            // Above a modal: Git can ask for credentials while one is open.
            .children(self.render_prompt(window, cx))
            .children(self.render_palette(cx))
            // Above what dims the window, so their buttons take clicks rather than close it.
            .child(self.render_toasts(cx))
            .children(self.render_menu(cx))
            .children(self.render_icon_picker(window, cx))
            // Last, so the window can be closed while a modal or menu is open.
            .map(|el| {
                #[cfg(target_os = "windows")]
                return el.child(crate::ui::window_controls::window_controls(window, cx));
                #[cfg(not(target_os = "windows"))]
                el
            })
    }
}

/// Puts macOS's traffic lights, which keep their size at every interface scale, halfway down
/// the header, however tall the scale draws it.
#[cfg(target_os = "macos")]
fn place_traffic_lights(window: &Window, cx: &App) {
    use crate::ui::fixed;
    use gpui::point;

    const LIGHTS: f32 = 14.;
    let top = (Theme::get(cx).scaled(HEADER_HEIGHT) - fixed(LIGHTS)) / 2.;
    window.set_traffic_light_position(point(fixed(14.), top));
}

/// The theme the appearance setting and the system call for.
fn theme_for(store: &Store, window: &Window) -> Theme {
    let prefs = &store.prefs;
    Theme::for_appearance(prefs.appearance, window.appearance(), prefs.code_size)
        .with_scale(prefs.ui_scale)
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
#[cfg(target_os = "macos")]
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

/// Whether a dev script drives the window, which then says when it's hidden.
fn scripted() -> bool {
    #[cfg(feature = "dev-script")]
    return crate::dev::scripted();
    #[cfg(not(feature = "dev-script"))]
    false
}
