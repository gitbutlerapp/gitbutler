//! The window's root: the project navigator on the left, details on the right, the keys,
//! and the actions that work anywhere in the window. `overlays.rs` draws what opens above.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyView, App, Bounds, ClipboardItem, Context, DispatchPhase, Div, DragMoveEvent, Entity,
    ExternalPaths, FocusHandle, Focusable, FontFeatures, KeyDownEvent, ModifiersChangedEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput, Point,
    PromptLevel, Role, SharedString, StyleRefinement, Subscription, Task, WeakEntity, Window,
    actions, div, prelude::*, relative,
};

use crate::backend::upstream::UpdateScope;
use crate::backend::{self, Checkout, short_ref};
use crate::clock;
use crate::detail::{Detail, DiffPane};
use crate::navigator::Navigator;
use crate::overlays::{ContextMenu, MenuItem, Prompt, overlay};
use crate::palette::{Choice, Palette};
use crate::projects::pick_folder;
use crate::state::{
    Area, Axis, Cut, Divider, DividerDrag, Drawn, FileView, Junction, Landing, MeshPick, Pane,
    Prefs, ProjectId, RowKey, Selection, Side, Store, StoreEvent, Tile, Tiling, Zones, snack,
    up_to_date,
};
use crate::terminal::NewTerminal;
use crate::theme::{ColorScheme, Theme, UiScale};
use crate::transfer::{Move, Moving, Op, Place, ProjectDrag, Transfer};
use crate::ui::fold::{OnTop, Slide, edge, inert, on_top};
use crate::ui::{
    Button, HEADER_HEIGHT, Icon, POPOVER_RADIUS, TILE_BAR, TILE_MARGIN, commit_title, dp, fixed,
    gap_divider, gap_junction, h_flex, icon_button, mark_dragged, placed, resize_pane, small_icon,
    tile_card, tile_content,
};

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
        StartAmend,
        ToggleNavigator,
        ToggleDetail,
        CommandPalette,
        ShowContextMenu,
        CheckForUpdates,
        BuildAndRestart,
        BuildAndRestartFromHere,
        OperationsHistory,
        NextProjectPage,
        PreviousProjectPage,
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
        SquashSelection,
        SquashIntoSelection,
        UseTargetMessage,
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
actions!(
    tiling,
    [
        SplitRight,
        SplitBelow,
        CloseTile,
        ZoomTile,
        EqualizeTiles,
        NextTile,
        PreviousTile,
        CopyTileDiagram
    ]
);
actions!(
    copy,
    [
        CopyCommitId,
        CopyBranchName,
        CopyPullRequestLink,
        CopySnapshotId,
        CopyPath,
        CopyRelativePath,
        CopyAbsolutePath,
        CopyErrorMessage,
    ]
);

/// Register `A` on `el` as copying `text`.
fn copy_on<A: gpui::Action>(
    el: gpui::Stateful<gpui::Div>,
    text: String,
    cx: &Context<AppView>,
) -> gpui::Stateful<gpui::Div> {
    el.on_action(cx.listener(move |this, _: &A, _, cx| this.copy_text(text.clone(), cx)))
}

pub struct AppView {
    pub store: Entity<Store>,
    pub detail: Entity<Detail>,
    diff: Entity<DiffPane>,
    /// The pages of the tiles pinned to the selected row, each a detail of its own with its
    /// diff. A pinned terminal is drawn as it is, with none.
    pages: HashMap<RowKey, Page>,
    /// The tile sliding open out of its split, or shut into it, while one is.
    tile_slide: Option<TileSlide>,
    /// The lines of the selected row's tiling sliding to where they're even, while they are.
    tile_glide: Option<TileGlide>,
    tile_zoom: Option<TileZoom>,
    row_drag: Option<RowDrag>,
    row_tile: Option<RowTile>,
    /// Whether the selected row's tiles are moving, sliding or dragged: see
    /// [`Self::move_tiles`].
    tiles_moving: bool,
    /// The tile pressed by its bar, dragged once the pointer's moved far enough, until it's
    /// let go and it's settled where it lands.
    tile_drag: Option<TileDrag>,
    /// The room the tiles share, in the window, as it was last drawn.
    tile_room: Rc<Cell<Bounds<Pixels>>>,
    pub(crate) navigator: Entity<Navigator>,
    pub menu: Option<ContextMenu>,
    /// A menu with commands in it, waiting for the frame that registers the actions of the
    /// selection it opened on, to say which it offers.
    pub pending_menu: Option<ContextMenu>,
    /// The open menu's, which takes the keys from what had them.
    pub(crate) menu_focus: FocusHandle,
    pub prompt: Option<Prompt>,
    /// The emoji picker open on a project's icon.
    pub icon_picker: Option<crate::projects::IconPicker>,
    pub navigator_open: bool,
    /// The navigator sliding open or shut.
    navigator_slide: Slide,
    /// Whether the detail shows beside the navigator. Hidden, the navigator takes the window;
    /// the two are never both hidden.
    pub(crate) detail_open: bool,
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
/// it, until the pointer strays from it, or as ⌘1 brings it, until the keys leave it.
#[derive(Default)]
struct Peek {
    open: bool,
    /// It sliding in from the window's edge or back off it.
    slide: Slide,
    /// Sends it back once the pointer has strayed from it for [`PEEK_GRACE`].
    leaving: Option<Task<()>>,
    /// Brought out by ⌘1, so it stays while it has the keys, whatever the pointer does.
    keyed: bool,
    /// Where ⌘1 was pressed: the selection, and what had the keys, which Esc goes back to.
    returns_to: Option<(Selection, Option<FocusHandle>)>,
    /// Brought out to place a commit in, as ⌘↩ does while it's hidden, so it goes back once
    /// that's committed or let go, as it went back from ⌘1.
    committing: bool,
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
    /// Where the diff starts beside `detail`'s file list at `now`, when its page has a diff.
    fn diff_left(detail: &Entity<Detail>, now: Instant, window: &Window, cx: &App) -> Option<f32> {
        let detail = detail.read(cx);
        let has_diff = detail.shows_page(cx) && !detail.diff_hidden(cx);
        has_diff.then(|| detail.diff_left(now, window, cx))
    }

    /// Make the details of the pages pinned to the selected row, and drop those of pages no
    /// longer pinned there, which only the tiles on screen keep.
    fn sync_pages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pinned = self.store.read(cx).pinned();
        let pages =
            (pinned.into_iter()).filter(|row| !matches!(row, RowKey::Row(Selection::Terminal(..))));
        let mut kept = std::mem::take(&mut self.pages);
        for row in pages {
            let page = kept.remove(&row).unwrap_or_else(|| {
                let (store, app) = (self.store.clone(), AppHandle::of(cx));
                let pinned = Some(row.clone());
                let detail = cx.new(|cx| Detail::new(store, app, pinned, window, cx));
                let diff = cx.new(|cx| DiffPane::new(&detail, window, cx));
                Page { detail, diff }
            });
            self.pages.insert(row, page);
        }
    }

    pub fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.set_global(theme_for(store.read(cx), window));
        #[cfg(target_os = "macos")]
        place_traffic_lights(false, window, cx);
        #[cfg(target_os = "macos")]
        if backend::settings::dev_build() {
            keep_drags_through_other_buttons();
        }
        let app = AppHandle::of(cx);
        let detail = cx.new(|cx| Detail::new(store.clone(), app.clone(), None, window, cx));
        let diff = cx.new(|cx| DiffPane::new(&detail, window, cx));
        let navigator = cx.new(|cx| Navigator::new(app, store.clone(), detail.clone(), window, cx));
        window.focus(&navigator.focus_handle(cx), cx);
        let subscriptions = vec![
            // A menu's items and the palette's commands act on the selection and the picks, so
            // when an agent's change moves them off what they opened on, they go rather than
            // act on something else. Menu items can hold the commit they opened on, so a menu
            // goes when an agent only rewrote it too, as ⌘K does while it lists such items;
            // commands read the selection when run.
            cx.observe_in(&store, window, |this, store, window, cx| {
                let store = store.read(cx);
                let menu_gone =
                    (this.menu.as_ref()).is_some_and(|m| m.acting_on != store.acting_on());
                if (this.pending_menu.as_ref()).is_some_and(|m| m.acting_on != store.acting_on()) {
                    this.pending_menu = None;
                }
                let palette_gone = (this.palette.as_ref()).is_some_and(|p| match p.holds_ids {
                    true => p.acting_on != store.acting_on(),
                    false => !store.still_acting_on(&p.acting_on),
                });
                if menu_gone {
                    this.close_menu(window, cx);
                }
                if palette_gone {
                    this.close_palette(window, cx);
                }
                cx.notify()
            }),
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::AuthorMissing(project) => this.ask_author(project.clone(), window, cx),
                StoreEvent::TerminalOpened => this.enter_terminal(window, cx),
                StoreEvent::CommitBlocked => this.ask_where_blocked_commit_goes(window, cx),
                StoreEvent::TilingChanged => {
                    this.drop_stale_tile_drag(cx);
                    this.drop_stale_row_tile(cx);
                    this.drop_stale_zoom(cx);
                    this.move_tiles(cx);
                    this.sync_pages(window, cx)
                }
                StoreEvent::TilePinned(pinned) => {
                    // A row dragged in slides on from where it's been held.
                    if this.row_tile.is_none() {
                        this.slide_tile(pinned.clone(), None, cx);
                    }
                    if !this.covered()
                        && let Some(focus) = this.tile_focus(&Tile::Pinned(pinned.clone()), cx)
                    {
                        window.focus(&focus, cx);
                    }
                }
                StoreEvent::TileClosed {
                    before,
                    tile,
                    terminal,
                } => {
                    let going = Going {
                        before: before.clone(),
                        terminal: terminal.clone(),
                        page: this.pages.remove(tile),
                    };
                    this.slide_tile(tile.clone(), Some(going), cx);
                }
                StoreEvent::RowDragEnded => {
                    this.row_drag = None;
                    this.move_tiles(cx);
                }
                // Another row's tiling, with nothing of this one's to slide.
                StoreEvent::SelectionChanged => {
                    this.tile_slide = None;
                    this.tile_drag = None;
                    this.tile_zoom = None;
                    this.take_row_tile_back(cx);
                    this.move_tiles(cx);
                    this.sync_pages(window, cx)
                }
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
            // the diff while a message is being written. A tile closed with them, or whose shell
            // exited, gives them to the tile that had them before, as tmux's last pane does.
            cx.on_focus_lost(window, |this, window, cx| {
                if !this.tile_lost_keys(window, cx) {
                    this.restore_focus(window, cx)
                }
                // GPUI says this during a draw, where moving focus asks for no frame, so what
                // took the keys would go on being drawn without them.
                window.on_next_frame(|window, _| window.refresh());
            }),
            // The navigator ⌘1 brought out goes back once the keys leave it, as ⌘2 and ⌘3 take
            // them. GPUI blurs it as another app comes in front too, which leaves it out.
            cx.on_focus_out(
                &navigator.focus_handle(cx),
                window,
                |this, _, window, cx| {
                    // After the frame, as a notify while it's drawn draws nothing.
                    if this.peek.keyed && window.is_window_active() {
                        cx.defer_in(window, |this, window, cx| this.close_peek(window, cx));
                    }
                },
            ),
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
            pages: HashMap::new(),
            tile_slide: None,
            tile_glide: None,
            tile_zoom: None,
            row_drag: None,
            row_tile: None,
            tiles_moving: false,
            tile_drag: None,
            tile_room: Rc::default(),
            navigator,
            menu: None,
            pending_menu: None,
            menu_focus: cx.focus_handle(),
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

    /// Whether to quit right away. While an operation hasn't finished or a terminal is open,
    /// ask instead, and quit on a yes: quitting would stop the operation partway and end what
    /// runs in the terminal.
    fn can_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let store = self.store.read(cx);
        let operations = match store.unfinished() {
            0 => None,
            1 => Some((
                "Quit before the operation finishes?".to_owned(),
                "It may stop partway.".to_owned(),
            )),
            n => Some((
                format!("Quit before {n} operations finish?"),
                "They may stop partway.".to_owned(),
            )),
        };
        let terminals = match store.terminals.count() {
            0 => None,
            1 => Some((
                "Quit with a terminal open?".to_owned(),
                "The terminal closes, and what runs in it ends.".to_owned(),
            )),
            n => Some((
                format!("Quit with {n} terminals open?"),
                format!("The {n} terminals close, and what runs in them ends."),
            )),
        };
        // Operations ask first, as they're what quitting would leave half done.
        let (message, detail) = match (operations, terminals) {
            (None, None) => return true,
            (Some(asked), None) | (None, Some(asked)) => asked,
            (Some((message, operations)), Some((_, terminals))) => {
                (message, format!("{operations} {terminals}"))
            }
        };
        self.confirm_then(None, message, &detail, "Quit", window, cx, |_, cx| {
            cx.quit()
        });
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
            move |store, name, cx| {
                store.select(
                    Some(Selection::Checkout(selected, Checkout::Linked(name))),
                    cx,
                )
            },
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
        if let Some(cut) = self.selection_cut(window, cx) {
            self.cut(cut, cx);
        }
    }

    /// Squash what's picked in the diff or the file list, as ⌘X cuts it there, with the keys
    /// in the navigator, from the selection's row, which holds it. The navigator squashes
    /// what's selected there itself.
    fn squash_from_detail(
        &mut self,
        _: &SquashSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.detail.read(cx).is_focused(window, cx) {
            return;
        }
        let cut = self.selection_cut(window, cx).and_then(Cut::movable);
        let Some(Cut::Files {
            project,
            from,
            worktree,
            files,
        }) = cut
        else {
            return;
        };
        // The composer's checks say what goes in, as it amends with them.
        if from.is_none() && self.detail.read(cx).composing() {
            return window.dispatch_action(Box::new(StartAmend), cx);
        }
        let what = Moving::Files {
            from,
            worktree,
            files,
        };
        (self.navigator).update(cx, |navigator, cx| {
            navigator.squash(project, what, None, window, cx);
        });
    }

    /// What ⌘X cuts where the keys are.
    fn selection_cut(&self, window: &Window, cx: &App) -> Option<Cut> {
        let in_diff = self.detail.read(cx).is_focused(window, cx);
        match self.selection(cx) {
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
            Some(Selection::Uncommitted(..)) if in_diff && !self.detail.read(cx).composing() => {
                self.detail.read(cx).cursor_cut(cx)
            }
            Some(Selection::Uncommitted(..)) => {
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
        }
    }

    fn paste_here(&mut self, _: &PasteHere, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(cut), Some(target)) = (self.store.read(cx).cut.clone(), self.selection(cx))
        else {
            return;
        };
        if self.navigator.read(cx).folded_heading(&target) {
            return snack(&self.store, "Open the worktree first", cx);
        }
        let place = Place::from_keys(window.modifiers(), &cut, &target);
        // Pasting lets go of the cut, before it lands: letting go clears snackbars, and landing
        // can show one. A drop leaves the cut alone.
        if Transfer::resolve(&cut, &target, place, self.store.read(cx)).is_some() {
            self.store.update(cx, |store, cx| store.set_cut(None, cx));
        }
        self.transfer(&cut, &target, place, window, cx);
    }

    /// Start cherry-picking `what` into `project` from a page of the detail: the navigator
    /// takes the keys to place the copies, coming out over the detail while it's hidden, as ⌘1
    /// brings it.
    pub(crate) fn cherry_pick(
        &mut self,
        project: ProjectId,
        what: Moving,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_navigator(&FocusNavigator, window, cx);
        (self.navigator).update(cx, |navigator, cx| {
            navigator.cherry_pick(project, what, window, cx)
        });
    }

    pub fn cut(&mut self, cut: Cut, cx: &mut Context<Self>) {
        let message = format!(
            "Cut {}, ready to paste with {}",
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
        self.land(payload, target, place, None, window, cx)
    }

    /// Do what `payload` landing on `target` at `place` does, as a drop does, and land the move
    /// with the keys `lands` if that's what it is.
    pub fn land(
        &mut self,
        payload: &Cut,
        target: &Selection,
        place: Place,
        lands: Option<crate::transfer::Move>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(transfer) = Transfer::resolve(payload, target, place, self.store.read(cx)) else {
            snack(&self.store, "That can't go there", cx);
            return;
        };
        let project = target.project().clone();
        let staged =
            (self.store).update(cx, |store, cx| transfer.perform(&project, lands, store, cx));
        if let Some((branch, files)) = staged {
            self.store.update(cx, |store, cx| {
                store.select(
                    Some(Selection::Uncommitted(project.clone(), Checkout::Main)),
                    cx,
                )
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
        if self.tile_dragged() {
            return self.let_go_tile(true, cx);
        }
        // The menu first, as it can be open over a modal.
        if self.menu.is_some() {
            return self.close_menu(window, cx);
        }
        if self.modal.is_some() {
            return self.close_modal(window, cx);
        }
        // A move under way, before what's in the pane with the keys, as it's what they do. A
        // commit being placed goes with the composer it's made from.
        let moving = self.store.read(cx).moving.as_ref();
        match moving.map(|moving| (moving.op, moving.amending())) {
            Some((Op::Commit, _) | (Op::Squash(_), true)) => {
                (self.detail).update(cx, |detail, cx| detail.cancel_commit(window, cx));
                if self.peek.committing {
                    self.send_peek_back(window, cx);
                }
                return;
            }
            Some((Op::Move | Op::Squash(_) | Op::CherryPick, _)) => {
                return self
                    .store
                    .update(cx, |store, cx| store.set_moving(None, cx));
            }
            None => {}
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
        // With nothing cut or picked, the navigator ⌘1 brought out goes back, and with it the
        // selection and the keys to where ⌘1 was pressed, while that's still there.
        let store = self.store.read(cx);
        if self.peek.keyed
            && store.cut.is_none()
            && store.picked.is_empty()
            && self.navigator.focus_handle(cx).contains_focused(window, cx)
        {
            return self.send_peek_back(window, cx);
        }
        // With nothing cut or picked, the branch opened on the Branches page goes back to its
        // table.
        if store.cut.is_none()
            && store.picked.is_empty()
            && self.detail.update(cx, |detail, cx| {
                detail.close_opened_branch(cx) || detail.close_opened_target(cx)
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
        let navigator = self.navigator.focus_handle(cx);
        // Asking for the hidden navigator brings it out over the detail, as the pointer does,
        // and back beside it while it's still sliding away.
        if !self.navigator_open && !self.navigator_slide.closing(clock::now()) {
            if !navigator.contains_focused(window, cx) {
                self.peek.returns_to = self.selection(cx).map(|s| (s, window.focused(cx)));
            }
            self.peek.keyed = true;
            if !self.peek.open {
                self.open_peek(cx);
            }
        } else if !self.navigator_open {
            self.set_navigator(true, window, cx);
        }
        window.focus(&navigator, cx);
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
        // The navigator ⌘1 brought out goes back now the keys left it, which no frame shows
        // when a picker's choice runs at once, so GPUI never says it lost them.
        if self.peek.keyed {
            self.close_peek(window, cx);
        }
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
    /// steps through, numbered from the navigator's 0. The tabs count while nothing under them
    /// takes the keys, as under the pull request or on a page without changes, and otherwise
    /// only while the page keeps them, as ↑ from the files under them reaches them. Says
    /// whether one took the keys.
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
            detail.tabs_reachable(cx),
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
    /// it does: a list page's list, else the tabs while nothing under them takes the keys or
    /// the page kept them, else the file list, else the diff. A page with none, as an overview
    /// alone or a branch with no commits, gives them to the detail.
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

    /// The selected project's next page, or going back the one before, as its page's tabs list
    /// them, round again past either end. From off its pages, forward goes to the first and
    /// back to the last, as a step onto the navigator's rows does.
    fn step_project_page(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.selection(cx) else {
            return;
        };
        let pages = Selection::project_pages(selection.project());
        let n = pages.len();
        let next = match (pages.iter().position(|page| *page == selection), forward) {
            (Some(at), true) => (at + 1) % n,
            (Some(at), false) => (at + n - 1) % n,
            (None, true) => 0,
            (None, false) => n - 1,
        };
        if let Some(page) = pages.into_iter().nth(next) {
            self.select(page, window, cx);
        }
    }

    /// As the window's close button does: closing it quits, asking first while operations run
    /// or terminals are open.
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

    /// Whether the navigator shows, beside the detail or out over it.
    pub(crate) fn navigator_shown(&self) -> bool {
        self.navigator_open || self.peek.open
    }

    /// Place a commit of the uncommitted changes of the selection's checkout with the keys,
    /// from the selection: the navigator steps through where it can go, and the composer shows
    /// what goes in and the message. While one's being placed, as Return does there. With the
    /// navigator out of sight, only the status bar says where it goes, the composer stays
    /// closed and what's shown stays as it is: Return, or this again, commits it all there
    /// without a message.
    fn start_commit(&mut self, _: &StartCommit, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(moving) = (self.store.read(cx).moving.clone()).filter(|m| m.op == Op::Commit) {
            // From the composer, with its message, written first; without it, as where the
            // navigator came out for the commit, as it is, the navigator going back once it's
            // made.
            if self.detail.read(cx).composing() {
                return (self.detail).update(cx, |detail, cx| detail.commit_here(window, cx));
            }
            (self.detail).update(cx, |detail, cx| detail.commit_now(moving, cx));
            if self.peek.committing && self.store.read(cx).moving.is_none() {
                self.send_peek_back(window, cx);
            }
            return;
        }
        self.place_changes(false, window, cx)
    }

    /// Amend a commit with the uncommitted changes of the selection's checkout, from the
    /// selection: the navigator steps through what they can go into, and the composer shows
    /// what goes in and the commit's message. While one's being placed, as Return does there:
    /// the keys go to the message, to keep or rewrite, and from there amend. With the navigator
    /// out of sight, the composer stays closed: Return, or this again, amends it with all of
    /// them, keeping its message.
    fn start_amend(&mut self, _: &StartAmend, window: &mut Window, cx: &mut Context<Self>) {
        // Not from what's open over it.
        if self.covered() {
            return;
        }
        if !(self.store.read(cx).moving.as_ref()).is_some_and(Move::amending) {
            return self.place_changes(true, window, cx);
        }
        if self.detail.read(cx).composing() {
            return (self.detail).update(cx, |detail, cx| detail.amend_here(window, cx));
        }
        (self.navigator).update(cx, |navigator, cx| navigator.land_move(window, cx));
        if self.peek.committing && self.store.read(cx).moving.is_none() {
            self.send_peek_back(window, cx);
        }
    }

    /// Start placing the uncommitted changes of the selection's checkout, as a new commit or
    /// with `amend`, into one.
    fn place_changes(&mut self, amend: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.selection(cx) else {
            return;
        };
        // Only what's on screen gets committed: from another page, ⌘↩ shows the changes
        // first, a linked worktree's from its branches and commits.
        let project = selection.project().clone();
        let checkout = match &selection {
            Selection::Uncommitted(..) => selection.clone(),
            _ => {
                let checkout = self.store.read(cx).checkout_of(&selection);
                Selection::Uncommitted(project.clone(), checkout.unwrap_or_default())
            }
        };
        let Some(worktree) = self.store.read(cx).checkout_of(&checkout) else {
            return;
        };
        // Refused here, what's shown stays as it is.
        let refused = (self.navigator.read(cx)).amend_refused(&project, &worktree, cx);
        if let Some(why) = refused.filter(|_| amend) {
            return snack(&self.store, why, cx);
        }
        let shown = self.navigator_shown();
        if shown {
            // The message is written in the detail.
            self.set_detail(true, window, cx);
        }
        // From where it's started, before the selection is the changes'.
        let start = (self.navigator).update(cx, |navigator, cx| match amend {
            true => navigator.amend_start(&project, &selection, cx),
            false => navigator.commit_start(&project, &selection, cx),
        });
        // The hidden navigator comes out over the detail to place it in, as ⌘1 brings it,
        // what's shown staying as it is, without the composer. An amend starts from the
        // changes' row, as what's squashed is selected.
        if !shown {
            self.focus_navigator(&FocusNavigator, window, cx);
            self.peek.committing = self.peek.keyed;
            if amend {
                self.store
                    .update(cx, |store, cx| store.select(Some(checkout), cx));
            }
            let placing = self.navigator.update(cx, |navigator, cx| {
                navigator.start_placing(amend, project, worktree, start, window, cx)
            });
            // With nowhere to place them, it has nothing to come out for.
            if !placing {
                self.send_peek_back(window, cx);
            }
            return;
        }
        if selection != checkout {
            self.store
                .update(cx, |store, cx| store.select(Some(checkout), cx));
        }
        // Once the detail has taken the new selection's checks, which it does as this update
        // ends: opened before, the composer would be of the checkout left, and close.
        let (detail, navigator) = (self.detail.clone(), self.navigator.clone());
        window.defer(cx, move |window, cx| {
            let opens = !detail.read(cx).composing();
            // Not where the composer won't open, as while a commit is being edited.
            if !detail.update(cx, |detail, cx| detail.open_composer(window, cx)) {
                return;
            }
            let placing = navigator.update(cx, |navigator, cx| {
                navigator.start_placing(amend, project, worktree, start, window, cx)
            });
            // A commit can still go on the branch the composer shows; an amend has no commit.
            if amend && !placing && opens {
                detail.update(cx, |detail, cx| detail.leave_composer(window, cx));
            }
        });
    }

    /// Open the pull request of the selected branch, or of the branch the selected commit
    /// is on, in the browser. Without one, start one where that's possible.
    /// The pull request of the selected branch, or of what a page has picked: the branch
    /// opened on the Branches page, or the target's merge.
    pub(crate) fn pull_request_url(&self, cx: &App) -> Option<String> {
        let store = self.store.read(cx);
        match store.branch_for(store.selection.as_ref()?) {
            Some(branch) => branch.review.as_ref()?.url.clone(),
            None => self.detail.read(cx).page_pull_request(cx),
        }
    }

    /// Register a copy of each thing the selection has to copy, as its menu offers them:
    /// its pull request's link, a commit's ID, a branch's name, a checkout's path, and why a
    /// project can't be read. The pages' own picks are the detail's.
    pub(crate) fn copy_actions(
        &self,
        el: gpui::Stateful<gpui::Div>,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let store = self.store.read(cx);
        let el = el.when_some(self.pull_request_url(cx), |el, url| {
            copy_on::<CopyPullRequestLink>(el, url, cx)
        });
        let path = |path: &std::path::Path| path.display().to_string();
        match store.selection.as_ref() {
            Some(Selection::Commit(_, id) | Selection::Upstream(_, _, id)) => {
                copy_on::<CopyCommitId>(el, id.0.to_string(), cx)
            }
            Some(Selection::Branch(_, Some(ref_name))) => {
                copy_on::<CopyBranchName>(el, short_ref(ref_name).into_owned(), cx)
            }
            Some(Selection::Project(project)) => {
                let error =
                    (store.project(project)).and_then(|p| Some(p.error.clone()?.to_string()));
                el.when_some(store.path(project), |el, dir| {
                    copy_on::<CopyPath>(el, path(&dir), cx)
                })
                .when_some(error, |el, error| {
                    copy_on::<CopyErrorMessage>(el, error, cx)
                })
            }
            Some(Selection::Checkout(project, Checkout::Linked(name))) => {
                let worktree =
                    (store.workspace(project)).and_then(|ws| ws.find_worktree(name.as_ref()));
                el.when_some(worktree, |el, worktree| {
                    copy_on::<CopyPath>(el, path(&worktree.path), cx)
                })
            }
            Some(Selection::Mesh(project, id, MeshPick::Checkout)) => el
                .when_some(store.mesh_checkout(project, id), |el, checkout| {
                    copy_on::<CopyPath>(el, checkout.path.clone(), cx)
                }),
            _ => el,
        }
    }

    fn open_pull_request(
        &mut self,
        _: &OpenPullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let url = self.pull_request_url(cx);
        let create = crate::pr::CreatePullRequest;
        match url {
            Some(url) => cx.open_url(&url),
            None if window.is_action_available(&create, cx) => {
                self.create_pull_request(&create, window, cx)
            }
            None => snack(&self.store, "No pull request here", cx),
        }
    }

    /// Copy what's selected in the form other tools take: a hunk or a file's diff as git prints
    /// it, a commit's id, a branch's name or a project's or worktree's directory, as dropping its
    /// row on a terminal pastes it. What it copied blinks, as the TUI highlights it.
    fn copy_selection(&mut self, _: &CopySelection, window: &mut Window, cx: &mut Context<Self>) {
        let file = (self.detail.read(cx).is_focused(window, cx))
            .then(|| self.detail.update(cx, |detail, cx| detail.copy(window, cx)))
            .flatten();
        let selected = file.is_none();
        let text = file.or_else(|| match self.selection(cx)? {
            Selection::Commit(project, id) => {
                let (_, commit) = self.store.read(cx).find_commit(&project, id)?;
                Some(commit.reference())
            }
            Selection::Upstream(_, _, id) => Some(id.0.to_string()),
            Selection::Terminal(..) => None,
            Selection::Branch(_, ref_name) => {
                ref_name.map(|ref_name| short_ref(&ref_name).into_owned())
            }
            // The main worktree's folder is the project's.
            Selection::Project(project) | Selection::Checkout(project, Checkout::Main) => {
                Some(self.store.read(cx).path(&project)?.display().to_string())
            }
            Selection::Checkout(project, Checkout::Linked(name)) => {
                let path =
                    (self.store.read(cx)).checkout_path(&project, &Checkout::Linked(name))?;
                Some(path.display().to_string())
            }
            Selection::Mesh(project, id, MeshPick::Checkout) => Some(
                self.store
                    .read(cx)
                    .mesh_checkout(&project, &id)?
                    .path
                    .clone(),
            ),
            Selection::Uncommitted(..)
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
        let peeking = self.peek.open || self.peek.slide.closing(clock::now());
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
                navigator: (self.detail.read(cx)).navigator_width(clock::now(), window, cx),
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
        let now = clock::now();
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
        let now = clock::now();
        if self.navigator_open || self.navigator_slide.closing(now) {
            return;
        }
        let Point { x, y } = at;
        if !self.peek.open {
            let beside = y >= Theme::get(cx).scaled(HEADER_HEIGHT);
            if x <= fixed(PEEK_EDGE) && beside && !self.covered() {
                self.open_peek(cx);
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

    /// Bring the closed navigator out over the detail.
    fn open_peek(&mut self, cx: &mut Context<AppView>) {
        self.peek.open = true;
        self.peek.slide.start(true, cx);
        (self.navigator).update(cx, |navigator, cx| navigator.set_floating(true, cx));
        cx.notify();
    }

    /// Whether the navigator out over the detail stays though the pointer strayed from it:
    /// what was opened from it is over the window, a drag may be headed for it, its filter
    /// has the keys, or it has them since ⌘1 brought it out. The pointer's next move looks
    /// again.
    fn peek_held(&self, window: &Window, cx: &App) -> bool {
        let navigator = self.navigator.focus_handle(cx);
        let keys = navigator.contains_focused(window, cx);
        let typing = keys && !navigator.is_focused(window);
        self.covered() || cx.has_active_drag() || typing || (self.peek.keyed && keys)
    }

    /// Send the navigator ⌘1 brought out back, and with it the selection and the keys to where
    /// ⌘1 was pressed, while that's still there.
    fn send_peek_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.peek.keyed = false;
        if let Some((selection, focus)) = self.peek.returns_to.take()
            && self.store.read(cx).still_there(&selection)
        {
            self.store
                .update(cx, |store, cx| store.select(Some(selection), cx));
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        }
        self.close_peek(window, cx);
    }

    /// Send the navigator out over the detail back, unless [`AppView::peek_held`], moving the
    /// keys off it, as it takes no more.
    fn close_peek(&mut self, window: &mut Window, cx: &mut Context<AppView>) {
        self.peek.leaving = None;
        if !self.peek.open || self.peek_held(window, cx) {
            return;
        }
        self.peek.open = false;
        self.peek.keyed = false;
        self.peek.returns_to = None;
        self.peek.committing = false;
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
        cx: &Context<Self>,
    ) -> Div {
        let theme = Theme::get(cx);
        let bar = self.detail.read(cx).top_strip_height(window, cx);
        let store = self.store.read(cx);
        let (areas, drawn_home) = (store.drawn_areas(), store.drawn_home());
        let (dividers, lifted) = (store.drawn_dividers(), store.lifted_tile().cloned());
        let home = drawn_home.unwrap_or(Area::WHOLE);
        // Below the location bar, which runs across every tile: the room the tiles share. The
        // detail draws the page in the home tile's area of it, under its bar while there are
        // others, and the diff and the terminal selected go over that.
        // A terminal there closes, a terminal pinned beside it taking its place, as terminals
        // are alike.
        let close_home = self.close_home(cx);
        let home_bar = drawn_home.and_then(|_| {
            let store = self.store.read(cx);
            let (kind, title) = tile_label(store, &store.row_key(store.selection.as_ref()?));
            let focus = self.tile_focus(&Tile::Home, cx);
            let close = close_home.clone().map(|close| {
                icon_button("close-home", Icon::Cross)
                    .small()
                    .tooltip_for("Close", &CloseTile)
                    .on_click(move |_, _, cx| close(cx))
            });
            let grab = self.grab_tile(Tile::Home, cx);
            let buttons = self.tile_buttons(&Tile::Home, cx).into_iter().chain(close);
            // Over the detail, which would take the keys for its page rather than a terminal's.
            Some(
                tile_bar(kind, title, buttons, focus, grab, theme)
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .occlude(),
            )
        });
        let inset = self.detail.read(cx).bar_inset(cx);
        let page = div()
            .absolute()
            .top(dp(inset))
            .bottom_0()
            .left_0()
            .right_0()
            .when_some(
                Self::diff_left(&self.detail, now, window, cx),
                |el, left| {
                    el.child(
                        div()
                            .absolute()
                            .top(dp(self.detail.read(cx).view_header_height(cx)))
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
                },
            )
            .children(self.shown_terminal(cx).map(|terminal| {
                div()
                    .absolute()
                    .inset_0()
                    .child(terminal.cached(StyleRefinement::default().size_full()))
            }));
        // The detail draws the card under it; a terminal's edge is drawn over it here.
        let home = placed(home)
            .when_some(close_home, |el, close| {
                el.on_action(move |_: &CloseTile, _, cx| close(cx))
            })
            .child(tile_content().child(page).children(home_bar))
            .when(self.has_keys(&Tile::Home, window, cx), |el| {
                el.child(card_edge(theme.focus_edge()))
            })
            .when(self.shown_terminal(cx).is_some(), |el| {
                el.child(self.paste_outline(theme))
            });
        // Dragged by its bar or zoomed, over the others, as the detail draws its card and page.
        let home = match lifted == Some(Tile::Home) {
            true => on_top(home).into_any_element(),
            false => home.into_any_element(),
        };
        let resting = self.store.read(cx).resting_tiling().cloned();
        let sliding = (self.tile_slide.as_ref().map(|slide| slide.tile.clone()))
            .or_else(|| self.row_tile.as_ref().and_then(RowTile::opening));
        let pinned = (areas.into_iter().flatten()).filter_map(|(tile, area)| {
            let on_top_of_others = lifted.as_ref() == Some(&tile);
            let el = match tile {
                Tile::Home => return None,
                Tile::Pinned(pinned) if sliding.as_ref() == Some(&pinned) => {
                    let rest = (resting.iter().flat_map(Tiling::areas))
                        .find(|(tile, _)| *tile == Tile::Pinned(pinned.clone()))?;
                    self.render_sliding(pinned, area, rest.1, now, window, theme, cx)?
                }
                Tile::Pinned(pinned) => self.render_pinned(pinned, area, now, window, theme, cx)?,
            };
            Some(match on_top_of_others {
                true => on_top(el).into_any_element(),
                false => el.into_any_element(),
            })
        });
        let junctions = (crate::state::junctions(&dividers).into_iter())
            .map(|junction| self.render_junction(junction, theme, cx));
        let dividers =
            (dividers.into_iter()).map(|divider| self.render_divider(divider, theme, cx));
        let store = self.store.clone();
        div()
            .relative()
            // Escape puts a tile dragged by its bar back, before the terminal with the keys,
            // the dragged one's, would send it to its shell.
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.tile_dragged() && event.keystroke.key == "escape" {
                    this.let_go_tile(true, cx);
                    cx.stop_propagation();
                }
            }))
            // Over the navigator as it slides, taking the pointer from it.
            .when(navigator_sliding, |el| el.bg(theme.bg_1).occlude())
            .child(
                self.detail
                    .clone()
                    .cached(StyleRefinement::default().size_full()),
            )
            .child(
                // The cards' room, in from its edges as far as they are from each other.
                div()
                    .absolute()
                    .top(dp(bar + self.detail.read(cx).room_top(window, cx)))
                    .bottom(dp(TILE_MARGIN))
                    .left(dp(self.detail.read(cx).room_left()))
                    .right(dp(TILE_MARGIN))
                    .on_drag_move({
                        let store = store.clone();
                        move |event: &DragMoveEvent<Divider>, _, cx| {
                            let divider = event.drag(cx).clone();
                            move_divider(&store, &divider, event.bounds, event.event.position, cx)
                        }
                    })
                    .on_drag_move(move |event: &DragMoveEvent<Junction>, _, cx| {
                        for divider in event.drag(cx).dividers.clone() {
                            move_divider(&store, &divider, event.bounds, event.event.position, cx)
                        }
                    })
                    .child(self.watch_tile_drag(cx))
                    .child(home)
                    .children(pinned)
                    .children(dividers)
                    .children(junctions)
                    .children(self.render_drop_zones(theme, cx))
                    .children(self.render_row_zones(theme, cx)),
            )
            .when(navigator_sliding, |el| {
                el.child(edge(theme.border_3).left(fixed(-1.)))
            })
    }

    /// A tile pinned beside the home tile, over `area` of the room below the header, edged
    /// where it meets the tiles before it, with a button in its corner that closes it: a
    /// terminal, or another row's page with its diff over it, as the home tile's has.
    fn render_pinned(
        &self,
        pinned: RowKey,
        area: Area,
        now: Instant,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let terminal = matches!(pinned, RowKey::Row(Selection::Terminal(..)));
        let keys = self.has_keys(&Tile::Pinned(pinned.clone()), window, cx);
        let content = match &pinned {
            RowKey::Row(Selection::Terminal(_, id)) => div().size_full().child(
                (self.store.read(cx).terminals.get(*id).cloned())
                    .or_else(|| self.going(&pinned)?.terminal.clone())?
                    .cached(StyleRefinement::default().size_full()),
            ),
            row => {
                let page = (self.pages.get(row)).or_else(|| self.going(row)?.page.as_ref())?;
                let diff = Self::diff_left(&page.detail, now, window, cx).map(|left| {
                    div()
                        .absolute()
                        .top(dp(page.detail.read(cx).view_header_height(cx)))
                        .bottom_0()
                        .right_0()
                        .left(dp(left))
                        .child((page.diff.clone()).cached(StyleRefinement::default().size_full()))
                });
                div()
                    .relative()
                    .size_full()
                    .child((page.detail.clone()).cached(StyleRefinement::default().size_full()))
                    .children(diff)
            }
        };
        let (kind, title) = tile_label(self.store.read(cx), &pinned);
        let focus = self.tile_focus(&Tile::Pinned(pinned.clone()), cx);
        let grab = self.grab_tile(Tile::Pinned(pinned.clone()), cx);
        let tile_buttons = self.tile_buttons(&Tile::Pinned(pinned.clone()), cx);
        let id = SharedString::from(format!("close-{pinned:?}"));
        let close = {
            let store = self.store.clone();
            move |cx: &mut App| store.update(cx, |store, cx| store.close_tile(&pinned, cx))
        };
        let (on_key, on_click) = (close.clone(), close);
        let close = icon_button(id, Icon::Cross)
            .small()
            .tooltip_for("Close", &CloseTile)
            .on_click(move |_, _, cx| on_click(cx));
        let buttons = tile_buttons.into_iter().chain(Some(close));
        let bar = tile_bar(kind, title, buttons, focus, grab, theme).flex_none();
        Some(
            placed(area)
                // Over the home tile's detail, which spans the room under it, gaps and all.
                .occlude()
                .key_context("PinnedTile")
                .on_action(move |_: &CloseTile, _, cx| on_key(cx))
                .child(tile_card(theme))
                .child(
                    (tile_content().flex().flex_col())
                        .child(bar)
                        .child(div().flex_1().min_h_0().relative().child(content)),
                )
                .when(keys, |el| el.child(card_edge(theme.focus_edge())))
                .when(terminal, |el| el.child(self.paste_outline(theme))),
        )
    }

    /// A tile sliding open or shut, over `area` of the room in this frame, of `rest` it has at
    /// rest: drawn at rest, from the edge of it that moves, the rest cut off, as the detail
    /// slides in beside the navigator, so it lays out once. Shutting, it takes no pointer.
    #[expect(
        clippy::too_many_arguments,
        reason = "render_pinned's, and the area at rest"
    )]
    fn render_sliding(
        &self,
        pinned: RowKey,
        area: Area,
        rest: Area,
        now: Instant,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let going = self.going(&pinned).is_some();
        let (width, height) = (
            rest.width / area.width.max(1e-4),
            rest.height / area.height.max(1e-4),
        );
        // Where its leading edge stays put, as the first part of a division's does, its trailing
        // edge moves, and it's drawn from that one.
        let trailing = (area.left - rest.left).abs() < 1e-4 && (area.top - rest.top).abs() < 1e-4;
        let inner = Area {
            left: if trailing { 1. - width } else { 0. },
            top: if trailing { 1. - height } else { 0. },
            width,
            height,
        };
        let tile = self.render_pinned(pinned, inner, now, window, theme, cx)?;
        Some(placed(area).overflow_hidden().child(inert(tile, going)))
    }

    /// Over the tiles, the one dragged by its bar too, where the pointer lands it where, once
    /// the right button shows it: the landings that lay the tiles out alike in a colour of their
    /// own and named together once, in their biggest box, those it's over stronger.
    fn render_drop_zones(&self, theme: &Theme, cx: &App) -> Option<OnTop> {
        let drag = (self.tile_drag.as_ref())
            .filter(|drag| drag.lifted && !drag.released && !drag.zones.is_empty())?;
        let store = self.store.read(cx);
        let home = store.row_key(store.selection.as_ref()?);
        let least = self.least_tile(cx);
        // Each layout the landings make, the tiling as it is first, and the landings making it.
        let mut layouts: Vec<(Tiling, Vec<&Landing>)> = vec![(drag.before.clone(), Vec::new())];
        let mut layout_of: Vec<(&Landing, usize)> = Vec::new();
        for landing in drag.zones.iter().filter_map(|zone| zone.landing.as_ref()) {
            if layout_of.iter().any(|(other, _)| *other == landing) {
                continue;
            }
            let landed = drag.before.landed(&drag.tile, landing, least);
            let ix = match layouts.iter().position(|(layout, _)| *layout == landed) {
                Some(ix) => ix,
                None => {
                    layouts.push((landed, Vec::new()));
                    layouts.len() - 1
                }
            };
            if let Some((_, landings)) = layouts.get_mut(ix) {
                landings.push(landing);
            }
            layout_of.push((landing, ix));
        }
        let layout = |landing: &Landing| {
            (layout_of.iter()).find_map(|(other, ix)| (*other == landing).then_some(*ix))
        };
        let over = match &drag.pending {
            Some((landing, _)) => layout(landing),
            None => layout(&drag.landing),
        };
        let color = |ix: usize| gpui::hsla((ix as f32 * 0.618).fract(), 0.75, 0.5, 1.);
        let placed = |zone: &DropZone| {
            div()
                .absolute()
                .left(fixed(zone.left))
                .top(fixed(zone.top))
                .w(fixed(zone.right - zone.left))
                .h(fixed(zone.bottom - zone.top))
        };
        let boxes = drag.zones.iter().filter_map(|zone| {
            let ix = layout(zone.landing.as_ref()?)?;
            let strength = if Some(ix) == over { 0.28 } else { 0.1 };
            Some(placed(zone).bg(color(ix).opacity(strength)))
        });
        let area = |zone: &DropZone| (zone.right - zone.left) * (zone.bottom - zone.top);
        let labels = layouts
            .iter()
            .enumerate()
            .filter_map(|(ix, (_, landings))| {
                let biggest = (drag.zones.iter())
                    .filter(|zone| zone.landing.as_ref().and_then(layout) == Some(ix))
                    .max_by(|a, b| area(a).total_cmp(&area(b)))?;
                let text = match ix {
                    0 => "Stays".to_owned(),
                    _ => (landings.iter())
                        .map(|landing| landing_text(landing, store, &home))
                        .collect::<Vec<_>>()
                        .join(" = "),
                };
                let label = div()
                    .px(dp(6.))
                    .py(dp(2.))
                    .rounded(dp(4.))
                    .bg(theme.bg_1)
                    .border_1()
                    .border_color(color(ix))
                    .text_size(dp(11.))
                    .text_color(theme.text_1)
                    .child(text);
                Some((placed(biggest).flex().items_center().justify_center()).child(label))
            });
        let zones = div().absolute().inset_0().children(boxes).children(labels);
        // Over the tile dragged, drawn on top itself.
        Some(on_top(zones).priority(1))
    }

    /// Over the tiles, the two strips a row dragged from the navigator lands as a tile in, once
    /// the right button or ⌥ shows them, as [`Self::render_drop_zones`] shows a tile's.
    fn render_row_zones(&self, theme: &Theme, cx: &App) -> Option<OnTop> {
        let drag = self.row_drag.as_ref().filter(|drag| drag.zones)?;
        let room = self.tile_room.get().size;
        let feel = self.store.read(cx).tile_drag_feel;
        let width = f32::from(Theme::get(cx).scaled(feel.row_zone));
        let strips = [(Side::Left, 0.), (Side::Right, 0.618)].into_iter();
        let strips = strips.filter_map(|(side, hue)| {
            self.row_landed(side, cx)?;
            let (left, text) = match side {
                Side::Right => (f32::from(room.width) - width, "Right side"),
                Side::Left | Side::Top | Side::Bottom => (0., "Left side"),
            };
            let color = gpui::hsla(hue, 0.75, 0.5, 1.);
            let over = drag.over.is_some_and(|(over, _)| over == side);
            let strength = if over { 0.28 } else { 0.1 };
            let label = div()
                .px(dp(6.))
                .py(dp(2.))
                .rounded(dp(4.))
                .bg(theme.bg_1)
                .border_1()
                .border_color(color)
                .text_size(dp(11.))
                .text_color(theme.text_1)
                .child(text);
            Some(
                div()
                    .absolute()
                    .left(fixed(left))
                    .top_0()
                    .bottom_0()
                    .w(fixed(width))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(color.opacity(strength))
                    .child(label),
            )
        });
        let zones = div().absolute().inset_0().children(strips);
        Some(on_top(zones).priority(1))
    }

    /// A strip over the gap between the cards of two parts of a division: drag it to move the
    /// line between them, double-click it to share the division's room evenly again. A pill in
    /// the middle of the gap shows it while the pointer's on it or it's dragged.
    fn render_divider(&self, divider: Divider, theme: &Theme, cx: &Context<Self>) -> Div {
        let id = SharedString::from(format!("divider-{:?}-{}", divider.path, divider.after));
        let which = DividerDrag::Tiles(divider.path.clone(), divider.after);
        let (this, path) = (cx.entity().downgrade(), divider.path.clone());
        let strip = gap_divider(id, divider.axis, which.clone(), &self.store, theme, cx)
            .on_click(move |event, _, cx| {
                if event.click_count() == 2 {
                    let paths = std::slice::from_ref(&path);
                    (this.update(cx, |this, cx| this.even_out_tiles(paths, cx))).ok();
                }
            })
            .on_drag(divider.clone(), mark_dragged(&self.store, which));
        // A line with no breadth along the division, and the strip across it, the gap's width.
        let room = divider.room;
        match divider.axis {
            Axis::Across => div()
                .absolute()
                .left(relative(divider.at))
                .top(relative(room.top))
                .h(relative(room.height))
                .child(
                    strip
                        .top_0()
                        .bottom_0()
                        .left(dp(-TILE_MARGIN))
                        .w(dp(2. * TILE_MARGIN)),
                ),
            Axis::Down => div()
                .absolute()
                .top(relative(divider.at))
                .left(relative(room.left))
                .w(relative(room.width))
                .child(
                    strip
                        .left_0()
                        .right_0()
                        .top(dp(-TILE_MARGIN))
                        .h(dp(2. * TILE_MARGIN)),
                ),
        }
    }

    /// A handle over where the lines between tiles meet: drag it to move them all at once,
    /// double-click it to share each of their divisions' room evenly again.
    fn render_junction(&self, junction: Junction, theme: &Theme, cx: &Context<Self>) -> Div {
        let lines = (junction.dividers.iter())
            .map(|divider| (divider.path.clone(), divider.after))
            .collect::<Vec<_>>();
        let id = SharedString::from(format!("junction-{lines:?}"));
        let which = DividerDrag::Junction(lines);
        let (this, at) = (cx.entity().downgrade(), junction.at);
        let paths: Vec<Vec<usize>> = (junction.dividers.iter())
            .map(|divider| divider.path.clone())
            .collect();
        let handle = gap_junction(id, &junction, &which, &self.store, theme, cx)
            .on_click(move |event, _, cx| {
                if event.click_count() == 2 {
                    (this.update(cx, |this, cx| this.even_out_tiles(&paths, cx))).ok();
                }
            })
            .on_drag(junction, mark_dragged(&self.store, which))
            .left(dp(-JUNCTION_HANDLE / 2.))
            .top(dp(-JUNCTION_HANDLE / 2.))
            .size(dp(JUNCTION_HANDLE));
        div()
            .absolute()
            .left(relative(at.0))
            .top(relative(at.1))
            .child(handle)
    }

    /// What takes the keys in `tile` of the selected row's tiling, if it's open: the home
    /// tile's terminal or page, or a pinned one's.
    fn tile_focus(&self, tile: &Tile, cx: &App) -> Option<FocusHandle> {
        match tile {
            Tile::Home => Some(match self.shown_terminal(cx) {
                Some(terminal) => terminal.focus_handle(cx),
                None => self.detail.focus_handle(cx),
            }),
            Tile::Pinned(RowKey::Row(Selection::Terminal(_, id))) => {
                Some(self.store.read(cx).terminals.get(*id)?.focus_handle(cx))
            }
            Tile::Pinned(row) => Some(self.pages.get(row)?.detail.focus_handle(cx)),
        }
    }

    /// Whether `tile` of the selected row has the keys, the home tile with no others too.
    fn has_keys(&self, tile: &Tile, window: &Window, cx: &App) -> bool {
        (self.tile_focus(tile, cx)).is_some_and(|focus| focus.contains_focused(window, cx))
    }

    /// The tile of the selected row's tiling that has the keys, if one does.
    fn tile_with_keys(&self, window: &Window, cx: &App) -> Option<Tile> {
        let tiling = self.store.read(cx).tiling()?;
        (tiling.areas().into_iter())
            .map(|(tile, _)| tile)
            .find(|tile| {
                (self.tile_focus(tile, cx)).is_some_and(|focus| focus.contains_focused(window, cx))
            })
    }

    /// Give the keys back to the tile of the selected row's tiling that had them last, or its
    /// home tile, as when the row is clicked or the tile with them closes. Whether it has one.
    pub(crate) fn focus_last_tile(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let store = self.store.read(cx);
        if store.tiling().is_none() {
            return false;
        }
        let last = store.last_tile_with_keys().unwrap_or(Tile::Home);
        match self.tile_focus(&last, cx) {
            Some(focus) => window.focus(&focus, cx),
            None => self.focus_main(window, cx),
        }
        true
    }

    /// Give the keys to the tile after the one with them in the selected row's tiling, in the
    /// order they're laid out, or the one before it going `back`, round past the ends, as
    /// iTerm's ⌘] and ⌘[ go through its panes. From outside the tiles, the first or the last.
    fn focus_next_tile(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.covered() {
            return;
        }
        self.zoom_out(cx);
        let Some(tiling) = self.store.read(cx).tiling() else {
            return;
        };
        let tiles: Vec<Tile> = tiling.areas().into_iter().map(|(tile, _)| tile).collect();
        let at = (self.tile_with_keys(window, cx))
            .and_then(|tile| tiles.iter().position(|other| *other == tile));
        let count = tiles.len();
        let next = match at {
            Some(at) if back => (at + count - 1) % count,
            Some(at) => (at + 1) % count,
            None if back => count - 1,
            None => 0,
        };
        if let Some(focus) = tiles.get(next).and_then(|tile| self.tile_focus(tile, cx)) {
            window.focus(&focus, cx);
        }
    }

    /// Copy how the selected row's tiles are laid out, drawn in text, each by its name.
    fn copy_tile_diagram(&mut self, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let (Some(tiling), Some(selection)) = (store.tiling(), &store.selection) else {
            return;
        };
        let home = store.row_key(selection);
        let label = |tile: &Tile| {
            let row = match tile {
                Tile::Home => &home,
                Tile::Pinned(pinned) => pinned,
            };
            tile_label(store, row).1.to_string()
        };
        let diagram = tiling.diagram(label, 60, 18);
        self.copy_text(diagram, cx);
    }

    /// Whether the tile of the selected row that had the keys last is gone, and they went to
    /// the tile that had them before it.
    fn tile_lost_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.covered() {
            return false;
        }
        if !(self.store).update(cx, |store, _| store.forget_gone_tile_with_keys()) {
            return false;
        }
        // The last tile beside the home one takes the row's tiling with it, so the home tile,
        // all that's left, has them.
        if !self.focus_last_tile(window, cx)
            && let Some(focus) = self.tile_focus(&Tile::Home, cx)
        {
            window.focus(&focus, cx);
        }
        true
    }

    /// Note which tile of the selected row has the keys, for [`Self::focus_last_tile`].
    fn note_tile_with_keys(&mut self, window: &Window, cx: &mut App) {
        let Some(tile) = self.tile_with_keys(window, cx) else {
            return;
        };
        (self.store).update(cx, |store, _| store.note_tile_with_keys(&tile));
    }

    /// Zoom the tile with the keys, or the one that had them last, or zoom back out.
    fn zoom_with_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let last = self.store.read(cx).last_tile_with_keys();
        let tile = self.tile_with_keys(window, cx).or(last);
        self.toggle_zoom(tile.unwrap_or(Tile::Home), window, cx);
    }

    /// Have `tile` of the selected row's tiling grow over the others to take all their room, and
    /// the keys, or go back to its place while a tile is zoomed.
    fn toggle_zoom(&mut self, tile: Tile, window: &mut Window, cx: &mut Context<Self>) {
        if self.zoomed().is_some() {
            return self.zoom_out(cx);
        }
        let Some(tiling) = self.store.read(cx).tiling().cloned() else {
            return;
        };
        if self.tile_dragged() || self.row_tile.is_some() {
            return;
        }
        match self.tile_zoom.as_mut().filter(|zoom| zoom.tile == tile) {
            Some(zoom) => {
                zoom.open = true;
                zoom.slide.start(true, cx);
            }
            None => {
                let reflow = self.store.read(cx).tile_drag_feel.reflow;
                let mut slide = Slide::lasting(millis(reflow));
                slide.start(true, cx);
                self.tile_zoom = Some(TileZoom {
                    tile: tile.clone(),
                    tiling,
                    open: true,
                    slide,
                });
            }
        }
        self.tile_slide = None;
        self.tile_glide = None;
        if let Some(focus) = self.tile_focus(&tile, cx) {
            window.focus(&focus, cx);
        }
        self.move_tiles(cx);
    }

    /// The tile zoomed, or growing to be, not one on its way back.
    fn zoomed(&self) -> Option<&Tile> {
        (self.tile_zoom.as_ref())
            .filter(|zoom| zoom.open)
            .map(|zoom| &zoom.tile)
    }

    /// Send the tile zoomed back to its place.
    fn zoom_out(&mut self, cx: &mut Context<Self>) {
        if let Some(zoom) = self.tile_zoom.as_mut().filter(|zoom| zoom.open) {
            zoom.open = false;
            zoom.slide.start(false, cx);
            self.move_tiles(cx);
        }
    }

    /// Forget the zoom once the tiling changes under it, as with a tile split or closed.
    fn drop_stale_zoom(&mut self, cx: &App) {
        let tiling = self.store.read(cx).tiling();
        if (self.tile_zoom.as_ref()).is_some_and(|zoom| tiling != Some(&zoom.tiling)) {
            self.tile_zoom = None;
        }
    }

    /// Where the selected row's tiles are drawn at `now` around the tile zoomed, and whether
    /// it's moving. Once it's back in its place, the zoom's over.
    fn zoomed_tiling(&mut self, now: Instant) -> Option<(Drawn, bool)> {
        let zoom = self.tile_zoom.as_ref()?;
        let moving = zoom.slide.openness(now).is_some();
        if !zoom.open && !moving {
            self.tile_zoom = None;
            return None;
        }
        let at = zoom.slide.share(zoom.open, now);
        let drawn = Drawn {
            areas: zoom.tiling.zoomed(&zoom.tile, at),
            dividers: Vec::new(),
            rest: zoom.tiling.clone(),
            lifted: Some(zoom.tile.clone()),
            zoomed: true,
        };
        Some((drawn, moving))
    }

    /// Slide `tile` open out of its split, or shut into it while it's `going`.
    fn slide_tile(&mut self, tile: RowKey, going: Option<Going>, cx: &mut Context<Self>) {
        let mut slide = Slide::lasting(TILE_SLIDE);
        slide.start(going.is_none(), cx);
        self.tile_slide = Some(TileSlide { tile, slide, going });
        self.move_tiles(cx);
        cx.notify();
    }

    /// The selected row's tiling as it's drawn at `now` while a tile slides, and as it is at
    /// rest with that tile in it. Once the slide's over, what it kept goes.
    fn sliding_tiling(&mut self, now: Instant, cx: &App) -> Option<(Tiling, Tiling)> {
        let slide = self.tile_slide.as_ref()?;
        let rest = match &slide.going {
            Some(going) => Some(going.before.clone()),
            None => (self.store.read(cx).tiling())
                .filter(|tiling| tiling.has(&Tile::Pinned(slide.tile.clone())))
                .cloned(),
        };
        let sliding = (slide.slide.openness(now).zip(rest))
            .map(|(open, rest)| (rest.opening(&slide.tile, open), rest));
        if sliding.is_none() {
            self.tile_slide = None;
        }
        sliding
    }

    /// Give each part of the divisions at `paths` of the selected row's tiling the same room,
    /// the lines between them sliding there.
    fn even_out_tiles(&mut self, paths: &[Vec<usize>], cx: &mut Context<Self>) {
        let from = self.store.read(cx).tiling().cloned();
        self.store.update(cx, |store, cx| {
            for path in paths {
                store.even_out_tiles(path, cx);
            }
        });
        let to = self.store.read(cx).tiling().cloned();
        if let (Some(from), Some(to)) = (from, to)
            && from != to
        {
            let mut slide = Slide::lasting(TILE_SLIDE);
            slide.start(true, cx);
            self.tile_glide = Some(TileGlide { from, to, slide });
            self.move_tiles(cx);
        }
    }

    /// Give each part of every division of the selected row's tiling the same room.
    fn equalize_tiles(&mut self, cx: &mut Context<Self>) {
        let paths = self.store.read(cx).tiling().map(Tiling::divisions);
        self.even_out_tiles(&paths.unwrap_or_default(), cx);
    }

    /// The selected row's tiling as it's drawn at `now` while its lines slide to where they're
    /// even, and as it is, until that's over or it changes under them.
    fn gliding_tiling(&mut self, now: Instant, cx: &App) -> Option<(Tiling, Tiling)> {
        let glide = self.tile_glide.as_ref()?;
        let still = self.store.read(cx).tiling() == Some(&glide.to);
        let at = glide.slide.openness(now).filter(|_| still);
        let gliding = at.map(|at| (glide.from.toward(&glide.to, at), glide.to.clone()));
        if gliding.is_none() {
            self.tile_glide = None;
        }
        gliding
    }

    /// What tile `pinned`, shutting, showed, kept to draw it going.
    fn going(&self, pinned: &RowKey) -> Option<&Going> {
        (self.tile_slide.as_ref())
            .filter(|slide| slide.tile == *pinned)
            .and_then(|slide| slide.going.as_ref())
    }

    /// Start dragging `tile` of the selected row's tiling by its bar, pressed `at` in the
    /// window: it comes off once the pointer's moved far enough.
    fn press_tile(&mut self, tile: Tile, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(before) = self.store.read(cx).tiling().cloned() else {
            return;
        };
        if self.tile_zoom.is_some() {
            return;
        }
        let room = self.tile_room.get();
        let (width, height) = (f32::from(room.size.width), f32::from(room.size.height));
        let Some(area) = before.area_of(&tile).filter(|_| width > 0. && height > 0.) else {
            return;
        };
        let at = at - room.origin;
        let grab = (
            (f32::from(at.x) / width - area.left) / area.width.max(1e-4),
            f32::from(at.y) - area.top * height,
        );
        self.tile_drag = Some(TileDrag {
            from: before.areas(),
            size_from: (area.width, area.height),
            landed: before.clone(),
            shown: before.clone(),
            tile,
            before,
            pressed: at,
            pointer: at,
            grab,
            lifted: false,
            zones: Vec::new(),
            option: false,
            landing: Landing::Stay,
            pending: None,
            reflow: Slide::default(),
            resize: Slide::default(),
            released: false,
        });
    }

    /// Follow the pointer with the tile pressed by its bar, past the lift: what lands it
    /// where, and the other tiles make room there once it's rested over it. Whether the move
    /// was the drag's, so nothing under the pointer sees it.
    fn drag_tile(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) -> bool {
        if self.tile_drag.as_ref().is_none_or(|drag| drag.released) {
            return false;
        }
        // Let go where the app didn't see it, as over another window.
        if !event.dragging() {
            self.let_go_tile(false, cx);
            return false;
        }
        let feel = self.store.read(cx).tile_drag_feel;
        let theme = Theme::get(cx);
        let lift = theme.scaled(feel.lift);
        let room = self.tile_room.get();
        let sizes = (
            f32::from(theme.scaled(feel.edge)),
            f32::from(theme.scaled(feel.right_edge)),
            f32::from(theme.scaled(feel.bar)),
        );
        let Some(drag) = self.tile_drag.as_mut() else {
            return false;
        };
        let at = event.position - room.origin;
        if !drag.lifted {
            let moved = at - drag.pressed;
            if f32::from(moved.x).hypot(f32::from(moved.y)) < f32::from(lift) {
                return false;
            }
            drag.lifted = true;
            self.tile_slide = None;
        }
        drag.pointer = at;
        let over = drag.landing_under((at.x.into(), at.y.into()), room.size, sizes);
        match over {
            None => drag.pending = None,
            Some(landing) => {
                if landing == drag.landing {
                    drag.pending = None;
                } else if (drag.pending.as_ref()).is_none_or(|(pending, _)| *pending != landing) {
                    drag.pending = Some((landing, clock::now()));
                }
            }
        }
        self.move_tiles(cx);
        true
    }

    /// Let go of the tile dragged by its bar, or put it back where it was `back`, as Escape
    /// does: it settles into its place, and the row's tiling is as it lands.
    fn let_go_tile(&mut self, back: bool, cx: &mut Context<Self>) {
        let reflow = self.store.read(cx).tile_drag_feel.reflow;
        let room = self.tile_room.get().size;
        let least = self.least_tile(cx);
        let Some(drag) = self.tile_drag.as_mut().filter(|drag| !drag.released) else {
            return;
        };
        if !drag.lifted {
            self.tile_drag = None;
            return;
        }
        let now = clock::now();
        drag.from = drag.areas(now, room);
        drag.released = true;
        // Where the pointer is, though the tiles haven't made room there yet.
        if let Some((landing, _)) = drag.pending.take() {
            drag.landed = drag.before.landed(&drag.tile, &landing, least);
        }
        if back {
            drag.landed = drag.before.clone();
        }
        drag.reflow = moving_for(reflow, cx);
        let landed = (drag.landed != drag.before).then(|| drag.landed.clone());
        if let Some(landed) = landed {
            self.store
                .update(cx, |store, cx| store.land_tiling(landed, cx));
        }
        self.move_tiles(cx);
    }

    /// The selected row's tiling with the row dragged from the navigator along `side`, from
    /// how it was before that row's tile was tried out in it.
    fn row_landed(&self, side: Side, cx: &App) -> Option<Tiling> {
        let store = self.store.read(cx);
        let pinned = store.row_key(store.dragged_row.as_ref()?);
        let tiling = match self
            .row_tile
            .as_ref()
            .filter(|tile| tile.settling.is_none())
        {
            Some(tile) => tile.before.as_ref(),
            None => store.tiling(),
        };
        store.tiling_with(tiling, &pinned, side)
    }

    /// The side of the tiles a row dragged from the navigator lands as a tile with the pointer
    /// at `position`: only in a strip along the left or the right of them, the rest of them
    /// being left to the drops they take, as a terminal's paste.
    fn row_zone_at(&self, position: Point<Pixels>, window: &Window, cx: &App) -> Option<Side> {
        // A tile zoomed has all the room.
        if self.tile_zoom.is_some() {
            return None;
        }
        let store = self.store.read(cx);
        let room = self.tile_room.get();
        let at = position - room.origin;
        let (x, y) = (f32::from(at.x), f32::from(at.y));
        let theme = Theme::get(cx);
        let width = f32::from(theme.scaled(store.tile_drag_feel.row_zone));
        let margin = f32::from(theme.scaled(TILE_MARGIN));
        let (room_width, room_height) = (f32::from(room.size.width), f32::from(room.size.height));
        // Out over the detail, the navigator covers the left of the tiles.
        let covered = match self.peek.open {
            true => {
                let navigator = self
                    .detail
                    .read(cx)
                    .navigator_width(clock::now(), window, cx);
                f32::from(theme.scaled(PEEK_INSET + navigator + PEEK_SHADOW))
            }
            false => -margin,
        };
        let side = match () {
            _ if y < 0. || y > room_height + margin => return None,
            _ if x >= covered && x < width => Side::Left,
            _ if x >= room_width - width && x <= room_width + margin => Side::Right,
            _ => return None,
        };
        self.row_landed(side, cx).map(|_| side)
    }

    /// Follow the pointer with a row dragged from the navigator: what's in the zone it's over,
    /// the row's tile starts to slide in once it's rested there.
    fn drag_row(&mut self, event: &MouseMoveEvent, window: &Window, cx: &mut Context<Self>) {
        if !cx.has_active_drag() || self.store.read(cx).dragged_row.is_none() {
            return;
        }
        let side = self.row_zone_at(event.position, window, cx);
        let drag = self.row_drag.get_or_insert_default();
        if drag.over.map(|(over, _)| over) != side {
            drag.over = side.map(|side| (side, clock::now()));
            (self.store).update(cx, |store, cx| {
                store.tile_zone = side;
                cx.notify()
            });
            self.move_tiles(cx);
        }
    }

    /// Let go of a row dragged from the navigator: in a zone, its tile lands there, sliding on
    /// from as far as it's come, and nothing under the pointer takes the drop. Whether it did.
    fn let_go_row(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((side, _)) = self.row_drag.as_ref().and_then(|drag| drag.over) else {
            return false;
        };
        if self.row_tile.as_ref().is_some_and(|tile| tile.side != side) {
            self.take_row_tile_back(cx);
        }
        if self.row_tile.is_none() {
            self.bring_row_tile(side, cx);
        }
        let reflow = self.store.read(cx).tile_drag_feel.reflow;
        let now = clock::now();
        let Some(tile) = self.row_tile.as_mut() else {
            return false;
        };
        tile.from = tile.areas(now);
        tile.settling = Some(moving_for(reflow, cx));
        let Tile::Pinned(pinned) = tile.tile.clone() else {
            return false;
        };
        (self.store).update(cx, |store, cx| store.row_landed(pinned, cx));
        self.move_tiles(cx);
        true
    }

    /// Try the row dragged out as a tile along `side`, as it'd land there, drawn from where the
    /// tiles are now.
    fn bring_row_tile(&mut self, side: Side, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let (Some(dragged), Some(selection)) = (&store.dragged_row, &store.selection) else {
            return;
        };
        let (pinned, row) = (store.row_key(dragged), store.row_key(selection));
        let Some(landed) = self.row_landed(side, cx) else {
            return;
        };
        let before = store.tiling().cloned();
        let tile = Tile::Pinned(pinned);
        let mut from = before.clone().unwrap_or_default().areas();
        // Nothing yet, at the side it comes in from.
        let area = Area {
            left: if side == Side::Right { 1. } else { 0. },
            top: 0.,
            width: 0.,
            height: 1.,
        };
        from.push((tile.clone(), area));
        let feel = store.tile_drag_feel;
        let room = f32::from(self.tile_room.get().size.width).max(1.);
        let peek = f32::from(Theme::get(cx).scaled(feel.row_peek)) / room;
        let width = landed.area_of(&tile).map_or(1., |area| area.width);
        let mut peeking = Slide::lasting(millis(feel.reflow));
        peeking.start(true, cx);
        self.tile_slide = None;
        self.tile_glide = None;
        self.row_tile = Some(RowTile {
            tile,
            side,
            row: row.clone(),
            before: before.filter(|tiling| !tiling.is_home_only()),
            landed: landed.clone(),
            from,
            held: (peek / width.max(1e-4)).min(1.),
            open: true,
            peeking,
            settling: None,
        });
        (self.store).update(cx, |store, cx| store.try_tiling(row, Some(landed), cx));
    }

    /// Take the tile a row was tried out as away again, its row's tiling as it was, unless
    /// it's been let go there.
    fn take_row_tile_back(&mut self, cx: &mut Context<Self>) {
        let Some(tile) = self.row_tile.take() else {
            return;
        };
        if tile.settling.is_none() {
            (self.store).update(cx, |store, cx| store.try_tiling(tile.row, tile.before, cx));
        }
    }

    /// Forget the tile a row was tried out as once the tiling changes under it, as when a
    /// terminal's shell exits.
    fn drop_stale_row_tile(&mut self, cx: &App) {
        let tiling = self.store.read(cx).tiling();
        if (self.row_tile.as_ref()).is_some_and(|tile| tiling != Some(&tile.landed)) {
            self.row_tile = None;
        }
    }

    /// Move the tile of a row dragged in on: it starts to slide in once the pointer's rested in
    /// a zone, slides back as it leaves, and goes once it's back, or has settled where it was
    /// let go.
    fn move_row_tile(&mut self, now: Instant, cx: &mut Context<Self>) {
        let feel = self.store.read(cx).tile_drag_feel;
        let rested = (self.row_drag.as_ref())
            .and_then(|drag| drag.over)
            .filter(|(_, since)| now.saturating_duration_since(*since) >= millis(feel.row_wait))
            .map(|(side, _)| side);
        let Some(tile) = self.row_tile.as_mut() else {
            if let Some(side) = rested {
                self.bring_row_tile(side, cx);
            }
            return;
        };
        if let Some(settling) = &tile.settling {
            if settling.openness(now).is_none() {
                self.row_tile = None;
            }
            return;
        }
        let wanted = rested == Some(tile.side);
        if tile.open != wanted {
            tile.open = wanted;
            tile.peeking.start(wanted, cx);
        }
        if !tile.open && tile.peeking.openness(now).is_none() {
            self.take_row_tile_back(cx);
        }
    }

    /// Where the selected row's tiles are drawn at `now` around a row dragged in, and whether
    /// they're moving.
    fn row_tiling(&self, now: Instant) -> Option<(Drawn, bool)> {
        let tile = self.row_tile.as_ref()?;
        let moving = match &tile.settling {
            Some(settling) => settling.openness(now).is_some(),
            None => tile.peeking.openness(now).is_some(),
        };
        let drawn = Drawn {
            areas: tile.areas(now),
            dividers: Vec::new(),
            rest: tile.landed.clone(),
            lifted: None,
            zoomed: false,
        };
        Some((drawn, moving))
    }

    /// Forget the tile dragged once the tiling it's dragged in changes under it, as when a
    /// terminal's shell exits.
    fn drop_stale_tile_drag(&mut self, cx: &App) {
        let tiling = self.store.read(cx).tiling();
        let stale = (self.tile_drag.as_ref()).is_some_and(|drag| {
            let was = if drag.released {
                &drag.landed
            } else {
                &drag.before
            };
            tiling != Some(was)
        });
        if stale {
            self.tile_drag = None;
        }
    }

    /// Draw the selected row's tiles where they are now, while one slides open or shut or is
    /// dragged, and as they are once they're still. Before the frame, from what moves them and
    /// from the frame before while they move, as a view notified while the window draws draws
    /// again only in the frame after: the detail, which places the home tile, would lag a frame
    /// behind the tiles drawn here.
    fn move_tiles(&mut self, cx: &mut Context<Self>) {
        let now = clock::now();
        self.move_row_tile(now, cx);
        let dragged = (self.dragged_tiling(now, cx))
            .or_else(|| self.row_tiling(now))
            .or_else(|| self.zoomed_tiling(now));
        let (drawn, moving) = match dragged {
            Some((drawn, moving)) => (Some(drawn), moving),
            None => match (self.sliding_tiling(now, cx)).or_else(|| self.gliding_tiling(now, cx)) {
                Some((drawn, rest)) => {
                    let (areas, dividers) = (drawn.areas(), drawn.dividers());
                    let (lifted, zoomed) = (None, false);
                    (
                        Some(Drawn {
                            areas,
                            dividers,
                            rest,
                            lifted,
                            zoomed,
                        }),
                        true,
                    )
                }
                None => (None, false),
            },
        };
        // A row resting in a zone, before its tile starts to slide in, moves them next.
        let waiting = (self.row_drag.as_ref()).is_some_and(|drag| drag.over.is_some())
            && self.row_tile.is_none();
        let moving = moving || waiting;
        self.tiles_moving = moving;
        let changed = (self.store).update(cx, |store, _| store.set_drawn(drawn));
        if changed {
            self.detail.update(cx, |_, cx| cx.notify());
        }
        // Drawn again while they move, if only to move on the frame after, as while the pointer
        // rests before the tiles make room.
        if changed || moving {
            cx.notify();
        }
    }

    /// The selected row's tiling as it's drawn at `now` while a tile's dragged by its bar,
    /// once the pointer's rested long enough over where it would land, and whether it's moving.
    /// Once it's settled where it was let go, it's forgotten.
    fn dragged_tiling(&mut self, now: Instant, cx: &App) -> Option<(Drawn, bool)> {
        let feel = self.store.read(cx).tile_drag_feel;
        let room = self.tile_room.get().size;
        let least = self.least_tile(cx);
        let drag = self.tile_drag.as_mut().filter(|drag| drag.lifted)?;
        let rested = (drag.pending.as_ref())
            .filter(|(_, since)| now.saturating_duration_since(*since) >= millis(feel.wait))
            .map(|(landing, _)| landing.clone());
        if let Some(landing) = rested {
            drag.from = drag.areas(now, room);
            drag.size_from =
                (drag.from.last()).map_or(drag.size_from, |(_, area)| (area.width, area.height));
            drag.landed = drag.before.landed(&drag.tile, &landing, least);
            drag.shown = drag.before.shown_landing(&drag.tile, &landing, least);
            drag.landing = landing;
            drag.pending = None;
            drag.reflow = moving_for(feel.reflow, cx);
            drag.resize = moving_for(feel.resize, cx);
        }
        let moving = drag.pending.is_some()
            || drag.reflow.openness(now).is_some()
            || drag.resize.openness(now).is_some();
        if drag.released && !moving {
            self.tile_drag = None;
            return None;
        }
        let drawn = Drawn {
            areas: drag.areas(now, room),
            dividers: Vec::new(),
            rest: drag.drawn_tiling().clone(),
            lifted: Some(drag.tile.clone()),
            zoomed: false,
        };
        Some((drawn, moving))
    }

    /// The least a tile is dragged down to, in shares of the room's width and height.
    fn least_tile(&self, cx: &App) -> (f32, f32) {
        let room = self.tile_room.get().size;
        let least = f32::from(Theme::get(cx).scaled(MIN_TILE));
        (
            least / f32::from(room.width).max(1.),
            least / f32::from(room.height).max(1.),
        )
    }

    /// The buttons on `tile`'s bar that zoom it and split it, as Zoom tile, Split right and
    /// Split below do with the keys in it.
    fn tile_buttons(&self, tile: &Tile, cx: &Context<Self>) -> [Button; 3] {
        let button = |axis: Axis, icon: Icon, label: &'static str, action: &dyn gpui::Action| {
            let (this, tile) = (cx.entity().downgrade(), tile.clone());
            let id = SharedString::from(format!("{label}-{tile:?}"));
            icon_button(id, icon)
                .small()
                .tooltip_for(label, action)
                .on_click(move |_, window, cx| {
                    let tile = tile.clone();
                    (this.update(cx, |this, cx| this.split_tile(tile, axis, window, cx))).ok();
                })
        };
        let (icon, label) = match self.zoomed() == Some(tile) {
            true => (Icon::TileUnzoom, "Zoom out"),
            false => (Icon::TileZoom, "Zoom"),
        };
        let (this, zoomed) = (cx.entity().downgrade(), tile.clone());
        let zoom = icon_button(SharedString::from(format!("zoom-{tile:?}")), icon)
            .small()
            .tooltip_for(label, &ZoomTile)
            .on_click(move |_, window, cx| {
                let tile = zoomed.clone();
                (this.update(cx, |this, cx| this.toggle_zoom(tile, window, cx))).ok();
            });
        [
            zoom,
            button(Axis::Across, Icon::SplitRight, "Split right", &SplitRight),
            button(Axis::Down, Icon::SplitBelow, "Split below", &SplitBelow),
        ]
    }

    /// What pressing `tile`'s bar does, besides giving it the keys: it can be dragged.
    fn grab_tile(
        &self,
        tile: Tile,
        cx: &Context<Self>,
    ) -> impl Fn(&MouseDownEvent, &mut App) + 'static + use<> {
        let this = cx.entity().downgrade();
        move |event, cx| {
            (this.update(cx, |this, cx| {
                this.press_tile(tile.clone(), event.position, cx)
            }))
            .ok();
        }
    }

    /// Over the room the tiles share: keeps where it is for a tile dragged by its bar, and
    /// follows the pointer over the whole window while one is.
    fn watch_tile_drag(&self, cx: &Context<Self>) -> impl IntoElement {
        let (this, room) = (cx.entity().downgrade(), self.tile_room.clone());
        gpui::canvas(
            |_, _, _| {},
            move |bounds, (), window, _| {
                room.set(bounds);
                let on_move = this.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let taken = on_move.update(cx, |this, cx| {
                        this.drag_row(event, window, cx);
                        this.drag_tile(event, cx)
                    });
                    if taken.unwrap_or(false) {
                        cx.stop_propagation();
                    }
                });
                let on_up = this.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        let landed = on_up.update(cx, |this, cx| {
                            this.let_go_tile(false, cx);
                            this.store.update(cx, |store, cx| {
                                if store.dragged_divider.take().is_some() {
                                    cx.notify();
                                }
                            });
                            this.let_go_row(cx)
                        });
                        if landed.unwrap_or(false) {
                            cx.stop_propagation();
                        }
                    }
                });
                // While one's dragged, in a build of its own, for testing how it lands: the
                // right button shows where the pointer lands it where, or hides it, and the
                // middle one saves a picture of the window.
                if !backend::settings::dev_build() {
                    return;
                }
                let on_down = this;
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    let dragged = (on_down.upgrade()).is_some_and(|this| {
                        this.read(cx).tile_dragged() || this.read(cx).row_dragged(cx)
                    });
                    if phase != DispatchPhase::Capture || !dragged {
                        return;
                    }
                    let done = on_down.update(cx, |this, cx| match event.button {
                        MouseButton::Right => this.toggle_drop_zones(cx),
                        MouseButton::Middle => this.save_screenshot(window, cx),
                        _ => false,
                    });
                    if done.unwrap_or(false) {
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .inset_0()
    }

    /// Show where the pointer lands the tile dragged by its bar where, or stop showing it, for
    /// this drag alone. Whether it did, as one's dragged.
    fn toggle_drop_zones(&mut self, cx: &mut Context<Self>) -> bool {
        if self.row_dragged(cx) {
            let drag = self.row_drag.get_or_insert_default();
            drag.zones = !drag.zones;
            cx.notify();
            return true;
        }
        let feel = self.store.read(cx).tile_drag_feel;
        let theme = Theme::get(cx);
        let sizes = (
            f32::from(theme.scaled(feel.edge)),
            f32::from(theme.scaled(feel.right_edge)),
            f32::from(theme.scaled(feel.bar)),
        );
        let room = self.tile_room.get().size;
        let Some(drag) = self.tile_drag.as_mut() else {
            return false;
        };
        drag.zones = match drag.zones.is_empty() {
            true => drag.zones(room, sizes),
            false => Vec::new(),
        };
        cx.notify();
        true
    }

    /// Save a picture of the window where macOS saves screenshots, and copy it. Whether it
    /// does, as only macOS does.
    #[cfg(target_os = "macos")]
    fn save_screenshot(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let Some(number) = window_number(window) else {
            return false;
        };
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            let saved = backend::run(move || backend::screenshot::window(number)).await;
            store.update(cx, |store, cx| match saved {
                Ok((path, png)) => {
                    let image = gpui::Image::from_bytes(gpui::ImageFormat::Png, png);
                    cx.write_to_clipboard(ClipboardItem::new_image(&image));
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned());
                    store.snack(format!("Saved and copied {}", name.unwrap_or_default()), cx);
                }
                Err(err) => store.failed("Couldn't take a screenshot", &err, cx),
            })
        })
        .detach();
        true
    }

    #[cfg(not(target_os = "macos"))]
    fn save_screenshot(&mut self, _: &Window, _: &mut Context<Self>) -> bool {
        false
    }

    /// Show or hide where the tile dragged lands where as ⌥ goes down, not as it comes up.
    fn option_changed(&mut self, held: bool, cx: &mut Context<Self>) {
        if self.row_dragged(cx) {
            let drag = self.row_drag.get_or_insert_default();
            let pressed = held && !drag.option;
            drag.option = held;
            if pressed {
                self.toggle_drop_zones(cx);
            }
            return;
        }
        if !self.tile_dragged() {
            return;
        }
        let Some(drag) = self.tile_drag.as_mut() else {
            return;
        };
        let pressed = held && !drag.option;
        drag.option = held;
        if pressed {
            self.toggle_drop_zones(cx);
        }
    }

    /// Whether a row's being dragged from the navigator.
    fn row_dragged(&self, cx: &App) -> bool {
        cx.has_active_drag() && self.store.read(cx).dragged_row.is_some()
    }

    /// Whether a tile's being dragged by its bar, come off and not let go yet.
    fn tile_dragged(&self) -> bool {
        (self.tile_drag.as_ref()).is_some_and(|drag| drag.lifted && !drag.released)
    }

    /// Close the selected terminal, the home tile, while a terminal pinned beside it would take
    /// its place, as its shell exiting does.
    fn close_home(&self, cx: &App) -> Option<impl Fn(&mut App) + Clone + 'static> {
        let store = self.store.read(cx);
        if store.home_heirs(&[]).is_empty() {
            return None;
        }
        let Some(Selection::Terminal(_, id)) = store.selection else {
            return None;
        };
        let store = self.store.clone();
        Some(move |cx: &mut App| store.update(cx, |store, cx| store.close_terminal(id, cx)))
    }

    /// The tile of the selected row's tiling that has the keys: a pinned one, else its home
    /// tile.
    pub(crate) fn focused_tile(&self, window: &Window, cx: &App) -> Tile {
        self.tile_with_keys(window, cx).unwrap_or(Tile::Home)
    }

    /// Over a terminal's card, its edge in pop while what's held over it would paste there, a
    /// row or files from the Finder, as a row that takes a drop has. It's drawn here, where
    /// GPUI styles it again each frame of the drag: the terminal's own view draws only when it
    /// changes, and the detail under the home tile is cached, so one there would stay up after
    /// a drag let go elsewhere or stopped with Escape.
    fn paste_outline(&self, theme: &Theme) -> Div {
        let (store, color) = (self.store.clone(), theme.pop);
        card_edge(gpui::transparent_black())
            .drag_over::<Cut>(move |style, _, _, _| style.border_1().border_color(color))
            .drag_over::<ProjectDrag>(move |style, _, _, _| style.border_1().border_color(color))
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.border_1().border_color(color))
            // Not once a commit or worktree the drag holds is gone, which pastes nothing, nor
            // where a row lands as a tile instead.
            .can_drop(move |dragged, _, cx| {
                let store = store.read(cx);
                store.tile_zone.is_none() && (!dragged.is::<Cut>() || store.pasted().is_some())
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
        cx: &Context<Self>,
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

/// A line over the edge of a tile's card, in `color`: the tile with the keys', in gray, or one
/// a drag would paste into, in pop.
fn card_edge(color: gpui::Hsla) -> Div {
    crate::ui::inset_by(div(), TILE_MARGIN)
        .rounded(dp(6.))
        .border_1()
        .border_color(color)
}

/// A tile's bar, `TILE_BAR` tall: what it shows, by `kind`'s icon and `title`, and `buttons`, as
/// its ×. A press anywhere else on it gives `focus` the keys, the tile's, and `grab`s the tile,
/// to drag it.
fn tile_bar(
    kind: Icon,
    title: SharedString,
    buttons: impl IntoIterator<Item = Button>,
    focus: Option<FocusHandle>,
    grab: impl Fn(&MouseDownEvent, &mut App) + 'static,
    theme: &Theme,
) -> Div {
    h_flex()
        .h(dp(TILE_BAR))
        .pl(dp(8.))
        .pr(dp(2.))
        .gap(dp(6.))
        .text_size(dp(12.))
        .text_color(theme.text_2)
        .child(small_icon(kind).text_color(theme.text_3))
        .child(div().flex_grow(1.).min_w_0().truncate().child(title))
        .child(h_flex().flex_none().children(buttons))
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            if let Some(focus) = &focus {
                window.focus(focus, cx);
            }
            grab(event, cx);
        })
}

/// Where a tile dragged by its bar lands, in a few words, the tiles by what they show.
fn landing_text(landing: &Landing, store: &Store, home: &RowKey) -> String {
    let name = |tile: &Tile| {
        let row = match tile {
            Tile::Home => home,
            Tile::Pinned(pinned) => pinned,
        };
        tile_label(store, row).1
    };
    let edge = |side: &Side| match side {
        Side::Left => "left",
        Side::Right => "right",
        Side::Top => "top",
        Side::Bottom => "bottom",
    };
    match landing {
        Landing::Stay => "Stays".to_owned(),
        Landing::Beside(tile, Side::Left) => format!("Left of {}", name(tile)),
        Landing::Beside(tile, Side::Right) => format!("Right of {}", name(tile)),
        Landing::Beside(tile, Side::Top) => format!("Above {}", name(tile)),
        Landing::Beside(tile, Side::Bottom) => format!("Below {}", name(tile)),
        Landing::Edge(side) => format!("Along the {}", edge(side)),
        Landing::BelowRow => "Below its row".to_owned(),
    }
}

/// What a tile pinned to `row` shows, as Go to anything names it: its kind's icon, and its
/// name.
pub(crate) fn tile_label(store: &Store, row: &RowKey) -> (Icon, SharedString) {
    let Some(selection) = store.selection_of(row) else {
        return (Icon::Commit, "Gone".into());
    };
    let workspace = store.workspace(selection.project());
    let worktree = |checkout: &Checkout| -> SharedString {
        checkout
            .linked()
            .map_or("Main worktree".into(), |name| name.to_string().into())
    };
    match &selection {
        Selection::Terminal(_, id) => {
            let title = store.terminals.title(*id).cloned();
            (Icon::TextCode, title.unwrap_or("Terminal".into()))
        }
        Selection::Commit(project, id) | Selection::Upstream(project, _, id) => {
            let title = store
                .find_commit(project, *id)
                .map(|(_, c)| commit_title(&c.title));
            (Icon::Commit, title.unwrap_or_else(|| id.short().into()))
        }
        Selection::Branch(project, ref_name) => {
            let branch = workspace.and_then(|ws| ws.find_branch(ref_name.as_ref()));
            let name = match (branch, ref_name) {
                (Some(branch), _) => store.name_of(project, branch).to_owned(),
                (None, Some(ref_name)) => short_ref(ref_name).into_owned(),
                (None, None) => "Unnamed branch".to_owned(),
            };
            (Icon::Branch, name.into())
        }
        Selection::Stack(..) => (Icon::Branch, "Stack".into()),
        Selection::Branches(_) => (Icon::Branch, "Branches".into()),
        Selection::History(_) => (Icon::Clock, "Operations history".into()),
        Selection::Graph(_) => (Icon::Fork, "Graph".into()),
        Selection::Project(project) => {
            let name = store.project(project).map(|e| e.project.name.clone());
            (Icon::Repo, name.unwrap_or_default().into())
        }
        Selection::Checkout(_, checkout) => (Icon::FolderCopy, worktree(checkout)),
        Selection::Uncommitted(..) => (Icon::FolderCopy, "Uncommitted changes".into()),
        Selection::Section(..) | Selection::Mesh(..) => (Icon::FolderCopy, "Worktrees".into()),
    }
}

/// A pinned tile's page: a detail of its own, and the diff drawn over it.
struct Page {
    detail: Entity<Detail>,
    diff: Entity<DiffPane>,
}

/// A tile sliding open out of the split it's pinned at, or shut back into it as it closes, as
/// the detail slides in beside the navigator.
struct TileSlide {
    tile: RowKey,
    slide: Slide,
    /// Shutting, what it goes from.
    going: Option<Going>,
}

/// The lines of a tiling sliding from where they were to where they are now.
struct TileGlide {
    from: Tiling,
    to: Tiling,
    slide: Slide,
}

/// A tile shutting: the tiling as it was with it there, and what it showed, kept to draw it
/// going: the terminal that closed with it, or its page.
struct Going {
    before: Tiling,
    terminal: Option<Entity<crate::terminal::Terminal>>,
    page: Option<Page>,
}

/// A tile pressed by its bar, from then until it's let go and has settled where it lands.
/// Positions are in the room the tiles share, in pixels.
struct TileDrag {
    tile: Tile,
    /// The row's tiling as it was pressed.
    before: Tiling,
    pressed: Point<Pixels>,
    pointer: Point<Pixels>,
    /// Where the pointer holds the tile: the share of its width it's in from its left, and
    /// how far down from its top, in pixels, as it's held by its bar.
    grab: (f32, f32),
    /// Whether the pointer's moved far enough for the tile to come off.
    lifted: bool,
    /// Where the pointer lands it where, in pixels in the room, while the right button or ⌥
    /// has them shown in this drag.
    zones: Vec<DropZone>,
    /// Whether ⌥ was down as the modifiers last changed, so it shows them only as it goes down.
    option: bool,
    /// Where it lands, the tiling it makes there, and that tiling as it's drawn until it's let
    /// go: see [`Tiling::shown_landing`].
    landing: Landing,
    landed: Tiling,
    shown: Tiling,
    /// Where the pointer is over, until it's rested there long enough for the tiles to make
    /// room, and since when.
    pending: Option<(Landing, Instant)>,
    /// Where each tile was drawn as it last landed somewhere new, which they move on from,
    /// and the dragged one's width and height then.
    from: Vec<(Tile, Area)>,
    size_from: (f32, f32),
    /// The other tiles moving out of its way, and it growing or shrinking to its room there.
    reflow: Slide,
    resize: Slide,
    /// Whether it's been let go, and settles into its place.
    released: bool,
}

/// A tile of the selected row grown over the others to take all their room, or on its way
/// there or back.
struct TileZoom {
    tile: Tile,
    /// The tiling it's zoomed in, which the others stay as they are in.
    tiling: Tiling,
    /// Whether it's zoomed, not going back, and how far it's come.
    open: bool,
    slide: Slide,
}

/// A row of the navigator being dragged, as far as the tiles are concerned.
#[derive(Default)]
struct RowDrag {
    /// The side whose zone the pointer's in, and since when.
    over: Option<(Side, Instant)>,
    /// Whether the right button or ⌥ has the zones shown in this drag.
    zones: bool,
    /// Whether ⌥ was down as the modifiers last changed, so it shows them only as it goes down.
    option: bool,
}

/// The tile of a row dragged from the navigator, held in a zone or let go there: from the
/// start it's in its row's tiling as it would land, and drawn in part of the way there.
struct RowTile {
    /// Drawn at its final size and cut off, as it comes in.
    tile: Tile,
    side: Side,
    /// The row whose tiling it's in, and that tiling without it, with `None` the row alone.
    row: RowKey,
    before: Option<Tiling>,
    landed: Tiling,
    /// Where the tiles move on from.
    from: Vec<(Tile, Area)>,
    /// How much of the way there it comes while it's held.
    held: f32,
    /// Whether it's coming in or going back, and how far.
    open: bool,
    peeking: Slide,
    /// Sliding the rest of the way, from `from`, once it's let go.
    settling: Option<Slide>,
}

impl RowTile {
    /// Where each tile is drawn at `now`.
    fn areas(&self, now: Instant) -> Vec<(Tile, Area)> {
        let at = match &self.settling {
            Some(settling) => settling.openness(now).unwrap_or(1.),
            None => self.held * self.peeking.share(self.open, now),
        };
        (self.landed.areas().into_iter())
            .map(|(tile, area)| {
                let was = (self.from.iter()).find(|(other, _)| *other == tile);
                let drawn = was.map_or(area, |(_, was)| was.toward(area, at));
                (tile, drawn)
            })
            .collect()
    }

    /// The row whose tile is coming in, drawn at its final size.
    fn opening(&self) -> Option<RowKey> {
        match &self.tile {
            Tile::Pinned(row) => Some(row.clone()),
            Tile::Home => None,
        }
    }
}

/// A box of the room, in pixels, where the pointer lands a tile dragged by its bar at
/// `landing`: nowhere, with `None`.
#[derive(Debug, Clone, PartialEq)]
struct DropZone {
    landing: Option<Landing>,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

/// How finely [`TileDrag::zones`] looks where the pointer lands the tile, in pixels.
const ZONE_STEP: f32 = 6.;

/// How far past the room [`TileDrag::zones`] looks, for the landings along its sides.
const ZONE_BEYOND: f32 = 48.;

impl TileDrag {
    /// Where it lands with the pointer `at`, in pixels in the room, `room` big. How far in a
    /// side's zone reaches and how tall a tile's bar's row is are in pixels.
    fn landing_under(
        &self,
        at: (f32, f32),
        room: gpui::Size<Pixels>,
        (edge, right, bar): (f32, f32, f32),
    ) -> Option<Landing> {
        let (room_width, room_height) = (
            f32::from(room.width).max(1.),
            f32::from(room.height).max(1.),
        );
        let pointer = (at.0 / room_width, at.1 / room_height);
        let zones = Zones {
            edge: (edge / room_width, edge / room_height),
            right: right / room_width,
            bar: bar / room_height,
        };
        (self.before).landing_at(&self.tile, pointer, zones)
    }

    /// Where the pointer lands it where, over the room and a little past it, by asking
    /// [`Self::landing_under`] every [`ZONE_STEP`] pixels: boxes of the same landing, a row's
    /// run at a time, run on down while the rows below match.
    fn zones(&self, room: gpui::Size<Pixels>, sizes: (f32, f32, f32)) -> Vec<DropZone> {
        let (width, height) = (f32::from(room.width), f32::from(room.height));
        let steps = |length: f32| ((length + 2. * ZONE_BEYOND) / ZONE_STEP).ceil() as usize;
        let mut zones: Vec<DropZone> = Vec::new();
        // The zones still open at the row above, by their run's left and right.
        let mut open: Vec<usize> = Vec::new();
        for row in 0..steps(height) {
            let top = row as f32 * ZONE_STEP - ZONE_BEYOND;
            let mut runs: Vec<(Option<Landing>, f32, f32)> = Vec::new();
            for column in 0..steps(width) {
                let left = column as f32 * ZONE_STEP - ZONE_BEYOND;
                let middle = (left + ZONE_STEP / 2., top + ZONE_STEP / 2.);
                let landing = self.landing_under(middle, room, sizes);
                match runs.last_mut() {
                    Some((was, _, right)) if *was == landing => *right = left + ZONE_STEP,
                    _ => runs.push((landing, left, left + ZONE_STEP)),
                }
            }
            let mut still_open = Vec::new();
            for (landing, left, right) in runs {
                let above = (open.iter().copied()).find(|ix| {
                    zones.get(*ix).is_some_and(|zone| {
                        zone.landing == landing && zone.left == left && zone.right == right
                    })
                });
                match above.and_then(|ix| Some((ix, zones.get_mut(ix)?))) {
                    Some((ix, zone)) => {
                        zone.bottom = top + ZONE_STEP;
                        still_open.push(ix);
                    }
                    None => {
                        still_open.push(zones.len());
                        let bottom = top + ZONE_STEP;
                        zones.push(DropZone {
                            landing,
                            left,
                            top,
                            right,
                            bottom,
                        });
                    }
                }
            }
            open = still_open;
        }
        zones
    }

    /// What the tiles move to: as it would land while it's held, as it lands once it's let go.
    fn drawn_tiling(&self) -> &Tiling {
        match self.released {
            true => &self.landed,
            false => &self.shown,
        }
    }

    /// Where each tile is drawn at `now`, the dragged one last, over the others: under the
    /// pointer, as big as its room where it lands, until it's let go.
    fn areas(&self, now: Instant, room: gpui::Size<Pixels>) -> Vec<(Tile, Area)> {
        let reflow = self.reflow.openness(now).unwrap_or(1.);
        let to = self.drawn_tiling().areas();
        let from = |tile: &Tile| (self.from.iter()).find(|(other, _)| other == tile);
        let mut areas: Vec<(Tile, Area)> = (to.iter())
            .filter(|(tile, _)| *tile != self.tile)
            .map(|(tile, area)| {
                let was = from(tile).map_or(*area, |(_, was)| *was);
                (tile.clone(), was.toward(*area, reflow))
            })
            .collect();
        let Some(slot) = (to.iter())
            .find(|(tile, _)| *tile == self.tile)
            .map(|(_, a)| *a)
        else {
            return areas;
        };
        let dragged = match self.released {
            true => from(&self.tile)
                .map_or(slot, |(_, was)| *was)
                .toward(slot, reflow),
            false => {
                let resize = self.resize.openness(now).unwrap_or(1.);
                let (width, height) = (
                    self.size_from.0 + (slot.width - self.size_from.0) * resize,
                    self.size_from.1 + (slot.height - self.size_from.1) * resize,
                );
                let (room_width, room_height) = (
                    f32::from(room.width).max(1.),
                    f32::from(room.height).max(1.),
                );
                let down = self.grab.1.clamp(0., height * room_height);
                Area {
                    left: f32::from(self.pointer.x) / room_width - self.grab.0 * width,
                    top: (f32::from(self.pointer.y) - down) / room_height,
                    width,
                    height,
                }
            }
        };
        areas.push((self.tile.clone(), dragged));
        areas
    }
}

/// The least a tile is dragged down to, in design pixels.
const MIN_TILE: f32 = 160.;

/// How long a tile takes to slide open out of its split or shut into it: quicker than a fold,
/// as it's often a step on the way to typing in it.
const TILE_SLIDE: std::time::Duration = std::time::Duration::from_millis(110);

/// A motion `ms` long, set off now.
fn moving_for(ms: f32, cx: &App) -> Slide {
    let mut slide = Slide::lasting(millis(ms));
    slide.start(true, cx);
    slide
}

fn millis(ms: f32) -> Duration {
    Duration::from_secs_f32(ms.max(0.) / 1000.)
}

/// How big the handle over a junction is, a little past the gaps that meet there.
const JUNCTION_HANDLE: f32 = 16.;

/// Move `divider` to `pointer`, in the room the tiles share, `bounds` big.
fn move_divider(
    store: &Entity<Store>,
    divider: &Divider,
    bounds: Bounds<Pixels>,
    pointer: Point<Pixels>,
    cx: &mut App,
) {
    let (along, length, start, share) = match divider.axis {
        Axis::Across => (
            pointer.x - bounds.left(),
            bounds.size.width,
            divider.room.left,
            divider.room.width,
        ),
        Axis::Down => (
            pointer.y - bounds.top(),
            bounds.size.height,
            divider.room.top,
            divider.room.height,
        ),
    };
    let room = f32::from(length) * share;
    if room <= 0. {
        return;
    }
    let at = (f32::from(along) / f32::from(length) - start) / share;
    let min = f32::from(Theme::get(cx).scaled(MIN_TILE)) / room;
    store.update(cx, |store, cx| {
        store.move_divider(&divider.path, divider.after, at, min, cx)
    });
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
        // GPUI draws the window again whenever the keys move.
        self.note_tile_with_keys(window, cx);
        // After this frame, whose actions are the selection's now.
        if self.pending_menu.is_some() {
            let this = cx.entity().downgrade();
            window.on_next_frame(move |window, cx| {
                (this.update(cx, |this, cx| this.open_pending_menu(window, cx))).ok();
            });
        }
        let theme = Theme::get(cx).clone();
        let ligatures = self.store.read(cx).prefs.ligatures;
        let show_fps = self.store.read(cx).prefs.show_fps;
        // The detail lays out its header and its file list beside as much of the navigator as
        // shows, so it's told before it draws in this frame.
        let now = clock::now();
        let navigator_share = self.navigator_slide.share(self.navigator_open, now);
        let navigator_sliding = self.navigator_slide.openness(now).is_some();
        // Pinned pages fit their file lists to the same room.
        for detail in std::iter::once(&self.detail).chain(self.pages.values().map(|p| &p.detail)) {
            let detail = detail.read(cx);
            detail.navigator_shown.set(navigator_share);
            (detail.window_width).set(self.narrowing.map(|narrowing| narrowing.wide));
        }
        // Tiles sliding open or shut, or dragged, move on before the next frame.
        let tile_sliding = self.tiles_moving;
        if tile_sliding {
            let this = cx.entity().downgrade();
            window.on_next_frame(move |_, cx| {
                (this.update(cx, |this, cx| this.move_tiles(cx))).ok();
            });
        }
        let detail = self.detail.read(cx);
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
            .on_modifiers_changed(cx.listener(|this, event: &ModifiersChangedEvent, _, cx| {
                if cx.has_active_drag() || this.store.read(cx).cut.is_some() {
                    this.navigator.update(cx, |_, cx| cx.notify());
                }
                // In a build of its own, ⌥ shows where a tile dragged lands where, as the right
                // button does.
                if backend::settings::dev_build() {
                    this.option_changed(event.modifiers.alt, cx);
                }
            }))
            // GPUI counts nothing as under the pointer while the last input was a key, and a
            // drag in from another app, as files from the Finder onto a terminal, comes as moves
            // it doesn't count as the mouse's: after typing, it would land nowhere. Sent again
            // as the mouse's move, the drag counts.
            .on_drag_move(|event: &DragMoveEvent<ExternalPaths>, window, cx| {
                if window.last_input_was_keyboard() {
                    let event = event.event.clone();
                    window.defer(cx, move |window, cx| {
                        window.dispatch_event(PlatformInput::MouseMove(event), cx);
                    });
                }
            })
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
            .map(|el| self.copy_actions(el, cx))
            .map(|el| self.navigator_actions(el, cx))
            .map(|el| self.project_list_actions(el, cx))
            .map(|el| self.resolve_actions(el, cx))
            .on_action(cx.listener(Self::go_to))
            .on_action(cx.listener(Self::go_to_terminal))
            .on_action(cx.listener(Self::show_context_menu))
            .on_action(cx.listener(Self::copy_more))
            .on_action(cx.listener(Self::open_selected_in_terminal))
            .on_action(cx.listener(Self::new_terminal))
            .on_action(
                cx.listener(|this, _: &SplitRight, window, cx| {
                    this.split(Axis::Across, window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &SplitBelow, window, cx| this.split(Axis::Down, window, cx)),
            )
            .when(self.store.read(cx).tiling().is_some(), |el| {
                el.on_action(
                    cx.listener(|this, _: &ZoomTile, window, cx| this.zoom_with_keys(window, cx)),
                )
                .on_action(cx.listener(|this, _: &EqualizeTiles, _, cx| this.equalize_tiles(cx)))
                .on_action(cx.listener(|this, _: &NextTile, window, cx| {
                    this.focus_next_tile(false, window, cx)
                }))
                .on_action(cx.listener(|this, _: &PreviousTile, window, cx| {
                    this.focus_next_tile(true, window, cx)
                }))
                // Only in a build of its own, to show how tiles are laid out in a report.
                .when(backend::settings::dev_build(), |el| {
                    el.on_action(
                        cx.listener(|this, _: &CopyTileDiagram, _, cx| this.copy_tile_diagram(cx)),
                    )
                })
            })
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
            // Where the diff shows what's selected, which can squash.
            .when(
                matches!(
                    self.store.read(cx).selection,
                    Some(Selection::Commit(..) | Selection::Uncommitted(..))
                ),
                |el| el.on_action(cx.listener(Self::squash_from_detail)),
            )
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
                    store.set_prefs(
                        |prefs| {
                            prefs.file_view = match prefs.file_view {
                                FileView::Tree => FileView::List,
                                FileView::List => FileView::Tree,
                            }
                        },
                        cx,
                    )
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
            .on_action(cx.listener(|this, _: &NextProjectPage, window, cx| {
                this.step_project_page(true, window, cx)
            }))
            .on_action(cx.listener(|this, _: &PreviousProjectPage, window, cx| {
                this.step_project_page(false, window, cx)
            }))
            .on_action(cx.listener(Self::close_window))
            .on_action(cx.listener(Self::start_commit))
            .on_action(cx.listener(Self::start_amend))
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
                    .on_drag_move(resize_pane(
                        Pane::Navigator,
                        TILE_MARGIN,
                        self.store.clone(),
                    ))
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
            .when(self.status_bar_shown(cx), |el| {
                el.child(self.render_status_bar(window, cx))
            })
            .children(self.render_modal(cx).map(overlay))
            // Above a modal: Git can ask for credentials while one is open.
            .children(self.render_prompt(window, cx).map(overlay))
            .children(self.render_palette(cx).map(overlay))
            // Above what dims the window, so their buttons take clicks rather than close it.
            .child(overlay(self.render_toasts(cx)))
            .children(self.render_menu(window, cx))
            .children(self.render_icon_picker(window, cx))
            // Last, so the window can be closed while a modal or menu is open.
            .when(crate::ui::draws_window_controls(window), |el| {
                el.child(overlay(crate::ui::window_controls::window_controls(
                    window, cx,
                )))
            })
            // Over everything, and it takes no clicks, as it has no hitbox.
            .when(show_fps, |el| el.child(self.fps.clone()))
            .when(cfg!(target_os = "macos"), |el| {
                el.child(crate::ui::holds_back_first_click())
            });
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

/// Keep a drag going through a click of the right or middle button, as showing where a row
/// lands and taking a picture of it are, in a build of its own: GPUI ends a drag as any button
/// comes up, so AppKit keeps those releases from it while the left one's down.
#[cfg(target_os = "macos")]
fn keep_drags_through_other_buttons() {
    use block2::RcBlock;
    use objc2::{class, msg_send, runtime::AnyObject};

    // `NSEventMaskRightMouseUp` and `NSEventMaskOtherMouseUp`.
    const RELEASES: u64 = (1 << 4) | (1 << 26);
    let keep = RcBlock::new(|event: *mut AnyObject| -> *mut AnyObject {
        // SAFETY: `pressedMouseButtons` only reads which buttons are down.
        let pressed: usize = unsafe { msg_send![class!(NSEvent), pressedMouseButtons] };
        match pressed & 1 {
            0 => event,
            _ => std::ptr::null_mut(),
        }
    });
    // SAFETY: AppKit copies the block, and calls it on the main thread with each event it's
    // about to send, as this method's contract says; it keeps the monitor for the app's life.
    unsafe {
        let _: *mut AnyObject = msg_send![
            class!(NSEvent),
            addLocalMonitorForEventsMatchingMask: RELEASES,
            handler: &*keep
        ];
    }
}

/// The number AppKit gives `window`, which `screencapture` takes.
#[cfg(target_os = "macos")]
fn window_number(window: &Window) -> Option<isize> {
    use objc2::{msg_send, runtime::AnyObject};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    let view = handle.ns_view.as_ptr().cast::<AnyObject>();
    // SAFETY: `view` is the window's live NSView, used on the main thread, and `windowNumber`
    // only reads its window's number.
    unsafe {
        let ns_window: *mut AnyObject = msg_send![view, window];
        (!ns_window.is_null()).then(|| msg_send![ns_window, windowNumber])
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
