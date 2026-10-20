//! The project's pages: the selected project's name, target branch and forge, force-push
//! protection, commit signing and Git hooks, and removing it from GitButler.

use std::collections::HashSet;

use gpui::{App, ClickEvent, Context, DismissEvent, Window, div, prelude::*, px};

use super::{Card, Page, Row, SettingsView, field_with_save};
use crate::backend::{Forge, settings as backend};
use crate::state::{Prefs, ProjectId};
use crate::theme::Theme;
use crate::ui::{ButtonStyle, button, h_flex, muted, switch};

impl SettingsView {
    /// Show `project`'s settings on its pages, loading them.
    pub(super) fn show_project(&mut self, project: Option<ProjectId>, cx: &mut Context<Self>) {
        // Reloading the same project keeps what the field holds: a new name reaches the
        // store only once the project list is read again.
        if self.project != project {
            self.project = project.clone();
            self.reset_name(cx);
        }
        self.project_settings = None;
        self.signing = None;
        let Some(project) = project else {
            return;
        };
        let Some(path) = self.store.read(cx).path(&project) else {
            return;
        };
        self._load = cx.spawn(async move |this, cx| {
            let loaded = crate::backend::run(move || backend::project_settings(&path)).await;
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

    /// Read the project list and the project's settings again, as after changing them.
    fn reload_project(&mut self, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| store.load_projects(cx));
        self.show_project(self.project.clone(), cx);
    }

    /// [`Self::reload_project`], then ask the forge about pull requests and CI again, as after
    /// picking which forge it is.
    fn reload_forge(&mut self, cx: &mut Context<Self>) {
        self.reload_project(cx);
        if let Some(project) = self.project.clone() {
            self.store
                .update(cx, |store, _| store.refresh_forge(&project));
        }
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
            Self::reload_project,
            cx,
        );
    }

    /// The project's own pages: Project, with its name and target, and Git, with how it
    /// pushes, commits and signs.
    pub(super) fn project_cards(
        &self,
        page: Page,
        window: &Window,
        cx: &Context<Self>,
    ) -> Vec<Card> {
        let theme = Theme::get(cx);
        let (Some(project), Some(settings)) = (&self.project, &self.project_settings) else {
            return Vec::new();
        };
        // Settings are only read for a project with a path.
        let Some(path) = self.store.read(cx).path(project) else {
            return Vec::new();
        };
        let name_changed =
            self.project_name(cx).as_deref() != Some(self.name.read(cx).text().trim());
        let id = project.clone();
        let force = settings.force_push_protection;
        let sign = settings.sign_commits;
        let protect = cx.listener(move |this, _: &ClickEvent, _, cx| {
            let id = id.clone();
            this.apply(
                "Couldn't change force-push protection",
                move || backend::update_project(&id, None, Some(!force)),
                Self::reload_project,
                cx,
            );
        });
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
        let (test_path, tested) = (path, project.clone());
        let test_signing = cx.listener(move |this, _: &ClickEvent, _, cx| {
            let path = test_path.clone();
            this.signing = None;
            let tested = tested.clone();
            cx.spawn(async move |this, cx| {
                let result = crate::backend::run(move || backend::test_signing(&path)).await;
                this.update(cx, |this, cx| {
                    // Unless Settings moved on to another project meanwhile.
                    if this.project.as_ref() == Some(&tested) {
                        this.signing = Some(result.map_err(|err| format!("{err:#}").into()));
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        });
        // A switch for one of this app's choices made per project: the projects it's on.
        let project_switch =
            |id: &'static str, on: bool, projects: fn(&mut Prefs) -> &mut HashSet<String>| {
                let project = project.to_string();
                switch(id, on, theme).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let project = project.clone();
                    this.store.update(cx, |store, cx| {
                        store.set_prefs(
                            |prefs| {
                                let projects = projects(prefs);
                                if !projects.remove(&project) {
                                    projects.insert(project);
                                }
                            },
                            cx,
                        )
                    });
                }))
            };
        let store = self.store.read(cx);
        let (runs_hooks, lands) = (store.runs_hooks(project), store.lands_directly(project));
        let hooks = project_switch("run-hooks", runs_hooks, |prefs| &mut prefs.run_hooks);
        let land = project_switch("land-directly", lands, |prefs| &mut prefs.land_directly);
        let workspace = self.store.read(cx).workspace(project);
        let target = workspace
            .and_then(|ws| ws.target.as_ref())
            .map(|t| t.name.clone());
        // With stacks applied, their commits would be read against the new target from
        // where the old one forked; GitButler's other apps don't allow it either. Outside
        // the workspace, the branch checked out shows as a stack, but a new target leaves it
        // as it is.
        let locked =
            target.is_some() && workspace.is_some_and(|ws| ws.managed && !ws.stacks.is_empty());
        let target_project = project.clone();
        let change_target = cx.listener(move |this, _: &ClickEvent, window, cx| {
            let project = target_project.clone();
            this.app
                .update(cx, |app, cx| app.pick_target(project, window, cx))
                .ok();
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
                .gap(px(8.))
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
            move |this, forge, cx| {
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
        let force = Row::new(
            "Protect force pushes",
            "A force push stops if someone else pushed to the branch since you last fetched, \
                 rather than dropping their commits.",
            switch("force-push", force, theme).on_click(protect),
        );
        let sign = Row::new(
            "Sign commits",
            "Sign the commits GitButler makes, with the key and program your Git config names.",
            h_flex()
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
                .child(switch("sign", sign, theme).on_click(toggle_sign)),
        );
        let hooks = Row::new(
            "Run Git hooks",
            "Runs the repository's pre-commit, commit-msg, post-commit and pre-push hooks \
                 in the main checkout. A linked worktree's commits don't run them.",
            hooks,
        );
        let land = Row::new(
            "Land branches directly",
            "A branch offers Land in place of Create pull request, putting it straight \
                 onto the target and pushing that, with no pull request. Works without a forge.",
            land,
        );
        let remove = Row::new(
            "Remove project",
            "Takes it out of GitButler. The repository stays on disk as it is.",
            button("remove-project", "Remove…")
                .small()
                .style(ButtonStyle::Danger)
                .on_click(remove),
        );
        match page {
            Page::Project => vec![
                Card::new(None, vec![name, target, forge]),
                Card::new(None, vec![remove]),
            ],
            _ => vec![
                Card::new(None, vec![force, hooks, land]),
                Card::new(Some("Signing"), vec![sign]),
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
