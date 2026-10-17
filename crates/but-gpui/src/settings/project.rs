//! The Project section: the selected project's name, target branch, force-push
//! protection and commit signing, and removing it from GitButler.

use gpui::{App, ClickEvent, Context, DismissEvent, Window, div, prelude::*, px};

use super::{Row, SettingsView, field_with_save};
use crate::backend::Forge;
use crate::backend::settings::{self as backend, GitHubStacks, StackInfo};
use crate::state::ProjectId;
use crate::theme::Theme;
use crate::ui::{ButtonStyle, button, h_flex, muted, switch};

const STACK_INFO: [(StackInfo, &str); 3] = [
    (StackInfo::Bottom, "At the bottom"),
    (StackInfo::Top, "At the top"),
    (StackInfo::Disabled, "Off"),
];

const GITHUB_STACKS: [(GitHubStacks, &str); 3] = [
    (GitHubStacks::Auto, "When GitHub offers them"),
    (GitHubStacks::Native, "Always"),
    (GitHubStacks::Disabled, "Never"),
];

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

    /// Read the project list and the project's settings again, as after changing them.
    fn reload_project(&mut self, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| store.load_projects(cx));
        self.show_project(self.project.clone(), cx);
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

    pub(super) fn project_rows(&self, window: &Window, cx: &Context<Self>) -> Vec<Row> {
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
        let (test_path, tested) = (path.clone(), project.clone());
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
        let workspace = self.store.read(cx).workspace(project);
        let target = workspace
            .and_then(|ws| ws.target.as_ref())
            .map(|t| t.name.clone());
        // With stacks applied, their commits would be read against the new target from
        // where the old one forked; GitButler's other apps don't allow it either.
        let has_stacks = workspace.is_some_and(|ws| !ws.stacks.is_empty());
        let target_project = project.clone();
        let change_target = cx.listener(move |this, _: &ClickEvent, window, cx| {
            let project = target_project.clone();
            this.app
                .update(cx, |app, cx| app.pick_target(project, window, cx))
                .ok();
        });
        let info_label = (STACK_INFO.iter())
            .find(|(info, _)| *info == settings.stack_info)
            .map_or("", |(_, label)| *label);
        let stack_info = self.choice(
            "stack-info",
            info_label,
            STACK_INFO.map(|(info, label)| (info, label.to_owned())),
            {
                let path = path.clone();
                move |this, info: StackInfo, cx| {
                    let path = path.clone();
                    this.apply(
                        "Couldn't change how stacks show",
                        move || backend::set_stacking(&path, Some(info), None),
                        Self::reload_project,
                        cx,
                    )
                }
            },
            cx,
        );
        let github_label = (GITHUB_STACKS.iter())
            .find(|(github, _)| *github == settings.github_stacks)
            .map_or("", |(_, label)| *label);
        let github_stacks = self.choice(
            "github-stacks",
            github_label,
            GITHUB_STACKS.map(|(github, label)| (github, label.to_owned())),
            move |this, github: GitHubStacks, cx| {
                let path = path.clone();
                this.apply(
                    "Couldn't change how stacks show",
                    move || backend::set_stacking(&path, None, Some(github)),
                    Self::reload_project,
                    cx,
                )
            },
            cx,
        );
        let on_github = self
            .store
            .read(cx)
            .workspace(project)
            .and_then(|ws| ws.forge)
            == Some(Forge::GitHub);
        let remove_id = project.clone();
        let project_name = self.project_name(cx).unwrap_or_default();
        let remove = cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.remove_project(remove_id.clone(), &project_name, window, cx)
        });
        let signing_note = self.signing.as_ref().map(|result| match result {
            Ok(()) => div().text_color(theme.safe).child("Signing works"),
            Err(err) => div().text_color(theme.danger).child(err.clone()),
        });
        let mut rows = vec![
            Row::new(
                "Name",
                "What the project is called in the navigator.",
                field_with_save(
                    &self.name,
                    name_changed,
                    |this, _, cx| this.save_name(cx),
                    window,
                    cx,
                ),
            ),
            Row::new(
                "Target branch",
                if has_stacks {
                    "What the workspace integrates into and pull requests aim at. To change it, \
                     unapply every branch first."
                } else {
                    "What the workspace integrates into and pull requests aim at."
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
                            .disabled(has_stacks)
                            .on_click(change_target),
                    ),
            ),
            Row::new(
                "Protect force pushes",
                "A force push stops if someone else pushed to the branch since you last fetched, \
                 rather than dropping their commits.",
                switch("force-push", force, theme).on_click(protect),
            ),
            Row::new(
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
            ),
            Row::new(
                "Stack in pull requests",
                "Each pull request of a stack lists the others in its description, kept up to \
                 date.",
                stack_info,
            ),
            Row::new(
                "Remove project",
                "Takes it out of GitButler. The repository stays on disk as it is.",
                button("remove-project", "Remove…")
                    .small()
                    .style(ButtonStyle::Danger)
                    .on_click(remove),
            ),
        ];
        if on_github {
            let at = rows.len() - 1;
            rows.insert(
                at,
                Row::new(
                    "GitHub stacks",
                    "Show a stack's pull requests as one of GitHub's own stacks, where GitHub \
                     offers them.",
                    github_stacks,
                ),
            );
        }
        rows
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
