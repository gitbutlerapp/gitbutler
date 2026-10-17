//! Settings: one large modal with the sections down the left and a search across all of
//! them. Changes apply as they're made. What only this app uses is kept with its other
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
    ButtonStyle, Icon, button, dropdown, faint, field, h_flex, icon_button, list_row, muted,
    switch, v_flex,
};

actions!(settings, [OpenSettings]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("cmd-,", OpenSettings, None)]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    General,
    Git,
    Ai,
    Accounts,
    Project,
}

impl Section {
    const ALL: [Section; 5] = [
        Section::General,
        Section::Git,
        Section::Ai,
        Section::Accounts,
        Section::Project,
    ];

    fn title(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Git => "Git",
            Section::Ai => "AI",
            Section::Accounts => "Accounts",
            Section::Project => "Project",
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
    section: Section,
    /// The project the Project section is about.
    project: Option<ProjectId>,
    project_settings: Option<ProjectSettings>,
    name: Entity<TextInput>,
    editors: Vec<Program>,
    terminals: Vec<Program>,
    fetch_interval: Option<u64>,
    worktrees: bool,
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
        let ai = ai::Ai::new(cx);
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, search, event, _, cx| match event {
                InputEvent::Changed => {
                    // A search may match the AI section's rows, which load when needed.
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
            cx.subscribe(&name, |this, _, event, cx| match event {
                InputEvent::Submit => this.save_name(cx),
                InputEvent::Changed => cx.notify(),
                InputEvent::Cancel => this.reset_name(cx),
            }),
        ];
        let mut view = Self {
            store,
            app,
            search,
            section: Section::General,
            project: None,
            project_settings: None,
            name,
            editors: Vec::new(),
            terminals: Vec::new(),
            fetch_interval: None,
            worktrees: false,
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
                ))
            })
            .await;
            if let Ok((editors, terminals, fetch_interval, worktrees)) = loaded {
                this.update(cx, |this, cx| {
                    this.editors = editors;
                    this.terminals = terminals;
                    this.fetch_interval = fetch_interval;
                    this.worktrees = worktrees;
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

    fn general_rows(&self, cx: &Context<Self>) -> Vec<Row> {
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
        vec![
            Row::new(
                "Appearance",
                "Light, dark, or whichever macOS is set to.",
                appearance,
            ),
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

    fn git_rows(&self, cx: &Context<Self>) -> Vec<Row> {
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
        vec![
            Row::new(
                "Fetch automatically",
                "Keeps the target and each branch's remote current, for every expanded project. \
                 GitButler's other apps share this.",
                control,
            ),
            Row::new(
                "Linked worktrees",
                "Experimental. Lists each project's linked worktrees, like the ones agents work \
                 in, with their uncommitted changes. Worktrees that exist when this goes on start \
                 archived: a project's menu shows them again. GitButler's other apps share this.",
                worktrees,
            ),
        ]
    }

    fn section_rows(&self, section: Section, window: &Window, cx: &Context<Self>) -> Vec<Row> {
        match section {
            Section::General => self.general_rows(cx),
            Section::Git => self.git_rows(cx),
            Section::Ai => self.ai_rows(window, cx),
            Section::Accounts => self.account_rows(window, cx),
            Section::Project => self.project_rows(window, cx),
        }
    }

    /// The rows to show: the current section's, or with a `query` every row it matches, each
    /// with the section it's in.
    fn rows(
        &self,
        query: &str,
        window: &Window,
        cx: &Context<Self>,
    ) -> Vec<(Option<Section>, Row)> {
        if query.is_empty() {
            let rows = self.section_rows(self.section, window, cx);
            return rows.into_iter().map(|row| (None, row)).collect();
        }
        let matches = |section: Section, row: &Row| {
            let text =
                format!("{} {} {}", section.title(), row.title, row.description).to_lowercase();
            query.split_whitespace().all(|word| text.contains(word))
        };
        Section::ALL
            .into_iter()
            .flat_map(|section| {
                let rows = self.section_rows(section, window, cx);
                rows.into_iter()
                    .filter(move |row| matches(section, row))
                    .map(move |row| (Some(section), row))
            })
            .collect()
    }

    fn render_row(section: Option<Section>, row: Row, theme: &Theme) -> AnyElement {
        h_flex()
            .gap(px(24.))
            .py(px(14.))
            .border_b_1()
            .border_color(theme.border_3)
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .children(
                                section
                                    .map(|section| faint(format!("{} ›", section.title()), theme)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(row.title),
                            ),
                    )
                    .child(muted(row.description, theme)),
            )
            .child(row.control)
            .into_any_element()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let query = self.search.read(cx).text().trim().to_lowercase();
        let searching = !query.is_empty();
        let rows = self.rows(&query, window, cx);
        let search_focused = self.search.focus_handle(cx).is_focused(window);
        let sections = Section::ALL.into_iter().map(|section| {
            let selected = !searching && self.section == section;
            list_row(SharedString::from(section.title()), selected, &theme)
                .mx_0()
                .text_size(px(13.))
                .child(section.title())
                .on_click(cx.listener(move |this, _, _, cx| {
                    if section == Section::Ai {
                        this.ensure_ai(cx);
                    }
                    this.section = section;
                    this.search.update(cx, |search, cx| search.set_text("", cx));
                    cx.notify();
                }))
        });
        let title: SharedString = if searching {
            "Search results".into()
        } else if self.section == Section::Project {
            match self.project_name(cx) {
                Some(name) => format!("Project · {name}").into(),
                None => "Project".into(),
            }
        } else {
            self.section.title().into()
        };
        let projects = &self.store.read(cx).projects;
        let pick_project = (!searching && self.section == Section::Project && projects.len() > 1)
            .then(|| {
                self.choice(
                    "pick-project",
                    "Other project",
                    (projects.iter()).map(|entry| (entry.id.clone(), entry.project.name.clone())),
                    |this, id: ProjectId, cx| this.show_project(Some(id), cx),
                    cx,
                )
            });
        let empty = rows.is_empty();
        modal(ModalSize::Large, &theme)
            .id("settings")
            .h(px(560.))
            .flex_row()
            .child(
                v_flex()
                    .w(px(200.))
                    .flex_none()
                    .h_full()
                    .p(px(10.))
                    .gap(px(2.))
                    .bg(theme.bg_2)
                    .border_r_1()
                    .border_color(theme.border_3)
                    .child(field(&self.search, search_focused, &theme).mb(px(8.)))
                    .children(sections),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .child(
                        h_flex()
                            .flex_none()
                            .h(px(52.))
                            .px(px(24.))
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(15.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .children(pick_project)
                            .child(
                                icon_button("close-settings", Icon::Cross)
                                    .tooltip_with_key("Close", Some("esc".into()))
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("settings-rows")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .px(px(24.))
                            .when(empty, |el| {
                                el.child(
                                    div()
                                        .pt(px(24.))
                                        .text_size(px(13.))
                                        .text_color(theme.text_2)
                                        .child(if searching {
                                            "No settings match"
                                        } else {
                                            "Loading…"
                                        }),
                                )
                            })
                            .children(
                                rows.into_iter()
                                    .map(|(section, row)| Self::render_row(section, row, &theme)),
                            ),
                    ),
            )
    }
}

impl AppView {
    /// Settings, at Accounts, signing in to GitButler.
    pub fn sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_accounts(window, cx);
        if let Some(view) = self.modal_view::<SettingsView>() {
            view.update(cx, |view, cx| view.log_in_to_gitbutler(window, cx));
        }
    }

    /// Settings, at Accounts: where a forge account gets connected.
    pub fn open_accounts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings(&OpenSettings, window, cx);
        if let Some(view) = self.modal_view::<SettingsView>() {
            view.update(cx, |view, cx| {
                view.section = Section::Accounts;
                cx.notify();
            });
        }
    }

    pub fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let project = {
            let store = store.read(cx);
            store
                .selection
                .as_ref()
                .map(|s| s.project().clone())
                .or_else(|| store.projects.first().map(|entry| entry.id.clone()))
        };
        let app = cx.entity().downgrade();
        let view = cx.new(|cx| SettingsView::new(store, app, project, window, cx));
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
