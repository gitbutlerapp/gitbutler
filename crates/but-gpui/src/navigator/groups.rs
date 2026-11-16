//! Local repository groups: a folded group keeps watching its repositories, and its
//! summary counts repositories with changes, never files or commits.

use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, App, ClickEvent, Context, MouseButton, Role, SharedString, Window, div, prelude::*,
};

use super::{
    Navigator,
    rows::{Fold, Row, Rows},
};
use crate::app::AppView;
use crate::backend::Workspace;
use crate::overlays::MenuItem;
use crate::palette::Choice;
use crate::state::{ProjectEntry, ProjectGroup, ProjectId, Store};
use crate::theme::Theme;
use crate::transfer::ProjectDrag;
use crate::ui::text_input::TextInput;
use crate::ui::{Icon, Tooltip, dp, h_flex, icon, icon_button, small_icon, tab_stop};

pub(super) const GROUP_HEIGHT: f32 = 36.;

/// The empty id is the built-in Ungrouped section, never a user-created group's id.
#[derive(PartialEq)]
pub(super) struct GroupRow {
    pub(super) id: String,
    name: SharedString,
    total: usize,
    changed: usize,
    open: bool,
}

fn has_changes(workspace: &Workspace) -> bool {
    !workspace.uncommitted.is_empty()
        || workspace
            .worktrees
            .iter()
            .any(|w| !w.uncommitted.is_empty())
        || workspace
            .all_branches()
            .any(|branch| branch.unpushed_commits() > 0)
}

pub(super) fn changed_projects(store: &Store) -> HashSet<ProjectId> {
    if store.prefs.project_groups.is_empty() {
        return HashSet::new();
    }
    store
        .projects
        .iter()
        .filter(|entry| entry.expanded && entry.workspace.as_ref().is_some_and(has_changes))
        .map(|entry| entry.id.clone())
        .collect()
}

impl Navigator {
    /// Dropping on a repository places it in that group, before that repository in manual
    /// order. A group heading is also a target while its repositories are folded away.
    pub(super) fn group_project_drop(
        &self,
        el: gpui::Stateful<gpui::Div>,
        target: &ProjectId,
        grouped: bool,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        if !grouped || self.filtering(cx) {
            return el;
        }
        let target = target.clone();
        el.drag_over::<ProjectDrag>(|style, _, _, cx| style.bg(Theme::get(cx).blue_soft))
            .on_drop(cx.listener(move |this, drag: &ProjectDrag, _, cx| {
                cx.stop_propagation();
                if drag.id == target {
                    return;
                }
                let store = this.store.read(cx);
                if store.project(&target).is_none() {
                    return;
                }
                let group = store.prefs.project_group(&target).unwrap_or("").to_owned();
                let manual = store.prefs.watched_order == crate::state::WatchedOrder::Manual;
                let mut order: Vec<_> = this
                    .watched(store)
                    .into_iter()
                    .map(|e| e.id.clone())
                    .filter(|id| *id != drag.id)
                    .collect();
                if let Some(at) = order.iter().position(|id| *id == target) {
                    order.insert(at, drag.id.clone());
                }
                this.store.update(cx, |store, cx| {
                    store.move_to_project_group(&drag.id, &group, cx);
                    if manual {
                        store.order_watched(&order, cx);
                    }
                });
            }))
    }

    /// Build one card per group, retaining the existing order within each group.
    pub(super) fn group_rows(&self, store: &Store, rows: &mut Rows) {
        let mut buckets: HashMap<&str, Vec<&ProjectEntry>> = store
            .prefs
            .project_groups
            .iter()
            .map(|g| (g.id.as_str(), Vec::new()))
            .collect();
        let mut ungrouped = Vec::new();
        for entry in self.watched(store) {
            let group = store.prefs.project_memberships.get(entry.id.as_ref());
            match group.and_then(|id| buckets.get_mut(id.as_str())) {
                Some(bucket) => bucket.push(entry),
                None => ungrouped.push(entry),
            }
        }
        let mut push = |id: &str, name: &str, folded: bool, entries: Vec<&ProjectEntry>| {
            if !rows.rows.is_empty() {
                rows.push(Row::Gap);
            }
            rows.push(Row::Group(GroupRow {
                id: id.to_owned(),
                name: name.to_owned().into(),
                total: entries.len(),
                changed: entries
                    .iter()
                    .filter(|entry| self.group_changes.contains(&entry.id))
                    .count(),
                open: !folded,
            }));
            rows.fold(Fold::Group(id.to_owned()), !folded, &self.folds, |rows| {
                rows.open_in(8.);
                if entries.is_empty() {
                    rows.push(Row::Note("Drag repositories here".into(), false));
                }
                for entry in entries {
                    self.project_rows(store, entry, rows);
                }
                rows.close();
            });
        };
        for group in &store.prefs.project_groups {
            push(
                &group.id,
                &group.name,
                group.folded,
                buckets.remove(group.id.as_str()).unwrap_or_default(),
            );
        }
        // Keep an empty Ungrouped target available for dragging a repository out of a group.
        push("", "Ungrouped", store.prefs.ungrouped_folded, ungrouped);
    }

    pub(super) fn set_group_folded(&self, id: &str, folded: bool, cx: &mut Context<Self>) {
        let prefs = &self.store.read(cx).prefs;
        let current = if id.is_empty() {
            prefs.ungrouped_folded
        } else if let Some(group) = prefs.project_groups.iter().find(|g| g.id == id) {
            group.folded
        } else {
            return;
        };
        if current == folded {
            return;
        }
        self.store.update(cx, |store, cx| {
            store.set_prefs(
                |prefs| {
                    if id.is_empty() {
                        prefs.ungrouped_folded = folded;
                    } else if let Some(group) = prefs.project_groups.iter_mut().find(|g| g.id == id)
                    {
                        group.folded = folded;
                    }
                },
                cx,
            )
        });
    }

    pub(super) fn group_row(&self, group: &GroupRow, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let id = group.id.clone();
        let click_id = id.clone();
        let key_id = id.clone();
        let menu_id = id.clone();
        let drop_id = id.clone();
        let open = group.open;
        let fold_id = id.clone();
        let glyph = self.fold(
            format!("repository-group-fold-{id}"),
            icon(Icon::Folder).text_color(theme.text_2),
            Some(open),
            false,
            theme.text_3,
            cx,
            move |this, _, cx| this.set_group_folded(&fold_id, open, cx),
        );
        let label = format!(
            "{}, {} {}, {} with changes",
            group.name,
            group.total,
            if group.total == 1 {
                "repository"
            } else {
                "repositories"
            },
            group.changed
        );
        let title = tab_stop(h_flex().id(format!("repository-group-toggle-{id}")), theme)
            .group(super::ROW_GROUP)
            .role(Role::Button)
            .aria_label(label)
            .aria_expanded(open)
            .flex_1()
            .min_w_0()
            .h_full()
            .gap(dp(8.))
            .pl(dp(14.))
            .pr(dp(8.))
            .rounded(dp(theme.shape.control))
            .hover(|el| el.bg(theme.hover))
            .child(glyph)
            .child(
                div()
                    .flex_grow(1.)
                    .min_w_0()
                    .truncate()
                    .text_color(theme.text_1)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(group.name.clone()),
            )
            .child(
                div()
                    .text_size(dp(11.))
                    .text_color(theme.text_3)
                    .child(group.total.to_string()),
            )
            .when(group.changed > 0, |el| {
                let changed = group.changed;
                el.child(
                    h_flex()
                        .id(format!("repository-group-changes-{id}"))
                        .flex_none()
                        .gap(dp(4.))
                        .px(dp(6.))
                        .h(dp(20.))
                        .rounded(dp(theme.shape.control))
                        .bg(theme.blue_soft)
                        .text_color(theme.blue)
                        .text_size(dp(11.))
                        .child(small_icon(Icon::Diff).text_color(theme.blue))
                        .child(format!("{changed} changed"))
                        .tooltip(move |window, cx| {
                            Tooltip::text(format!(
                                "{changed} {} with uncommitted files or unpushed commits",
                                if changed == 1 {
                                    "repository"
                                } else {
                                    "repositories"
                                }
                            ))(window, cx)
                        }),
                )
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.set_group_folded(&click_id, open, cx)
            }))
            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                let folded = match event.keystroke.key.as_str() {
                    "enter" | "space" => open,
                    "left" => true,
                    "right" => false,
                    _ => return,
                };
                cx.stop_propagation();
                this.set_group_folded(&key_id, folded, cx);
            }));
        let row = h_flex()
            .id(format!("repository-group-{id}"))
            .h(dp(GROUP_HEIGHT))
            .mx(dp(3.))
            .pr(dp(3.))
            .gap(dp(2.))
            .text_size(dp(12.))
            .child(title)
            .child(
                icon_button(format!("repository-group-menu-{id}"), Icon::Kebab)
                    .small()
                    .tooltip("Group options")
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        let id = menu_id.clone();
                        this.open_menu(
                            event.position(),
                            move |this, cx| this.group_menu(&id, cx),
                            cx,
                        )
                    })),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    let id = id.clone();
                    this.open_menu(event.position, move |this, cx| this.group_menu(&id, cx), cx)
                }),
            )
            .drag_over::<ProjectDrag>(|style, _, _, cx| style.bg(Theme::get(cx).blue_soft))
            .on_drop(cx.listener(move |this, drag: &ProjectDrag, _, cx| {
                cx.stop_propagation();
                this.store.update(cx, |store, cx| {
                    store.set_moving(None, cx);
                    store.move_to_project_group(&drag.id, &drop_id, cx);
                });
            }));
        row.into_any_element()
    }

    fn group_menu(&self, id: &str, _cx: &App) -> Vec<MenuItem> {
        let add_id = id.to_owned();
        let mut items = vec![MenuItem::new(
            "Add repository…",
            move |app, window, cx| app.add_to_project_group(add_id.clone(), window, cx),
        )];
        if !id.is_empty() {
            let rename_id = id.to_owned();
            items.push(MenuItem::new("Rename group…", move |app, window, cx| {
                app.edit_project_group(Some(rename_id.clone()), None, window, cx)
            }));
            let remove_id = id.to_owned();
            items.push(
                MenuItem::new("Remove group", move |app, _, cx| {
                    app.store.update(cx, |store, cx| {
                        store.set_prefs(|prefs| prefs.remove_project_group(&remove_id), cx);
                        store.snack("Group removed; repositories moved to Ungrouped", cx);
                    });
                })
                .detail("Keep repositories")
                .separated(),
            );
        }
        items
    }
}

impl AppView {
    /// A short name form, shared by creating and renaming a group.
    pub(crate) fn edit_project_group(
        &mut self,
        id: Option<String>,
        project: Option<ProjectId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = match &id {
            Some(id) => match self
                .store
                .read(cx)
                .prefs
                .project_groups
                .iter()
                .find(|g| g.id == *id)
            {
                Some(group) => group.name.clone(),
                None => return,
            },
            None => String::new(),
        };
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Group name", false, cx);
            input.load(current, cx);
            input.select_all_text(cx);
            input
        });
        let check_id = id.clone();
        let check = move |app: &AppView, name: &str, cx: &App| {
            let name = name.trim();
            if name.is_empty() {
                return Some("Enter a group name".into());
            }
            let duplicate = name.eq_ignore_ascii_case("Ungrouped")
                || app.store.read(cx).prefs.project_groups.iter().any(|g| {
                    Some(&g.id) != check_id.as_ref() && g.name.to_lowercase() == name.to_lowercase()
                });
            duplicate.then(|| "A group with this name already exists".into())
        };
        self.open_prompt(
            None,
            if id.is_some() {
                "Rename group"
            } else {
                "New repository group"
            },
            Some(
                "Organize watched repositories. Changes stay visible when the group is collapsed."
                    .into(),
            ),
            if id.is_some() { "Save" } else { "Create group" },
            input,
            None,
            Some(Box::new(check)),
            window,
            cx,
            move |app, name, _, cx| {
                let group_id = id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
                app.store.update(cx, |store, cx| {
                    store.set_prefs(
                        |prefs| {
                            if let Some(group) =
                                prefs.project_groups.iter_mut().find(|g| g.id == group_id)
                            {
                                group.name = name.trim().to_owned();
                            } else if id.is_none() {
                                prefs.project_groups.push(ProjectGroup {
                                    id: group_id.clone(),
                                    name: name.trim().to_owned(),
                                    folded: false,
                                });
                            }
                        },
                        cx,
                    );
                    if let Some(project) = &project {
                        store.move_to_project_group(project, &group_id, cx);
                    }
                });
            },
        );
    }

    pub(crate) fn choose_project_group(
        &mut self,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let current = store.prefs.project_group(&project);
        let choices = std::iter::once((String::new(), "Ungrouped".to_owned())).chain(
            store
                .prefs
                .project_groups
                .iter()
                .map(|g| (g.id.clone(), g.name.clone())),
        );
        let mut choices: Vec<Choice> = choices
            .map(|(id, name)| {
                let project = project.clone();
                let detail = if current.unwrap_or("") == id {
                    "Current group"
                } else {
                    ""
                };
                Choice::new(name, move |app, _, cx| {
                    app.store.update(cx, |store, cx| {
                        store.move_to_project_group(&project, &id, cx)
                    });
                })
                .detail(detail)
            })
            .collect();
        let name = store
            .project(&project)
            .map(|e| e.project.name.clone().into());
        choices.push(Choice::new("New group…", move |app, window, cx| {
            app.edit_project_group(None, Some(project), window, cx)
        }));
        self.open_picker(
            name,
            "Move repository to group",
            "Move",
            choices,
            window,
            cx,
        );
    }

    fn add_to_project_group(&mut self, group: String, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let name = store
            .prefs
            .project_groups
            .iter()
            .find(|g| g.id == group)
            .map_or("Ungrouped", |g| g.name.as_str())
            .to_owned();
        let choices = store
            .projects
            .iter()
            .filter(|p| !p.expanded || store.prefs.project_group(&p.id).unwrap_or("") != group)
            .map(|entry| {
                let (project, group) = (entry.id.clone(), group.clone());
                Choice::new(entry.project.name.clone(), move |app, _, cx| {
                    app.store.update(cx, |store, cx| {
                        store.move_to_project_group(&project, &group, cx)
                    });
                })
                .detail(if entry.expanded {
                    "Move to group"
                } else {
                    "Watch and move to group"
                })
            })
            .collect();
        self.open_picker(
            Some(name.into()),
            "Add repository to group",
            "Add",
            choices,
            window,
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::has_changes;
    use crate::backend::model::build::{branch, workspace, worktree};
    use crate::backend::{FileChange, Workspace};

    #[test]
    fn counts_repositories_once_across_files_worktrees_and_unpushed_commits() {
        let mut changed = workspace(
            vec![vec![branch("feature", &[1, 2])]],
            vec![worktree("agent", None, vec![])],
        );
        changed.uncommitted = vec![FileChange::bare("one"), FileChange::bare("two")];
        changed
            .worktrees
            .first_mut()
            .expect("agent worktree exists")
            .uncommitted
            .push(FileChange::bare("three"));
        let repositories = [Workspace::default(), changed];
        assert_eq!(
            repositories.iter().filter(|ws| has_changes(ws)).count(),
            1,
            "multiple kinds of change in one repository count once"
        );
    }

    #[test]
    fn changes_in_a_linked_worktree_count_with_a_clean_main_checkout() {
        let mut ws = workspace(vec![], vec![worktree("agent", None, vec![])]);
        ws.worktrees
            .first_mut()
            .expect("agent worktree exists")
            .uncommitted
            .push(FileChange::bare("file"));
        assert!(
            has_changes(&ws),
            "a linked worktree's changes contribute to its repository"
        );
        ws.worktrees
            .first_mut()
            .expect("agent worktree exists")
            .uncommitted
            .clear();
        assert!(
            !has_changes(&ws),
            "the count clears after the last change is gone"
        );
    }

    #[test]
    fn unpushed_commits_count_without_uncommitted_files() {
        let ws = workspace(vec![vec![branch("feature", &[1])]], vec![]);
        assert!(
            has_changes(&ws),
            "a clean worktree can still have unpushed commits"
        );
        let mut ws = ws;
        for lane in &mut ws.lanes {
            for branch in &mut lane.branches {
                for commit in &mut branch.commits {
                    commit.state = crate::backend::CommitState::Integrated;
                }
            }
        }
        assert!(
            !has_changes(&ws),
            "integrated commits do not need attention"
        );
    }
}
