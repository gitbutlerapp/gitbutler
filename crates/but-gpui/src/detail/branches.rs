//! The Branches page: branches outside the workspace, local ones first, then the ones only a
//! remote has, each with what it would bring in, ready to apply or delete.

use std::ops::Range;

use gpui::{
    AnyElement, Context, Focusable as _, SharedString, Window, div, prelude::*, px, uniform_list,
};

use super::Detail;
use crate::backend::branches::{self, ApplyOutcome, OtherBranch};
use crate::backend::{self, DiffSource};
use crate::state::{Overlay, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{Icon, empty_state, field, list_row, relative_time, small_icon};

pub(super) struct Branches {
    pub project: ProjectId,
    /// The workspace's branches and target when the list loaded; a reload that changes
    /// neither leaves the list as it is.
    pub seen: u64,
    pub list: Vec<OtherBranch>,
    /// The selected branch, by ref.
    pub selected: Option<String>,
}

/// A row of the list: a group's heading or a branch, by index into the list.
enum Row {
    Heading(&'static str),
    Branch(usize),
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
        if !force && self.branches_for(project).is_some_and(|b| b.seen == seen) {
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
                let previous = this
                    .branches
                    .take()
                    .filter(|b| b.project == project)
                    .and_then(|b| b.selected);
                // Local branches list first, so the first of those when there is one.
                let first = list.iter().find(|b| b.local).or(list.first());
                let selected = previous
                    .filter(|ref_name| list.iter().any(|b| b.ref_name == *ref_name))
                    .or_else(|| first.map(|b| b.ref_name.clone()));
                this.branches = Some(Branches {
                    project,
                    seen,
                    list,
                    selected,
                });
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
        let branches = self.branches_for(project)?;
        let selected = branches.selected.as_ref()?;
        branches.list.iter().find(|b| b.ref_name == *selected)
    }

    pub(super) fn branches_source(&self, project: &ProjectId) -> Option<DiffSource> {
        let tip = self.selected_other_branch(project)?.tip;
        Some(DiffSource::Branch { tip })
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
                rows.extend(group.into_iter().map(Row::Branch));
            }
        }
        rows
    }

    fn select_other_branch(&mut self, ref_name: String, cx: &mut Context<Self>) {
        if let Some(branches) = &mut self.branches {
            branches.selected = Some(ref_name);
        }
        self.refresh_diff(true, cx);
        cx.notify();
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
        let Some(branches) = self.branches_for(project) else {
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
