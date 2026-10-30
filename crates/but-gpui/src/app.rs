//! The window's root: the project navigator on the left, details on the right, the keys,
//! and the actions that work anywhere in the window. `overlays.rs` draws what opens above.

use std::time::{Duration, Instant};

use gpui::{
    AnyView, App, ClipboardItem, Context, DispatchPhase, Div, Entity, FocusHandle, Focusable,
    FontFeatures, MouseMoveEvent, Pixels, Point, PromptLevel, Role, SharedString, StyleRefinement,
    Subscription, Task, WeakEntity, Window, actions, div, prelude::*, relative,
};

use crate::backend::upstream::UpdateScope;
use crate::backend::{self, Checkout, short_ref};
use crate::detail::{Detail, DiffPane};
use crate::navigator::Navigator;
use crate::overlays::{ContextMenu, MenuItem, Prompt};
use crate::palette::{Choice, Palette};
use crate::projects::pick_folder;
use crate::state::{
    Cut, MeshPick, Pane, Prefs, ProjectId, Selection, Store, StoreEvent, snack, up_to_date,
};
use crate::terminal::NewTerminal;
use crate::theme::{ColorScheme, Theme, UiScale};
use crate::transfer::{Place, Transfer};
use crate::ui::fold::{Slide, edge, inert};
use crate::ui::{HEADER_HEIGHT, POPOVER_RADIUS, dp, fixed, resize_pane};

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
        ToggleDetail,
        CommandPalette,
        CheckForUpdates,
        BuildAndRestart,
        BuildAndRestartFromHere,
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
        NewWorktree,
        NewWorktreeWithName,
        SelectPrevious,
        SelectNext,
        Expand,
        Collapse,
        ToggleFold,
        Confirm,
        CutSelection,
        PasteHere,
        MoveSelection,
        MoveHere,
        DeleteSelection,
        UncommitSelection,
        RenameSelection,
    ]
);
actions!(
    remote,
    [
        Fetch,
        Pull,
        UpdateFromTarget,
        UpdateWorkspace,
        UpdateWorktrees,
        Push
    ]
);
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
    /// The navigator sliding open or shut.
    navigator_slide: Slide,
    /// Whether the detail shows beside the navigator. Hidden, the navigator takes the window;
    /// the two are never both hidden.
    detail_open: bool,
    /// The detail sliding in from the window's right edge or back off it.
    detail_slide: Slide,
    /// The window narrowed to the navigator while the detail is hidden, or narrowing to it or
    /// widening back as the detail slides, where the window can be resized for it.
    narrowing: Option<Narrowing>,
    /// Resizes the window a step at a time as it narrows or widens, by
    /// [`AppView::step_narrowing`].
    resizing: Option<Task<()>>,
    /// The navigator out over the detail while it's closed.
    peek: Peek,
    /// macOS's traffic lights are out of the way of the navigator out over the detail.
    #[cfg(target_os = "macos")]
    lights_away: bool,
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
    /// The build under way or last ended, and what shows it when it fails out of sight.
    pub(crate) building: Option<(Entity<crate::rebuild::BuildOutput>, Subscription)>,
    /// The frames drawn each second, drawn over everything while `Prefs::show_fps` is on.
    fps: Entity<crate::fps::Fps>,
    _subscriptions: Vec<Subscription>,
}

/// How near the window's left edge the pointer brings the closed navigator out over the
/// detail, as Arc's sidebar comes out.
const PEEK_EDGE: f32 = 6.;
/// How far the navigator out over the detail floats from the window's edges.
const PEEK_INSET: f32 = 6.;
/// How far past its card the shadow of the navigator out over the detail reaches, which goes
/// off the window with it as it slides away.
const PEEK_SHADOW: f32 = 16.;
/// How long the pointer can stray past the navigator out over the detail before it goes back,
/// so overshooting its edge a little leaves it out.
const PEEK_GRACE: Duration = Duration::from_millis(300);

/// The closed navigator out over the detail, as the pointer at the window's left edge brings
/// it, until the pointer strays from it.
#[derive(Default)]
struct Peek {
    open: bool,
    /// It sliding in from the window's edge or back off it.
    slide: Slide,
    /// Sends it back once the pointer has strayed from it for [`PEEK_GRACE`].
    leaving: Option<Task<()>>,
}

/// The window narrowing to the navigator as the detail slides in under it, or widening back as
/// it slides out, in unscaled pixels.
#[derive(Clone, Copy)]
struct Narrowing {
    /// The window's width with the detail, which the panes are laid out for meanwhile, so
    /// they aren't laid out again at every width.
    wide: f32,
    /// The navigator's, which it keeps meanwhile, and the window's once narrowed.
    navigator: f32,
    /// Resized to where the slide ended, after which its width is the user's.
    settled: bool,
    /// How many steps the window has been asked for where the slide ended.
    asked: u8,
}

/// How often the window is resized as it narrows or widens: about a frame at 120Hz.
const RESIZE_STEP: Duration = Duration::from_millis(8);
/// How many steps the window is asked for its width where the detail's slide ended, until it
/// has it: a resize that never lands, as one the screen holds back, leaves it as it is.
const LAST_RESIZE_TRIES: u8 = 30;

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
    /// Where the diff starts beside the detail's file list at `now`, when the selection has a
    /// diff.
    fn diff_left(&self, now: Instant, window: &Window, cx: &App) -> Option<f32> {
        let store = self.store.read(cx);
        let detail = self.detail.read(cx);
        let has_diff = store.selection.is_some() && !detail.diff_hidden(cx);
        has_diff.then(|| detail.diff_left(now, window, cx))
    }

    pub fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.set_global(theme_for(store.read(cx), window));
        #[cfg(target_os = "macos")]
        place_traffic_lights(false, window, cx);
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
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::AuthorMissing(project) => this.ask_author(project.clone(), window, cx),
                StoreEvent::TerminalOpened => this.enter_terminal(window, cx),
                _ => {}
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                this.apply_appearance(window, cx)
            }),
            #[cfg(target_os = "macos")]
            cx.observe_global_in::<Theme>(window, |this, window, cx| {
                place_traffic_lights(this.lights_away, window, cx)
            }),
            // A scripted window opens off screen, which isn't where the user left theirs, and
            // one narrowed to the navigator opens again with the detail.
            cx.observe_window_bounds(window, |this, window, cx| {
                if !scripted() && this.narrowing.is_none() {
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
            navigator_slide: Slide::default(),
            detail_open: true,
            detail_slide: Slide::default(),
            narrowing: None,
            resizing: None,
            peek: Peek::default(),
            #[cfg(target_os = "macos")]
            lights_away: false,
            palette: None,
            modal: None,
            recent_commands: Vec::new(),
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

    /// List the colour schemes, GitButler's, the Ghostty config's and every Ghostty theme, to
    /// pick one to draw the app and its terminals in.
    pub fn pick_color_scheme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.store.read(cx).prefs.color_scheme.clone();
        let named = but_tty::Ghostty::themes()
            .into_iter()
            .map(ColorScheme::Named);
        let choices = ([ColorScheme::GitButler, ColorScheme::Ghostty].into_iter())
            .chain(named)
            .map(|scheme| {
                let detail = match &scheme {
                    _ if scheme == current => Some("In use"),
                    ColorScheme::GitButler => Some("Light or dark, as Appearance says"),
                    ColorScheme::Ghostty => Some("The themes your Ghostty config sets"),
                    ColorScheme::Named(_) => None,
                };
                let choice = Choice::new(scheme.label().to_owned(), move |app, window, cx| {
                    (app.store).update(cx, |store, cx| {
                        store.set_prefs(|prefs| prefs.color_scheme = scheme, cx)
                    });
                    app.apply_appearance(window, cx);
                });
                match detail {
                    Some(detail) => choice.detail(detail),
                    None => choice,
                }
            })
            .collect();
        self.open_picker(None, "Search colour schemes", "Use", choices, window, cx);
    }

    /// List the fonts to pick the one code in diffs shows in.
    pub fn pick_diff_font(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.store.read(cx).prefs.diff_font.clone();
        let own = Theme::get(cx).mono_font.clone();
        let set = |prefs: &mut Prefs, font| prefs.diff_font = font;
        self.pick_font(own, "GitButler's own".into(), current, set, window, cx);
    }

    /// List the fonts to pick the one terminals show in.
    pub fn pick_terminal_font(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.store.read(cx).prefs.terminal_font.clone();
        let ghostty = but_tty::Ghostty::load();
        let detail = (ghostty.font_families.into_iter().next())
            .map_or("Your Ghostty config sets none".into(), SharedString::from);
        let set = |prefs: &mut Prefs, font| prefs.terminal_font = font;
        self.pick_font("As Ghostty's".into(), detail, current, set, window, cx);
    }

    /// List `unpicked`, which `detail` explains and leaves no font picked, then the installed
    /// fonts, for `set` to keep the one chosen. `current` is the one in use.
    fn pick_font(
        &mut self,
        unpicked: SharedString,
        detail: SharedString,
        current: Option<String>,
        set: fn(&mut Prefs, Option<String>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text_system = cx.text_system().clone();
        cx.spawn_in(window, async move |this, cx| {
            let families = (cx.background_spawn(async move { text_system.all_font_names() })).await;
            let families = families
                .into_iter()
                .filter(|family| !family.starts_with('.'));
            let fonts = families.map(|family| (family.clone().into(), Some(family), None));
            let choices: Vec<Choice> = std::iter::once((unpicked, None, Some(detail)))
                .chain(fonts)
                .map(|(label, font, detail)| {
                    let in_use = font == current;
                    let choice = Choice::new(label, move |app, window, cx| {
                        (app.store).update(cx, |store, cx| {
                            store.set_prefs(|prefs| set(prefs, font), cx)
                        });
                        app.apply_appearance(window, cx);
                        (app.detail).update(cx, |detail, cx| detail.remeasure(cx));
                    });
                    match detail {
                        _ if in_use => choice.detail("In use"),
                        Some(detail) => choice.detail(detail),
                        None => choice,
                    }
                })
                .collect();
            this.update_in(cx, |this, window, cx| {
                this.open_picker(None, "Search fonts", "Use", choices, window, cx)
            })
            .ok();
        })
        .detach();
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
    /// on a yes: quitting would stop it partway.
    fn can_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let (message, detail) = match self.store.read(cx).unfinished() {
            0 => return true,
            1 => (
                "Quit before the operation finishes?".to_owned(),
                "It may stop partway.",
            ),
            n => (
                format!("Quit before {n} operations finish?"),
                "They may stop partway.",
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
                store.fetch(&project, None, cx);
                store.refresh_forge(&project);
            });
        }
    }

    /// Fetch the selected project, then update its workspace and worktrees, as `but pull` does.
    fn pull(&mut self, _: &Pull, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.selected_project(cx) {
            self.store.update(cx, |store, cx| {
                store.fetch(&project, Some(UpdateScope::All), cx);
                store.refresh_forge(&project);
            });
        }
    }

    /// The selected project, else the first watched one, to start a branch or worktree in. None
    /// while a prompt or picker is open: it would be made out of sight behind it.
    fn project_to_start_in(&self, cx: &App) -> Option<ProjectId> {
        if self.covered() {
            return None;
        }
        self.selected_project(cx).or_else(|| {
            let store = self.store.read(cx);
            (store.projects.iter())
                .find(|p| p.expanded)
                .map(|p| p.id.clone())
        })
    }

    fn new_branch(&mut self, _: &NewBranch, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.project_to_start_in(cx) else {
            return;
        };
        let create = backend::CreateBranch { name: None };
        self.new_branch_at(project, true, None, None, window, cx, create);
    }

    fn new_worktree(&mut self, _: &NewWorktree, _: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.project_to_start_in(cx) else {
            return;
        };
        let create = backend::worktrees::CreateWorktree::New(None);
        (self.store).update(cx, |store, cx| store.create_worktree(&project, create, cx))
    }

    fn new_worktree_with_name(
        &mut self,
        _: &NewWorktreeWithName,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.project_to_start_in(cx) else {
            return;
        };
        let selected = project.clone();
        self.prompt_branch(
            Selection::Project(project),
            "New worktree",
            window,
            cx,
            |name| backend::worktrees::CreateWorktree::New(Some(name)),
            move |store, name, cx| store.select(Some(Selection::Worktree(selected, name)), cx),
        )
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
        // A move under way, before what's in the pane with the keys, as it's what they do.
        if self.store.read(cx).moving.is_some() {
            return self
                .store
                .update(cx, |store, cx| store.set_moving(None, cx));
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

    /// Open a terminal on the checkout the selection is in, the main one when it's in none.
    fn new_terminal(&mut self, _: &NewTerminal, _: &mut Window, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            let Some(selection) = store.selection.clone() else {
                return;
            };
            let checkout = store.checkout_of(&selection).unwrap_or(Checkout::Main);
            store.open_terminal(selection.project().clone(), checkout, cx);
        });
    }

    /// The terminal the detail's place shows, while one is selected.
    fn shown_terminal(&self, cx: &App) -> Option<Entity<crate::terminal::Terminal>> {
        let store = self.store.read(cx);
        match store.selection.as_ref()? {
            Selection::Terminal(_, id) => store.terminals.get(*id).cloned(),
            _ => None,
        }
    }

    /// Give the keys to the terminal shown, which covers the detail, if one is.
    fn focus_terminal(&self, window: &mut Window, cx: &mut App) -> bool {
        let terminal = self.shown_terminal(cx);
        if let Some(terminal) = &terminal {
            window.focus(&terminal.focus_handle(cx), cx);
        }
        terminal.is_some()
    }

    /// Go into the selected terminal, to type in: the detail it shows in comes back, and it
    /// takes the keys. Selecting it alone leaves them where they were, so ↑ and ↓ go past it.
    pub(crate) fn enter_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Not from under what's open over it, as when a prompt opened while its shell started.
        if self.covered() || self.shown_terminal(cx).is_none() {
            return;
        }
        self.set_detail(true, window, cx);
        self.focus_terminal(window, cx);
    }

    /// Focus the navigator, or the changes while the navigator isn't on screen, so keys
    /// always reach something visible. Out over the detail, it's on screen.
    pub(crate) fn focus_main(&self, window: &mut Window, cx: &mut App) {
        if self.navigator_open || self.peek.open {
            window.focus(&self.navigator.focus_handle(cx), cx);
        } else if !self.focus_terminal(window, cx) {
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
            None if self.detail_open && self.detail.read(cx).composing() => self
                .detail
                .update(cx, |detail, cx| detail.focus(window, cx)),
            None => self.focus_main(window, cx),
        }
    }

    /// Tab, or ⇧Tab going back: the next of the navigator, a Branches, Operations history or
    /// project page's list, the files beside that list, a page's Changes and pull request
    /// tabs, the file list and the diff that show, round again. Without `wrap`, as → and ←
    /// go, only the panes on that side, and on the tabs, the tab on that side first.
    fn focus_pane(
        &mut self,
        forward: bool,
        wrap: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // With the detail hidden, the navigator is the only pane.
        if self.covered() || !self.detail_open {
            return;
        }
        // A terminal takes every key it's given, so only the navigator moves to it.
        if let Some(terminal) = self.shown_terminal(cx) {
            if self.navigator.focus_handle(cx).contains_focused(window, cx) {
                window.focus(&terminal.focus_handle(cx), cx);
            }
            return;
        }
        let detail = self.detail.read(cx);
        let at = if self.navigator.focus_handle(cx).contains_focused(window, cx) {
            0
        } else if detail.side_list_focused(window) {
            1
        } else if detail.files_focused(window) && detail.list_page(cx) {
            2
        } else if detail.tabs_focused(window) {
            3
        } else if detail.files_focused(window) {
            4
        } else {
            5
        };
        if at == 3 && !wrap {
            let stepped =
                (self.detail).update(cx, |detail, cx| detail.step_tab(forward, window, cx));
            // Right of the last tab is nothing: what the tabs show is under them.
            if stepped || forward {
                return;
            }
        }
        let moved = self.focus_pane_from(at, forward, wrap, window, cx);
        // Going left out of the tabs, the page keeps them for coming back from the left, and
        // going out of what's under them, it forgets them.
        if moved && at >= 3 {
            let kept = at == 3 && !forward;
            (self.detail).update(cx, |detail, _| detail.keep_tabs(kept));
        }
    }

    /// Focus the pane after `at`, or before it going back, of those [`AppView::focus_pane`]
    /// steps through, numbered from the navigator's 0. The tabs count while they show the
    /// pull request, which has nothing under it to take the keys, and otherwise only while
    /// the page keeps them, as ↑ from the files under them reaches them. Says whether one
    /// took the keys.
    fn focus_pane_from(
        &mut self,
        at: usize,
        forward: bool,
        wrap: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let detail = self.detail.read(cx);
        // A page that's an overview alone has no pane past the navigator to take the keys, nor
        // has the file list or the diff without changes to show.
        let list_page = detail.list_page(cx);
        let changes = !detail.without_changes(cx);
        let review = detail.showing_review(cx);
        let shown = [
            self.navigator_open,
            list_page,
            changes && list_page && detail.beside_files_shown(cx),
            review || detail.tabs_kept(cx),
            changes && !list_page && !review && detail.files_shown(cx),
            changes && !review && !detail.overview_only(cx),
        ];
        let next = |n: usize| (at + if forward { n } else { shown.len() - n }) % shown.len();
        match (1..shown.len())
            .map(next)
            .filter(|&pane| wrap || (pane > at) == forward)
            .find(|&pane| shown.get(pane) == Some(&true))
        {
            Some(0) => window.focus(&self.navigator.focus_handle(cx), cx),
            Some(2) => (self.detail).update(cx, |detail, cx| detail.focus_beside_files(window, cx)),
            Some(3) => (self.detail).update(cx, |detail, cx| detail.focus_tabs(window, cx)),
            // A list page's list, or the file list.
            Some(1 | 4) => (self.detail).update(cx, |detail, cx| detail.focus_files(window, cx)),
            Some(_) => (self.detail).update(cx, |detail, cx| detail.focus(window, cx)),
            None => return false,
        }
        true
    }

    /// Move the keys off the navigator as it goes, to the leftmost pane beside it, as → from
    /// it does: a list page's list, else the tabs while they show the pull request or the page
    /// kept them, else the file list, else the diff. A page with none, as an overview alone or
    /// a branch with no commits, gives them to the detail.
    fn focus_past_navigator(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_terminal(window, cx) && !self.focus_pane_from(0, true, false, window, cx) {
            (self.detail).update(cx, |detail, cx| detail.focus(window, cx));
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
        if self.covered() || self.focus_terminal(window, cx) {
            return;
        }
        // Asking for what's in the detail brings it back.
        self.set_detail(true, window, cx);
        self.detail
            .update(cx, |detail, cx| detail.focus_files(window, cx));
    }

    fn focus_diff(&mut self, _: &FocusDiff, window: &mut Window, cx: &mut Context<Self>) {
        if self.covered() || self.focus_terminal(window, cx) {
            return;
        }
        self.set_detail(true, window, cx);
        self.detail
            .update(cx, |detail, cx| detail.focus(window, cx));
    }

    fn start_commit(&mut self, _: &StartCommit, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.selection(cx) else {
            return;
        };
        // The message is written in the detail.
        self.set_detail(true, window, cx);
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
    /// a branch's name or a project's directory. What it copied blinks, as the TUI highlights
    /// it.
    fn copy_selection(&mut self, _: &CopySelection, window: &mut Window, cx: &mut Context<Self>) {
        let file = (self.detail.read(cx).is_focused(window, cx))
            .then(|| self.detail.update(cx, |detail, cx| detail.copy(cx)))
            .flatten();
        let selected = file.is_none();
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
            Selection::Terminal(..) => None,
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
            | Selection::Graph(_)
            | Selection::Stack(..)
            | Selection::Section(..) => None,
        });
        if let Some(text) = text {
            if selected {
                (self.navigator).update(cx, |navigator, cx| navigator.blink_selection(cx));
            }
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

    fn update_selected_project(&mut self, scope: UpdateScope, cx: &mut Context<Self>) {
        if let Some(project) = self.selected_project(cx) {
            self.update_from_target(project, scope, cx);
        }
    }

    /// Rebase what `scope` picks onto the target. When that would leave commits conflicted
    /// that aren't now, the card saying so offers to go ahead.
    pub fn update_from_target(
        &mut self,
        project: ProjectId,
        scope: UpdateScope,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let Some(target) = store.workspace(&project).and_then(|ws| ws.target.as_ref()) else {
            return;
        };
        let behind = match scope {
            UpdateScope::All => target.needs_update(),
            UpdateScope::Workspace => target.workspace_behind,
            UpdateScope::Worktrees => target.worktrees_behind,
        };
        if !behind {
            snack(&self.store, up_to_date(scope, &target.name), cx);
            return;
        }
        self.store.update(cx, |store, cx| {
            store.update_from_target(&project, scope, None, cx)
        });
    }

    fn toggle_navigator(
        &mut self,
        _: &ToggleNavigator,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_navigator(!self.navigator_open, window, cx);
    }

    fn toggle_detail(&mut self, _: &ToggleDetail, window: &mut Window, cx: &mut Context<Self>) {
        self.set_detail(!self.detail_open, window, cx);
    }

    fn command_palette(&mut self, _: &CommandPalette, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.is_some() {
            self.close_palette(window, cx);
        } else {
            self.open_palette(window, cx);
        }
    }

    /// Show or hide the navigator, sliding it there. The keys stay where they are, but for
    /// moving off it when it goes, and onto it when it comes back while they're on a diff with
    /// nothing for them. Out over the detail already, it shows beside it at once rather than
    /// sliding in again. Hidden while the detail is, the detail comes back in its place.
    pub(crate) fn set_navigator(
        &mut self,
        navigator_open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !navigator_open {
            self.set_detail(true, window, cx);
        }
        let peeking = self.peek.open || self.peek.slide.closing(Instant::now());
        if navigator_open {
            self.peek = Peek::default();
            (self.navigator).update(cx, |navigator, cx| navigator.set_floating(false, cx));
        }
        if navigator_open && peeking {
            self.navigator_slide = Slide::default();
        } else if navigator_open != self.navigator_open {
            self.navigator_slide.start(navigator_open, cx);
        }
        self.navigator_open = navigator_open;
        if !navigator_open && self.navigator.focus_handle(cx).contains_focused(window, cx) {
            self.focus_past_navigator(window, cx);
        } else if navigator_open && self.detail.read(cx).keys_stranded(window, cx) {
            window.focus(&self.navigator.focus_handle(cx), cx);
        }
        cx.notify();
    }

    /// Show or hide the detail. Hiding it narrows the window to the navigator as the detail
    /// slides in under it, and showing it widens the window back, as [`AppView::narrow_window`]
    /// can; otherwise the detail slides off the window's right edge, the navigator's edge
    /// following it to take the window. Hidden while the navigator is, the navigator comes back
    /// in its place. The keys it takes with it go to the navigator.
    pub(crate) fn set_detail(
        &mut self,
        detail_open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if detail_open == self.detail_open {
            return;
        }
        if !detail_open {
            self.set_navigator(true, window, cx);
        }
        self.narrow_window(detail_open, window, cx);
        self.detail_slide.start(detail_open, cx);
        self.detail_open = detail_open;
        let navigator = self.navigator.focus_handle(cx);
        if !detail_open && !self.covered() && !navigator.contains_focused(window, cx) {
            window.focus(&navigator, cx);
        }
        cx.notify();
    }

    /// Have the window narrow to the navigator as the detail goes, or widen back by as much as
    /// that took as it comes back, the navigator keeping the width it has. Only on macOS, where
    /// the window's least width can be lowered for it, and out of full screen.
    fn narrow_window(&mut self, detail_open: bool, window: &mut Window, cx: &mut Context<Self>) {
        let scale = Theme::get(cx).scale;
        let width = f32::from(window.viewport_size().width) / scale;
        let resizes = cfg!(target_os = "macos") && !window.is_fullscreen();
        self.narrowing = match self.narrowing {
            _ if !resizes => None,
            // Turned back partway, it goes back from where it is.
            Some(narrowing) if !narrowing.settled => Some(Narrowing {
                asked: 0,
                ..narrowing
            }),
            Some(narrowing) => Some(Narrowing {
                wide: width + narrowing.wide - narrowing.navigator,
                navigator: width,
                settled: false,
                asked: 0,
            }),
            None if detail_open => None,
            None => Some(Narrowing {
                wide: width,
                navigator: (self.detail.read(cx)).navigator_width(Instant::now(), window, cx),
                settled: false,
                asked: 0,
            }),
        };
        let least = match self.narrowing {
            Some(_) => Pane::Navigator.limits().0 * scale,
            None => crate::LEAST_WINDOW_WIDTH,
        };
        set_least_width(least, window);
        // From a task rather than as frames are drawn: each resize has AppKit draw the window
        // at once, in step with its new width, where a frame drawn on the display's beat for the
        // width before can land after it and be stretched to the new one.
        self.resizing = self.narrowing.is_some().then(|| {
            cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor().timer(RESIZE_STEP).await;
                    let step = |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                        this.step_narrowing(window, cx)
                    };
                    if !this.update_in(cx, step).unwrap_or(false) {
                        break;
                    }
                }
            })
        });
    }

    /// Resize the window to as much of the detail as shows while it narrows to the navigator or
    /// widens back, then to where the slide ended until it's there, after which its width is
    /// the user's. Widened back, the panes lay out for the window again, and it can't be made
    /// as narrow as the navigator. Whether there's another step.
    fn step_narrowing(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(mut narrowing) = self.narrowing.filter(|narrowing| !narrowing.settled) else {
            return false;
        };
        let now = Instant::now();
        let shown = self.detail_slide.share(self.detail_open, now);
        let width = narrowing.navigator + (narrowing.wide - narrowing.navigator) * shown;
        let width = Theme::get(cx).scaled(width).round();
        let viewport = window.viewport_size();
        let sliding = self.detail_slide.openness(now).is_some();
        // A resize lands a step or more later, so the last is asked for again until the window
        // has it, for a while.
        let there = (viewport.width - width).abs() < fixed(0.5);
        if sliding || (!there && narrowing.asked < LAST_RESIZE_TRIES) {
            window.resize(gpui::size(width, viewport.height));
            if !sliding {
                narrowing.asked += 1;
            }
            self.narrowing = Some(narrowing);
            return true;
        }
        if self.detail_open {
            self.narrowing = None;
            set_least_width(crate::LEAST_WINDOW_WIDTH, window);
        } else {
            self.narrowing = Some(Narrowing {
                settled: true,
                ..narrowing
            });
        }
        cx.notify();
        false
    }

    /// Bring the closed navigator out over the detail as the pointer, `at` from the panes' top
    /// left, reaches the window's left edge below the location bar, so going for macOS's
    /// traffic lights doesn't send them out of its way, and send it back once the pointer has
    /// strayed past it for [`PEEK_GRACE`], unless [`AppView::peek_held`].
    fn follow_pointer(
        &mut self,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<AppView>,
    ) {
        let now = Instant::now();
        if self.navigator_open || self.navigator_slide.closing(now) {
            return;
        }
        let Point { x, y } = at;
        if !self.peek.open {
            let beside = y >= Theme::get(cx).scaled(HEADER_HEIGHT);
            if x <= fixed(PEEK_EDGE) && beside && !self.covered() {
                self.peek.open = true;
                self.peek.slide.start(true, cx);
                (self.navigator).update(cx, |navigator, cx| navigator.set_floating(true, cx));
                cx.notify();
            }
            return;
        }
        let width = self.detail.read(cx).navigator_width(now, window, cx);
        let strayed = x > Theme::get(cx).scaled(PEEK_INSET + width);
        if !strayed {
            self.peek.leaving = None;
        } else if self.peek.leaving.is_none() && !self.peek_held(window, cx) {
            self.peek.leaving = Some(cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(PEEK_GRACE).await;
                this.update_in(cx, |this, window, cx| this.close_peek(window, cx))
                    .ok();
            }));
        }
    }

    /// Whether the navigator out over the detail stays though the pointer strayed from it:
    /// what was opened from it is over the window, a drag may be headed for it, or its filter
    /// has the keys. The pointer's next move looks again.
    fn peek_held(&self, window: &Window, cx: &App) -> bool {
        let navigator = self.navigator.focus_handle(cx);
        let typing = navigator.contains_focused(window, cx) && !navigator.is_focused(window);
        self.covered() || cx.has_active_drag() || typing
    }

    /// Send the navigator out over the detail back, unless [`AppView::peek_held`], moving the
    /// keys off it, as it takes no more.
    fn close_peek(&mut self, window: &mut Window, cx: &mut Context<AppView>) {
        self.peek.leaving = None;
        if !self.peek.open || self.peek_held(window, cx) {
            return;
        }
        self.peek.open = false;
        self.peek.slide.start(false, cx);
        if self.navigator.focus_handle(cx).contains_focused(window, cx) {
            self.focus_past_navigator(window, cx);
        }
        cx.notify();
    }

    /// Follows the pointer anywhere over the panes, over what takes it too, for the closed
    /// navigator to come out over the detail: in from their left edge, which is the window's
    /// but for the frame a desktop that leaves it to the app has drawn around them.
    fn watch_pointer(&self, cx: &Context<AppView>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        gpui::canvas(
            |_, _, _| {},
            move |bounds, (), window, _| {
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture {
                        let at = event.position - bounds.origin;
                        this.update(cx, |this, cx| this.follow_pointer(at, window, cx))
                            .ok();
                    }
                });
            },
        )
        .absolute()
        .inset_0()
    }

    /// The detail, and the diff beside its file list, for the caller to place.
    fn render_detail(
        &self,
        now: Instant,
        navigator_sliding: bool,
        files_sliding: bool,
        window: &Window,
        cx: &App,
    ) -> Div {
        let theme = Theme::get(cx);
        div()
            .relative()
            // Over the navigator as it slides, taking the pointer from it.
            .when(navigator_sliding, |el| el.bg(theme.bg_1).occlude())
            .child(
                self.detail
                    .clone()
                    .cached(StyleRefinement::default().size_full()),
            )
            .when_some(self.diff_left(now, window, cx), |el, left| {
                el.child(
                    div()
                        .absolute()
                        .top(dp(
                            HEADER_HEIGHT + self.detail.read(cx).view_header_height(cx)
                        ))
                        .bottom_0()
                        .right_0()
                        .left(dp(left))
                        // Over the files as they slide, edged as they are.
                        .when(files_sliding, |el| {
                            el.bg(theme.bg_1)
                                .occlude()
                                .child(edge(theme.border_3).left(fixed(-1.)))
                        })
                        .child(
                            self.diff
                                .clone()
                                .cached(StyleRefinement::default().size_full()),
                        ),
                )
            })
            .children(self.shown_terminal(cx).map(|terminal| {
                div()
                    .absolute()
                    .top(dp(HEADER_HEIGHT))
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .child(terminal.cached(StyleRefinement::default().size_full()))
            }))
            .when(navigator_sliding, |el| {
                el.child(edge(theme.border_3).left(fixed(-1.)))
            })
    }

    /// The navigator and the detail while the detail slides in from the window's right edge or
    /// back off it, `shown` of it on screen, or is hidden. The detail keeps the width it has
    /// beside as much of the navigator as shows, `navigator_share` of it, and moves, so it's
    /// not laid out again as it goes, and the navigator reaches its edge, taking the window
    /// once it's gone. As the window narrows to the navigator or widens back, the detail
    /// slides under the navigator instead, along with the window's right edge.
    fn render_detail_aside(
        &self,
        now: Instant,
        shown: f32,
        navigator_share: f32,
        files_sliding: bool,
        window: &Window,
        cx: &App,
    ) -> Div {
        let navigator_sliding = self.navigator_slide.openness(now).is_some();
        let sliding = self.detail_slide.openness(now).is_some();
        if let Some(narrowing) = self.narrowing.filter(|narrowing| !narrowing.settled) {
            let theme = Theme::get(cx);
            let beside = narrowing.navigator * navigator_share;
            let navigator = div().w(dp(narrowing.navigator)).h_full().child(
                self.navigator
                    .clone()
                    .cached(StyleRefinement::default().size_full()),
            );
            return div()
                .absolute()
                .inset_0()
                .when(self.detail_open || sliding, |el| {
                    el.child(
                        self.render_detail(now, false, files_sliding, window, cx)
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .w(dp(narrowing.wide - beside)),
                    )
                })
                // Over the detail, as much of it as shows while it slides open or shut, edged
                // as it is.
                .when(beside > 0., |el| {
                    el.child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left_0()
                            .w(dp(beside))
                            .overflow_hidden()
                            .occlude()
                            .child(inert(navigator, !self.navigator_open))
                            .when(navigator_sliding, |el| {
                                el.child(edge(theme.border_3).right_0())
                            }),
                    )
                });
        }
        let beside = navigator_share * self.detail.read(cx).navigator_width(now, window, cx);
        // From where the detail's left edge is when it shows to the window's right edge, so
        // its width, and how far along it is, are shares of this one's.
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(dp(beside))
            .right_0()
            .when(
                self.navigator_open || self.navigator_slide.closing(now) || sliding,
                |el| {
                    let navigator = div().size_full().child(
                        self.navigator
                            .clone()
                            .cached(StyleRefinement::default().size_full()),
                    );
                    el.child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(dp(-beside))
                            .right(relative(shown))
                            .child(inert(navigator, !self.navigator_open)),
                    )
                },
            )
            .when(self.detail_open || sliding, |el| {
                el.child(
                    self.render_detail(now, navigator_sliding, files_sliding, window, cx)
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(1. - shown))
                        .w_full(),
                )
            })
    }

    /// The navigator out over the detail: a card sliding in from past the window's edge,
    /// shadow and all, keeping its width as it goes.
    fn render_peek(&self, now: Instant, window: &Window, cx: &App) -> impl IntoElement {
        let width = self.detail.read(cx).navigator_width(now, window, cx);
        let away = PEEK_INSET + width + PEEK_SHADOW;
        let left = PEEK_INSET - away * (1. - self.peek.slide.share(self.peek.open, now));
        let navigator = div().size_full().child(
            self.navigator
                .clone()
                .cached(StyleRefinement::default().size_full()),
        );
        div()
            .absolute()
            .top(dp(PEEK_INSET))
            .bottom(dp(PEEK_INSET))
            .left(dp(left))
            .w(dp(width))
            .rounded(dp(POPOVER_RADIUS))
            .shadow_lg()
            // Over the detail, taking the pointer from it.
            .occlude()
            .child(inert(navigator, !self.peek.open))
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
        // The detail lays out its header and its file list beside as much of the navigator as
        // shows, so it's told before it draws in this frame.
        let now = Instant::now();
        let navigator_share = self.navigator_slide.share(self.navigator_open, now);
        let navigator_sliding = self.navigator_slide.openness(now).is_some();
        let detail = self.detail.read(cx);
        detail.navigator_shown.set(navigator_share);
        detail
            .window_width
            .set(self.narrowing.map(|narrowing| narrowing.wide));
        let files_sliding = detail.sliding(now);
        let peek_sliding = self.peek.slide.openness(now).is_some();
        // Likewise the navigator, beside as much of the detail as shows.
        let detail_share = self.detail_slide.share(self.detail_open, now);
        let detail_sliding = self.detail_slide.openness(now).is_some();
        self.navigator.read(cx).detail_shown.set(detail_share);
        // As the window narrows or widens, each resize draws it, and a frame drawn on the
        // display's beat would be for the width before.
        let resizing = self.narrowing.is_some_and(|narrowing| !narrowing.settled);
        let sliding = navigator_sliding || files_sliding || peek_sliding || detail_sliding;
        if sliding && !resizing {
            window.request_animation_frame();
        }
        // The navigator out over the detail goes over where macOS's traffic lights are, and
        // AppKit draws them over everything the app does, so they make way while it's drawn.
        #[cfg(target_os = "macos")]
        {
            let away = self.peek.open || self.peek.slide.closing(now);
            if away != self.lights_away {
                self.lights_away = away;
                place_traffic_lights(away, window, cx);
            }
        }
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
            .when(cx.has_global::<backend::rebuild::Rebuild>(), |el| {
                el.on_action(cx.listener(Self::build_and_restart))
                    .on_action(cx.listener(Self::build_and_restart_from_here))
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
            .on_action(cx.listener(Self::new_terminal))
            // The selection doesn't decide: an operation that finished in another project is
            // still the last thing done.
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.store.update(cx, Store::undo_last)))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.store.update(cx, Store::redo_last)))
            .on_action(cx.listener(Self::fetch))
            .on_action(cx.listener(Self::pull))
            .on_action(cx.listener(Self::new_branch))
            .on_action(cx.listener(Self::new_worktree))
            .on_action(cx.listener(Self::new_worktree_with_name))
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
            .on_action(cx.listener(|this, _: &UpdateFromTarget, _, cx| {
                this.update_selected_project(UpdateScope::All, cx)
            }))
            .on_action(cx.listener(|this, _: &UpdateWorkspace, _, cx| {
                this.update_selected_project(UpdateScope::Workspace, cx)
            }))
            .on_action(cx.listener(|this, _: &UpdateWorktrees, _, cx| {
                this.update_selected_project(UpdateScope::Worktrees, cx)
            }))
            .on_action(cx.listener(Self::toggle_navigator))
            .on_action(cx.listener(Self::toggle_detail))
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
                cx.listener(|this, _: &crate::detail::ToggleFiles, window, cx| {
                    this.detail
                        .update(cx, |detail, cx| detail.toggle_files(window, cx))
                }),
            )
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
                    .relative()
                    // What the navigator's menus and keys, and the commit header's menu, do to
                    // the selection.
                    .map(|el| self.edit_actions(el, cx))
                    .on_drag_move(resize_pane(Pane::Navigator, self.store.clone()))
                    .map(|el| match detail_share < 1. {
                        true => el.overflow_hidden().child(self.render_detail_aside(
                            now,
                            detail_share,
                            navigator_share,
                            files_sliding,
                            window,
                            cx,
                        )),
                        false => el
                            .when(
                                self.navigator_open || self.navigator_slide.closing(now),
                                |el| {
                                    let width =
                                        self.detail.read(cx).navigator_width(now, window, cx);
                                    // Sliding, it keeps its width and its place, so it's drawn
                                    // again only when it changes, not as it slides or the diff
                                    // changes: the detail beside it moves its edge across it.
                                    let navigator = div().w(dp(width)).h_full().child(
                                        self.navigator
                                            .clone()
                                            .cached(StyleRefinement::default().size_full()),
                                    );
                                    el.child(
                                        div()
                                            .w(dp(width * navigator_share))
                                            .flex_none()
                                            .h_full()
                                            .child(inert(navigator, !self.navigator_open)),
                                    )
                                },
                            )
                            .child(
                                self.render_detail(
                                    now,
                                    navigator_sliding,
                                    files_sliding,
                                    window,
                                    cx,
                                )
                                .flex_1()
                                .min_w_0()
                                .h_full(),
                            ),
                    })
                    // Closed, the navigator comes out over the detail as the pointer reaches
                    // the window's left edge, as Arc's sidebar does.
                    .when(!self.navigator_open, |el| el.child(self.watch_pointer(cx)))
                    .when(self.peek.open || self.peek.slide.closing(now), |el| {
                        el.child(self.render_peek(now, window, cx))
                    }),
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
/// the header, however tall the scale draws it. While `away`, they're past the window's left
/// edge, where AppKit doesn't draw them.
#[cfg(target_os = "macos")]
fn place_traffic_lights(away: bool, window: &Window, cx: &App) {
    use crate::ui::fixed;
    use gpui::point;

    const LIGHTS: f32 = 14.;
    // Further than the three of them reach.
    const AWAY: f32 = -100.;
    let top = (Theme::get(cx).scaled(HEADER_HEIGHT) - fixed(LIGHTS)) / 2.;
    let left = match away {
        true => fixed(AWAY),
        false => fixed(14.),
    };
    window.set_traffic_light_position(point(left, top));
}

/// The theme the appearance setting and the system call for.
fn theme_for(store: &Store, window: &Window) -> Theme {
    let prefs = &store.prefs;
    let (scheme, system) = (&prefs.color_scheme, window.appearance());
    let ghostty = but_tty::Ghostty::load();
    let digit = |family: &str| crate::ui::digit_advance(family, window.text_system());
    let installed = |family: &str| digit(family).is_some();
    Theme::new(prefs.appearance, scheme, system, prefs.code_size, &ghostty)
        .with_scale(prefs.ui_scale)
        .with_diff_font(prefs.diff_font.as_deref(), digit)
        .with_ligatures(prefs.ligatures)
        .with_terminal_font(prefs.terminal_font.as_deref(), &ghostty, installed)
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

/// Let the window be made as narrow as `width`, in logical pixels, as GPUI only sets how small
/// it can be made as it opens it.
#[cfg(target_os = "macos")]
fn set_least_width(width: f32, window: &Window) {
    use objc2::{msg_send, runtime::AnyObject};
    use objc2_foundation::NSSize;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    let view = handle.ns_view.as_ptr().cast::<AnyObject>();
    let least = NSSize::new(width.into(), crate::LEAST_WINDOW_HEIGHT.into());
    // SAFETY: `view` is the window's live NSView, used on the main thread, and
    // `setContentMinSize:` only changes how small the window can be made.
    unsafe {
        let ns_window: *mut AnyObject = msg_send![view, window];
        if !ns_window.is_null() {
            let _: () = msg_send![ns_window, setContentMinSize: least];
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn set_least_width(_: f32, _: &Window) {}

/// Whether a dev script drives the window, which then says when it's hidden.
pub(crate) fn scripted() -> bool {
    #[cfg(feature = "dev-script")]
    return crate::dev::scripted();
    #[cfg(not(feature = "dev-script"))]
    false
}
