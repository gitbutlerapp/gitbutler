//! The Project section: the selected project's name, target branch, force-push
//! protection and commit signing, and removing it from GitButler.

use gpui::{App, ClickEvent, Context, DismissEvent, Focusable as _, Window, div, prelude::*, px};

use super::{Row, Section, SettingsView};
use crate::backend::settings::{self as backend, GitHubStacks, StackInfo};
use crate::state::ProjectId;
use crate::theme::Theme;
use crate::ui::{ButtonStyle, button, dropdown, h_flex, switch};

impl SettingsView {
    /// Show `project`'s settings in the Project section, loading them.
    pub(super) fn show_project(&mut self, project: Option<ProjectId>, cx: &mut Context<Self>) {
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

    pub(super) fn project_name(&self, cx: &App) -> Option<String> {
        let project = self.project.as_ref()?;
        Some(self.store.read(cx).project(project)?.project.name.clone())
    }

    pub(super) fn reset_name(&mut self, cx: &mut Context<Self>) {
        let name = self.project_name(cx).unwrap_or_default();
        self.name.update(cx, |input, cx| input.set_text(name, cx));
    }

    pub(super) fn save_name(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn project_rows(&self, window: &Window, cx: &Context<Self>) -> Vec<Row> {
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
        let info_items = self.choices(
            [
                (StackInfo::Bottom, "At the bottom".to_owned()),
                (StackInfo::Top, "At the top".to_owned()),
                (StackInfo::Disabled, "Off".to_owned()),
            ],
            {
                let path = path.clone();
                move |this, info: StackInfo, cx| {
                    let Some(path) = path.clone() else {
                        return;
                    };
                    this.apply(
                        "Couldn't change how stacks show",
                        move || backend::set_stacking(&path, Some(info), None),
                        cx,
                    )
                }
            },
            cx,
        );
        let github_items = self.choices(
            [
                (GitHubStacks::Auto, "When GitHub offers them".to_owned()),
                (GitHubStacks::Native, "Always".to_owned()),
                (GitHubStacks::Disabled, "Never".to_owned()),
            ],
            {
                let path = path.clone();
                move |this, github: GitHubStacks, cx| {
                    let Some(path) = path.clone() else {
                        return;
                    };
                    this.apply(
                        "Couldn't change how stacks show",
                        move || backend::set_stacking(&path, None, Some(github)),
                        cx,
                    )
                }
            },
            cx,
        );
        let info_label = match settings.stack_info {
            StackInfo::Bottom => "At the bottom",
            StackInfo::Top => "At the top",
            StackInfo::Disabled => "Off",
        };
        let github_label = match settings.github_stacks {
            GitHubStacks::Auto => "When GitHub offers them",
            GitHubStacks::Native => "Always",
            GitHubStacks::Disabled => "Never",
        };
        let (menu_a, menu_b) = (cx.entity().downgrade(), cx.entity().downgrade());
        let stack_info = dropdown("stack-info", info_label, theme)
            .on_click(move |event, _, cx| {
                menu_a
                    .update(cx, |this, cx| this.open_menu(event, info_items.clone(), cx))
                    .ok();
            })
            .into_any_element();
        let github_stacks = dropdown("github-stacks", github_label, theme)
            .on_click(move |event, _, cx| {
                menu_b
                    .update(cx, |this, cx| {
                        this.open_menu(event, github_items.clone(), cx)
                    })
                    .ok();
            })
            .into_any_element();
        let on_github = self
            .store
            .read(cx)
            .workspace(project)
            .and_then(|ws| ws.forge.as_deref())
            == Some("GitHub");
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
        let mut rows = vec![
            Row {
                section: Section::Project,
                title: "Name".into(),
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
                title: "Target branch".into(),
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
                title: "Protect force pushes".into(),
                description: "A force push stops if someone else pushed to the branch since \
                              you last fetched, rather than dropping their commits."
                    .into(),
                control: switch("force-push", force, theme)
                    .on_click(protect)
                    .into_any_element(),
            },
            Row {
                section: Section::Project,
                title: "Sign commits".into(),
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
                title: "Stack in pull requests".into(),
                description: "Each pull request of a stack lists the others in its description, \
                              kept up to date."
                    .into(),
                control: stack_info,
            },
            Row {
                section: Section::Project,
                title: "Remove project".into(),
                description: "Takes it out of GitButler. The repository stays on disk as it is."
                    .into(),
                control: button("remove-project", "Remove…")
                    .small()
                    .style(ButtonStyle::Danger)
                    .on_click(remove)
                    .into_any_element(),
            },
        ];
        if on_github {
            let at = rows.len() - 1;
            rows.insert(
                at,
                Row {
                    section: Section::Project,
                    title: "GitHub stacks".into(),
                    description: "Show a stack's pull requests as one of GitHub's own stacks, \
                                  where GitHub offers them."
                        .into(),
                    control: github_stacks,
                },
            );
        }
        rows
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
}
