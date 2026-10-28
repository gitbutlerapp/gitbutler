//! Settings: one large modal laid out as Lite's, its pages down the left, the app's and then
//! the project's, each a column of cards, with a search across all of them. Changes apply
//! as they're made. What only this app uses is kept with its other
//! state; fetching is GitButler's shared setting; a project's settings are its own.

use std::collections::HashMap;

use gpui::{
    AnyElement, App, ClickEvent, Context, DismissEvent, Entity, EventEmitter, FocusHandle,
    Focusable, ScrollHandle, SharedString, Subscription, Task, Window, actions, div, prelude::*,
};

mod accounts;
mod ai;
mod cli;
mod keyboard;
mod mesh;
mod metrics;
mod project;

use crate::app::{AppHandle, AppView, SelectNext, SelectPrevious};
use crate::backend::settings::{self as backend, Program, ProjectSettings};
use crate::overlays::MenuItem;
use crate::state::{Prefs, ProjectEntry, ProjectId, ScopeLines, Store};
use crate::theme::{Appearance, CodeSize, Theme, UiScale};
use crate::ui::modal::{MODAL_RADIUS, ModalSize, modal};
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    self, ButtonStyle, Icon, Switch, button, dp, dropdown, error_summary, field, h_flex, icon,
    muted, side_row, small_icon, switch, v_flex,
};

pub use cli::welcome_card;

actions!(settings, [OpenSettings, OpenKeyboardShortcuts]);

/// " ⌥Z in a diff turns it on and off.", with the keys bound to `action`, or nothing when no key
/// is, as the user's keymap can leave it.
fn keys_in_diff(action: &dyn gpui::Action, does: &str, cx: &App) -> String {
    crate::ui::shortcut(action, cx)
        .map_or_else(String::new, |keys| format!(" {keys} in a diff {does}."))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppPage {
    General,
    Appearance,
    Keyboard,
    Ai,
    Git,
    Integrations,
    Experimental,
    Developer,
}

impl AppPage {
    const ALL: [AppPage; 8] = [
        AppPage::General,
        AppPage::Appearance,
        AppPage::Keyboard,
        AppPage::Ai,
        AppPage::Git,
        AppPage::Integrations,
        AppPage::Experimental,
        AppPage::Developer,
    ];

    /// The pages this build has: Developer only in a build of its own, not a nightly or a
    /// release.
    fn shown() -> impl Iterator<Item = AppPage> {
        let dev = backend::dev_build();
        (AppPage::ALL.into_iter()).filter(move |page| dev || *page != AppPage::Developer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectPage {
    Project,
    Git,
}

impl ProjectPage {
    const ALL: [ProjectPage; 2] = [ProjectPage::Project, ProjectPage::Git];
}

/// A page of settings, in the sidebar's order: the app's, then each project's.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Page {
    App(AppPage),
    Project(ProjectId, ProjectPage),
}

impl Page {
    fn project(&self) -> Option<&ProjectId> {
        match self {
            Page::App(_) => None,
            Page::Project(project, _) => Some(project),
        }
    }

    fn title(&self) -> &'static str {
        match self {
            Page::App(AppPage::General) => "General",
            Page::App(AppPage::Appearance) => "Appearance",
            Page::App(AppPage::Keyboard) => "Keyboard",
            Page::App(AppPage::Ai) => "AI",
            Page::App(AppPage::Git) | Page::Project(_, ProjectPage::Git) => "Git",
            Page::App(AppPage::Integrations) => "Integrations",
            Page::App(AppPage::Experimental) => "Experimental",
            Page::App(AppPage::Developer) => "Developer",
            Page::Project(_, ProjectPage::Project) => "Project",
        }
    }

    fn icon(&self) -> Icon {
        match self {
            Page::App(AppPage::General) => Icon::Settings,
            Page::App(AppPage::Appearance) => Icon::Contrast,
            Page::App(AppPage::Keyboard) => Icon::Keyboard,
            Page::App(AppPage::Ai) => Icon::Ai,
            Page::App(AppPage::Git) | Page::Project(_, ProjectPage::Git) => Icon::Pr,
            Page::App(AppPage::Integrations) => Icon::Plug,
            Page::App(AppPage::Experimental) => Icon::Lab,
            Page::App(AppPage::Developer) => Icon::TextCode,
            Page::Project(_, ProjectPage::Project) => Icon::Mixer,
        }
    }
}

/// A page's link in the sidebar, which the focus moves along.
#[derive(Clone)]
struct Link {
    page: Page,
    focus: FocusHandle,
}

impl Link {
    fn new(page: Page, cx: &mut App) -> Self {
        Link {
            page,
            focus: cx.focus_handle().tab_stop(true),
        }
    }
}

/// The projects the sidebar lists, each group by name: the watched ones, then those not
/// watched, all of them while they're shown, else only the one shown, and how many that
/// leaves out. A search lists every project its words are all in the name of.
struct Listed {
    watched: Vec<ProjectId>,
    unwatched: Vec<ProjectId>,
    hidden: usize,
}

impl Listed {
    fn iter(&self) -> impl Iterator<Item = &ProjectId> {
        self.watched.iter().chain(&self.unwatched)
    }
}

/// A card of rows, one setting each, under a heading unless it's the page's first.
struct Card {
    heading: Option<SharedString>,
    rows: Vec<Row>,
}

impl Card {
    fn new(heading: Option<&'static str>, rows: Vec<Row>) -> Self {
        Card {
            heading: heading.map(SharedString::from),
            rows,
        }
    }
}

/// The hover group of a row, for a control shown only while the pointer is on it.
const ROW_GROUP: &str = "settings-row";

/// One setting: what it's called, what it does, and the control that changes it.
struct Row {
    title: SharedString,
    description: SharedString,
    control: AnyElement,
    /// What shows under its description, as a command to copy.
    below: Option<AnyElement>,
    /// More that a search finds it by, as a command's keys.
    keywords: SharedString,
}

impl Row {
    fn new(
        title: impl Into<SharedString>,
        description: impl Into<SharedString>,
        control: impl IntoElement,
    ) -> Self {
        Row {
            title: title.into(),
            description: description.into(),
            control: control.into_any_element(),
            below: None,
            keywords: SharedString::default(),
        }
    }

    fn with_below(mut self, below: impl IntoElement) -> Self {
        self.below = Some(below.into_any_element());
        self
    }

    fn with_keywords(mut self, keywords: impl Into<SharedString>) -> Self {
        self.keywords = keywords.into();
        self
    }
}

/// How often background fetching can run, in minutes; `None` is off.
const FETCH_INTERVALS: [Option<u64>; 5] = [None, Some(5), Some(15), Some(30), Some(60)];

fn interval_label(minutes: Option<u64>) -> String {
    match minutes {
        None => "Off".into(),
        Some(60) => "Every hour".into(),
        Some(minutes) => format!("Every {minutes} minutes"),
    }
}

pub struct SettingsView {
    store: Entity<Store>,
    app: AppHandle,
    /// The diff's settings change through it, as its own toggles do.
    detail: Entity<crate::detail::Detail>,
    search: Entity<TextInput>,
    page: Page,
    /// The shown project's settings as last read, or why they couldn't be; `None` while first
    /// read.
    project_settings: Option<Result<ProjectSettings, SharedString>>,
    name: Entity<TextInput>,
    editors: Vec<Program>,
    terminals: Vec<Program>,
    fetch_interval: Option<u64>,
    /// The desktop app opens this client at start: GitButler's `featureFlags.gpui`. `None`
    /// while first read, as the metrics are, and their switches aren't drawn until then: one
    /// drawn off first would slide over to what's read.
    open_at_start: Option<bool>,
    /// GitButler's apps send usage metrics, and error reports.
    usage_metrics: Option<bool>,
    error_reporting: Option<bool>,
    accounts: accounts::Accounts,
    ai: ai::Ai,
    /// How the last signing test went, while it applies to the project shown.
    signing: Option<Result<(), SharedString>>,
    /// How fetching and then pushing went in the last credentials test of the project shown,
    /// each `None` while it runs.
    credentials: Option<[Option<Result<(), SharedString>>; 2]>,
    /// The credentials test running: a new test or another project's settings drop it, so only
    /// its own results show.
    _credentials_test: Task<()>,
    recording: Option<keyboard::Recording>,
    app_links: Vec<Link>,
    /// Each listed project's row, and its Project page's link while it's shown.
    project_focus: HashMap<ProjectId, FocusHandle>,
    /// The shown project's Git page's link.
    project_git_focus: FocusHandle,
    /// The sidebar lists the projects not watched too.
    more: bool,
    pages_scroll: ScrollHandle,
    cards_scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
    _load: Task<()>,
}

impl EventEmitter<DismissEvent> for SettingsView {}

impl Focusable for SettingsView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl SettingsView {
    fn new(
        store: Entity<Store>,
        app: AppHandle,
        detail: Entity<crate::detail::Detail>,
        page: Page,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Search settings", false, cx));
        let name = cx.new(|cx| TextInput::new("Project name", false, cx));
        let ai = ai::Ai::new(window, cx);
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, search, event, _, cx| match event {
                InputEvent::Changed => {
                    // A search may match the AI page's rows, which load when needed.
                    if !search.read(cx).text().trim().is_empty() {
                        this.ensure_ai(cx);
                    }
                    cx.notify()
                }
                InputEvent::Cancel if search.read(cx).text().is_empty() => cx.emit(DismissEvent),
                InputEvent::Cancel => search.update(cx, |search, cx| search.set_text("", cx)),
                InputEvent::Submit => {}
            }),
            // The target and the project's name show what the store has.
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe_global::<crate::keymap::UserKeymap>(|_, cx| cx.notify()),
            cx.subscribe_in(&name, window, |this, _, event, window, cx| match event {
                InputEvent::Submit => this.save_name(cx),
                InputEvent::Changed => cx.notify(),
                InputEvent::Cancel => {
                    this.reset_name(cx);
                    this.back_to_search(window, cx);
                }
            }),
        ];
        let unwatched = (page
            .project()
            .and_then(|project| store.read(cx).project(project)))
        .is_some_and(|entry| !entry.expanded);
        let mut view = Self {
            store,
            app,
            detail,
            search,
            page: Page::App(AppPage::General),
            project_settings: None,
            name,
            editors: Vec::new(),
            terminals: Vec::new(),
            fetch_interval: None,
            open_at_start: None,
            usage_metrics: None,
            error_reporting: None,
            accounts: Default::default(),
            ai,
            signing: None,
            credentials: None,
            _credentials_test: Task::ready(()),
            recording: None,
            app_links: (AppPage::shown())
                .map(|page| Link::new(Page::App(page), cx))
                .collect(),
            project_focus: HashMap::new(),
            project_git_focus: cx.focus_handle().tab_stop(true),
            more: unwatched,
            pages_scroll: ScrollHandle::new(),
            cards_scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
            _load: Task::ready(()),
        };
        view.show_page(page, cx);
        view.load_accounts(cx);
        // A terminal may have installed `but` meanwhile.
        (view.store).update(cx, |store, cx| store.read_cli(cx));
        view.load_app_settings(cx);
        view
    }

    /// Read the app's settings shown here, as saved, which a change that failed left as
    /// they were.
    fn load_app_settings(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let loaded = crate::backend::run(|| {
                Ok((
                    backend::editors(),
                    backend::terminals(),
                    backend::fetch_interval(),
                    backend::opens_at_start(),
                    crate::backend::metrics::enabled(),
                ))
            })
            .await;
            if let Ok((editors, terminals, fetch_interval, open_at_start, (usage, errors))) = loaded
            {
                this.update(cx, |this, cx| {
                    this.editors = editors;
                    this.terminals = terminals;
                    this.fetch_interval = fetch_interval;
                    this.open_at_start = Some(open_at_start);
                    this.usage_metrics = Some(usage);
                    this.error_reporting = Some(errors);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn error(&self, title: &str, err: anyhow::Error, cx: &mut App) {
        self.store
            .update(cx, |store, cx| store.failed(title.to_owned(), &err, cx));
    }

    /// Run `op` in the background, report it if it fails, then `reload` what it changed so
    /// the dialog shows the result.
    fn apply(
        &mut self,
        failed: &'static str,
        op: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
        reload: fn(&mut Self, &mut Context<Self>),
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = crate::backend::run(op).await;
            this.update(cx, |this, cx| {
                if let Err(err) = result {
                    this.error(failed, err, cx);
                }
                reload(this, cx);
            })
            .ok();
        })
        .detach();
    }

    /// The menu `items` makes from this view, open where `event` clicked once the update
    /// running now is over.
    fn open_menu(
        &self,
        event: &ClickEvent,
        items: impl FnOnce(&mut Self, &mut Context<Self>) -> Vec<MenuItem> + 'static,
        cx: &mut Context<Self>,
    ) {
        (self.app).open_menu(&cx.entity().downgrade(), event.position(), items, cx);
    }

    /// A dropdown showing `label`, whose menu sets one of `options` through `set` and ticks
    /// the one `label` names.
    fn choice<T: Clone + 'static>(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = (T, String)>,
        set: impl Fn(&mut Self, T, &mut Window, &mut Context<Self>) + Clone + 'static,
        cx: &Context<Self>,
    ) -> AnyElement {
        let label = label.into();
        let this = cx.entity().downgrade();
        let items: Vec<MenuItem> = options
            .into_iter()
            .map(|(value, option)| {
                let (this, set) = (this.clone(), set.clone());
                let ticked = option == *label;
                MenuItem::new(option, move |_, window, cx| {
                    (this.update(cx, |this, cx| set(this, value.clone(), window, cx))).ok();
                })
                .ticked(ticked)
            })
            .collect();
        dropdown(id, label, Theme::get(cx))
            .on_click(move |event, _, cx| {
                let items = items.clone();
                (this.update(cx, |this, cx| this.open_menu(event, move |_, _| items, cx))).ok();
            })
            .into_any_element()
    }

    /// Where files and the project open.
    fn program_rows(&self, cx: &Context<Self>) -> Vec<Row> {
        let prefs = &self.store.read(cx).prefs;
        let name_of = |programs: &[Program], id: Option<&String>, unset: &str| {
            id.and_then(|id| programs.iter().find(|(p, _)| p == id))
                .map_or_else(|| unset.to_owned(), |(_, name)| name.clone())
        };
        let editor = name_of(&self.editors, prefs.editor.as_ref(), "Ask the first time");
        let terminal = name_of(&self.terminals, prefs.terminal.as_ref(), "Terminal");
        let editor = self.choice(
            "editor",
            editor,
            (self.editors.iter()).map(|(id, name)| (id.clone(), name.clone())),
            |this, id: String, _, cx| {
                this.store
                    .update(cx, |store, cx| store.set_prefs(|p| p.editor = Some(id), cx));
            },
            cx,
        );
        let terminal = self.choice(
            "terminal",
            terminal,
            (self.terminals.iter()).map(|(id, name)| (id.clone(), name.clone())),
            |this, id: String, _, cx| {
                this.store.update(cx, |store, cx| {
                    store.set_prefs(|p| p.terminal = Some(id), cx)
                });
            },
            cx,
        );
        vec![
            Row::new(
                "Editor",
                "Where \"Open in editor\" opens files, at the line you're on.",
                editor,
            ),
            Row::new(
                "Terminal",
                "Where \"Open in terminal\" opens the project.",
                terminal,
            ),
        ]
    }

    /// The theme, then how diffs show.
    fn appearance_cards(&self, cx: &Context<Self>) -> Vec<Card> {
        let theme = Theme::get(cx);
        let prefs = &self.store.read(cx).prefs;
        let (store, app) = (self.store.clone(), self.app.clone());
        let appearance = segmented(
            "appearance",
            [
                (Appearance::System, "System"),
                (Appearance::Light, "Light"),
                (Appearance::Dark, "Dark"),
            ],
            prefs.appearance,
            move |choice, window, cx| {
                store.update(cx, |store, cx| {
                    store.set_prefs(|prefs| prefs.appearance = choice, cx)
                });
                app.update_later(window, cx, |app, window, cx| {
                    app.apply_appearance(window, cx)
                });
            },
            theme,
        );
        let detail = self.detail.clone();
        let code = segmented(
            "code",
            [
                (CodeSize::Small, "Small"),
                (CodeSize::Medium, "Medium"),
                (CodeSize::Large, "Large"),
            ],
            prefs.code_size,
            move |size, _, cx| {
                detail.update(cx, |detail, cx| detail.set_code_size(size, cx));
            },
            theme,
        );
        let appearance = Row::new(
            "Appearance",
            "Light, dark, or whichever macOS is set to.",
            appearance,
        );
        let percent = |scale: UiScale| format!("{}%", scale.percent());
        let size = self.choice(
            "ui-scale",
            percent(prefs.ui_scale),
            UiScale::STEPS.map(|scale| (scale, percent(scale))),
            |this, scale, window, cx| {
                (this.app).update_later(window, cx, move |app, _, cx| app.set_ui_scale(scale, cx))
            },
            cx,
        );
        let size = Row::new(
            "Interface size",
            crate::ui::keys_text(
                "How large everything draws, as a browser zooms. ⌘+ and ⌘− step it, ⌘0 puts it back.",
            ),
            size,
        );
        let mut diff = vec![
            Row::new("Code size", "How large code shows in diffs.", code),
            Row::new(
                "Font ligatures",
                "Code joins pairs of characters like != and == into one symbol.",
                self.pref_switch("ligatures", prefs.ligatures, |p| p.ligatures ^= true, cx)
                    // The diff is a cached view, which only a refresh draws again.
                    .on_click(|_, _, cx| cx.refresh_windows()),
            ),
            Row::new(
                "Wrap long lines",
                crate::ui::keys_text(&format!(
                    "Diff lines wider than the pane continue on the next row.{}",
                    keys_in_diff(&crate::detail::ToggleWrap, "turns it on and off", cx)
                )),
                switch("wrap", prefs.wrap, theme).on_click(cx.listener(|this, _, _, cx| {
                    this.detail.update(cx, |detail, cx| detail.toggle_wrap(cx));
                })),
            ),
            Row::new(
                "Side by side",
                crate::ui::keys_text(&format!(
                    "Diffs show the old lines beside the new ones.{}",
                    keys_in_diff(&crate::detail::ToggleSplit, "switches", cx)
                )),
                switch("split", prefs.split, theme).on_click(cx.listener(|this, _, _, cx| {
                    this.detail.update(cx, |detail, cx| detail.toggle_split(cx));
                })),
            ),
            Row::new(
                "Semantic context",
                "Each hunk's box names the function or type it changes, and a short one shows \
                 whole around it. deltoids reads it from the code with tree-sitter, for Rust, \
                 TypeScript, JavaScript, Python, Go and 15 other languages, though not Svelte. \
                 Off, diffs skip reading it, and load a little faster.",
                switch("semantic", !prefs.no_semantic_context, theme).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.detail.update(cx, |detail, cx| {
                            detail
                                .set_semantic_context(|prefs| prefs.no_semantic_context ^= true, cx)
                        });
                    },
                )),
            ),
        ];
        if !prefs.no_semantic_context {
            let detail = self.detail.clone();
            let scope = segmented(
                "scope",
                [
                    (ScopeLines::Short, "20 lines"),
                    (ScopeLines::Medium, "50 lines"),
                    (ScopeLines::Long, "200 lines"),
                ],
                prefs.whole_scopes,
                move |lines, _, cx| {
                    detail.update(cx, |detail, cx| {
                        detail.set_semantic_context(|prefs| prefs.whole_scopes = lines, cx)
                    });
                },
                theme,
            );
            diff.push(Row::new(
                "Whole scopes up to",
                "The most unchanged lines a function or type may add around its hunks to show \
                 whole. A longer one keeps three lines of context.",
                scope,
            ));
        }
        let navigator = vec![
            Row::new(
                "Show branches' commits",
                "Branches in the navigator start with their commits listed. Off, each starts \
                 folded to one row, which ⇧→ or its chevron opens.",
                self.pref_switch(
                    "branches-open",
                    prefs.branches_start_open,
                    |p| p.branches_start_open ^= true,
                    cx,
                ),
            ),
            Row::new(
                "Show all of a branch's commits",
                "An open branch in the navigator lists every commit it has. Off, it lists the \
                 first eight, then how many more, which a click lists too.",
                self.pref_switch(
                    "all-branch-commits",
                    prefs.all_branch_commits,
                    |p| p.all_branch_commits ^= true,
                    cx,
                ),
            ),
            Row::new(
                "Open branches on click",
                "Clicking a branch in the navigator lists its commits too, and folds the ones the \
                 last click listed. Off, a click only selects it.",
                self.pref_switch(
                    "branch-click-opens",
                    prefs.open_branch_on_click,
                    |p| p.open_branch_on_click ^= true,
                    cx,
                ),
            ),
        ];
        vec![
            Card::new(None, vec![appearance, size]),
            Card::new(Some("Navigator"), navigator),
            Card::new(Some("Diff"), diff),
        ]
    }

    fn fetch_row(&self, cx: &Context<Self>) -> Row {
        let control = self.choice(
            "fetch",
            interval_label(self.fetch_interval),
            FETCH_INTERVALS.map(|minutes| (minutes, interval_label(minutes))),
            |this, minutes: Option<u64>, _, cx| {
                this.fetch_interval = minutes;
                cx.notify();
                this.apply(
                    "Couldn't change how often to fetch",
                    move || backend::set_fetch_interval(minutes),
                    Self::load_app_settings,
                    cx,
                );
            },
            cx,
        );
        Row::new(
            "Fetch automatically",
            "Keeps the target and each branch's remote current, for every expanded project. \
             GitButler's other apps share this.",
            control,
        )
    }

    /// A switch for one of this app's own preferences, which a click turns over with `flip`.
    fn pref_switch(
        &self,
        id: &'static str,
        on: bool,
        flip: fn(&mut Prefs),
        cx: &Context<Self>,
    ) -> Switch {
        switch(id, on, Theme::get(cx)).on_click(cx.listener(move |this, _, _, cx| {
            this.store.update(cx, |store, cx| store.set_prefs(flip, cx))
        }))
    }

    /// A switch's click for one of GitButler's settings, which this view holds in `value` once
    /// read: it shows the change at once, saves it with `save`, then `reload`s what was saved.
    fn toggle_saved(
        value: fn(&mut Self) -> Option<&mut bool>,
        failed: &'static str,
        save: fn(bool) -> anyhow::Result<()>,
        reload: fn(&mut Self, &mut Context<Self>),
        cx: &Context<Self>,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        cx.listener(move |this, _, _, cx| {
            // Its switch is drawn only once it's read.
            let Some(value) = value(this) else { return };
            *value ^= true;
            let on = *value;
            cx.notify();
            this.apply(failed, move || save(on), reload, cx);
        })
    }

    /// Nothing until it's read.
    fn start_row(&self, cx: &Context<Self>) -> Option<Row> {
        let start = switch("open-at-start", self.open_at_start?, Theme::get(cx)).on_click(
            Self::toggle_saved(
                |this| this.open_at_start.as_mut(),
                "Couldn't change what opens at start",
                backend::set_opens_at_start,
                Self::load_app_settings,
                cx,
            ),
        );
        Some(Row::new(
            "Open this app at start",
            "GitButler opens this app rather than its web app when it starts, as \
             featureFlags.gpui in its settings.json says. Starting it with --gpui opens this app \
             either way.",
            start,
        ))
    }

    fn fps_row(&self, cx: &Context<Self>) -> Row {
        let on = self.store.read(cx).prefs.show_fps;
        Row::new(
            "Show FPS",
            "The frames drawn in the last second, in the window's top right corner. When a \
             frame of the last three seconds was much slower, its pace shows beside them in \
             amber. While it shows, the window draws every frame it can, so a slow view brings \
             them down.",
            self.pref_switch("show-fps", on, |p| p.show_fps ^= true, cx),
        )
        .with_keywords("frames per second performance")
    }

    /// Escape in a field puts it back and goes to the search box, where Escape closes.
    fn back_to_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.search.focus_handle(cx), cx);
    }

    fn page_cards(&self, page: &Page, window: &Window, cx: &Context<Self>) -> Vec<Card> {
        let cards = match page {
            Page::App(AppPage::General) => vec![
                Card::new(
                    None,
                    vec![self.gitbutler_row(window, cx), self.mesh_row(cx)],
                ),
                Card::new(None, self.program_rows(cx)),
                // Only once it's been looked for, which only macOS does.
                Card::new(None, self.cli_row(cx).into_iter().collect()),
                // Last, out of the way, as the desktop app keeps it on a page of its own.
                Card::new(Some("Metrics"), self.metrics_rows(cx)),
            ],
            Page::App(AppPage::Appearance) => self.appearance_cards(cx),
            Page::App(AppPage::Keyboard) => self.keyboard_cards(cx),
            Page::App(AppPage::Ai) => vec![Card::new(None, self.ai_rows(window, cx))],
            Page::App(AppPage::Git) => vec![Card::new(None, vec![self.fetch_row(cx)])],
            Page::App(AppPage::Integrations) => self.integration_cards(window, cx),
            Page::App(AppPage::Experimental) => {
                // Only where the desktop app's binary has this client built in.
                let start = cx
                    .has_global::<crate::Embedded>()
                    .then(|| self.start_row(cx))
                    .flatten();
                vec![Card::new(None, start.into_iter().collect())]
            }
            Page::App(AppPage::Developer) => vec![Card::new(None, vec![self.fps_row(cx)])],
            Page::Project(project, page) => self.project_cards(project, *page, window, cx),
        };
        // Nothing for a card with no rows yet, as before what it shows has loaded.
        cards
            .into_iter()
            .filter(|card| !card.rows.is_empty())
            .collect()
    }

    fn listed_projects(&self, cx: &App) -> Listed {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let mut projects: Vec<(&ProjectEntry, String)> = (self.store.read(cx).projects.iter())
            .map(|entry| (entry, entry.project.name.to_lowercase()))
            .filter(|(_, name)| query.split_whitespace().all(|word| name.contains(word)))
            .collect();
        projects.sort_by(|(_, a), (_, b)| a.cmp(b));
        let (watched, unwatched): (Vec<_>, Vec<_>) =
            (projects.into_iter().map(|(entry, _)| entry)).partition(|entry| entry.expanded);
        let all = unwatched.len();
        let unwatched: Vec<ProjectId> = (unwatched.into_iter())
            .filter(|entry| {
                self.more || !query.is_empty() || self.page.project() == Some(&entry.id)
            })
            .map(|entry| entry.id.clone())
            .collect();
        Listed {
            watched: watched.into_iter().map(|entry| entry.id.clone()).collect(),
            hidden: all - unwatched.len(),
            unwatched,
        }
    }

    /// The links in the sidebar, in its order: the app's pages, then each listed project's
    /// row, which is its Project page's link, and the shown project's Git page.
    fn links(&self, cx: &App) -> Vec<Link> {
        let listed = self.listed_projects(cx);
        let projects = listed.iter().cloned().flat_map(|project| {
            let row = (self.project_focus.get(&project)).map(|focus| Link {
                page: Page::Project(project.clone(), ProjectPage::Project),
                focus: focus.clone(),
            });
            let git = (self.page.project() == Some(&project)).then(|| Link {
                page: Page::Project(project, ProjectPage::Git),
                focus: self.project_git_focus.clone(),
            });
            row.into_iter().chain(git)
        });
        self.app_links.iter().cloned().chain(projects).collect()
    }

    /// The pages a search looks through: the app's, and the shown project's, the only one
    /// whose settings are read.
    fn searched_pages(&self) -> Vec<Page> {
        let project = (self.page.project().cloned())
            .into_iter()
            .flat_map(|project| ProjectPage::ALL.map(|page| Page::Project(project.clone(), page)));
        AppPage::shown().map(Page::App).chain(project).collect()
    }

    /// What a page is called where it isn't under its group's heading: a project's by the
    /// project too.
    fn page_name(&self, page: &Page, cx: &App) -> String {
        let project = page
            .project()
            .and_then(|id| self.store.read(cx).project(id));
        match project {
            Some(entry) => format!("{} › {}", entry.project.name, page.title()),
            None => page.title().to_owned(),
        }
    }

    /// The cards to show: the current page's, or with a `query` every row it matches, in
    /// cards named by their page.
    fn cards(&self, query: &str, window: &Window, cx: &Context<Self>) -> Vec<Card> {
        if query.is_empty() {
            return self.page_cards(&self.page, window, cx);
        }
        (self.searched_pages().into_iter())
            .flat_map(|page| {
                let name = self.page_name(&page, cx);
                let cards = self.page_cards(&page, window, cx);
                cards.into_iter().map(move |card| (name.clone(), card))
            })
            .filter_map(|(name, card)| {
                let heading = match &card.heading {
                    Some(heading) => format!("{name} › {heading}"),
                    None => name,
                };
                let rows: Vec<Row> = (card.rows.into_iter())
                    .filter(|row| {
                        let text = format!(
                            "{heading} {} {} {}",
                            row.title, row.description, row.keywords
                        );
                        let text = text.to_lowercase();
                        query.split_whitespace().all(|word| text.contains(word))
                    })
                    .collect();
                (!rows.is_empty()).then(|| Card {
                    heading: Some(heading.into()),
                    rows,
                })
            })
            .collect()
    }

    /// A card as Lite's settings draw one: its heading outside it, then its rows divided by
    /// hairlines.
    fn render_card(card: Card, theme: &Theme) -> AnyElement {
        v_flex()
            .gap(dp(8.))
            .children(card.heading.map(|heading| {
                div()
                    .pl(dp(16.))
                    .text_size(dp(12.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.text_3)
                    .child(heading)
            }))
            .child(
                v_flex()
                    .rounded(dp(8.))
                    .border_1()
                    .border_color(theme.border_3)
                    .bg(theme.bg_1)
                    .overflow_hidden()
                    .children(
                        (card.rows.into_iter().enumerate())
                            .map(|(ix, row)| Self::render_row(row, ix > 0, theme)),
                    ),
            )
            .into_any_element()
    }

    /// One setting: its name over what it does, the control at the end; under a hairline
    /// unless it's the card's first.
    fn render_row(row: Row, divided: bool, theme: &Theme) -> AnyElement {
        let hint = !row.description.is_empty();
        h_flex()
            .group(ROW_GROUP)
            .p(dp(16.))
            .gap(dp(16.))
            // Against its name when there's a line under it, else on its line.
            .map(|el| {
                if hint {
                    el.items_start()
                } else {
                    el.items_center()
                }
            })
            .when(divided, |el| el.border_t_1().border_color(theme.border_3))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(dp(6.))
                    .child(
                        div()
                            .text_size(dp(15.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text_1)
                            .child(row.title),
                    )
                    .when(hint, |el| {
                        el.child(
                            div()
                                .text_size(dp(12.))
                                .line_height(dp(18.))
                                .text_color(theme.text_2)
                                .child(row.description),
                        )
                    })
                    .children(row.below.map(|below| div().mt(dp(4.)).child(below))),
            )
            .child(div().flex_none().child(row.control))
            .into_any_element()
    }

    /// A page's link in the sidebar, filled in while it's the one shown.
    fn page_link(
        &self,
        link: &Link,
        searching: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let page = link.page.clone();
        let selected = !searching && self.page == page;
        // Solid while it's shown, as a side list's is with the focus.
        ui::tab_stop(
            side_row(
                SharedString::from(format!("{page:?}")),
                selected,
                true,
                theme,
            ),
            theme,
        )
        .mx_0()
        .track_focus(&link.focus)
        .child(icon(page.icon()).text_color(match selected {
            true => theme.muted_on_solid(),
            false => theme.text_2,
        }))
        .child(page.title())
        .on_click(cx.listener(move |this, _, _, cx| this.show_page(page.clone(), cx)))
    }

    /// A listed project in the sidebar as a folder: its row shows its Project page and opens
    /// it to its Git page under it.
    fn project_entry(
        &self,
        project: &ProjectId,
        searching: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let name = self.store.read(cx).project(project)?.project.name.clone();
        let focus = self.project_focus.get(project)?;
        let open = self.page.project() == Some(project);
        let page = Page::Project(project.clone(), ProjectPage::Project);
        let selected = !searching && self.page == page;
        let row = side_row(
            SharedString::from(format!("{page:?}")),
            selected,
            true,
            theme,
        );
        let row = ui::tab_stop(row, theme)
            .mx_0()
            .track_focus(focus)
            .child(
                small_icon(if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                })
                .text_color(match selected {
                    true => theme.muted_on_solid(),
                    false => theme.text_3,
                }),
            )
            .child(icon(Icon::Folder).text_color(match selected {
                true => theme.muted_on_solid(),
                false => theme.text_2,
            }))
            .child(div().flex_1().min_w_0().truncate().child(name))
            .on_click(cx.listener(move |this, _, _, cx| this.show_page(page.clone(), cx)));
        let git = open.then(|| Link {
            page: Page::Project(project.clone(), ProjectPage::Git),
            focus: self.project_git_focus.clone(),
        });
        let git = git.map(|link| {
            div()
                .pl(dp(20.))
                .child(self.page_link(&link, searching, theme, cx))
        });
        Some(v_flex().child(row).children(git).into_any_element())
    }

    fn show_page(&mut self, page: Page, cx: &mut Context<Self>) {
        if page == Page::App(AppPage::Ai) {
            self.retry_ai();
            self.ensure_ai(cx);
        }
        let other_project = page.project() != self.page.project();
        self.page = page;
        if other_project {
            self.forget_project(cx);
            self.load_project(cx);
        }
        self.search.update(cx, |search, cx| search.set_text("", cx));
        cx.notify();
    }

    /// Show the page `delta` steps on in the sidebar from the link with the focus, or else the
    /// page shown, none past either end. The focus goes along with a link's, scrolled into view
    /// as Tab scrolls it.
    fn step_page(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let links = self.links(cx);
        let focused = links.iter().position(|link| link.focus.is_focused(window));
        let next = focused
            .or_else(|| links.iter().position(|link| link.page == self.page))
            .and_then(|at| at.checked_add_signed(delta))
            .and_then(|at| links.get(at));
        let Some(next) = next else { return };
        let (page, focus) = (next.page.clone(), next.focus.clone());
        self.show_page(page, cx);
        if focused.is_some() {
            window.focus(&focus, cx);
            if cx.has_global::<ui::FocusedAt>() {
                cx.remove_global::<ui::FocusedAt>();
            }
            window.dispatch_action(Box::new(ui::ShowFocused), cx);
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let query = self.search.read(cx).text().trim().to_lowercase();
        let searching = !query.is_empty();
        let cards = self.cards(&query, window, cx);
        let search_focused = self.search.focus_handle(cx).is_focused(window);
        let listed = self.listed_projects(cx);
        for project in listed.iter() {
            (self.project_focus.entry(project.clone()))
                .or_insert_with(|| cx.focus_handle().tab_stop(true));
        }
        let app_links =
            (self.app_links.iter()).map(|link| self.page_link(link, searching, &theme, cx));
        let any = listed.iter().next().is_some() || listed.hidden > 0;
        let projects = any.then(|| {
            let heading = |label: &'static str| {
                div()
                    .px(dp(8.))
                    .pb(dp(8.))
                    .text_size(dp(12.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.text_3)
                    .child(label)
            };
            let entries = |projects: &[ProjectId]| {
                (projects.iter())
                    .filter_map(|project| self.project_entry(project, searching, &theme, cx))
                    .collect::<Vec<_>>()
            };
            let more = match self.more {
                true => "Show fewer".to_owned(),
                false => format!("Show {} more", listed.hidden),
            };
            let more = (self.more || listed.hidden > 0).then(|| {
                div().pt(dp(4.)).child(
                    button("settings-more-projects", more)
                        .small()
                        .style(ButtonStyle::Ghost)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.more ^= true;
                            cx.notify();
                        })),
                )
            });
            v_flex()
                .mt(dp(16.))
                .pt(dp(16.))
                .px(dp(12.))
                .border_t_1()
                .border_color(theme.border_3)
                .child(heading("Projects"))
                .children(entries(&listed.watched))
                .when(
                    !listed.watched.is_empty() && !listed.unwatched.is_empty(),
                    |el| el.child(heading("Not watched").pt(dp(12.))),
                )
                .children(entries(&listed.unwatched))
                .children(more)
        });
        let empty = cards.is_empty();
        let height = (f32::from(window.viewport_size().height) / theme.scale * 0.76).min(1000.);
        let links = v_flex()
            .id("settings-pages")
            .size_full()
            .pb(dp(12.))
            .overflow_y_scroll()
            .track_scroll(&self.pages_scroll)
            .child(
                div()
                    .px(dp(20.))
                    .pt(dp(20.))
                    .pb(dp(10.))
                    .text_size(dp(16.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.text_1)
                    .child("Settings"),
            )
            .child(
                field(&self.search, search_focused, &theme)
                    .mx(dp(12.))
                    .mb(dp(10.)),
            )
            .child(v_flex().px(dp(12.)).children(app_links))
            .children(projects);
        let content = v_flex()
            .id("settings-cards")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.cards_scroll)
            .child(
                v_flex()
                    .p(dp(20.))
                    .gap(dp(12.))
                    .when(empty, |el| {
                        el.child(muted(
                            if searching {
                                "No settings match"
                            } else {
                                "Loading…"
                            },
                            &theme,
                        ))
                    })
                    .children(
                        cards
                            .into_iter()
                            .map(|card| Self::render_card(card, &theme)),
                    ),
            );
        modal(ModalSize::Large, &theme)
            .id("settings")
            .h(dp(height))
            .flex_row()
            .child(
                v_flex()
                    .w(dp(211.))
                    .flex_none()
                    .h_full()
                    .bg(theme.bg_2)
                    // Its ground fills the modal's left corners, which it rounds as the modal does.
                    .rounded_l(dp(MODAL_RADIUS - 1.))
                    .key_context("SettingsPages")
                    .on_action(
                        cx.listener(|this, _: &SelectNext, window, cx| {
                            this.step_page(1, window, cx)
                        }),
                    )
                    .on_action(cx.listener(|this, _: &SelectPrevious, window, cx| {
                        this.step_page(-1, window, cx)
                    }))
                    .child(scrolled(
                        links,
                        "settings-pages-scrollbar",
                        Scroll::Div(self.pages_scroll.clone()),
                        &theme,
                    )),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .border_l_1()
                    .border_color(theme.border_3)
                    .bg(theme.bg_1)
                    .rounded_r(dp(MODAL_RADIUS - 1.))
                    .child(scrolled(
                        content,
                        "settings-cards-scrollbar",
                        Scroll::Div(self.cards_scroll.clone()),
                        &theme,
                    )),
            )
    }
}

impl AppView {
    /// Settings, at General, signing in to GitButler.
    pub fn sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = self.open_settings_at(Page::App(AppPage::General), window, cx);
        view.update(cx, |view, cx| view.log_in_to_gitbutler(window, cx));
    }

    /// Settings, at Integrations: where a forge account gets connected.
    pub fn open_accounts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings_at(Page::App(AppPage::Integrations), window, cx);
    }

    /// Settings, at Project, for `project`.
    pub fn open_project_settings(
        &mut self,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings_at(Page::Project(project, ProjectPage::Project), window, cx);
    }

    /// Settings, at General; already open, it stays on the page it shows.
    pub fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_settings_view().is_none() {
            self.open_settings_at(Page::App(AppPage::General), window, cx);
        }
    }

    pub fn open_keyboard_shortcuts(
        &mut self,
        _: &OpenKeyboardShortcuts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings_at(Page::App(AppPage::Keyboard), window, cx);
    }

    /// Settings, at `page`. Already open, it shows `page` there: built again, it would draw
    /// without what it reads until that's read, and forget what had the keys before it opened.
    fn open_settings_at(
        &mut self,
        page: Page,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SettingsView> {
        if let Some(view) = self.open_settings_view() {
            view.update(cx, |view, cx| view.show_page(page, cx));
            return view;
        }
        let (store, app, detail) = (self.store.clone(), AppHandle::of(cx), self.detail.clone());
        let view = cx.new(|cx| SettingsView::new(store, app, detail, page, window, cx));
        self.open_modal(view.clone(), window, cx);
        view
    }

    /// Settings, while it's the modal open.
    fn open_settings_view(&self) -> Option<Entity<SettingsView>> {
        let (view, ..) = self.modal.as_ref()?;
        view.clone().downcast().ok()
    }
}

/// A text field with Save beside it while `changed`, which runs `save`.
fn field_with_save(
    input: &Entity<TextInput>,
    changed: bool,
    save: impl Fn(&mut SettingsView, &Entity<TextInput>, &mut Context<SettingsView>) + 'static,
    window: &Window,
    cx: &Context<SettingsView>,
) -> gpui::Div {
    let focused = input.focus_handle(cx).is_focused(window);
    let saved = input.clone();
    h_flex()
        .gap(dp(6.))
        .when(changed, |el| {
            el.child(
                button(
                    SharedString::from(format!("save-{}", input.entity_id())),
                    "Save",
                )
                .small()
                .style(ButtonStyle::Gray)
                .on_click(cx.listener(move |this, _, _, cx| save(this, &saved, cx))),
            )
        })
        .child(field(input, focused, Theme::get(cx)).w(dp(220.)))
}

/// In place of `page`'s settings when they couldn't be read: `title` over why, and Try again,
/// which runs `retry`.
fn read_failed(
    page: Page,
    title: &'static str,
    err: &str,
    retry: fn(&mut SettingsView, &mut Context<SettingsView>),
    cx: &Context<SettingsView>,
) -> Row {
    let again = button(SharedString::from(format!("retry-{page:?}")), "Try again")
        .small()
        .on_click(cx.listener(move |this, _, _, cx| retry(this, cx)));
    Row::new(title, error_summary(err), again)
}

/// A segmented control for a setting with a few values, `current` picked. Unlike a dropdown's
/// menu, a segment's click doesn't run inside the app's own update, so `pick` can update the
/// app.
fn segmented<T: Copy + PartialEq + 'static>(
    id: &str,
    choices: impl IntoIterator<Item = (T, &'static str)>,
    current: T,
    pick: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
    theme: &Theme,
) -> gpui::Div {
    ui::segmented(
        true,
        theme,
        choices.into_iter().map(|(choice, label)| {
            let pick = pick.clone();
            let segment = div()
                .id(SharedString::from(format!("{id}-{label}")))
                .child(label)
                .on_click(move |_, window, cx| pick(choice, window, cx));
            (segment, current == choice)
        }),
    )
}
