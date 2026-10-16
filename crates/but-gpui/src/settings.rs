//! Settings: one large modal with the sections down the left and a search across all of
//! them. Changes apply as they're made. What only this app uses is kept with its other
//! state; fetching is GitButler's shared setting; a project's settings are its own.

use gpui::{
    AnyElement, App, ClickEvent, Context, DismissEvent, Entity, EventEmitter, FocusHandle,
    Focusable, KeyBinding, SharedString, Subscription, Task, WeakEntity, Window, actions, div,
    prelude::*, px,
};

use crate::app::{AppView, MenuItem};
use crate::backend::settings::{self as backend, Program, ProjectSettings};
use crate::state::{ProjectId, Store};
use crate::theme::{Appearance, Theme};
use crate::ui::modal::{ModalSize, modal};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, Icon, button, dropdown, h_flex, icon_button, list_row, switch, v_flex,
};

actions!(settings, [OpenSettings]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("cmd-,", OpenSettings, None)]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    General,
    Git,
    Project,
}

impl Section {
    const ALL: [Section; 3] = [Section::General, Section::Git, Section::Project];

    fn title(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Git => "Git",
            Section::Project => "Project",
        }
    }
}

/// One setting: what it's called, what it does, and the control that changes it.
struct Row {
    section: Section,
    title: &'static str,
    description: SharedString,
    control: AnyElement,
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
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |_, search, event, _, cx| match event {
                InputEvent::Changed => cx.notify(),
                InputEvent::Cancel if search.read(cx).text().is_empty() => cx.emit(DismissEvent),
                InputEvent::Cancel => search.update(cx, |search, cx| search.set_text("", cx)),
                InputEvent::Submit => {}
            }),
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
            signing: None,
            _subscriptions: subscriptions,
            _load: Task::ready(()),
        };
        view.show_project(project, cx);
        cx.spawn(async move |this, cx| {
            let loaded = crate::backend::run(|| {
                Ok((
                    backend::editors(),
                    backend::terminals(),
                    backend::fetch_interval(),
                ))
            })
            .await;
            if let Ok((editors, terminals, fetch_interval)) = loaded {
                this.update(cx, |this, cx| {
                    this.editors = editors;
                    this.terminals = terminals;
                    this.fetch_interval = fetch_interval;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        view
    }

    /// Show `project`'s settings in the Project section, loading them.
    fn show_project(&mut self, project: Option<ProjectId>, cx: &mut Context<Self>) {
        self.project = project.clone();
        self.project_settings = None;
        self.signing = None;
        self.reset_name(cx);
        let Some(project) = project else {
            return;
        };
        let Some(path) = self.store.read(cx).path(&project) else {
            return;
        };
        self._load = cx.spawn(async move |this, cx| {
            let id = project.to_string();
            let loaded = crate::backend::run(move || backend::project_settings(&id, &path)).await;
            this.update(cx, |this, cx| {
                match loaded {
                    Ok(settings) => this.project_settings = Some(settings),
                    Err(err) => this.error("Couldn't read the project's settings", err, cx),
                }
                cx.notify();
            })
            .ok();
        });
    }

    fn project_name(&self, cx: &App) -> Option<String> {
        let project = self.project.as_ref()?;
        Some(self.store.read(cx).project(project)?.project.name.clone())
    }

    fn reset_name(&mut self, cx: &mut Context<Self>) {
        let name = self.project_name(cx).unwrap_or_default();
        self.name.update(cx, |input, cx| input.set_text(name, cx));
    }

    fn error(&self, title: &str, err: anyhow::Error, cx: &mut App) {
        self.store.update(cx, |store, cx| {
            store.error(title.to_owned(), format!("{err:#}"), cx)
        });
    }

    /// Run `op` in the background, then reload the project list or the project's
    /// settings so the dialog and the navigator show the result.
    fn apply(
        &mut self,
        failed: &'static str,
        op: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = crate::backend::run(op).await;
            this.update(cx, |this, cx| {
                if let Err(err) = result {
                    this.error(failed, err, cx);
                }
                this.store.update(cx, |store, cx| store.load_projects(cx));
                let project = this.project.clone();
                this.show_project(project, cx);
            })
            .ok();
        })
        .detach();
    }

    fn save_name(&mut self, cx: &mut Context<Self>) {
        let (Some(project), Some(current)) = (self.project.clone(), self.project_name(cx)) else {
            return;
        };
        let name = self.name.read(cx).text().trim().to_owned();
        if name.is_empty() || name == current {
            return self.reset_name(cx);
        }
        self.apply(
            "Couldn't rename the project",
            move || backend::update_project(&project, Some(name), None),
            cx,
        );
    }

    fn open_menu(&self, event: &ClickEvent, items: Vec<MenuItem>, cx: &mut App) {
        let position = event.position();
        self.app
            .update(cx, |app, cx| app.open_menu(position, items, cx))
            .ok();
    }

    /// Menu items that each set a choice through `set` on this view.
    fn choices<T: Clone + 'static>(
        &self,
        options: impl IntoIterator<Item = (T, String)>,
        set: impl Fn(&mut Self, T, &mut Context<Self>) + Clone + 'static,
        cx: &Context<Self>,
    ) -> Vec<MenuItem> {
        let this = cx.entity().downgrade();
        options
            .into_iter()
            .map(|(value, label)| {
                let (this, set) = (this.clone(), set.clone());
                MenuItem::new(label, move |_, _, cx| {
                    this.update(cx, |this, cx| set(this, value.clone(), cx))
                        .ok();
                })
            })
            .collect()
    }

    fn general_rows(&self, cx: &Context<Self>) -> Vec<Row> {
        let theme = Theme::get(cx);
        let prefs = &self.store.read(cx).prefs;
        let appearance = h_flex().gap(px(2.)).children(
            [
                (Appearance::System, "System"),
                (Appearance::Light, "Light"),
                (Appearance::Dark, "Dark"),
            ]
            .into_iter()
            .map(|(choice, label)| {
                let store = self.store.clone();
                let app = self.app.clone();
                button(SharedString::from(format!("appearance-{label}")), label)
                    .small()
                    .style(if prefs.appearance == choice {
                        ButtonStyle::Gray
                    } else {
                        ButtonStyle::Ghost
                    })
                    .on_click(move |_, window, cx| {
                        store.update(cx, |store, cx| {
                            store.set_prefs(|prefs| prefs.appearance = choice, cx)
                        });
                        app.update(cx, |app, cx| app.apply_appearance(window, cx))
                            .ok();
                    })
            }),
        );
        let name_of = |programs: &[Program], id: Option<&String>, unset: &str| {
            id.and_then(|id| programs.iter().find(|(p, _)| p == id))
                .map_or_else(|| unset.to_owned(), |(_, name)| name.clone())
        };
        let editor = name_of(&self.editors, prefs.editor.as_ref(), "Ask the first time");
        let terminal = name_of(&self.terminals, prefs.terminal.as_ref(), "Terminal");
        let editor_items = self.choices(
            self.editors
                .iter()
                .map(|(id, name)| (id.clone(), name.clone())),
            |this, id: String, cx| {
                this.store
                    .update(cx, |store, cx| store.set_prefs(|p| p.editor = Some(id), cx));
            },
            cx,
        );
        let terminal_items = self.choices(
            self.terminals
                .iter()
                .map(|(id, name)| (id.clone(), name.clone())),
            |this, id: String, cx| {
                this.store.update(cx, |store, cx| {
                    store.set_prefs(|p| p.terminal = Some(id), cx)
                });
            },
            cx,
        );
        let this = cx.entity().downgrade();
        let this2 = this.clone();
        vec![
            Row {
                section: Section::General,
                title: "Appearance",
                description: "Light, dark, or whichever macOS is set to.".into(),
                control: appearance.into_any_element(),
            },
            Row {
                section: Section::General,
                title: "Editor",
                description: "Where \"Open in editor\" opens files, at the line you're on.".into(),
                control: dropdown("editor", editor, theme)
                    .on_click(move |event, _, cx| {
                        this.update(cx, |this, cx| {
                            this.open_menu(event, editor_items.clone(), cx)
                        })
                        .ok();
                    })
                    .into_any_element(),
            },
            Row {
                section: Section::General,
                title: "Terminal",
                description: "Where \"Open in terminal\" opens the project.".into(),
                control: dropdown("terminal", terminal, theme)
                    .on_click(move |event, _, cx| {
                        this2
                            .update(cx, |this, cx| {
                                this.open_menu(event, terminal_items.clone(), cx)
                            })
                            .ok();
                    })
                    .into_any_element(),
            },
        ]
    }

    fn git_rows(&self, cx: &Context<Self>) -> Vec<Row> {
        let theme = Theme::get(cx);
        let items = self.choices(
            FETCH_INTERVALS.map(|minutes| (minutes, interval_label(minutes))),
            |this, minutes: Option<u64>, cx| {
                this.fetch_interval = minutes;
                cx.notify();
                let failed = "Couldn't change how often to fetch";
                cx.spawn(async move |this, cx| {
                    let saved =
                        crate::backend::run(move || backend::set_fetch_interval(minutes)).await;
                    if let Err(err) = saved {
                        this.update(cx, |this, cx| this.error(failed, err, cx)).ok();
                    }
                })
                .detach();
            },
            cx,
        );
        let this = cx.entity().downgrade();
        vec![Row {
            section: Section::Git,
            title: "Fetch automatically",
            description: "Keeps the target and each branch's remote current, for every \
                          expanded project. GitButler's other apps share this."
                .into(),
            control: dropdown("fetch", interval_label(self.fetch_interval), theme)
                .on_click(move |event, _, cx| {
                    this.update(cx, |this, cx| this.open_menu(event, items.clone(), cx))
                        .ok();
                })
                .into_any_element(),
        }]
    }

    fn project_rows(&self, window: &Window, cx: &Context<Self>) -> Vec<Row> {
        let theme = Theme::get(cx);
        let (Some(project), Some(settings)) = (&self.project, &self.project_settings) else {
            return Vec::new();
        };
        let name_changed =
            self.project_name(cx).as_deref() != Some(self.name.read(cx).text().trim());
        let name_focused = self.name.focus_handle(cx).is_focused(window);
        let path = self.store.read(cx).path(project);
        let id = project.clone();
        let force = settings.force_push_protection;
        let sign = settings.sign_commits;
        let protect = cx.listener(move |this, _: &ClickEvent, _, cx| {
            let id = id.clone();
            this.apply(
                "Couldn't change force-push protection",
                move || backend::update_project(&id, None, Some(!force)),
                cx,
            );
        });
        let sign_path = path.clone();
        let toggle_sign = cx.listener(move |this, _: &ClickEvent, _, cx| {
            let Some(path) = sign_path.clone() else {
                return;
            };
            this.apply(
                "Couldn't change commit signing",
                move || backend::set_sign_commits(&path, !sign),
                cx,
            );
        });
        let test_path = path.clone();
        let test_signing = cx.listener(move |this, _: &ClickEvent, _, cx| {
            let Some(path) = test_path.clone() else {
                return;
            };
            this.signing = None;
            cx.spawn(async move |this, cx| {
                let result = crate::backend::run(move || backend::test_signing(&path)).await;
                this.update(cx, |this, cx| {
                    this.signing = Some(result.map_err(|err| format!("{err:#}").into()));
                    cx.notify();
                })
                .ok();
            })
            .detach();
        });
        let target = self
            .store
            .read(cx)
            .workspace(project)
            .and_then(|ws| ws.target.as_ref())
            .map(|t| t.name.clone());
        let target_project = project.clone();
        let change_target = cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.pick_target(target_project.clone(), window, cx)
        });
        let remove_id = project.clone();
        let project_name = self.project_name(cx).unwrap_or_default();
        let remove = cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.remove_project(remove_id.clone(), &project_name, window, cx)
        });
        let signing_note = self.signing.as_ref().map(|result| match result {
            Ok(()) => div()
                .text_color(theme.safe)
                .child("Signing works")
                .into_any_element(),
            Err(err) => div()
                .text_color(theme.danger)
                .child(err.clone())
                .into_any_element(),
        });
        vec![
            Row {
                section: Section::Project,
                title: "Name",
                description: "What the project is called in the navigator.".into(),
                control: h_flex()
                    .gap(px(6.))
                    .when(name_changed, |el| {
                        el.child(
                            button("save-name", "Save")
                                .small()
                                .style(ButtonStyle::Gray)
                                .on_click(cx.listener(|this, _, _, cx| this.save_name(cx))),
                        )
                    })
                    .child(
                        div()
                            .w(px(220.))
                            .px(px(8.))
                            .py(px(5.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(if name_focused {
                                theme.pop
                            } else {
                                theme.border_2
                            })
                            .text_size(px(13.))
                            .child(self.name.clone()),
                    )
                    .into_any_element(),
            },
            Row {
                section: Section::Project,
                title: "Target branch",
                description: "What the workspace integrates into and pull requests aim at. \
                              Branches move onto a new one at the next update."
                    .into(),
                control: h_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .font_family(theme.mono_font.clone())
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child(target.clone().unwrap_or_else(|| "None".into())),
                    )
                    .child(
                        button("change-target", "Change…")
                            .small()
                            .on_click(change_target),
                    )
                    .into_any_element(),
            },
            Row {
                section: Section::Project,
                title: "Protect force pushes",
                description: "A force push stops if someone else pushed to the branch since \
                              you last fetched, rather than dropping their commits."
                    .into(),
                control: switch("force-push", force, theme)
                    .on_click(protect)
                    .into_any_element(),
            },
            Row {
                section: Section::Project,
                title: "Sign commits",
                description: "Sign the commits GitButler makes, with the key and program \
                              your Git config names."
                    .into(),
                control: h_flex()
                    .gap(px(8.))
                    .children(signing_note.map(|note| {
                        div()
                            .max_w(px(220.))
                            .text_size(px(12.))
                            .truncate()
                            .child(note)
                    }))
                    .when(sign, |el| {
                        el.child(
                            button("test-signing", "Test")
                                .small()
                                .style(ButtonStyle::Ghost)
                                .on_click(test_signing),
                        )
                    })
                    .child(switch("sign", sign, theme).on_click(toggle_sign))
                    .into_any_element(),
            },
            Row {
                section: Section::Project,
                title: "Remove project",
                description: "Takes it out of GitButler. The repository stays on disk as it is."
                    .into(),
                control: button("remove-project", "Remove…")
                    .small()
                    .style(ButtonStyle::Danger)
                    .on_click(remove)
                    .into_any_element(),
            },
        ]
    }

    /// Offer the project's remote branches to pick a new target from.
    fn pick_target(&mut self, project: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.store.read(cx).path(&project) else {
            return;
        };
        let app = self.app.clone();
        let window_handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let listed = {
                let path = path.clone();
                crate::backend::run(move || backend::remote_branches(&path)).await
            };
            let names = match listed {
                Ok(names) => names,
                Err(err) => {
                    this.update(cx, |this, cx| {
                        this.error("Couldn't list the remote branches", err, cx)
                    })
                    .ok();
                    return;
                }
            };
            let view = this.clone();
            let choices = names
                .into_iter()
                .map(|name| {
                    let short = name
                        .strip_prefix("refs/remotes/")
                        .unwrap_or(&name)
                        .to_owned();
                    let (path, view, project) = (path.clone(), view.clone(), project.clone());
                    crate::palette::Choice::new(short.clone(), move |app, _, cx| {
                        let store = app.store.clone();
                        cx.spawn(async move |_, cx| {
                            let set =
                                crate::backend::run(move || backend::set_target(&path, &name))
                                    .await;
                            store.update(cx, |store, cx| match set {
                                Ok(()) => {
                                    store.reload(&project, cx);
                                    store.snack(format!("The target is now {short}"), cx)
                                }
                                Err(err) => store.error(
                                    "Couldn't change the target",
                                    format!("{err:#}"),
                                    cx,
                                ),
                            });
                            view.update(cx, |_, cx| cx.notify()).ok();
                        })
                        .detach();
                    })
                })
                .collect();
            window_handle
                .update(cx, |_, window, cx| {
                    app.update(cx, |app, cx| {
                        app.open_picker("Pick the target branch", "Pick", choices, window, cx)
                    })
                    .ok();
                })
                .ok();
        })
        .detach();
    }

    fn remove_project(
        &mut self,
        project: ProjectId,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let this = cx.entity().downgrade();
        self.app
            .update(cx, |app, cx| {
                app.confirm_then(
                    format!("Remove {name} from GitButler?"),
                    "The repository and its branches stay on disk; add it again any time.",
                    "Remove",
                    window,
                    cx,
                    move |app, cx| {
                        this.update(cx, |_, cx| cx.emit(DismissEvent)).ok();
                        app.store
                            .update(cx, |store, cx| store.remove_project(&project, cx));
                    },
                )
            })
            .ok();
    }

    /// The rows to show: the section's, or every row the search matches.
    fn rows(&self, window: &Window, cx: &Context<Self>) -> (Vec<Row>, bool) {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let all = self
            .general_rows(cx)
            .into_iter()
            .chain(self.git_rows(cx))
            .chain(self.project_rows(window, cx));
        if query.is_empty() {
            return (
                all.filter(|row| row.section == self.section).collect(),
                false,
            );
        }
        let matches = |row: &Row| {
            let text =
                format!("{} {} {}", row.section.title(), row.title, row.description).to_lowercase();
            query.split_whitespace().all(|word| text.contains(word))
        };
        (all.filter(matches).collect(), true)
    }

    fn render_row(row: Row, show_section: bool, theme: &Theme) -> AnyElement {
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
                            .when(show_section, |el| {
                                el.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(theme.text_3)
                                        .child(format!("{} ›", row.section.title())),
                                )
                            })
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(row.title),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child(row.description),
                    ),
            )
            .child(row.control)
            .into_any_element()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let (rows, searching) = self.rows(window, cx);
        let search_focused = self.search.focus_handle(cx).is_focused(window);
        let sections = Section::ALL.into_iter().map(|section| {
            let selected = !searching && self.section == section;
            list_row(SharedString::from(section.title()), selected, &theme)
                .mx_0()
                .text_size(px(13.))
                .child(section.title())
                .on_click(cx.listener(move |this, _, _, cx| {
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
        let projects: Vec<(ProjectId, String)> = self
            .store
            .read(cx)
            .projects
            .iter()
            .map(|entry| (entry.id.clone(), entry.project.name.clone()))
            .collect();
        let pick_project = (!searching && self.section == Section::Project && projects.len() > 1)
            .then(|| {
                let items = self.choices(
                    projects,
                    |this, id: ProjectId, cx| this.show_project(Some(id), cx),
                    cx,
                );
                let this = cx.entity().downgrade();
                dropdown("pick-project", "Other project", &theme).on_click(move |event, _, cx| {
                    this.update(cx, |this, cx| this.open_menu(event, items.clone(), cx))
                        .ok();
                })
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
                    .child(
                        div()
                            .mb(px(8.))
                            .px(px(8.))
                            .py(px(5.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(if search_focused {
                                theme.pop
                            } else {
                                theme.border_2
                            })
                            .bg(theme.bg_1)
                            .text_size(px(13.))
                            .child(self.search.clone()),
                    )
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
                                    .map(|row| Self::render_row(row, searching, &theme)),
                            ),
                    ),
            )
    }
}

impl AppView {
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
