//! A project's pages: its name, target branch and forge, commit signing and Git hooks, and
//! removing it from GitButler.

use gpui::{App, ClickEvent, Context, DismissEvent, Task, Window, div, prelude::*};

use super::{Card, Page, ProjectPage, Row, SettingsView, field_with_save, read_failed};
use crate::backend::{Checkout, Forge, settings as backend};
use crate::state::ProjectId;
use crate::theme::Theme;
use crate::ui::{ButtonStyle, button, dp, error_summary, h_flex, muted, switch};

impl SettingsView {
    /// Drop what was shown of the project before, as another's settings load.
    pub(super) fn forget_project(&mut self, cx: &mut Context<Self>) {
        self.reset_name(cx);
        self.project_settings = None;
        self.signing = None;
        self.credentials = None;
        self._credentials_test = Task::ready(());
    }

    /// Read the shown project's settings, and its workspace once when it isn't watched, for
    /// its target. What's shown of them stays until they're read, so reading them again
    /// doesn't flash Loading, and the name field keeps what it holds: a new name reaches the
    /// store only once the project list is read again.
    pub(super) fn load_project(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.page.project().cloned() else {
            return;
        };
        let Some(entry) = self.store.read(cx).project(&project) else {
            return;
        };
        let path = entry.project.path.clone();
        let unread = !entry.expanded && entry.workspace.is_none();
        self._load = cx.spawn(async move |this, cx| {
            let loaded = crate::backend::run(move || backend::project_settings(&path)).await;
            this.update(cx, |this, cx| {
                this.project_settings = Some(loaded.map_err(|err| format!("{err:#}").into()));
                cx.notify();
            })
            .ok();
            if unread {
                // A moment later, as the store waits, so arrowing past projects not watched
                // doesn't read each one: showing another drops this.
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;
                this.update(cx, |this, cx| {
                    this.store
                        .update(cx, |store, cx| store.reload(&project, cx))
                })
                .ok();
            }
        });
    }

    /// Read the project list and the project's settings again, as after changing them.
    fn reload_project(&mut self, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| store.load_projects(cx));
        self.load_project(cx);
    }

    /// [`Self::reload_project`], then ask the forge about pull requests and CI again, as after
    /// picking which forge it is.
    fn reload_forge(&mut self, cx: &mut Context<Self>) {
        self.reload_project(cx);
        if let Some(project) = self.page.project().cloned() {
            self.store
                .update(cx, |store, _| store.refresh_forge(&project));
        }
    }

    pub(super) fn project_name(&self, cx: &App) -> Option<String> {
        let project = self.page.project()?;
        Some(self.store.read(cx).project(project)?.project.name.clone())
    }

    pub(super) fn reset_name(&mut self, cx: &mut Context<Self>) {
        let name = self.project_name(cx).unwrap_or_default();
        self.name.update(cx, |input, cx| input.set_text(name, cx));
    }

    pub(super) fn save_name(&mut self, cx: &mut Context<Self>) {
        let (Some(project), Some(current)) = (self.page.project().cloned(), self.project_name(cx))
        else {
            return;
        };
        let name = self.name.read(cx).text().trim().to_owned();
        if name.is_empty() || name == current {
            return self.reset_name(cx);
        }
        self.apply(
            "Couldn't rename the project",
            move || backend::rename_project(&project, name),
            Self::reload_project,
            cx,
        );
    }

    /// `project`'s own pages, while it's the one shown: Project, with its name and target, and
    /// Git, with how it pushes, commits and signs.
    pub(super) fn project_cards(
        &self,
        project: &ProjectId,
        page: ProjectPage,
        window: &Window,
        cx: &Context<Self>,
    ) -> Vec<Card> {
        let theme = Theme::get(cx);
        let settings =
            (self.project_settings.as_ref()).filter(|_| self.page.project() == Some(project));
        let Some(settings) = settings else {
            return Vec::new();
        };
        let settings = match settings {
            Ok(settings) => settings,
            Err(err) => {
                let retry = |this: &mut Self, cx: &mut Context<Self>| this.load_project(cx);
                let title = "Couldn't read the project's settings";
                let failed =
                    read_failed(Page::Project(project.clone(), page), title, err, retry, cx);
                return vec![Card::new(None, vec![failed])];
            }
        };
        // Settings are only read for a project with a path.
        let Some(path) = self.store.read(cx).path(project) else {
            return Vec::new();
        };
        let name_changed =
            self.project_name(cx).as_deref() != Some(self.name.read(cx).text().trim());
        let sign = settings.sign_commits;
        let sign_path = path.clone();
        let toggle_sign = cx.listener(move |this, _: &ClickEvent, _, cx| {
            let path = sign_path.clone();
            this.apply(
                "Couldn't change commit signing",
                move || backend::set_sign_commits(&path, !sign),
                Self::reload_project,
                cx,
            );
        });
        let credentials_path = path.clone();
        let test_credentials = cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.credentials = Some([None, None]);
            cx.notify();
            let path = credentials_path.clone();
            this._credentials_test = cx.spawn(async move |this, cx| {
                // Push even when fetching failed: reading and writing don't always take the
                // same credentials.
                for (step, push) in [false, true].into_iter().enumerate() {
                    let path = path.clone();
                    let result =
                        crate::backend::run(move || backend::test_credentials(&path, push)).await;
                    this.update(cx, |this, cx| {
                        if let Some(check) = this.credentials.iter_mut().flatten().nth(step) {
                            *check = Some(result.map_err(|err| format!("{err:#}").into()));
                            cx.notify();
                        }
                    })
                    .ok();
                }
            });
        });
        let push_path = path.clone();
        let (test_path, tested) = (path, project.clone());
        let test_signing = cx.listener(move |this, _: &ClickEvent, _, cx| {
            let path = test_path.clone();
            this.signing = None;
            let tested = tested.clone();
            cx.spawn(async move |this, cx| {
                let result = crate::backend::run(move || backend::test_signing(&path)).await;
                this.update(cx, |this, cx| {
                    // Unless Settings moved on to another project meanwhile.
                    if this.page.project() == Some(&tested) {
                        this.signing = Some(result.map_err(|err| format!("{err:#}").into()));
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        });
        // Whether hooks run is this app's choice, kept per project: the prefs list the projects
        // that run them.
        let hooks_project = project.to_string();
        let hooks = switch("run-hooks", self.store.read(cx).runs_hooks(project), theme).on_click(
            cx.listener(move |this, _: &ClickEvent, _, cx| {
                let project = hooks_project.clone();
                this.store.update(cx, |store, cx| {
                    store.set_prefs(
                        |prefs| {
                            if !prefs.run_hooks.remove(&project) {
                                prefs.run_hooks.insert(project);
                            }
                        },
                        cx,
                    )
                });
            }),
        );
        let workspace = self.store.read(cx).workspace(project);
        let target = workspace
            .and_then(|ws| ws.target_name())
            .map(|name| name.into_owned());
        // With stacks applied, their commits would be read against the new target from
        // where the old one forked; GitButler's other apps don't allow it either. Outside
        // the workspace, the branch checked out shows as a stack, but a new target leaves it
        // as it is.
        let locked = target.is_some()
            && workspace
                .is_some_and(|ws| ws.managed && ws.lanes_in(&Checkout::Main).next().is_some());
        let target_project = project.clone();
        let change_target = cx.listener(move |this, _: &ClickEvent, window, cx| {
            let project = target_project.clone();
            this.app.update_later(window, cx, move |app, window, cx| {
                app.pick_target(project, window, cx)
            });
        });
        let remove_id = project.clone();
        let project_name = self.project_name(cx).unwrap_or_default();
        let remove = cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.remove_project(remove_id.clone(), &project_name, window, cx)
        });
        let signing_note = self.signing.as_ref().map(|result| match result {
            Ok(()) => div().text_color(theme.safe).child("Signing works"),
            Err(err) => div().text_color(theme.danger).child(err.clone()),
        });
        let name = Row::new(
            "Name",
            "What the project is called in the navigator.",
            field_with_save(
                &self.name,
                name_changed,
                |this, _, cx| this.save_name(cx),
                window,
                cx,
            ),
        );
        let target = Row::new(
            "Target branch",
            if locked {
                "The branch your work goes into, and pull requests aim at. To change it, \
                     unapply every branch first."
            } else {
                "The branch your work goes into, and pull requests aim at."
            },
            h_flex()
                .gap(dp(8.))
                .child(
                    muted(target.unwrap_or_else(|| "None".into()), theme)
                        .font_family(theme.mono_font.clone()),
                )
                .child(
                    button("change-target", "Change…")
                        .small()
                        .disabled_because(locked.then_some("Unapply every branch first"))
                        .on_click(change_target),
                ),
        );
        // As GitButler's other apps have it: the forge the remote names can't be changed, and
        // a pick is offered only when it names none, or to clear one saved before.
        let name_of = |forge: Option<Forge>| forge.map_or("None", Forge::name);
        let forge_id = project.clone();
        let pick_forge = self.choice(
            "forge",
            name_of(settings.forge_override),
            [
                None,
                Some(Forge::GitHub),
                Some(Forge::GitLab),
                Some(Forge::Bitbucket),
                Some(Forge::Azure),
            ]
            .map(|forge| (forge, name_of(forge).to_owned())),
            move |this, forge, _, cx| {
                let id = forge_id.clone();
                this.apply(
                    "Couldn't change the project's forge",
                    move || backend::set_forge_override(&id, forge),
                    Self::reload_forge,
                    cx,
                )
            },
            cx,
        );
        let forge = match (settings.detected_forge, settings.forge_override) {
            (Some(detected), None) => Row::new(
                "Forge",
                "Where pull requests and CI are, as the remote's host names it.",
                muted(detected.name(), theme),
            ),
            (Some(detected), Some(_)) => Row::new(
                "Forge",
                format!(
                    "The remote's host names {}, so the forge picked here doesn't count. \
                     None clears it.",
                    detected.name()
                ),
                pick_forge,
            ),
            (None, _) => Row::new(
                "Forge",
                "Where pull requests and CI are, for a remote whose host doesn't say, as a \
                 self-hosted forge's. GitHub, GitLab and Bitbucket can open pull requests.",
                pick_forge,
            ),
        };
        // As the desktop app offers it, only where there's a choice.
        let push_to = (settings.push_remote.clone())
            .filter(|_| settings.remotes.len() > 1)
            .map(|current| {
                let pick = self.choice(
                    "push-remote",
                    current,
                    (settings.remotes.iter()).map(|remote| (remote.clone(), remote.clone())),
                    move |this, remote, _, cx| {
                        let path = push_path.clone();
                        this.apply(
                            "Couldn't change where branches are pushed",
                            move || backend::set_push_remote(&path, remote),
                            Self::reload_project,
                            cx,
                        )
                    },
                    cx,
                );
                Row::new(
                    "Push new branches to",
                    "The remote a branch goes to on its first push, as your fork when the \
                     target is the original repository's. Updates come from the target \
                     branch's remote.",
                    pick,
                )
            });
        // The push checks this itself, whatever the setting GitButler's other apps share says.
        let force = Row::new(
            "Force pushes",
            "A force push fetches first, stops if someone else pushed to the branch since, and \
             asks before dropping commits only the remote has, so there's no protection to turn \
             on here.",
            muted("Always checked", theme),
        );
        let sign = Row::new(
            "Sign commits",
            "Sign the commits GitButler makes, with the key and program your Git config names.",
            h_flex()
                .gap(dp(8.))
                .children(signing_note.map(|note| {
                    div()
                        .max_w(dp(220.))
                        .text_size(dp(12.))
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
                .child(switch("sign", sign, theme).on_click(toggle_sign)),
        );
        let remote = (workspace.and_then(|ws| ws.target.as_ref())).is_some_and(|t| !t.local);
        let testing = (self.credentials.iter().flatten()).any(Option::is_none);
        let label = if testing { "Testing…" } else { "Test" };
        let credentials = Row::new(
            "Credentials",
            if remote {
                "Fetches from the target branch's remote, then pushes an empty branch to the one \
                 branches are pushed to and deletes it."
            } else {
                "Needs a target branch with a remote to test against."
            },
            button("test-credentials", label)
                .small()
                .disabled_because(testing.then_some("Testing"))
                .disabled_because((!remote).then_some("The target branch has no remote"))
                .on_click(test_credentials),
        );
        let checks = ["Fetch", "Push"]
            .into_iter()
            .zip(self.credentials.iter().flatten());
        let checks = checks.map(|(name, check)| match check {
            None => Row::new(name, "", muted("Checking…", theme)),
            Some(Ok(())) => Row::new(name, "", muted("Works", theme).text_color(theme.safe)),
            Some(Err(err)) => Row::new(
                name,
                error_summary(err),
                muted("Failed", theme).text_color(theme.danger),
            ),
        });
        let hooks = Row::new(
            "Run Git hooks",
            "Runs the repository's pre-commit, commit-msg, post-commit and pre-push hooks \
                 in the main checkout. A linked worktree's commits don't run them.",
            hooks,
        );
        let remove = Row::new(
            "Remove project",
            "Takes it out of GitButler, and its operations history with it. The repository's \
             files and branches stay on disk.",
            button("remove-project", "Remove…")
                .small()
                .style(ButtonStyle::Danger)
                .on_click(remove),
        );
        match page {
            ProjectPage::Project => vec![
                Card::new(None, vec![name, target, forge]),
                Card::new(None, vec![remove]),
            ],
            ProjectPage::Git => vec![
                Card::new(None, push_to.into_iter().chain([force, hooks]).collect()),
                Card::new(Some("Signing"), vec![sign]),
                Card::new(
                    Some("Authentication"),
                    std::iter::once(credentials).chain(checks).collect(),
                ),
            ],
        }
    }

    fn remove_project(
        &mut self,
        project: ProjectId,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (this, title) = (
            cx.entity().downgrade(),
            format!("Remove {name} from GitButler?"),
        );
        self.app.update_later(window, cx, move |app, window, cx| {
            app.confirm_then(
                None,
                title,
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
        });
    }
}
