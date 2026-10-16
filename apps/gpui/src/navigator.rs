//! The left column: every project as a card, each listing its uncommitted changes,
//! stacks, branches and commits as one tree of rows.

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, Entity, Focusable as _, MouseButton,
    MouseDownEvent, Pixels, Point, SharedString, Stateful, Window, div, prelude::*, px,
};

use crate::app::{
    AppView, Collapse, Confirm, DeleteSelection, Expand, MenuItem, NewBranchOnTop, RenameSelection,
    SelectNext, SelectPrevious, UncommitSelection,
};
use crate::backend::{Branch, Commit, CommitState, PushState};
use crate::state::{Cut, ProjectEntry, Selection, Store};
use crate::theme::Theme;
use crate::transfer::{Transfer, short_ref};
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, ROW_HEIGHT, Tooltip, button, count, dot, h_flex, icon,
    icon_button, just_now, relative_time, small_icon, v_flex,
};

/// Commits shown per branch before folding the rest into "N more".
const FOLD_AFTER: usize = 8;

const NAV_WIDTH: f32 = 340.;

/// Rows in display order. Only the variants holding a [`Selection`] can be selected.
enum Row<'a> {
    Project(&'a ProjectEntry),
    Note(SharedString, bool),
    Uncommitted(&'a ProjectEntry, usize),
    StackCaption(usize),
    Section(&'static str),
    Branch(&'a ProjectEntry, &'a Branch),
    Commit(&'a ProjectEntry, &'a Commit),
    More(&'a ProjectEntry, &'a Branch, usize),
}

impl Row<'_> {
    fn selection(&self) -> Option<Selection> {
        match self {
            Row::Project(entry) => Some(Selection::Project(entry.id.clone())),
            Row::Uncommitted(entry, _) => Some(Selection::Uncommitted(entry.id.clone())),
            Row::Branch(entry, branch) => {
                Some(Selection::Branch(entry.id.clone(), branch.ref_name.clone()))
            }
            Row::Commit(entry, commit) => Some(Selection::Commit(entry.id.clone(), commit.id)),
            Row::Note(..) | Row::StackCaption(_) | Row::Section(_) | Row::More(..) => None,
        }
    }
}

/// Rows of every card, with `None` marking the gap between two cards. Expanded projects
/// get a card each; the collapsed ones share one compact card at the end.
fn rows<'a>(store: &'a Store, view: &AppView, cx: &gpui::App) -> Vec<Option<Row<'a>>> {
    let query = view.filter.read(cx).text().trim().to_lowercase();
    let matches = |entry: &&ProjectEntry| {
        query.is_empty() || entry.project.name.to_lowercase().contains(&query)
    };
    let mut rows = Vec::new();
    for entry in store
        .projects
        .iter()
        .filter(|entry| entry.expanded)
        .filter(matches)
    {
        if !rows.is_empty() {
            rows.push(None);
        }
        rows.push(Some(Row::Project(entry)));
        let Some(workspace) = &entry.workspace else {
            let note = match &entry.error {
                Some(err) => (err.clone(), true),
                None => ("Loading…".into(), false),
            };
            rows.push(Some(Row::Note(note.0, note.1)));
            continue;
        };
        if let Some(err) = &entry.error {
            rows.push(Some(Row::Note(err.clone(), true)));
        }
        rows.push(Some(Row::Uncommitted(entry, workspace.uncommitted.len())));
        for stack in &workspace.stacks {
            if stack.branches.len() > 1 {
                rows.push(Some(Row::StackCaption(stack.branches.len())));
            }
            for branch in &stack.branches {
                rows.push(Some(Row::Branch(entry, branch)));
                let unfolded = view
                    .unfolded_branches
                    .contains(&(entry.id.clone(), branch.ref_name.clone()));
                let shown = if unfolded {
                    branch.commits.len()
                } else {
                    FOLD_AFTER
                };
                for commit in branch.commits.iter().take(shown) {
                    rows.push(Some(Row::Commit(entry, commit)));
                }
                if branch.commits.len() > shown {
                    rows.push(Some(Row::More(entry, branch, branch.commits.len() - shown)));
                }
            }
        }
        if workspace.stacks.is_empty() {
            rows.push(Some(Row::Note("No branches yet".into(), false)));
        }
    }
    let mut collapsed = store
        .projects
        .iter()
        .filter(|entry| !entry.expanded)
        .filter(matches)
        .peekable();
    if collapsed.peek().is_some() {
        if !rows.is_empty() {
            rows.push(None);
        }
        rows.push(Some(Row::Section(if rows.len() > 1 {
            "More projects"
        } else {
            "All projects"
        })));
        rows.extend(collapsed.map(|entry| Some(Row::Project(entry))));
    }
    rows
}

impl AppView {
    fn selectable(&self, cx: &gpui::App) -> Vec<Selection> {
        rows(self.store.read(cx), self, cx)
            .iter()
            .flatten()
            .filter_map(Row::selection)
            .collect()
    }

    fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.selectable(cx);
        if items.is_empty() {
            return;
        }
        let current = self.store.read(cx).selection.clone();
        let next = match current.and_then(|s| items.iter().position(|item| *item == s)) {
            Some(ix) => (ix as isize + delta).clamp(0, items.len() as isize - 1) as usize,
            None => 0,
        };
        self.select(items[next].clone(), window, cx);
    }

    fn select_previous(&mut self, _: &SelectPrevious, window: &mut Window, cx: &mut Context<Self>) {
        self.step(-1, window, cx);
    }

    fn select_next(&mut self, _: &SelectNext, window: &mut Window, cx: &mut Context<Self>) {
        self.step(1, window, cx);
    }

    fn expand(&mut self, _: &Expand, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(Selection::Project(id)) = self.store.read(cx).selection.clone() {
            self.store
                .update(cx, |store, cx| store.set_expanded(&id, true, cx));
        }
    }

    fn collapse(&mut self, _: &Collapse, window: &mut Window, cx: &mut Context<Self>) {
        match self.store.read(cx).selection.clone() {
            Some(Selection::Project(id)) => {
                self.store
                    .update(cx, |store, cx| store.set_expanded(&id, false, cx));
            }
            // Jump up to the project, like closing a folder in a tree.
            Some(selection) => {
                self.select(Selection::Project(selection.project().clone()), window, cx)
            }
            None => {}
        }
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        match self.store.read(cx).selection.clone() {
            Some(Selection::Project(id)) => {
                let expanded = self.store.read(cx).project(&id).is_some_and(|p| p.expanded);
                self.store
                    .update(cx, |store, cx| store.set_expanded(&id, !expanded, cx));
            }
            Some(Selection::Uncommitted(_)) => {
                self.detail
                    .update(cx, |detail, cx| detail.focus_message(window, cx));
            }
            _ => {}
        }
    }

    fn delete_selection(
        &mut self,
        _: &DeleteSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.store.read(cx).selection.clone() {
            Some(Selection::Commit(project, id)) => self.discard_commit(project, id, window, cx),
            Some(Selection::Branch(project, ref_name)) => {
                self.remove_branch(project, ref_name, window, cx)
            }
            Some(Selection::Uncommitted(project)) => {
                let changes = self.detail.read(cx).selected_changes(cx);
                self.discard(project, changes, window, cx);
            }
            _ => {}
        }
    }

    fn rename_selection(
        &mut self,
        _: &RenameSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.store.read(cx).selection.clone() {
            Some(Selection::Branch(project, ref_name)) => {
                self.rename_branch(project, ref_name, window, cx)
            }
            Some(Selection::Commit(..)) => self
                .detail
                .update(cx, |detail, cx| detail.start_reword(window, cx)),
            _ => {}
        }
    }

    fn new_branch_on_top(
        &mut self,
        _: &NewBranchOnTop,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let below = match store.selection.clone() {
            Some(Selection::Branch(project, ref_name)) => Some((project, ref_name)),
            Some(Selection::Commit(project, id)) => store
                .workspace(&project)
                .and_then(|ws| ws.find_commit(id))
                .map(|(branch, _)| (project.clone(), branch.ref_name.clone())),
            _ => None,
        };
        if let Some((project, ref_name)) = below.filter(|(_, ref_name)| !ref_name.is_empty()) {
            self.add_branch_on_top(project, ref_name, window, cx);
        }
    }

    fn uncommit_selection(
        &mut self,
        _: &UncommitSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.store.read(cx).selection.clone() {
            Some(Selection::Commit(project, id)) => self.uncommit(project, id, cx),
            Some(Selection::Branch(project, ref_name)) => {
                self.remove_branch(project, ref_name, window, cx)
            }
            _ => {}
        }
    }

    pub fn scroll_selection_into_view(&mut self, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(selection) = &store.selection else {
            return;
        };
        if let Some(ix) = rows(store, self, cx)
            .iter()
            .position(|row| row.as_ref().and_then(Row::selection).as_ref() == Some(selection))
        {
            self.nav_scroll.scroll_to_item(ix);
        }
    }

    pub fn render_navigator(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let store = self.store.read(cx);
        let focused = self.nav_focus.contains_focused(window, cx);
        let selection = store.selection.clone();
        // What a drop or a paste would carry: the drag while there is one, else the cut.
        let pending = if cx.has_active_drag() {
            store.dragging.clone()
        } else {
            store.cut.clone()
        };
        let alt = window.modifiers().alt;
        let rows = rows(store, self, cx);
        let project_count = store.projects.len();
        let loaded = store.projects_loaded;

        let mut children: Vec<AnyElement> = Vec::with_capacity(rows.len());
        let last_in_card: Vec<bool> = (0..rows.len())
            .map(|ix| rows.get(ix + 1).is_none_or(|next| next.is_none()))
            .collect();
        for (ix, row) in rows.iter().enumerate() {
            let Some(row) = row else {
                children.push(div().h(px(10.)).into_any_element());
                continue;
            };
            let is_selected = row.selection().is_some() && row.selection() == selection;
            let el = self.render_row(
                ix,
                row,
                is_selected,
                focused,
                pending.as_ref(),
                alt,
                &theme,
                cx,
            );
            let first = ix == 0 || rows[ix - 1].is_none();
            let last = last_in_card[ix];
            children.push(
                div()
                    .mx(px(10.))
                    .bg(theme.bg_1)
                    .border_color(theme.border_3)
                    .border_l_1()
                    .border_r_1()
                    .when(first, |el| el.border_t_1().rounded_t(px(6.)).pt(px(3.)))
                    .when(last, |el| el.border_b_1().rounded_b(px(6.)).pb(px(3.)))
                    .child(el)
                    .into_any_element(),
            );
        }

        let header = h_flex()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .pl(px(84.))
            .pr(px(8.))
            .gap(px(6.))
            .child(
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_size(px(14.))
                    .child("Projects"),
            )
            .when(project_count > 0, |el| {
                el.child(
                    div()
                        .px(px(6.))
                        .rounded_full()
                        .bg(theme.fill_gray.opacity(0.1))
                        .text_size(px(11.))
                        .text_color(theme.text_2)
                        .child(project_count.to_string()),
                )
            })
            .child(div().flex_1())
            .child(
                icon_button("refresh", Icon::Refresh)
                    .tooltip("Reload  ⌘R")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.refresh(&crate::app::Refresh, window, cx)
                    })),
            )
            .child(
                icon_button("add-project", Icon::Plus)
                    .tooltip("Add project  ⌘O")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.add_project(&crate::app::AddProject, window, cx)
                    })),
            );

        let list = div()
            .id("navigator-list")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .track_scroll(&self.nav_scroll)
            .pt(px(2.))
            .pb(px(12.))
            .children(children)
            .when(loaded && project_count == 0, |el| {
                el.child(
                    v_flex()
                        .p(px(20.))
                        .gap(px(6.))
                        .text_color(theme.text_2)
                        .child("No projects yet.")
                        .child("Add a Git repository with ⌘O."),
                )
            });

        v_flex()
            .id("navigator")
            .key_context("Navigator")
            .track_focus(&self.nav_focus)
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::expand))
            .on_action(cx.listener(Self::collapse))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::delete_selection))
            .on_action(cx.listener(Self::rename_selection))
            .on_action(cx.listener(Self::new_branch_on_top))
            .on_action(cx.listener(Self::uncommit_selection))
            .w(px(NAV_WIDTH))
            .flex_none()
            .h_full()
            .bg(theme.bg_2)
            .border_r_1()
            .border_color(theme.border_3)
            .child(header)
            .when(project_count > 5, |el| {
                let filter_focused = self.filter.focus_handle(cx).is_focused(window);
                el.child(
                    h_flex()
                        .mx(px(10.))
                        .mb(px(8.))
                        .h(px(28.))
                        .px(px(8.))
                        .gap(px(6.))
                        .rounded(px(6.))
                        .bg(theme.bg_1)
                        .border_1()
                        .border_color(if filter_focused {
                            theme.pop
                        } else {
                            theme.border_3
                        })
                        .child(small_icon(Icon::Search).text_color(theme.text_3))
                        .child(div().flex_1().text_size(px(12.)).child(self.filter.clone())),
                )
            })
            .child(list)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_row(
        &self,
        ix: usize,
        row: &Row,
        selected: bool,
        focused: bool,
        cut: Option<&Cut>,
        alt: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let base = |id: ElementId, indent: f32| -> Stateful<Div> {
            let solid = selected && focused;
            h_flex()
                .id(id)
                .h(px(ROW_HEIGHT))
                .mx(px(3.))
                .pl(px(8. + indent))
                .pr(px(8.))
                .gap(px(8.))
                .rounded(px(4.))
                .when(solid, |el| {
                    el.bg(theme.fill_gray).text_color(theme.text_inverted)
                })
                .when(selected && !focused, |el| el.bg(theme.selected_unfocused()))
                .when(!selected, |el| el.hover(|el| el.bg(theme.hover())))
        };
        let solid = selected && focused;
        let muted = |selected: bool| {
            if selected && focused {
                theme.text_inverted.opacity(0.7)
            } else {
                theme.text_2
            }
        };
        // Meaningful colours wash out on the dark selection ground; keep them legible.
        let meta = |color| if solid { theme.text_inverted } else { color };

        let mut el = match row {
            Row::Project(entry) => {
                let id = entry.id.clone();
                let toggle_id = entry.id.clone();
                let expanded = entry.expanded;
                let summary = entry.workspace.as_ref().map(|ws| {
                    (
                        ws.uncommitted.len(),
                        ws.unpushed_commits(),
                        ws.target.as_ref().map_or(0, |t| t.behind),
                    )
                });
                let unread = entry.workspace.as_ref().is_some_and(|ws| {
                    ws.branches()
                        .flat_map(|branch| &branch.commits)
                        .any(|commit| store.is_unread(&entry.id, commit))
                });
                let activity = entry.workspace.as_ref().and_then(|ws| ws.last_activity());
                base(ElementId::Name(format!("project-{}", entry.id).into()), 0.)
                    .h(px(if expanded { 32. } else { ROW_HEIGHT }))
                    .pl(px(4.))
                    .child(
                        div()
                            .id(ElementId::Name(format!("chevron-{}", entry.id).into()))
                            .flex_none()
                            .size(px(20.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .hover(|el| el.bg(theme.hover()))
                            .child(
                                small_icon(if expanded {
                                    Icon::ChevronDown
                                } else {
                                    Icon::ChevronRight
                                })
                                .text_color(muted(selected)),
                            )
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.store.update(cx, |store, cx| {
                                    store.set_expanded(&toggle_id, !expanded, cx)
                                });
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .when(expanded, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                            .child(entry.project.name.clone()),
                    )
                    .when(unread, |el| el.child(dot(meta(theme.pop))))
                    .when_some(summary, |el, (uncommitted, unpushed, behind)| {
                        el.when(uncommitted > 0, |el| {
                            el.child(count(Some(Icon::File), uncommitted, meta(theme.warn)))
                        })
                        .when(unpushed > 0, |el| {
                            el.child(count(Some(Icon::ArrowUp), unpushed, meta(theme.blue)))
                        })
                        .when(behind > 0, |el| {
                            el.child(count(Some(Icon::ArrowDown), behind, meta(theme.warn)))
                        })
                    })
                    .when(entry.is_loading(), |el| {
                        el.child(small_icon(Icon::Clock).text_color(theme.text_3))
                    })
                    // A project something wrote to in the last two minutes is likely an
                    // agent at work; otherwise say how long it's been quiet.
                    .when_some(activity, |el, at| {
                        if just_now(at) {
                            el.child(
                                h_flex()
                                    .id(ElementId::Name(format!("active-{}", entry.id).into()))
                                    .gap(px(4.))
                                    .text_size(px(11.))
                                    .text_color(meta(theme.safe))
                                    .child(dot(meta(theme.safe)))
                                    .child("active")
                                    .tooltip(|_, cx| {
                                        Tooltip::text("Changed in the last 2 minutes".into(), cx)
                                    }),
                            )
                        } else {
                            el.child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(muted(selected))
                                    .child(relative_time(at)),
                            )
                        }
                    })
                    .when(
                        expanded
                            && entry
                                .workspace
                                .as_ref()
                                .and_then(|ws| ws.target.as_ref())
                                .is_some_and(|t| t.needs_update),
                        |el| {
                            let project = entry.id.clone();
                            el.child(
                                button(
                                    ElementId::Name(format!("update-{}", entry.id).into()),
                                    "Update",
                                )
                                .small()
                                .style(ButtonStyle::Gray)
                                .inverted(solid)
                                .tooltip("Rebase branches onto the latest target")
                                .on_click(cx.listener(
                                    move |this, _, _, cx| this.update_project(project.clone(), cx),
                                )),
                            )
                        },
                    )
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        if event.click_count() == 2 {
                            this.store
                                .update(cx, |store, cx| store.set_expanded(&id, !expanded, cx));
                        }
                        this.select(Selection::Project(id.clone()), window, cx);
                    }))
                    .into_any_element()
            }
            Row::Note(text, is_error) => h_flex()
                .min_h(px(ROW_HEIGHT))
                .px(px(12.))
                .py(px(4.))
                .gap(px(6.))
                .text_size(px(12.))
                .text_color(if *is_error {
                    theme.danger
                } else {
                    theme.text_2
                })
                .when(*is_error, |el| el.child(small_icon(Icon::Warning)))
                .child(div().flex_1().min_w(px(0.)).child(text.clone()))
                .into_any_element(),
            Row::Section(title) => h_flex()
                .h(px(26.))
                .px(px(12.))
                .text_size(px(11.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text_3)
                .child(*title)
                .into_any_element(),
            Row::StackCaption(n) => h_flex()
                .h(px(22.))
                .pl(px(14.))
                .gap(px(6.))
                .text_size(px(11.))
                .text_color(theme.text_3)
                .child(small_icon(Icon::Stack))
                .child(format!("Stack of {n}"))
                .into_any_element(),
            Row::Uncommitted(entry, n) => {
                let selection = Selection::Uncommitted(entry.id.clone());
                base(
                    ElementId::Name(format!("uncommitted-{}", entry.id).into()),
                    6.,
                )
                .child(icon(Icon::FileDiff).text_color(if *n > 0 {
                    meta(theme.warn)
                } else {
                    muted(selected)
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .when(*n == 0, |el| el.text_color(muted(selected)))
                        .child(if *n > 0 {
                            "Uncommitted changes"
                        } else {
                            "No uncommitted changes"
                        }),
                )
                .when(*n > 0, |el| {
                    let to_review = store.to_review(&entry.id);
                    if to_review > 0 {
                        el.child(count(Some(Icon::Eye), to_review, meta(theme.warn)))
                    } else {
                        el.child(count(Some(Icon::Tick), *n, muted(selected)))
                    }
                })
                .on_click(cx.listener({
                    let selection = selection.clone();
                    move |this, _: &ClickEvent, window, cx| {
                        this.select(selection.clone(), window, cx)
                    }
                }))
                .when(*n > 0, |el| {
                    let files = self.detail.read(cx).files_to_move(&entry.id, cx);
                    let label: SharedString = match files.len() {
                        1 => files[0].path.clone().into(),
                        n => format!("{n} files").into(),
                    };
                    let payload = Cut::Files {
                        project: entry.id.clone(),
                        from: None,
                        files,
                    };
                    el.on_drag(payload, drag_preview(self.store.clone(), label))
                })
                .into_any_element()
            }
            Row::Branch(entry, branch) => {
                let selection = Selection::Branch(entry.id.clone(), branch.ref_name.clone());
                let unpushed = branch
                    .commits
                    .iter()
                    .filter(|c| c.state == CommitState::Local)
                    .count();
                let (glyph, glyph_color) = match (&branch.review, branch.push) {
                    (_, PushState::Integrated) => (Icon::Tick, theme.safe),
                    (Some(_), _) => (Icon::Pr, theme.safe),
                    (None, _) => (Icon::Branch, muted(selected)),
                };
                base(
                    ElementId::Name(format!("branch-{}-{}", entry.id, branch.ref_name).into()),
                    6.,
                )
                .child(icon(glyph).text_color(meta(glyph_color)))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .font_family(theme.mono_font.clone())
                        .text_size(px(12.))
                        .text_color(if selected && focused {
                            theme.text_inverted
                        } else {
                            theme.blue
                        })
                        .child(branch.name.clone()),
                )
                .when_some(branch.review.as_ref(), |el, review| {
                    el.child(
                        div()
                            .text_size(px(11.))
                            .text_color(muted(selected))
                            .child(format!("#{}", review.number)),
                    )
                })
                .when(branch.upstream_only > 0, |el| {
                    el.child(count(
                        Some(Icon::ArrowDown),
                        branch.upstream_only,
                        meta(theme.warn),
                    ))
                })
                .when(unpushed > 0, |el| {
                    el.child(count(Some(Icon::ArrowUp), unpushed, meta(theme.blue)))
                })
                .when(branch.commits.is_empty(), |el| {
                    el.child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.text_3)
                            .child("empty"),
                    )
                })
                .when(
                    branch
                        .commits
                        .iter()
                        .any(|commit| store.is_unread(&entry.id, commit)),
                    |el| el.child(dot(meta(theme.pop))),
                )
                .on_click(cx.listener({
                    let selection = selection.clone();
                    move |this, _: &ClickEvent, window, cx| {
                        this.select(selection.clone(), window, cx)
                    }
                }))
                .into_any_element()
            }
            Row::Commit(entry, commit) => {
                let selection = Selection::Commit(entry.id.clone(), commit.id);
                let is_cut = matches!(cut, Some(Cut::Commit(_, id)) if *id == commit.id);
                let (glyph, color) = if commit.conflicted {
                    (Icon::Warning, theme.danger)
                } else {
                    match commit.state {
                        CommitState::Local => (Icon::Commit, theme.blue),
                        CommitState::Pushed => (Icon::CommitFill, muted(selected)),
                        CommitState::Integrated => (Icon::Tick, theme.safe),
                    }
                };
                let payload = Cut::Commit(entry.id.clone(), commit.id);
                let label: SharedString = commit.title.clone().into();
                let unread = store.is_unread(&entry.id, commit);
                base(
                    ElementId::Name(format!("commit-{}-{}", entry.id, commit.id.short()).into()),
                    20.,
                )
                .child(small_icon(glyph).text_color(meta(color)))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .when(commit.title.is_empty(), |el| {
                            el.italic().text_color(muted(selected))
                        })
                        .when(unread, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                        .child(if commit.title.is_empty() {
                            "No message".to_owned()
                        } else {
                            commit.title.clone()
                        }),
                )
                .when(unread, |el| el.child(dot(meta(theme.pop))))
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .text_color(muted(selected))
                        .child(relative_time(commit.time)),
                )
                .when(is_cut, |el| {
                    el.opacity(0.5)
                        .border_1()
                        .border_dashed()
                        .border_color(theme.pop)
                })
                .on_click(cx.listener({
                    let selection = selection.clone();
                    move |this, _: &ClickEvent, window, cx| {
                        this.select(selection.clone(), window, cx)
                    }
                }))
                .on_drag(payload, drag_preview(self.store.clone(), label))
                .into_any_element()
            }
            Row::More(entry, branch, hidden) => {
                let key = (entry.id.clone(), branch.ref_name.clone());
                h_flex()
                    .id(("more", ix))
                    .h(px(24.))
                    .pl(px(36.))
                    .text_size(px(12.))
                    .text_color(theme.text_2)
                    .hover(|el| el.text_color(theme.text_1))
                    .child(format!("{hidden} more"))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.unfolded_branches.insert(key.clone());
                        cx.notify();
                    }))
                    .into_any_element()
            }
        };

        if let Some(target) = row.selection() {
            el = self.drop_target(el, target, cut, alt, theme, cx);
        }
        el
    }

    /// Make a row accept drags it has a meaning for, and offer its right-click menu.
    /// While something is cut or dragged, the row says what landing there would do, or
    /// steps back when it would do nothing.
    fn drop_target(
        &self,
        el: AnyElement,
        target: Selection,
        pending: Option<&Cut>,
        alt: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let verdict = pending.map(|payload| {
            Transfer::resolve(payload, &target, alt, self.store.read(cx)).map(|t| t.verb())
        });
        let store = self.store.clone();
        let can_drop_target = target.clone();
        let menu_target = target.clone();
        let highlight = theme.pop;
        div()
            .id(ElementId::Name(format!("drop-{target:?}").into()))
            .relative()
            .rounded(px(4.))
            .child(el)
            .when(matches!(verdict, Some(None)), |el| el.opacity(0.35))
            .when_some(verdict.flatten(), |el, verb| {
                el.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right(px(11.))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .px(px(6.))
                                .py(px(1.))
                                .rounded(px(4.))
                                .bg(theme.bg_1)
                                .border_1()
                                .border_color(highlight.opacity(0.6))
                                .text_size(px(11.))
                                .text_color(highlight)
                                .child(verb),
                        ),
                )
            })
            .can_drop(move |payload, window, cx| {
                payload.downcast_ref::<Cut>().is_some_and(|payload| {
                    let alt = window.modifiers().alt;
                    Transfer::resolve(payload, &can_drop_target, alt, store.read(cx)).is_some()
                })
            })
            .drag_over::<Cut>(move |style, _, _, _| {
                style
                    .bg(highlight.opacity(0.14))
                    .border_1()
                    .border_color(highlight)
            })
            .on_drop(cx.listener(move |this, payload: &Cut, window, cx| {
                let alternate = window.modifiers().alt;
                this.transfer(&payload.clone(), &target, alternate, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    let items = this.menu_items(&menu_target, window, cx);
                    if !items.is_empty() {
                        this.select(menu_target.clone(), window, cx);
                        this.open_menu(event.position, items, cx);
                    }
                }),
            )
            .into_any_element()
    }

    fn menu_items(
        &self,
        target: &Selection,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<MenuItem> {
        let store = self.store.read(cx);
        let mut items = Vec::new();
        match target.clone() {
            Selection::Project(project) => {
                let expanded = store.project(&project).is_some_and(|p| p.expanded);
                let path = store.path(&project);
                items.push(MenuItem::new(
                    if expanded { "Collapse" } else { "Expand" },
                    {
                        let project = project.clone();
                        move |this, _, cx| {
                            this.store
                                .update(cx, |store, cx| store.set_expanded(&project, !expanded, cx))
                        }
                    },
                ));
                if expanded {
                    let p = project.clone();
                    items.push(MenuItem::new("Fetch", move |this, _, cx| {
                        this.fetch_project(p.clone(), cx)
                    }));
                    if let Some(target) =
                        store.workspace(&project).and_then(|ws| ws.target.as_ref())
                    {
                        let p = project.clone();
                        items.push(MenuItem::new(
                            format!("Update from {}", target.name),
                            move |this, _, cx| this.update_project(p.clone(), cx),
                        ));
                    }
                }
                if let Some(path) = path {
                    items.push(MenuItem::new("Reveal in Finder", move |_, _, cx| {
                        cx.reveal_path(&path)
                    }));
                }
            }
            Selection::Uncommitted(project) => {
                let has_changes = store
                    .workspace(&project)
                    .is_some_and(|ws| !ws.uncommitted.is_empty());
                if has_changes {
                    items.push(MenuItem::new("Commit…", |this, window, cx| {
                        this.detail
                            .update(cx, |detail, cx| detail.focus_message(window, cx))
                    }));
                    items.push(
                        MenuItem::new("Discard checked changes…", move |this, window, cx| {
                            let changes = this.detail.read(cx).selected_changes(cx);
                            this.discard(project.clone(), changes, window, cx)
                        })
                        .danger(),
                    );
                }
            }
            Selection::Branch(project, ref_name) => {
                let branch = store
                    .workspace(&project)
                    .and_then(|ws| ws.find_branch(&ref_name));
                if branch.is_some_and(|b| b.push.can_push()) {
                    let (project, ref_name) = (project.clone(), ref_name.clone());
                    items.push(MenuItem::new("Push", move |this, _, cx| {
                        this.push(project.clone(), ref_name.clone(), cx)
                    }));
                }
                if !ref_name.is_empty() {
                    let (p, r) = (project.clone(), ref_name.clone());
                    items.push(MenuItem::new(
                        "New branch on top…",
                        move |this, window, cx| {
                            this.add_branch_on_top(p.clone(), r.clone(), window, cx)
                        },
                    ));
                    let (p, r) = (project.clone(), ref_name.clone());
                    items.push(MenuItem::new("Rename…", move |this, window, cx| {
                        this.rename_branch(p.clone(), r.clone(), window, cx)
                    }));
                    let name = short_ref(&ref_name).to_owned();
                    items.push(MenuItem::new("Copy name", move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(name.clone()))
                    }));
                    items.push(
                        MenuItem::new("Delete branch…", move |this, window, cx| {
                            this.remove_branch(project.clone(), ref_name.clone(), window, cx)
                        })
                        .danger(),
                    );
                }
            }
            Selection::Commit(project, id) => {
                let (p, i) = (project.clone(), id);
                items.push(MenuItem::new("Cut", move |this, _, cx| {
                    this.store.update(cx, |store, cx| {
                        store.set_cut(Some(Cut::Commit(p.clone(), i)), cx)
                    })
                }));
                let p = project.clone();
                items.push(MenuItem::new("Uncommit", move |this, _, cx| {
                    this.uncommit(p.clone(), id, cx)
                }));
                items.push(MenuItem::new("Edit message…", move |this, window, cx| {
                    this.detail
                        .update(cx, |detail, cx| detail.start_reword(window, cx))
                }));
                let full = id.0.to_string();
                items.push(MenuItem::new("Copy commit id", move |_, _, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(full.clone()))
                }));
                items.push(
                    MenuItem::new("Discard commit…", move |this, window, cx| {
                        this.discard_commit(project.clone(), id, window, cx)
                    })
                    .danger(),
                );
            }
        }
        items
    }
}

/// Start a drag of a `Cut` labelled `label`, letting the store know what's in flight so
/// the rows can say what a drop would do.
pub fn drag_preview(
    store: Entity<Store>,
    label: SharedString,
) -> impl Fn(&Cut, Point<Pixels>, &mut Window, &mut App) -> Entity<DragPreview> + 'static {
    move |payload, _, _, cx| {
        store.update(cx, |store, _| store.dragging = Some(payload.clone()));
        let preview = cx.new(|_| DragPreview {
            label: label.clone(),
            payload: payload.clone(),
        });
        // GPUI drops the preview when the drag ends, dropped or not.
        let store = store.clone();
        cx.observe_release(&preview, move |_, cx| {
            store.update(cx, |store, _| store.dragging = None);
        })
        .detach();
        preview
    }
}

/// The pill that follows the pointer while dragging, and what it carries.
pub struct DragPreview {
    pub label: SharedString,
    pub payload: Cut,
}

impl Render for DragPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let hint = match &self.payload {
            Cut::Commit(..) if window.modifiers().alt => Some("Squash"),
            Cut::Commit(..) => Some("⌥ to squash"),
            Cut::Files { .. } => None,
        };
        h_flex()
            .gap(px(8.))
            .px(px(10.))
            .h(px(28.))
            .max_w(px(320.))
            .rounded(px(6.))
            .bg(theme.bg_1)
            .border_1()
            .border_color(theme.pop)
            .shadow_lg()
            .font_family(theme.ui_font.clone())
            .text_size(px(12.))
            .text_color(theme.text_1)
            .child(
                small_icon(match self.payload {
                    Cut::Commit(..) => Icon::Commit,
                    Cut::Files { .. } => Icon::File,
                })
                .text_color(theme.pop),
            )
            .child(div().truncate().child(self.label.clone()))
            .when_some(hint, |el, hint| {
                el.child(div().flex_none().text_color(theme.text_2).child(hint))
            })
    }
}
