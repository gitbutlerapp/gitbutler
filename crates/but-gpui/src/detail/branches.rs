//! The Branches page: branches outside the workspace, local ones first, then the ones only a
//! remote has, each with what it would bring in, ready to apply or delete.

use std::ops::Range;

use gpui::{
    AnyElement, Context, Focusable as _, MouseButton, MouseDownEvent, Pixels, Point, SharedString,
    Window, div, prelude::*, px, uniform_list,
};

use super::Detail;
use crate::app::MenuItem;
use crate::backend::branches::{self, ApplyOutcome, BranchCommit, OtherBranch};
use crate::backend::{self, CommitId, DiffSource};
use crate::navigator::drag_preview;
use crate::state::{Cut, Overlay, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{Icon, empty_state, field, list_row, relative_time, small_icon};

pub(super) struct Branches {
    pub project: ProjectId,
    /// The workspace's branches and target when the list loaded; a reload that changes
    /// neither leaves the list as it is. `None` until it has loaded.
    pub seen: Option<u64>,
    pub list: Vec<OtherBranch>,
    /// The selected branch, by ref.
    pub selected: Option<String>,
    /// The selected branch's commits, by what they were loaded for.
    pub commits: Option<(Loaded, Vec<BranchCommit>)>,
    /// One of those commits, to see on its own and to copy into the workspace.
    pub commit: Option<CommitId>,
}

impl Branches {
    fn selected_branch(&self) -> Option<&OtherBranch> {
        let selected = self.selected.as_ref()?;
        self.list.iter().find(|b| b.ref_name == *selected)
    }
}

/// A branch's tip and how many commits it has that the target doesn't: a fetch can land
/// some of its commits on the target without moving its tip.
type Loaded = (CommitId, Option<usize>);

/// A row of the list: a group's heading, a branch by index into the list, or a commit of
/// the selected branch.
enum Row {
    Heading(&'static str),
    Branch(usize),
    Commit(usize),
}

impl Detail {
    /// List `project`'s other branches; unless `force`, only when its workspace changed.
    pub(super) fn load_branches(
        &mut self,
        project: &ProjectId,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.store.read(cx).path(project) else {
            return;
        };
        let seen = self.branches_fingerprint(project, cx);
        if !force
            && self
                .branches_for(project)
                .is_some_and(|b| b.seen == Some(seen))
        {
            return;
        }
        let project = project.clone();
        self._branches_load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || branches::other_branches(&path)).await;
            this.update(cx, |this, cx| {
                let list = match result {
                    Ok(list) => list,
                    Err(err) => {
                        this.store.update(cx, |store, cx| {
                            store.error("Couldn't list the branches", format!("{err:#}"), cx)
                        });
                        Vec::new()
                    }
                };
                let previous = this.branches.take().filter(|b| b.project == project);
                // Local branches list first, so the first of those when there is one.
                let first = list.iter().find(|b| b.local).or(list.first());
                let selected = (previous.as_ref().and_then(|b| b.selected.clone()))
                    .filter(|ref_name| list.iter().any(|b| b.ref_name == *ref_name))
                    .or_else(|| first.map(|b| b.ref_name.clone()));
                // What was loaded for the selected branch, as long as it stays selected.
                let (commits, commit) = (previous.filter(|b| b.selected == selected))
                    .map_or((None, None), |b| (b.commits, b.commit));
                this.branches = Some(Branches {
                    project,
                    seen: Some(seen),
                    list,
                    selected,
                    commits,
                    commit,
                });
                this.load_branch_commits(cx);
                this.refresh_diff(false, cx);
                cx.notify();
            })
            .ok();
        });
    }

    /// What of the workspace a branch listing depends on: its branch tips and how far
    /// behind the target it is, which changes when a fetch brings in new refs.
    fn branches_fingerprint(&self, project: &ProjectId, cx: &Context<Self>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        if let Some(workspace) = self.store.read(cx).workspace(project) {
            for branch in workspace.branches() {
                branch.ref_name.hash(&mut hasher);
                branch.commits.first().map(|c| c.id.0).hash(&mut hasher);
            }
            workspace
                .target
                .as_ref()
                .map(|t| t.behind)
                .hash(&mut hasher);
        }
        hasher.finish()
    }

    fn branches_for(&self, project: &ProjectId) -> Option<&Branches> {
        self.branches.as_ref().filter(|b| b.project == *project)
    }

    /// The branch the page has selected.
    pub(super) fn selected_other_branch(&self, project: &ProjectId) -> Option<&OtherBranch> {
        self.branches_for(project)?.selected_branch()
    }

    pub(super) fn branches_source(&self, project: &ProjectId) -> Option<DiffSource> {
        if let Some(id) = self.selected_branch_commit(project) {
            return Some(DiffSource::Commit(id));
        }
        let tip = self.selected_other_branch(project)?.tip;
        Some(DiffSource::Branch { tip })
    }

    /// The commit of the selected branch the page shows, if one is selected.
    pub(super) fn selected_branch_commit(&self, project: &ProjectId) -> Option<CommitId> {
        self.branches_for(project)?.commit
    }

    /// The title of the selected branch's selected commit.
    pub(super) fn selected_branch_commit_title(&self, project: &ProjectId) -> Option<String> {
        let branches = self.branches_for(project)?;
        let (_, commits) = branches.commits.as_ref()?;
        let commit = commits.iter().find(|c| Some(c.id) == branches.commit)?;
        Some(commit.title.clone())
    }

    /// Load the selected branch's commits, unless they're loaded for it as it is already. A
    /// selected commit the branch no longer has is let go.
    fn load_branch_commits(&mut self, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        let Some(branch) = branches.selected_branch() else {
            return;
        };
        let key = (branch.tip, branch.ahead);
        if branches
            .commits
            .as_ref()
            .is_some_and(|(loaded, _)| *loaded == key)
        {
            return;
        }
        let Some(path) = self.store.read(cx).path(&branches.project) else {
            return;
        };
        let ref_name = branch.ref_name.clone();
        self._branch_commits_load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || branches::branch_commits(&path, &ref_name)).await;
            this.update(cx, |this, cx| {
                let commits = match result {
                    Ok(commits) => commits,
                    // Left unloaded, to try again when the branch is next selected.
                    Err(err) => {
                        return tracing::warn!("couldn't list the branch's commits: {err:#}");
                    }
                };
                // Unless another branch was selected meanwhile, or this one moved.
                let Some(branches) = (this.branches.as_mut())
                    .filter(|b| b.selected_branch().is_some_and(|b| (b.tip, b.ahead) == key))
                else {
                    return;
                };
                let kept = branches
                    .commit
                    .filter(|id| commits.iter().any(|c| c.id == *id));
                let dropped = branches.commit.is_some() && kept.is_none();
                branches.commit = kept;
                branches.commits = Some((key, commits));
                if dropped {
                    this.refresh_diff(true, cx);
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// The branches the filter keeps, local ones first, under headings.
    fn branch_rows(&self, branches: &Branches, cx: &Context<Self>) -> Vec<Row> {
        let query = self.branch_filter.read(cx).text().trim().to_lowercase();
        let keeps = |branch: &OtherBranch| {
            query.is_empty()
                || branch.name.to_lowercase().contains(&query)
                || branch
                    .author
                    .as_ref()
                    .is_some_and(|author| author.to_lowercase().contains(&query))
        };
        let mut rows = Vec::new();
        for (heading, local) in [("Local", true), ("Remote", false)] {
            let group: Vec<usize> = (0..branches.list.len())
                .filter(|ix| branches.list[*ix].local == local && keeps(&branches.list[*ix]))
                .collect();
            if !group.is_empty() {
                rows.push(Row::Heading(heading));
                for ix in group {
                    rows.push(Row::Branch(ix));
                    let branch = &branches.list[ix];
                    // By its tip only, so the rows stay while a fetch has them reload.
                    if branches.selected.as_ref() == Some(&branch.ref_name)
                        && let Some(((tip, _), commits)) = &branches.commits
                        && *tip == branch.tip
                    {
                        rows.extend((0..commits.len()).map(Row::Commit));
                    }
                }
            }
        }
        rows
    }

    fn select_other_branch(&mut self, ref_name: String, cx: &mut Context<Self>) {
        if let Some(branches) = &mut self.branches {
            branches.selected = Some(ref_name);
            branches.commit = None;
        }
        self.load_branch_commits(cx);
        self.refresh_diff(true, cx);
        cx.notify();
    }

    /// Open the Branches page on `ref_name`.
    pub(super) fn show_other_branch(
        &mut self,
        project: &ProjectId,
        ref_name: String,
        cx: &mut Context<Self>,
    ) {
        match &mut self.branches {
            Some(branches) if branches.project == *project => {
                branches.selected = Some(ref_name);
                branches.commit = None;
            }
            // Kept as the selection when the page lists its branches.
            _ => {
                self.branches = Some(Branches {
                    project: project.clone(),
                    seen: None,
                    list: Vec::new(),
                    selected: Some(ref_name),
                    commits: None,
                    commit: None,
                })
            }
        }
        let shown = Selection::Branches(project.clone());
        self.store
            .update(cx, |store, cx| store.select(Some(shown), cx));
    }

    fn select_branch_commit(&mut self, id: CommitId, cx: &mut Context<Self>) {
        if let Some(branches) = &mut self.branches {
            branches.commit = Some(id);
        }
        self.refresh_diff(true, cx);
        cx.notify();
    }

    /// Select commit `id` and offer to copy it into the workspace, or its ID.
    fn branch_commit_menu(
        &mut self,
        project: &ProjectId,
        id: CommitId,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.select_branch_commit(id, cx);
        let project = project.clone();
        let items = vec![
            MenuItem::new("Cherry-pick", move |app, _, cx| {
                app.cut(Cut::Pick(project.clone(), id), cx)
            }),
            MenuItem::new("Copy commit ID", move |app, _, cx| {
                app.copy_text(id.0.to_string(), cx)
            }),
        ];
        self.app
            .update(cx, |app, cx| app.open_menu(at, items, cx))
            .ok();
    }

    /// Pick up the selected commit, to put a copy of it where ⌘V lands in the workspace.
    pub(super) fn copy_branch_commit(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some(id) = self.selected_branch_commit(project) else {
            return;
        };
        let cut = Cut::Pick(project.clone(), id);
        self.app.update(cx, |app, cx| app.cut(cut, cx)).ok();
    }

    pub(super) fn apply_branch(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some(branch) = self.selected_other_branch(project) else {
            return;
        };
        let (ref_name, name) = (branch.ref_name.clone(), branch.name.clone());
        let selected = project.clone();
        self.store.update(cx, |store, cx| {
            store.run(
                project,
                format!("Apply {name}"),
                Overlay::default(),
                move |path, _| branches::apply(path, &ref_name),
                move |store, applied, cx| match applied {
                    ApplyOutcome::Applied | ApplyOutcome::AlreadyApplied => {
                        let local = format!("refs/heads/{name}");
                        store.select(Some(Selection::Branch(selected, local)), cx);
                        store.snack_undoable(format!("Applied {name}"), cx)
                    }
                    ApplyOutcome::Conflicts(stacks) => store.error(
                        format!("{name} wasn't applied"),
                        format!("It conflicts with {}", stacks.join(", ")),
                        cx,
                    ),
                },
                cx,
            )
        });
    }

    pub(super) fn delete_other_branch(
        &mut self,
        project: &ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(branch) = self.selected_other_branch(project).filter(|b| b.local) else {
            return;
        };
        let ref_name = format!("refs/heads/{}", branch.name);
        let project = project.clone();
        let name = branch.name.clone();
        self.app
            .update(cx, |app, cx| {
                app.confirm_then(
                    format!("Delete branch {name}?"),
                    "Its commits go too, unless a remote has them. You can undo this with ⌘Z.",
                    "Delete",
                    window,
                    cx,
                    move |app, cx| {
                        app.store.update(cx, |store, cx| {
                            store.run(
                                &project,
                                "Delete branch",
                                Overlay::default(),
                                move |path, _| backend::remove_branch(path, &ref_name),
                                |store, (), cx| store.snack_undoable("Branch deleted", cx),
                                cx,
                            )
                        })
                    },
                )
            })
            .ok();
    }

    pub(super) fn render_branches_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let filter_focused = self.branch_filter.focus_handle(cx).is_focused(window);
        let filter = field(&self.branch_filter, filter_focused, theme)
            .flex_none()
            .m(px(10.));
        let Some(branches) = self.branches_for(project).filter(|b| b.seen.is_some()) else {
            return empty_state("Loading branches…", "", theme);
        };
        let rows = self.branch_rows(branches, cx);
        let list: AnyElement = if rows.is_empty() {
            empty_state(
                if branches.list.is_empty() {
                    "Every branch is applied"
                } else {
                    "No branches match"
                },
                "",
                theme,
            )
        } else {
            uniform_list(
                "other-branches",
                rows.len(),
                cx.processor(move |this, range: Range<usize>, _, cx| {
                    let theme = Theme::get(cx).clone();
                    let Some(branches) = this.branches.as_ref() else {
                        return Vec::new();
                    };
                    range
                        .filter_map(|ix| rows.get(ix).map(|row| (ix, row)))
                        .map(|(ix, row)| match row {
                            Row::Heading(heading) => div()
                                .w_full()
                                .h(px(crate::ui::ROW_HEIGHT))
                                .flex()
                                .items_end()
                                .px(px(14.))
                                .pb(px(4.))
                                .text_size(px(11.))
                                .text_color(theme.text_3)
                                .child(*heading)
                                .into_any_element(),
                            Row::Commit(commit_ix) => {
                                let Some((_, commits)) = &branches.commits else {
                                    return div().into_any_element();
                                };
                                let commit = &commits[*commit_ix];
                                let id = commit.id;
                                let label: SharedString = commit.title.clone().into();
                                let project = branches.project.clone();
                                let payload = Cut::Pick(project.clone(), id);
                                div()
                                    .w_full()
                                    .child(
                                        list_row(
                                            ("other-commit", ix),
                                            branches.commit == Some(id),
                                            &theme,
                                        )
                                        .pl(px(30.))
                                        .aria_label(label.clone())
                                        .child(small_icon(Icon::Commit).text_color(theme.text_3))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .truncate()
                                                .text_size(px(12.))
                                                .child(label.clone()),
                                        )
                                        .child(
                                            div()
                                                .flex_none()
                                                .text_size(px(11.))
                                                .text_color(theme.text_3)
                                                .child(relative_time(commit.time)),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.select_branch_commit(id, cx)
                                        }))
                                        .on_mouse_down(
                                            MouseButton::Right,
                                            cx.listener(
                                                move |this, event: &MouseDownEvent, _, cx| {
                                                    let at = event.position;
                                                    this.branch_commit_menu(&project, id, at, cx)
                                                },
                                            ),
                                        )
                                        .on_drag(payload, drag_preview(this.store.clone(), label)),
                                    )
                                    .into_any_element()
                            }
                            Row::Branch(branch_ix) => {
                                let branch = &branches.list[*branch_ix];
                                let selected = branches.selected.as_ref() == Some(&branch.ref_name);
                                let ref_name = branch.ref_name.clone();
                                let merged = branch.ahead == Some(0);
                                div()
                                    .w_full()
                                    .child(
                                        list_row(("other-branch", ix), selected, &theme)
                                            .child(
                                                small_icon(Icon::Branch).text_color(theme.text_2),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.))
                                                    .truncate()
                                                    .font_family(theme.mono_font.clone())
                                                    .text_size(px(12.))
                                                    .text_color(if merged {
                                                        theme.text_2
                                                    } else {
                                                        theme.blue
                                                    })
                                                    .child(SharedString::from(branch.name.clone())),
                                            )
                                            .when_some(branch.review, |el, (number, _)| {
                                                el.child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .text_color(theme.text_2)
                                                        .child(format!("#{number}")),
                                                )
                                            })
                                            .when(merged, |el| {
                                                el.child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .text_color(theme.text_3)
                                                        .child("in target"),
                                                )
                                            })
                                            .when_some(branch.time, |el, time| {
                                                el.child(
                                                    div()
                                                        .flex_none()
                                                        .text_size(px(11.))
                                                        .text_color(theme.text_3)
                                                        .child(relative_time(time)),
                                                )
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_other_branch(ref_name.clone(), cx)
                                            })),
                                    )
                                    .into_any_element()
                            }
                        })
                        .collect()
                }),
            )
            .track_scroll(&self.branches_scroll)
            .size_full()
            .into_any_element()
        };
        gpui::div()
            .flex()
            .flex_col()
            .size_full()
            .child(filter)
            .child(scrolled(
                list,
                "branches-scrollbar",
                Scroll::Uniform(self.branches_scroll.clone()),
                theme,
            ))
            .into_any_element()
    }

    /// What the page's header says about the selected branch: who and how far ahead.
    pub(super) fn other_branch_summary(&self, project: &ProjectId) -> Option<String> {
        let branch = self.selected_other_branch(project)?;
        let ahead = match branch.ahead {
            Some(0) => "already in the target".to_owned(),
            Some(1) => "1 commit".to_owned(),
            Some(n) => format!("{n} commits"),
            None => String::new(),
        };
        let parts = [branch.author.clone().unwrap_or_default(), ahead];
        Some(
            parts
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
        )
    }
}
