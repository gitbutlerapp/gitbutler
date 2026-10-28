//! The window's root: the project navigator on the left, details on the right, the keys,
//! and the actions that work anywhere in the window. `overlays.rs` draws what opens above.

use gpui::{
    AnyView, App, ClipboardItem, Context, Entity, FocusHandle, Focusable, FontFeatures, Pixels,
    Point, PromptLevel, Role, StyleRefinement, Subscription, WeakEntity, Window, actions, div,
    prelude::*,
};

use crate::backend::{self, short_ref};
use crate::detail::{Detail, DiffPane};
use crate::navigator::Navigator;
use crate::overlays::{ContextMenu, MenuItem, Prompt};
use crate::palette::Palette;
use crate::projects::pick_folder;
use crate::state::{Cut, MeshPick, Pane, ProjectId, Selection, Store, StoreEvent, snack};
use crate::theme::{Theme, UiScale};
use crate::transfer::{Place, Transfer};
use crate::ui::{HEADER_HEIGHT, dp, resize_pane};

// Named in the keymap as area::Action, by where they act.
actions!(
    app,
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
        ToggleFileTree,
        Undo,
        Redo,
        Cancel,
        FocusNavigator,
        FocusChanges,
        FocusDiff,
        FocusNextPane,
        FocusPreviousPane,
        FocusPaneLeft,
        FocusPaneRight,
        StartCommit,
        ToggleNavigator,
        CommandPalette,
        CheckForUpdates,
        BuildAndRestart,
        OperationsHistory,
        CloseWindow,
    ]
);
actions!(
    navigator,
    [
        CopySelection,
        ToggleRead,
        MarkAllRead,
        NewBranch,
        NewBranchOnTop,
        SelectPrevious,
        SelectNext,
        Expand,
        Collapse,
        Confirm,
        CutSelection,
        PasteHere,
        DeleteSelection,
        UncommitSelection,
        RenameSelection,
    ]
);
actions!(remote, [Fetch, UpdateFromTarget, Push]);
actions!(pr, [OpenPullRequest]);

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
    /// The modal on screen, if any, what takes its keys, its card's own, which Tab keeps
    /// to, the subscription that closes it, and what had the keys before it opened, to give
    /// them back.
    pub modal: Option<(
        AnyView,
        FocusHandle,
        FocusHandle,
        Subscription,
        Option<FocusHandle>,
    )>,
    /// Palette commands by label, most recently run first.
    pub recent_commands: Vec<&'static str>,
    /// How to build the app again, when `cargo run` started it, for Build and restart.
    pub(crate) rebuild: Option<backend::rebuild::Rebuild>,
    /// The build under way or last ended, and what shows it when it fails out of sight.
    pub(crate) building: Option<(Entity<crate::rebuild::BuildOutput>, Subscription)>,
    /// The frames drawn each second, drawn over everything while `Prefs::show_fps` is on.
    fps: Entity<crate::fps::Fps>,
    _subscriptions: Vec<Subscription>,
}

/// The app, as the views it owns reach it: only once the update running now is over. Reached
/// at once, the app could update or read the view that asked, which is being updated, and that
/// panics with "already being updated". So would a view the app's own handler is updating that
/// updated the app back. Work that depends on what the app does goes in the closure.
#[derive(Clone)]
pub struct AppHandle(WeakEntity<AppView>);

impl AppHandle {
    pub fn of(cx: &Context<AppView>) -> Self {
        Self(cx.entity().downgrade())
    }

    pub fn update_later(
        &self,
        window: &mut Window,
        cx: &mut App,
        update: impl FnOnce(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
    ) {
        let app = self.0.clone();
        window.defer(cx, move |window, cx| {
            app.update(cx, |app, cx| update(app, window, cx)).ok();
        });
    }

    /// Open at `at` the menu `items` makes from the view `from`, once the update running now is
    /// over, so building it can read any view.
    pub fn open_menu<V: 'static>(
        &self,
        from: &WeakEntity<V>,
        at: Point<Pixels>,
        items: impl FnOnce(&mut V, &mut Context<V>) -> Vec<MenuItem> + 'static,
        cx: &mut App,
    ) {
        let (app, from) = (self.0.clone(), from.clone());
        cx.defer(move |cx| {
            let items = from.update(cx, items).unwrap_or_default();
            app.update(cx, |app, cx| app.open_menu(at, items, cx)).ok();
        });
    }
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
        let app = AppHandle::of(cx);
        let detail = cx.new(|cx| Detail::new(store.clone(), app.clone(), window, cx));
        let diff = cx.new(|cx| DiffPane::new(&detail, window, cx));
        let navigator = cx.new(|cx| Navigator::new(app, store.clone(), detail.clone(), window, cx));
        window.focus(&navigator.focus_handle(cx), cx);
        let subscriptions = vec![
            // A menu's items and the palette's commands act on the selection and the picks, so
            // when an agent's change moves them off what they opened on, they go rather than
            // act on something else. Menu items can hold the commit they opened on, so a menu
            // goes when an agent only rewrote it too, as does M's list of them; the palette's
            // commands read the selection when run.
            cx.observe_in(&store, window, |this, store, window, cx| {
                let store = store.read(cx);
                let menu_gone =
                    (this.menu.as_ref()).is_some_and(|m| m.acting_on != store.acting_on());
                let palette_gone = (this.palette.as_ref()).is_some_and(|p| match p.holds_ids {
                    true => p.acting_on != store.acting_on(),
                    false => !store.still_acting_on(&p.acting_on),
                });
                if menu_gone {
                    this.menu = None;
                }
                if palette_gone {
                    this.close_palette(window, cx);
                }
                cx.notify()
            }),
            cx.subscribe_in(&store, window, |this, _, event, window, cx| {
                if let StoreEvent::AuthorMissing(project) = event {
                    this.ask_author(project.clone(), window, cx);
                }
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                this.apply_appearance(window, cx)
            }),
            #[cfg(target_os = "macos")]
            cx.observe_global_in::<Theme>(window, |_, window, cx| place_traffic_lights(window, cx)),
            // A scripted window opens off screen, which isn't where the user left theirs.
            cx.observe_window_bounds(window, |this, window, cx| {
                if !scripted() {
                    this.store
                        .update(cx, |store, cx| store.save_window_place(window, cx));
                }
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
            rebuild: backend::rebuild::Rebuild::detect(),
            building: None,
            fps: cx.new(|_| crate::fps::Fps::default()),
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
        self.navigator.update(cx, |navigator, _| {
            navigator.list.remeasure();
            navigator.drawer_list.remeasure()
        });
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
        let store = self.store.clone();
        let folder = pick_folder("Add project", store.clone(), cx);
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
        self.new_branch_at(project, true, None, None, window, cx, |path, _| {
            backend::create_branch(path, None)
        });
    }

    /// Ask the desktop app's update feed now: show the update card for the newer version it
    /// has, or say this one is the latest.
    fn check_for_updates(&mut self, _: &CheckForUpdates, _: &mut Window, cx: &mut Context<Self>) {
        let Some(feed) = cx
            .try_global::<crate::Embedded>()
            .and_then(|e| e.update_feed.clone())
        else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let newer = backend::run(move || backend::settings::newer_version(&feed)).await;
            this.update(cx, |app, cx| match newer {
                Ok(Some(version)) => app.store.update(cx, |store, cx| {
                    store.newer_version = Some(version);
                    store.show_update(cx);
                }),
                Ok(None) => snack(&app.store, "GitButler is up to date", cx),
                Err(err) => app.store.update(cx, |store, cx| {
                    store.failed("Couldn't check for updates", &err, cx)
                }),
            })
            .ok();
        })
        .detach();
    }

    /// Install the version the desktop app's update feed offers, with a run of the desktop
    /// app's binary that shows its progress here, then start again on it. The update card
    /// that asks says it restarts, so nothing asks again.
    pub fn install_update(&mut self, cx: &mut Context<Self>) {
        use futures::StreamExt as _;

        let store = self.store.read(cx);
        if store.newer_version.is_none() || store.updating.is_some() {
            return;
        }
        if store.unfinished() > 0 {
            snack(&self.store, "Wait for the running operations to finish", cx);
            return;
        }
        let store = self.store.clone();
        let (sender, mut progress) = futures::channel::mpsc::unbounded();
        store.update(cx, |store, cx| {
            store.updating = Some(0);
            cx.notify();
        });
        cx.spawn(async move |_, cx| {
            // Off the UI thread: on Windows, starting a process can run the window's
            // message loop, which would enter the app again while this holds it.
            let installed = backend::run(move || {
                crate::install_update(|percent| _ = sender.unbounded_send(percent))
            });
            let shown = async {
                while let Some(percent) = progress.next().await {
                    store.update(cx, |store, cx| {
                        store.updating = Some(percent);
                        cx.notify();
                    });
                }
            };
            let (installed, ()) = futures::join!(installed, shown);
            if let Err(err) = installed {
                store.update(cx, |store, cx| {
                    store.updating = None;
                    store.failed("Couldn't update GitButler", &err, cx)
                });
                return;
            }
            // What was started while it downloaded finishes first.
            while store.update(cx, |store, _| store.unfinished() > 0) {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
            }
            // The Windows installer it started closes this app and starts the new one itself.
            if !cfg!(windows)
                && let Err(err) = backend::run(|| Ok(crate::relaunch(false)?)).await
            {
                store.update(cx, |store, cx| {
                    store.updating = None;
                    store.failed("Couldn't restart GitButler", &err, cx)
                });
                return;
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
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
            Some(Selection::Branch(project, Some(ref_name))) => {
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
                (detail.selected_checkout(cx)).and_then(|(project, worktree)| {
                    let files = detail.files_to_move(&project, worktree.clone(), cx);
                    (!files.is_empty()).then(|| Cut::Files {
                        project,
                        from: None,
                        worktree,
                        files: files.into(),
                    })
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
    /// Tab, or ⇧Tab going back: the next of the navigator, the file list and the diff that show,
    /// round again. Without `wrap`, as → and ← go, only the panes on that side.
    fn focus_pane(
        &mut self,
        forward: bool,
        wrap: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.covered() {
            return;
        }
        let detail = self.detail.read(cx);
        let at = if self.navigator.focus_handle(cx).contains_focused(window, cx) {
            0
        } else if detail.files_focused(window) || detail.side_list_focused(window) {
            1
        } else {
            2
        };
        // A page that's an overview alone has no pane past the navigator to take the keys.
        let shown = [
            self.navigator_open,
            detail.files_shown(window, cx),
            !detail.overview_only(cx),
        ];
        let next = |n: usize| (at + if forward { n } else { 3 - n }) % 3;
        match (1..3)
            .map(next)
            .filter(|&pane| wrap || (pane > at) == forward)
            .find(|&pane| shown.get(pane) == Some(&true))
        {
            Some(0) => window.focus(&self.navigator.focus_handle(cx), cx),
            Some(1) => (self.detail).update(cx, |detail, cx| detail.focus_files(window, cx)),
            Some(_) => (self.detail).update(cx, |detail, cx| detail.focus(window, cx)),
            None => {}
        }
    }

    fn operations_history(
        &mut self,
        _: &OperationsHistory,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(selection) = self.selection(cx) {
            self.select(Selection::History(selection.project().clone()), window, cx);
        }
    }

    /// As the window's close button does: closing it quits, asking first while operations run.
    fn close_window(&mut self, _: &CloseWindow, window: &mut Window, cx: &mut Context<Self>) {
        if self.can_quit(window, cx) {
            cx.quit();
        }
    }

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
        let checkout = match (&selection, self.store.read(cx).worktree_for(&selection)) {
            (Selection::Uncommitted(_) | Selection::Worktree(..), _) => selection.clone(),
            (_, Some(worktree)) => Selection::Worktree(project, worktree.name.clone()),
            (_, None) => Selection::Uncommitted(project),
        };
        if selection == checkout {
            return self
                .detail
                .update(cx, |detail, cx| detail.commit_or_write(window, cx));
        }
        // From a branch, or one of its commits, the commit goes on that branch.
        let branch = (self.store.read(cx).branch_for(&selection))
            .and_then(|branch| branch.ref_name.clone())
            .filter(|_| matches!(checkout, Selection::Uncommitted(_)));
        self.store
            .update(cx, |store, cx| store.select(Some(checkout.clone()), cx));
        // Once the detail has taken the new selection's checks, which it does as this update
        // ends: opened before, the composer would be of the checkout left, and close.
        let detail = self.detail.clone();
        window.defer(cx, move |window, cx| {
            detail.update(cx, |detail, cx| {
                if let Some(branch) = branch {
                    detail.branch_created(checkout.project(), branch, cx);
                }
                detail.focus_message(window, cx)
            })
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
            .then(|| {
                detail
                    .selected_text()
                    .or_else(|| detail.shown_cursor_path(cx))
            })
            .flatten();
        let text = file.or_else(|| match self.selection(cx)? {
            Selection::Commit(project, id) => {
                let (_, commit) = self.store.read(cx).find_commit(&project, id)?;
                Some(if commit.has_change_id() {
                    commit.key.clone()
                } else {
                    id.0.to_string()
                })
            }
            Selection::Upstream(_, _, id) => Some(id.0.to_string()),
            Selection::Branch(_, ref_name) => {
                ref_name.map(|ref_name| short_ref(&ref_name).into_owned())
            }
            Selection::Project(project) => {
                Some(self.store.read(cx).path(&project)?.display().to_string())
            }
            Selection::Worktree(project, name) => {
                let workspace = self.store.read(cx).workspace(&project)?;
                let worktree = workspace.find_worktree(name.as_ref())?;
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
            | Selection::Stack(..)
            | Selection::Section(..) => None,
        });
        if let Some(text) = text {
            self.copy_text(text, cx);
        }
    }

    /// Do `act` in the composer, if it's open.
    fn in_composer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        act: impl FnOnce(&mut crate::detail::Detail, &mut Window, &mut Context<crate::detail::Detail>),
    ) {
        // Not from what's open over it.
        if self.covered() {
            return;
        }
        self.detail.update(cx, |detail, cx| {
            if detail.composing() {
                act(detail, window, cx)
            }
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
    pub(crate) fn set_navigator(
        &mut self,
        navigator_open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        let show_fps = self.store.read(cx).prefs.show_fps;
        let has_update_feed = cx
            .try_global::<crate::Embedded>()
            .is_some_and(|e| e.update_feed.is_some());
        let root = div()
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
            // Only in a build `cargo run` started, likewise.
            .when(self.rebuild.is_some(), |el| {
                el.on_action(cx.listener(Self::build_and_restart))
            })
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::open_keyboard_shortcuts))
            .on_action(cx.listener(Self::open_whats_new))
            .on_action(cx.listener(Self::clone_repository))
            // Here rather than around the navigator, so the details pane's buttons reach them.
            .map(|el| self.pr_actions(el, cx))
            .map(|el| self.resolve_actions(el, cx))
            .on_action(cx.listener(Self::go_to))
            .on_action(cx.listener(Self::show_actions))
            .on_action(cx.listener(Self::copy_more))
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
            .on_action(cx.listener(|this, _: &ToggleRead, _, cx| {
                if let Some(selection) = this.selection(cx) {
                    (this.store).update(cx, |store, cx| store.toggle_read(&selection, cx))
                }
            }))
            // The selected project's, from wherever in it the selection is.
            .on_action(cx.listener(|this, _: &MarkAllRead, _, cx| {
                if let Some(selection) = this.selection(cx) {
                    let project = Selection::Project(selection.project().clone());
                    (this.store).update(cx, |store, cx| store.toggle_read(&project, cx))
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleFileTree, _, cx| {
                (this.store).update(cx, |store, cx| {
                    store.set_prefs(|prefs| prefs.file_tree = !prefs.file_tree, cx)
                })
            }))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::focus_navigator))
            .on_action(cx.listener(Self::focus_changes))
            .on_action(cx.listener(Self::focus_diff))
            .on_action(cx.listener(|this, _: &FocusNextPane, window, cx| {
                this.focus_pane(true, true, window, cx)
            }))
            .on_action(cx.listener(|this, _: &FocusPreviousPane, window, cx| {
                this.focus_pane(false, true, window, cx)
            }))
            .on_action(cx.listener(|this, _: &FocusPaneLeft, window, cx| {
                this.focus_pane(false, false, window, cx)
            }))
            .on_action(cx.listener(|this, _: &FocusPaneRight, window, cx| {
                this.focus_pane(true, false, window, cx)
            }))
            .on_action(cx.listener(Self::operations_history))
            .on_action(cx.listener(Self::close_window))
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
            // The diff's own keys, pressed with the focus elsewhere, like the navigator: Space
            // there reads the diff beside it.
            .on_action(
                cx.listener(|this, _: &crate::detail::PageDown, window, cx| {
                    this.detail.update(cx, |detail, cx| {
                        detail.page(crate::detail::PAGE, window, cx)
                    })
                }),
            )
            .on_action(cx.listener(|this, _: &crate::detail::PageUp, window, cx| {
                this.detail.update(cx, |detail, cx| {
                    detail.page(-crate::detail::PAGE, window, cx)
                })
            }))
            .on_action(
                cx.listener(|this, _: &crate::detail::HalfPageDown, window, cx| {
                    this.detail
                        .update(cx, |detail, cx| detail.page(0.5, window, cx))
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::detail::HalfPageUp, window, cx| {
                    this.detail
                        .update(cx, |detail, cx| detail.page(-0.5, window, cx))
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::detail::SwitchTab, window, cx| {
                    this.detail
                        .update(cx, |detail, cx| detail.switch_tab(window, cx))
                }),
            )
            .on_action(cx.listener(|this, _: &crate::detail::ToggleSplit, _, cx| {
                this.detail.update(cx, |detail, cx| detail.toggle_split(cx))
            }))
            .on_action(cx.listener(|this, _: &crate::detail::ToggleWrap, _, cx| {
                this.detail.update(cx, |detail, cx| detail.toggle_wrap(cx))
            }))
            .on_action(
                cx.listener(|this, _: &crate::detail::NextHunk, window, cx| {
                    this.detail
                        .update(cx, |detail, cx| detail.move_hunk(true, window, cx))
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::detail::PreviousHunk, window, cx| {
                    this.detail
                        .update(cx, |detail, cx| detail.move_hunk(false, window, cx))
                }),
            )
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
            .children(self.render_menu(window, cx))
            .children(self.render_icon_picker(window, cx))
            // Last, so the window can be closed while a modal or menu is open.
            .when(crate::ui::draws_window_controls(window), |el| {
                el.child(crate::ui::window_controls::window_controls(window, cx))
            })
            // Over everything, and it takes no clicks, as it has no hitbox.
            .when(show_fps, |el| el.child(self.fps.clone()));
        crate::ui::window_frame::window_frame(root, window, cx)
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
pub(crate) fn scripted() -> bool {
    #[cfg(feature = "dev-script")]
    return crate::dev::scripted();
    #[cfg(not(feature = "dev-script"))]
    false
}
