//! The Branches page: branches outside the workspace, local ones first, then the ones only a
//! remote has, each with what it would bring in, ready to apply or delete.

use gpui::{
    AnyElement, ClickEvent, Context, Focusable as _, MouseButton, MouseDownEvent, Pixels, Point,
    SharedString, Window, div, prelude::*, px,
};

use super::Detail;
use super::panes::{Crumb, list_heading, side_list};
use crate::backend::branches::{self, ApplyOutcome, BranchCommit, OtherBranch};
use crate::backend::{self, CommitId, DiffSource};
use crate::overlays::MenuItem;
use crate::state::{Cut, Overlay, ProjectId, Selection};
use crate::theme::Theme;
use crate::transfer::drag_preview;
use crate::ui::illustration::Illustration;
use crate::ui::{
    ButtonStyle, Icon, button, empty_state, faint, field, list_row, muted, plural, relative_time,
    small_icon, v_flex,
};

pub(super) struct Branches {
    project: ProjectId,
    /// The project's branches when the list loaded; a reload that changes none of them
    /// leaves the list as it is. `None` until it has loaded.
    seen: Option<u64>,
    list: Vec<OtherBranch>,
    /// The selected branch, by ref.
    selected: Option<String>,
    /// The selected branch's commits, by what they were loaded for.
    commits: Option<(Loaded, Vec<BranchCommit>)>,
    /// One of those commits, to see on its own and to copy into the workspace.
    commit: Option<CommitId>,
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
                // The filter may hide what's selected now.
                this.select_shown_branch(cx);
                this.load_branch_commits(cx);
                this.refresh_diff(false, cx);
                cx.notify();
            })
            .ok();
        });
    }

    /// What a branch listing depends on: the project's branches, as its last full load saw
    /// them.
    fn branches_fingerprint(&self, project: &ProjectId, cx: &Context<Self>) -> u64 {
        self.store
            .read(cx)
            .workspace(project)
            .map_or(0, |ws| ws.refs)
    }

    fn branches_for(&self, project: &ProjectId) -> Option<&Branches> {
        self.branches.as_ref().filter(|b| b.project == *project)
    }

    fn selected_other_branch(&self, project: &ProjectId) -> Option<&OtherBranch> {
        self.branches_for(project)?.selected_branch()
    }

    pub(super) fn branches_source(&self, project: &ProjectId) -> Option<DiffSource> {
        if let Some(id) = self.selected_branch_commit(project) {
            return Some(DiffSource::Commit(id));
        }
        let branch = self.selected_other_branch(project)?;
        Some(DiffSource::Branch {
            tip: branch.tip,
            ahead: branch.ahead,
        })
    }

    fn selected_branch_commit(&self, project: &ProjectId) -> Option<CommitId> {
        self.branches_for(project)?.commit
    }

    fn selected_branch_commit_title(&self, project: &ProjectId) -> Option<String> {
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

    /// Select the first branch the filter shows when it hides the selected one, so the diff
    /// is of a branch in the list.
    pub(super) fn select_shown_branch(&mut self, cx: &mut Context<Self>) {
        let Some(branches) = &self.branches else {
            return;
        };
        let rows = self.branch_rows(branches, cx);
        let shown = |ix: &usize| Some(&branches.list[*ix].ref_name) == branches.selected.as_ref();
        if rows
            .iter()
            .any(|row| matches!(row, Row::Branch(ix) if shown(ix)))
        {
            return;
        }
        // With none shown, none is selected, and nothing shows beside the list either.
        let first = rows.iter().find_map(|row| match row {
            Row::Branch(ix) => Some(branches.list[*ix].ref_name.clone()),
            _ => None,
        });
        if first != branches.selected {
            self.select_other_branch(first, cx);
        }
    }

    fn select_other_branch(&mut self, ref_name: Option<String>, cx: &mut Context<Self>) {
        if let Some(branches) = &mut self.branches {
            branches.selected = ref_name;
            branches.commit = None;
        }
        self.load_branch_commits(cx);
        self.show_picked(cx);
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
        self.show_picked(cx);
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
                app.cut(Cut::CherryPick(project.clone(), id), cx)
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
    fn copy_branch_commit(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let Some(id) = self.selected_branch_commit(project) else {
            return;
        };
        let cut = Cut::CherryPick(project.clone(), id);
        self.app.update(cx, |app, cx| app.cut(cut, cx)).ok();
    }

    fn apply_branch(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
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

    fn delete_other_branch(
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
                    "Its commits go too, unless a remote has them. Undo can't bring back a branch \
                     outside the workspace.",
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
                                // The reload after it lists the branches again, as their
                                // refs changed.
                                |store, (), cx| store.snack("Branch deleted", cx),
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
            return empty_state(None, "Loading branches…", "", theme, cx);
        };
        let rows = self.branch_rows(branches, cx);
        let list = if rows.is_empty() {
            let (drawing, empty) = if branches.list.is_empty() {
                (Illustration::Cactus, "Every branch is applied".to_owned())
            } else {
                let filter = self.branch_filter.read(cx).text();
                let miss = format!("No branches match \"{}\"", filter.trim());
                (Illustration::Papers, miss)
            };
            div()
                .flex_1()
                .min_h(px(0.))
                .child(empty_state(Some(drawing), empty, "", theme, cx))
        } else {
            side_list(
                "other-branches",
                rows.len(),
                &self.branches_scroll,
                "branches-scrollbar",
                cx,
                move |this, ix, theme, _, cx| {
                    let branches = this.branches.as_ref()?;
                    Some(match rows.get(ix)? {
                        Row::Heading(heading) => list_heading(*heading, theme),
                        Row::Commit(commit_ix) => {
                            let (_, commits) = branches.commits.as_ref()?;
                            let commit = commits.get(*commit_ix)?;
                            let id = commit.id;
                            let label: SharedString = commit.title.clone().into();
                            let project = branches.project.clone();
                            let payload = Cut::CherryPick(project.clone(), id);
                            list_row(("other-commit", ix), branches.commit == Some(id), theme)
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
                                .child(faint(relative_time(commit.time), theme).flex_none())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_branch_commit(id, cx)
                                }))
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                        let at = event.position;
                                        this.branch_commit_menu(&project, id, at, cx)
                                    }),
                                )
                                .on_drag(payload, drag_preview(this.store.clone(), label))
                                .into_any_element()
                        }
                        Row::Branch(branch_ix) => {
                            let branch = &branches.list[*branch_ix];
                            let selected = branches.selected.as_ref() == Some(&branch.ref_name);
                            let ref_name = branch.ref_name.clone();
                            let merged = (branch.ahead == Some(0)).then(|| {
                                let ws = this.store.read(cx).workspace(&branches.project);
                                let target = ws.and_then(|ws| ws.target.as_ref());
                                format!("in {}", target.map_or("the target", |t| &t.name))
                            });
                            list_row(("other-branch", ix), selected, theme)
                                .child(small_icon(Icon::Branch).text_color(theme.text_2))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .truncate()
                                        .font_family(theme.mono_font.clone())
                                        .text_size(px(12.))
                                        .text_color(if merged.is_some() {
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
                                .when_some(merged, |el, merged| el.child(faint(merged, theme)))
                                .when_some(branch.time, |el, time| {
                                    el.child(faint(relative_time(time), theme).flex_none())
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_other_branch(Some(ref_name.clone()), cx)
                                }))
                                .into_any_element()
                        }
                    })
                },
            )
        };
        v_flex()
            .size_full()
            .child(filter)
            .child(list)
            .into_any_element()
    }

    /// The header's crumbs for the selected branch and commit, and what can be done with
    /// them.
    pub(super) fn branches_header(
        &self,
        project: &ProjectId,
        crumbs: &mut Vec<Crumb>,
        actions: &mut Vec<AnyElement>,
        theme: &Theme,
        cx: &Context<Self>,
    ) {
        let Some(branch) = self.selected_other_branch(project) else {
            return;
        };
        crumbs.push((branch.name.clone().into(), None));
        let local = branch.local;
        // Who made it and how far ahead it is.
        let target = (self.store.read(cx).workspace(project))
            .and_then(|ws| ws.target.as_ref())
            .map_or("the target".to_owned(), |t| t.name.clone());
        let ahead = match branch.ahead {
            Some(0) => format!("already in {target}"),
            Some(n) => plural(n, "commit"),
            None => String::new(),
        };
        let summary = [branch.author.clone().unwrap_or_default(), ahead];
        let summary: Vec<String> = summary.into_iter().filter(|p| !p.is_empty()).collect();
        actions.push(muted(summary.join(" · "), theme).into_any_element());
        if let Some(title) = self.selected_branch_commit_title(project) {
            crumbs.push((title.into(), None));
            let project = project.clone();
            actions.push(
                button("copy-commit", "Cherry-pick")
                    .style(ButtonStyle::Ghost)
                    .tooltip("Then press ⌘V on a branch or commit, or drag it there")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.copy_branch_commit(&project, cx)
                    }))
                    .into_any_element(),
            );
        }
        // A linked worktree has it checked out: deleting or applying it would pull it out
        // from under that worktree.
        let worktree = (self.store.read(cx).workspace(project).filter(|_| local))
            .and_then(|ws| {
                (ws.worktrees.iter()).find(|w| w.branch.as_deref() == Some(branch.name.as_str()))
            })
            .map(|worktree| worktree.name.clone());
        if let Some(name) = worktree {
            let shown = Selection::Worktree(project.clone(), name);
            let store = self.store.clone();
            actions.push(
                button("open-worktree", "Open worktree")
                    .style(ButtonStyle::Pop)
                    .tooltip("A linked worktree has it checked out")
                    .on_click(move |_, _, cx| {
                        store.update(cx, |store, cx| store.select(Some(shown.clone()), cx))
                    })
                    .into_any_element(),
            );
        } else {
            if local {
                let project = project.clone();
                actions.push(
                    button("delete-other", "Delete…")
                        .style(ButtonStyle::Ghost)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.delete_other_branch(&project, window, cx)
                        }))
                        .into_any_element(),
                );
            }
            let project = project.clone();
            actions.push(
                button("apply-branch", "Apply")
                    .style(ButtonStyle::Pop)
                    .tooltip("Add it to the workspace, beside your other branches")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.apply_branch(&project, cx)
                    }))
                    .into_any_element(),
            );
        }
    }
}
