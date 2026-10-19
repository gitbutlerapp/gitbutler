//! Settings: one large modal laid out as Lite's, its pages down the left, the app's and then
//! the project's, each a column of cards, with a search across all of them. Changes apply
//! as they're made. What only this app uses is kept with its other
//! state; fetching is GitButler's shared setting; a project's settings are its own.

use gpui::{
    AnyElement, App, ClickEvent, Context, DismissEvent, Entity, EventEmitter, FocusHandle,
    Focusable, KeyBinding, SharedString, Subscription, Task, WeakEntity, Window, actions, div,
    prelude::*, px,
};

mod accounts;
mod ai;
mod project;

use crate::app::AppView;
use crate::backend::settings::{self as backend, Program, ProjectSettings};
use crate::overlays::MenuItem;
use crate::state::{ProjectId, Store};
use crate::theme::{Appearance, CodeSize, Theme};
use crate::ui::modal::{ModalSize, modal};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, Icon, Tooltip, button, dropdown, field, h_flex, icon, muted, switch, v_flex,
};

actions!(settings, [OpenSettings]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("cmd-,", OpenSettings, None)]);
}

/// A page of settings, in the sidebar's order, as Lite's has them: the app's, then the
/// project's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    General,
    Appearance,
    Ai,
    Git,
    Integrations,
    Experimental,
    Project,
    ProjectGit,
}

impl Page {
    const APP: [Page; 6] = [
        Page::General,
        Page::Appearance,
        Page::Ai,
        Page::Git,
        Page::Integrations,
        Page::Experimental,
    ];
    const PROJECT: [Page; 2] = [Page::Project, Page::ProjectGit];

    fn title(self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Appearance => "Appearance",
            Page::Ai => "AI",
            Page::Git | Page::ProjectGit => "Git",
            Page::Integrations => "Integrations",
            Page::Experimental => "Experimental",
            Page::Project => "Project",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Page::General => Icon::Settings,
            Page::Appearance => Icon::Contrast,
            Page::Ai => Icon::Ai,
            Page::Git | Page::ProjectGit => Icon::Pr,
            Page::Integrations => Icon::Plug,
            Page::Experimental => Icon::Lab,
            Page::Project => Icon::Mixer,
        }
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

/// One setting: what it's called, what it does, and the control that changes it.
struct Row {
    title: SharedString,
    description: SharedString,
    control: AnyElement,
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
        }
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
    app: WeakEntity<AppView>,
    search: Entity<TextInput>,
    page: Page,
    /// The project the project's pages are about.
    project: Option<ProjectId>,
    project_settings: Option<ProjectSettings>,
    name: Entity<TextInput>,
    editors: Vec<Program>,
    terminals: Vec<Program>,
    fetch_interval: Option<u64>,
    worktrees: bool,
    /// The desktop app opens this client at start: GitButler's `featureFlags.gpui`.
    open_at_start: bool,
    accounts: accounts::Accounts,
    ai: ai::Ai,
    /// How the last signing test went, while it applies to the project shown.
    signing: Option<Result<(), SharedString>>,
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
    pub fn new(
        store: Entity<Store>,
        app: WeakEntity<AppView>,
        project: Option<ProjectId>,
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
            cx.subscribe_in(&name, window, |this, _, event, window, cx| match event {
                InputEvent::Submit => this.save_name(cx),
                InputEvent::Changed => cx.notify(),
                InputEvent::Cancel => {
                    this.reset_name(cx);
                    this.back_to_search(window, cx);
                }
            }),
        ];
        let mut view = Self {
            store,
            app,
            search,
            page: Page::General,
            project: None,
            project_settings: None,
            name,
            editors: Vec::new(),
            terminals: Vec::new(),
            fetch_interval: None,
            worktrees: false,
            open_at_start: false,
            accounts: Default::default(),
            ai,
            signing: None,
            _subscriptions: subscriptions,
            _load: Task::ready(()),
        };
        view.show_project(project, cx);
        view.load_accounts(cx);
        cx.spawn(async move |this, cx| {
            let loaded = crate::backend::run(|| {
                Ok((
                    backend::editors(),
                    backend::terminals(),
                    backend::fetch_interval(),
                    backend::worktrees_enabled(),
                    backend::opens_at_start(),
                ))
            })
            .await;
            if let Ok((editors, terminals, fetch_interval, worktrees, open_at_start)) = loaded {
                this.update(cx, |this, cx| {
                    this.editors = editors;
                    this.terminals = terminals;
                    this.fetch_interval = fetch_interval;
                    this.worktrees = worktrees;
                    this.open_at_start = open_at_start;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        view
    }

    fn error(&self, title: &str, err: anyhow::Error, cx: &mut App) {
        self.store.update(cx, |store, cx| {
            store.error(title.to_owned(), format!("{err:#}"), cx)
        });
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

    /// Show the fetch interval as saved, which a change that failed left as it was.
    fn reload_fetch_interval(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let saved = crate::backend::run(|| Ok(backend::fetch_interval())).await;
            if let Ok(minutes) = saved {
                this.update(cx, |this, cx| {
                    this.fetch_interval = minutes;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn open_menu(&self, event: &ClickEvent, items: Vec<MenuItem>, cx: &mut App) {
        let position = event.position();
        self.app
            .update(cx, |app, cx| app.open_menu(position, items, cx))
            .ok();
    }

    /// A dropdown showing `label`, whose menu sets one of `options` through `set`.
    fn choice<T: Clone + 'static>(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        options: impl IntoIterator<Item = (T, String)>,
        set: impl Fn(&mut Self, T, &mut Context<Self>) + Clone + 'static,
        cx: &Context<Self>,
    ) -> AnyElement {
        let this = cx.entity().downgrade();
        let items: Vec<MenuItem> = options
            .into_iter()
            .map(|(value, label)| {
                let (this, set) = (this.clone(), set.clone());
                MenuItem::new(label, move |_, _, cx| {
                    this.update(cx, |this, cx| set(this, value.clone(), cx))
                        .ok();
                })
            })
            .collect();
        dropdown(id, label, Theme::get(cx))
            .on_click(move |event, _, cx| {
                this.update(cx, |this, cx| this.open_menu(event, items.clone(), cx))
                    .ok();
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
            |this, id: String, cx| {
                this.store
                    .update(cx, |store, cx| store.set_prefs(|p| p.editor = Some(id), cx));
            },
            cx,
        );
        let terminal = self.choice(
            "terminal",
            terminal,
            (self.terminals.iter()).map(|(id, name)| (id.clone(), name.clone())),
            |this, id: String, cx| {
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
                app.update(cx, |app, cx| app.apply_appearance(window, cx))
                    .ok();
            },
        );
        let app = self.app.clone();
        let code = segmented(
            "code",
            [
                (CodeSize::Small, "Small"),
                (CodeSize::Medium, "Medium"),
                (CodeSize::Large, "Large"),
            ],
            prefs.code_size,
            move |size, _, cx| {
                app.update(cx, |app, cx| {
                    app.detail
                        .update(cx, |detail, cx| detail.set_code_size(size, cx))
                })
                .ok();
            },
        );
        let appearance = Row::new(
            "Appearance",
            "Light, dark, or whichever macOS is set to.",
            appearance,
        );
        let diff = vec![
            Row::new("Code size", "How large code shows in diffs.", code),
            Row::new(
                "Wrap long lines",
                "Diff lines wider than the pane continue on the next row. ⌥Z in a diff turns it \
                 on and off.",
                switch("wrap", prefs.wrap, theme).on_click(cx.listener(|this, _, _, cx| {
                    this.app
                        .update(cx, |app, cx| {
                            app.detail.update(cx, |detail, cx| detail.toggle_wrap(cx))
                        })
                        .ok();
                })),
            ),
            Row::new(
                "Side by side",
                "Diffs show the old lines beside the new ones. ⌘B in a diff switches.",
                switch("split", prefs.split, theme).on_click(cx.listener(|this, _, _, cx| {
                    this.app
                        .update(cx, |app, cx| {
                            app.detail.update(cx, |detail, cx| detail.toggle_split(cx))
                        })
                        .ok();
                })),
            ),
        ];
        vec![
            Card::new(None, vec![appearance]),
            Card::new(Some("Diff"), diff),
        ]
    }

    fn fetch_row(&self, cx: &Context<Self>) -> Row {
        let control = self.choice(
            "fetch",
            interval_label(self.fetch_interval),
            FETCH_INTERVALS.map(|minutes| (minutes, interval_label(minutes))),
            |this, minutes: Option<u64>, cx| {
                this.fetch_interval = minutes;
                cx.notify();
                this.apply(
                    "Couldn't change how often to fetch",
                    move || backend::set_fetch_interval(minutes),
                    Self::reload_fetch_interval,
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

    fn worktrees_row(&self, cx: &Context<Self>) -> Row {
        let theme = Theme::get(cx);
        let worktrees =
            switch("worktrees", self.worktrees, theme).on_click(cx.listener(|this, _, _, cx| {
                let on = !this.worktrees;
                this.worktrees = on;
                cx.notify();
                cx.spawn(async move |this, cx| {
                    let saved =
                        crate::backend::run(move || backend::set_worktrees_enabled(on)).await;
                    this.update(cx, |this, cx| match saved {
                        // Watched and listed again, with their worktrees or without.
                        Ok(()) => this.store.update(cx, |store, cx| store.reopen_expanded(cx)),
                        Err(err) => {
                            this.worktrees = !on;
                            cx.notify();
                            this.error("Couldn't change the worktree setting", err, cx)
                        }
                    })
                    .ok();
                })
                .detach();
            }));
        Row::new(
            "Linked worktrees",
            "Lists each project's linked worktrees, like the ones agents work in, with their \
             uncommitted changes. Worktrees that exist when this goes on start archived: a \
             project's menu shows them again. GitButler's other apps share this.",
            worktrees,
        )
    }

    fn start_row(&self, cx: &Context<Self>) -> Row {
        let theme = Theme::get(cx);
        let start = switch("open-at-start", self.open_at_start, theme).on_click(cx.listener(
            |this, _, _, cx| {
                let on = !this.open_at_start;
                this.open_at_start = on;
                cx.notify();
                cx.spawn(async move |this, cx| {
                    let saved = crate::backend::run(move || backend::set_opens_at_start(on)).await;
                    if let Err(err) = saved {
                        this.update(cx, |this, cx| {
                            this.open_at_start = !on;
                            cx.notify();
                            this.error("Couldn't change what opens at start", err, cx)
                        })
                        .ok();
                    }
                })
                .detach();
            },
        ));
        Row::new(
            "Open this app at start",
            "GitButler opens this app rather than its web app when it starts, as \
             featureFlags.gpui in its settings.json says. Starting it with --gpui opens this app \
             either way.",
            start,
        )
    }

    /// Escape in a field puts it back and goes to the search box, where Escape closes.
    fn back_to_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.search.focus_handle(cx), cx);
    }

    fn page_cards(&self, page: Page, window: &Window, cx: &Context<Self>) -> Vec<Card> {
        let cards = match page {
            Page::General => vec![
                Card::new(None, vec![self.gitbutler_row(window, cx)]),
                Card::new(None, self.program_rows(cx)),
            ],
            Page::Appearance => self.appearance_cards(cx),
            Page::Ai => vec![Card::new(None, self.ai_rows(window, cx))],
            Page::Git => vec![Card::new(None, vec![self.fetch_row(cx)])],
            Page::Integrations => self.integration_cards(window, cx),
            Page::Experimental => {
                // Only where the desktop app's binary has this client built in.
                let start = cx
                    .has_global::<crate::Embedded>()
                    .then(|| self.start_row(cx));
                let rows = std::iter::once(self.worktrees_row(cx)).chain(start);
                vec![Card::new(None, rows.collect())]
            }
            Page::Project | Page::ProjectGit => self.project_cards(page, window, cx),
        };
        // Nothing for a card with no rows yet, as before what it shows has loaded.
        cards
            .into_iter()
            .filter(|card| !card.rows.is_empty())
            .collect()
    }

    /// The pages there are: the app's, and the project's when there's one.
    fn pages(&self) -> impl Iterator<Item = Page> + use<> {
        let project = self.project.is_some();
        (Page::APP.into_iter()).chain(Page::PROJECT.into_iter().filter(move |_| project))
    }

    /// What a page is called where it isn't under its group's heading: a project's by the
    /// project too.
    fn page_name(&self, page: Page, cx: &App) -> String {
        match (Page::PROJECT.contains(&page), self.project_name(cx)) {
            (true, Some(name)) => format!("{name} › {}", page.title()),
            _ => page.title().to_owned(),
        }
    }

    /// The cards to show: the current page's, or with a `query` every row it matches, in
    /// cards named by their page.
    fn cards(&self, query: &str, window: &Window, cx: &Context<Self>) -> Vec<Card> {
        if query.is_empty() {
            return self.page_cards(self.page, window, cx);
        }
        let pages: Vec<Page> = self.pages().collect();
        (pages.into_iter())
            .flat_map(|page| {
                let cards = self.page_cards(page, window, cx);
                cards.into_iter().map(move |card| (page, card))
            })
            .filter_map(|(page, card)| {
                let heading = match &card.heading {
                    Some(heading) => format!("{} › {heading}", self.page_name(page, cx)),
                    None => self.page_name(page, cx),
                };
                let rows: Vec<Row> = (card.rows.into_iter())
                    .filter(|row| {
                        let text = format!("{heading} {} {}", row.title, row.description);
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
            .gap(px(8.))
            .children(card.heading.map(|heading| {
                div()
                    .pl(px(16.))
                    .text_size(px(12.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.text_3)
                    .child(heading)
            }))
            .child(
                v_flex()
                    .rounded(px(8.))
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
            .p(px(16.))
            .gap(px(16.))
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
                    .min_w(px(0.))
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text_1)
                            .child(row.title),
                    )
                    .when(hint, |el| {
                        el.child(
                            div()
                                .text_size(px(12.))
                                .line_height(px(18.))
                                .text_color(theme.text_2)
                                .child(row.description),
                        )
                    }),
            )
            .child(div().flex_none().child(row.control))
            .into_any_element()
    }

    /// A page's link in the sidebar, filled in while it's the one shown.
    fn page_link(
        &self,
        page: Page,
        searching: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let selected = !searching && self.page == page;
        let hover = theme.hover();
        h_flex()
            .id(SharedString::from(format!("{page:?}")))
            .px(px(8.))
            .py(px(6.))
            .gap(px(10.))
            .rounded(px(6.))
            .text_size(px(13.))
            .cursor_pointer()
            .map(|el| match selected {
                true => el.bg(theme.fill_gray).text_color(theme.text_inverted),
                false => el.text_color(theme.text_1).hover(move |el| el.bg(hover)),
            })
            .child(icon(page.icon()).text_color(match selected {
                true => theme.text_inverted.opacity(0.7),
                false => theme.text_2,
            }))
            .child(page.title())
            .on_click(cx.listener(move |this, _, _, cx| {
                if page == Page::Ai {
                    this.ensure_ai(cx);
                }
                this.page = page;
                this.search.update(cx, |search, cx| search.set_text("", cx));
                cx.notify();
            }))
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let query = self.search.read(cx).text().trim().to_lowercase();
        let searching = !query.is_empty();
        let cards = self.cards(&query, window, cx);
        let search_focused = self.search.focus_handle(cx).is_focused(window);
        let app_links = Page::APP.map(|page| self.page_link(page, searching, &theme, cx));
        // The project's pages, under its name, which picks another when there are more.
        let projects = &self.store.read(cx).projects;
        let project = self.project_name(cx).map(|name| {
            let others = projects.len() > 1;
            let heading = h_flex()
                .id("settings-project")
                .px(px(8.))
                .pb(px(8.))
                .gap(px(10.))
                .text_size(px(13.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text_3)
                .child(icon(Icon::Folder).text_color(theme.text_2))
                .child(div().flex_1().min_w(px(0.)).truncate().child(name))
                .when(others, |el| {
                    let items: Vec<MenuItem> = (projects.iter())
                        .map(|entry| {
                            let this = cx.entity().downgrade();
                            let id = entry.id.clone();
                            MenuItem::new(entry.project.name.clone(), move |_, _, cx| {
                                this.update(cx, |this, cx| this.show_project(Some(id.clone()), cx))
                                    .ok();
                            })
                        })
                        .collect();
                    el.cursor_pointer()
                        .child(icon(Icon::ChevronDown).text_color(theme.text_3))
                        .tooltip(Tooltip::text("Another project's settings"))
                        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                            this.open_menu(event, items.clone(), cx)
                        }))
                });
            v_flex()
                .mt(px(16.))
                .pt(px(16.))
                .px(px(12.))
                .gap(px(3.))
                .border_t_1()
                .border_color(theme.border_3)
                .child(heading)
                .children(Page::PROJECT.map(|page| self.page_link(page, searching, &theme, cx)))
        });
        let empty = cards.is_empty();
        let height = (f32::from(window.viewport_size().height) * 0.76).min(1000.);
        modal(ModalSize::Large, &theme)
            .id("settings")
            .h(px(height))
            .flex_row()
            .child(
                v_flex()
                    .id("settings-pages")
                    .w(px(211.))
                    .flex_none()
                    .h_full()
                    .pb(px(12.))
                    .overflow_y_scroll()
                    .bg(theme.bg_2)
                    .child(
                        div()
                            .px(px(20.))
                            .pt(px(20.))
                            .pb(px(10.))
                            .text_size(px(16.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text_1)
                            .child("Settings"),
                    )
                    .child(
                        field(&self.search, search_focused, &theme)
                            .mx(px(12.))
                            .mb(px(10.)),
                    )
                    .child(v_flex().px(px(12.)).gap(px(3.)).children(app_links))
                    .children(project),
            )
            .child(
                v_flex()
                    .id("settings-cards")
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .overflow_y_scroll()
                    .border_l_1()
                    .border_color(theme.border_3)
                    .bg(theme.bg_1)
                    .child(
                        v_flex()
                            .p(px(20.))
                            .gap(px(12.))
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
                    ),
            )
    }
}

impl AppView {
    /// Settings, at General, signing in to GitButler.
    pub fn sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings_at(None, Page::General, window, cx);
        if let Some(view) = self.modal_view::<SettingsView>() {
            view.update(cx, |view, cx| view.log_in_to_gitbutler(window, cx));
        }
    }

    /// Settings, at Integrations: where a forge account gets connected.
    pub fn open_accounts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings_at(None, Page::Integrations, window, cx)
    }

    /// Settings, at Project, for `project`.
    pub fn open_project_settings(
        &mut self,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings_at(Some(project), Page::Project, window, cx)
    }

    pub fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings_at(None, Page::General, window, cx)
    }

    /// Settings, at `page`, for `project`, or else the selected project or the first.
    fn open_settings_at(
        &mut self,
        project: Option<ProjectId>,
        page: Page,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        let project = project.or_else(|| {
            let store = store.read(cx);
            (store.selection.as_ref())
                .map(|s| s.project().clone())
                .or_else(|| store.projects.first().map(|entry| entry.id.clone()))
        });
        let app = cx.entity().downgrade();
        let view = cx.new(|cx| {
            let mut view = SettingsView::new(store, app, project, window, cx);
            if page == Page::Ai {
                view.ensure_ai(cx);
            }
            view.page = page;
            view
        });
        self.open_modal(view, window, cx);
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
        .gap(px(6.))
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
        .child(field(input, focused, Theme::get(cx)).w(px(220.)))
}

/// Buttons for a setting with a few values, the current one `current` filled in. Unlike a
/// dropdown's menu, a button's click doesn't run inside the app's own update, so `pick`
/// can update the app.
fn segmented<T: Copy + PartialEq + 'static>(
    id: &str,
    choices: [(T, &'static str); 3],
    current: T,
    pick: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
) -> gpui::Div {
    h_flex()
        .gap(px(2.))
        .children(choices.into_iter().map(|(choice, label)| {
            let pick = pick.clone();
            button(SharedString::from(format!("{id}-{label}")), label)
                .small()
                .selected(current == choice)
                .on_click(move |_, window, cx| pick(choice, window, cx))
        }))
}
