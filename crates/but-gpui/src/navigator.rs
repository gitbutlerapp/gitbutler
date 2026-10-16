//! The left column: every project as a card, each listing its uncommitted changes,
//! stacks, branches and commits as one tree of rows.

use std::collections::HashSet;

use gpui::{
    AnyElement, App, ClickEvent, Context, ElementId, Entity, FocusHandle, Focusable, ListAlignment,
    ListState, MouseButton, MouseDownEvent, Pixels, Point, SharedString, Subscription, WeakEntity,
    Window, div, list, prelude::*, px,
};

use crate::app::{
    AddProject, AppView, Collapse, Confirm, CopySelection, CutSelection, DeleteSelection, Expand,
    Fetch, MenuItem, NewBranchOnTop, OpenPullRequest, Push, Refresh, RenameSelection, SelectNext,
    SelectPrevious, StartCommit, UncommitSelection, UpdateFromTarget,
};
use crate::backend::{CommitState, PushState};
use crate::detail::{ChecksChanged, Detail};
use crate::state::{Cut, Pane, ProjectEntry, ProjectId, Selection, Store, StoreEvent};
use crate::theme::Theme;
use crate::transfer::Transfer;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, ROW_HEIGHT, Tooltip, button, count, dot, h_flex, icon,
    icon_button, just_now, list_row, popover, relative_time, resize_handle, review, small_icon,
    v_flex,
};

/// Commits shown per branch before folding the rest into "N more".
const FOLD_AFTER: usize = 8;

/// Rows in display order. Only [`Row::Item`] can be selected.
enum Row {
    /// The space between two cards.
    Gap,
    /// A project, its uncommitted changes, a branch or a commit.
    Item(Selection),
    Note(SharedString, bool),
    StackCaption(usize),
    Section(&'static str),
    /// A branch's commits folded away, and how many.
    More(ProjectId, String, usize),
    /// A commit an operation is still making, by its title.
    Pending(SharedString),
}

pub struct Navigator {
    app: WeakEntity<AppView>,
    store: Entity<Store>,
    detail: Entity<Detail>,
    focus: FocusHandle,
    /// The rows as last drawn, which the list draws as they scroll into view.
    rows: Vec<Row>,
    list: ListState,
    /// The selection changed: scroll to it once the rows are up to date.
    reveal_selection: bool,
    /// Branches showing all their commits instead of the first few.
    unfolded_branches: HashSet<(ProjectId, String)>,
    /// Narrows the navigator to projects whose name contains this text.
    pub filter: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
}

impl Navigator {
    pub fn new(
        app: WeakEntity<AppView>,
        store: Entity<Store>,
        detail: Entity<Detail>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = cx.new(|cx| TextInput::new("Filter projects", false, cx));
        let subscriptions = vec![
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.subscribe(&store, |this, _, event, _| {
                if let StoreEvent::SelectionChanged = event {
                    this.reveal_selection = true;
                }
            }),
            // Dragging the uncommitted changes carries the checked ones.
            cx.subscribe(&detail, |_, _, _: &ChecksChanged, cx| cx.notify()),
            cx.subscribe_in(
                &filter,
                window,
                |this, filter, event, window, cx| match event {
                    InputEvent::Changed => cx.notify(),
                    InputEvent::Submit => {
                        // Jump into the first match.
                        let query = filter.read(cx).text().to_lowercase();
                        let first = this
                            .store
                            .read(cx)
                            .projects
                            .iter()
                            .find(|entry| entry.project.name.to_lowercase().contains(&query))
                            .map(|entry| entry.id.clone());
                        if let Some(id) = first {
                            this.select(Selection::Project(id), window, cx);
                        }
                    }
                    InputEvent::Cancel => {
                        filter.update(cx, |filter, cx| filter.set_text("", cx));
                        window.focus(&this.focus, cx);
                    }
                },
            ),
        ];
        Self {
            app,
            store,
            detail,
            focus: cx.focus_handle(),
            rows: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, px(200.)),
            reveal_selection: false,
            unfolded_branches: HashSet::new(),
            filter,
            _subscriptions: subscriptions,
        }
    }

    /// Rows of every card, with a gap between two cards. Expanded projects get a card
    /// each; the collapsed ones share one compact card at the end.
    fn build_rows(&self, cx: &App) -> Vec<Row> {
        let store = self.store.read(cx);
        let query = self.filter.read(cx).text().trim().to_lowercase();
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
                rows.push(Row::Gap);
            }
            let id = &entry.id;
            rows.push(Row::Item(Selection::Project(id.clone())));
            let Some(workspace) = &entry.workspace else {
                rows.push(match &entry.error {
                    Some(err) => Row::Note(err.clone(), true),
                    None => Row::Note("Loading…".into(), false),
                });
                continue;
            };
            if let Some(err) = &entry.error {
                rows.push(Row::Note(err.clone(), true));
            }
            rows.push(Row::Item(Selection::Uncommitted(id.clone())));
            for stack in &workspace.stacks {
                if stack.branches.len() > 1 {
                    rows.push(Row::StackCaption(stack.branches.len()));
                }
                for branch in &stack.branches {
                    let key = (id.clone(), branch.ref_name.clone());
                    let shown = if self.unfolded_branches.contains(&key) {
                        branch.commits.len()
                    } else {
                        FOLD_AFTER
                    };
                    rows.push(Row::Item(Selection::Branch(key.0, key.1)));
                    rows.extend(
                        store
                            .new_commits(id, &branch.ref_name)
                            .into_iter()
                            .map(Row::Pending),
                    );
                    // Commits a pending operation takes away go, unless selected.
                    rows.extend(
                        branch
                            .commits
                            .iter()
                            .filter(|commit| {
                                !store.hides_commit(id, &commit.key)
                                    || store.selection
                                        == Some(Selection::Commit(id.clone(), commit.id))
                            })
                            .take(shown)
                            .map(|commit| Row::Item(Selection::Commit(id.clone(), commit.id))),
                    );
                    if branch.commits.len() > shown {
                        let hidden = branch.commits.len() - shown;
                        rows.push(Row::More(id.clone(), branch.ref_name.clone(), hidden));
                    }
                }
            }
            if workspace.stacks.is_empty() {
                rows.push(Row::Note("No branches yet".into(), false));
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
                rows.push(Row::Gap);
            }
            rows.push(Row::Section(if rows.len() > 1 {
                "More projects"
            } else {
                "All projects"
            }));
            rows.extend(collapsed.map(|entry| Row::Item(Selection::Project(entry.id.clone()))));
        }
        rows
    }

    fn select(&mut self, selection: Selection, window: &mut Window, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.select(Some(selection), cx));
        window.focus(&self.focus, cx);
    }

    fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let items: Vec<&Selection> = self
            .rows
            .iter()
            .filter_map(|row| match row {
                Row::Item(item) => Some(item),
                _ => None,
            })
            .collect();
        if items.is_empty() {
            return;
        }
        let current = self.store.read(cx).selection.as_ref();
        let next = match current.and_then(|s| items.iter().position(|item| *item == s)) {
            Some(ix) => (ix as isize + delta).clamp(0, items.len() as isize - 1) as usize,
            None => 0,
        };
        let next = items[next].clone();
        self.select(next, window, cx);
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

    /// Bring the list up to date with `rows`, and show the selection if it just changed.
    fn sync_list(&mut self, cx: &App) {
        if self.list.item_count() != self.rows.len() {
            // Keep the row at the top where it is.
            let top = self.list.logical_scroll_top();
            self.list
                .reset_with_uniform_height(self.rows.len(), px(ROW_HEIGHT));
            self.list.scroll_to(top);
        }
        if std::mem::take(&mut self.reveal_selection)
            && let Some(selection) = &self.store.read(cx).selection
            && let Some(ix) = self
                .rows
                .iter()
                .position(|row| matches!(row, Row::Item(item) if item == selection))
        {
            self.list.scroll_to_reveal_item(ix);
        }
    }

    /// The row at `ix` of the list, drawn inside its card.
    fn render_row(&self, ix: usize, window: &Window, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let row = match self.rows.get(ix) {
            None | Some(Row::Gap) => return div().h(px(10.)).into_any_element(),
            Some(row) => row,
        };
        let el = match row {
            Row::Item(item) => self.render_item(item, window, theme, cx),
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
            Row::Pending(title) => h_flex()
                .min_h(px(ROW_HEIGHT))
                .mx(px(3.))
                .pl(px(28.))
                .pr(px(12.))
                .gap(px(6.))
                .opacity(0.5)
                .child(small_icon(Icon::Commit).text_color(theme.blue))
                .child(div().flex_1().min_w(px(0.)).truncate().child(title.clone()))
                .child(div().text_size(px(11.)).text_color(theme.text_2).child("…"))
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
            Row::More(project, ref_name, hidden) => {
                let key = (project.clone(), ref_name.clone());
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
            Row::Gap => unreachable!("drawn above"),
        };
        let first = ix == 0 || matches!(self.rows[ix - 1], Row::Gap);
        let last = self
            .rows
            .get(ix + 1)
            .is_none_or(|next| matches!(next, Row::Gap));
        // Padded, not margined: a list lays its rows out from their left edge.
        div()
            .px(px(10.))
            .child(
                div()
                    .bg(theme.bg_1)
                    .border_color(theme.border_3)
                    .border_l_1()
                    .border_r_1()
                    .when(first, |el| el.border_t_1().rounded_t(px(6.)).pt(px(3.)))
                    .when(last, |el| el.border_b_1().rounded_b(px(6.)).pb(px(3.)))
                    .child(el),
            )
            .into_any_element()
    }

    fn render_item(
        &self,
        item: &Selection,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let selected = store.selection.as_ref() == Some(item);
        let focused = self.focus.contains_focused(window, cx);
        let solid = selected && focused;
        let base = |id: String, indent: f32| {
            list_row(ElementId::Name(id.into()), selected, theme)
                .mx(px(3.))
                .pl(px(8. + indent))
                .when(solid, |el| {
                    el.bg(theme.fill_gray).text_color(theme.text_inverted)
                })
        };
        let muted = if solid {
            theme.text_inverted.opacity(0.7)
        } else {
            theme.text_2
        };
        // Meaningful colours wash out on the dark selection ground; keep them legible.
        let meta = |color| if solid { theme.text_inverted } else { color };
        let selection = item.clone();
        // On press, as native lists do: a click waits for the release.
        let select = cx.listener(move |this, _: &MouseDownEvent, window, cx| {
            this.select(selection.clone(), window, cx)
        });

        let el = match item {
            Selection::Project(id) => {
                let Some(entry) = store.project(id) else {
                    return div().into_any_element();
                };
                let toggle_id = id.clone();
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
                        .any(|commit| store.is_unread(id, commit))
                });
                let activity = entry.workspace.as_ref().and_then(|ws| ws.last_activity());
                base(format!("project-{id}"), 0.)
                    .aria_label(entry.project.name.clone())
                    .aria_expanded(expanded)
                    .h(px(if expanded { 32. } else { ROW_HEIGHT }))
                    .pl(px(4.))
                    .child(
                        div()
                            .id(ElementId::Name(format!("chevron-{id}").into()))
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
                                .text_color(muted),
                            )
                            // Folding the project doesn't select it.
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
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
                                    .id(ElementId::Name(format!("active-{id}").into()))
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
                                    .text_color(muted)
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
                            let project = id.clone();
                            el.child(
                                button(ElementId::Name(format!("update-{id}").into()), "Update")
                                    .small()
                                    .style(ButtonStyle::Gray)
                                    .inverted(solid)
                                    .tooltip("Rebase branches onto the latest target")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.store.update(cx, |store, cx| {
                                            store.update_from_target(&project, cx)
                                        })
                                    })),
                            )
                        },
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let id = id.clone();
                            move |this, event: &MouseDownEvent, window, cx| {
                                if event.click_count == 2 {
                                    this.store.update(cx, |store, cx| {
                                        store.set_expanded(&id, !expanded, cx)
                                    });
                                }
                                this.select(Selection::Project(id.clone()), window, cx);
                            }
                        }),
                    )
                    .into_any_element()
            }
            Selection::Uncommitted(id) => {
                let n = store.workspace(id).map_or(0, |ws| ws.uncommitted.len());
                base(format!("uncommitted-{id}"), 6.)
                    .aria_label("Uncommitted changes")
                    .child(icon(Icon::FileDiff).text_color(if n > 0 {
                        meta(theme.warn)
                    } else {
                        muted
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .when(n == 0, |el| el.text_color(muted))
                            .child(if n > 0 {
                                "Uncommitted changes"
                            } else {
                                "No uncommitted changes"
                            }),
                    )
                    .when(n > 0, |el| {
                        let to_review = store.to_review(id);
                        if to_review > 0 {
                            el.child(count(Some(Icon::Eye), to_review, meta(theme.warn)))
                        } else {
                            el.child(count(Some(Icon::Tick), n, muted))
                        }
                    })
                    .on_mouse_down(MouseButton::Left, select)
                    .when(n > 0, |el| {
                        let files = self.detail.read(cx).files_to_move(id, cx);
                        let label: SharedString = match files.len() {
                            1 => files[0].file.path.clone().into(),
                            n => format!("{n} files").into(),
                        };
                        let payload = Cut::Files {
                            project: id.clone(),
                            from: None,
                            files: files.into(),
                        };
                        el.on_drag(payload, drag_preview(self.store.clone(), label))
                    })
                    .into_any_element()
            }
            Selection::Branch(id, ref_name) => {
                let Some(branch) = store.branch_for(item) else {
                    return div().into_any_element();
                };
                let unpushed = branch
                    .commits
                    .iter()
                    .filter(|c| c.state == CommitState::Local)
                    .count();
                let (glyph, glyph_color) = match (&branch.review, branch.push) {
                    (Some(review), _) => review::state_glyph(review.state, theme),
                    (None, PushState::Integrated) => (Icon::Tick, theme.safe),
                    (None, _) => (Icon::Branch, muted),
                };
                base(format!("branch-{id}-{ref_name}"), 6.)
                    .aria_label(format!("Branch {}", branch.name))
                    .child(icon(glyph).text_color(meta(glyph_color)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .font_family(theme.mono_font.clone())
                            .text_size(px(12.))
                            .text_color(meta(theme.blue))
                            .child(branch.name.clone()),
                    )
                    // As in t3code: the number in the state's colour, then one glyph for CI.
                    .when_some(branch.review.as_ref(), |el, review| {
                        let (_, color) = review::state_glyph(review.state, theme);
                        el.child(
                            div()
                                .text_size(px(11.))
                                .text_color(meta(color))
                                .child(format!("#{}", review.number)),
                        )
                        .when_some(review.checks, |el, checks| {
                            let (glyph, color, text) = review::checks_summary(checks, theme);
                            el.child(
                                div()
                                    .id(ElementId::Name(format!("checks-{ref_name}").into()))
                                    .child(small_icon(glyph).text_color(meta(color)))
                                    .tooltip(move |_, cx| Tooltip::text(text.clone().into(), cx)),
                            )
                        })
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
                            .any(|commit| store.is_unread(id, commit)),
                        |el| el.child(dot(meta(theme.pop))),
                    )
                    .on_mouse_down(MouseButton::Left, select)
                    .into_any_element()
            }
            Selection::Commit(id, commit_id) => {
                let Some((_, commit)) = store
                    .workspace(id)
                    .and_then(|ws| ws.find_commit(*commit_id))
                else {
                    return div().into_any_element();
                };
                let cut = store.cut.as_ref();
                let is_cut = matches!(cut, Some(Cut::Commit(_, cut)) if cut == commit_id);
                let (glyph, color) = if commit.conflicted {
                    (Icon::Warning, theme.danger)
                } else {
                    match commit.state {
                        CommitState::Local => (Icon::Commit, theme.blue),
                        CommitState::Pushed => (Icon::CommitFill, muted),
                        CommitState::Integrated => (Icon::Tick, theme.safe),
                    }
                };
                let payload = Cut::Commit(id.clone(), *commit_id);
                let label: SharedString = commit.title.clone().into();
                let unread = store.is_unread(id, commit);
                base(format!("commit-{id}-{}", commit_id.short()), 20.)
                    .aria_label(label.clone())
                    .child(small_icon(glyph).text_color(meta(color)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .when(commit.title.is_empty(), |el| el.italic().text_color(muted))
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
                            .text_color(muted)
                            .child(relative_time(commit.time)),
                    )
                    .when(is_cut, |el| {
                        el.opacity(0.5)
                            .border_1()
                            .border_dashed()
                            .border_color(theme.pop)
                    })
                    // Selected but on its way out.
                    .when(store.hides_commit(id, &commit.key), |el| el.opacity(0.5))
                    .on_mouse_down(MouseButton::Left, select)
                    .on_drag(payload, drag_preview(self.store.clone(), label))
                    .into_any_element()
            }
        };
        self.drop_target(el, item.clone(), window, theme, cx)
    }

    /// Make a row accept drags it has a meaning for, and offer its right-click menu.
    /// While something is cut or dragged, the row says what landing there would do, or
    /// steps back when it would do nothing.
    fn drop_target(
        &self,
        el: AnyElement,
        target: Selection,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        // What a drop or a paste would carry: the drag while there is one, else the cut.
        let pending = if cx.has_active_drag() {
            store.dragging.as_ref()
        } else {
            store.cut.as_ref()
        };
        let alt = window.modifiers().alt;
        let verdict = pending
            .map(|payload| Transfer::resolve(payload, &target, alt, store).map(|t| t.verb()));
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
                this.app
                    .update(cx, |app, cx| {
                        app.transfer(payload, &target, alternate, window, cx)
                    })
                    .ok();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    let items = this.menu_items(&menu_target, cx);
                    if !items.is_empty() {
                        this.select(menu_target.clone(), window, cx);
                        this.app
                            .update(cx, |app, cx| app.open_menu(event.position, items, cx))
                            .ok();
                    }
                }),
            )
            .into_any_element()
    }

    /// What right-clicking `target` offers. Right-clicking selects the row first, so the
    /// items run the same actions as their keys.
    fn menu_items(&self, target: &Selection, cx: &App) -> Vec<MenuItem> {
        let store = self.store.read(cx);
        let action =
            |label: &'static str, action: &dyn gpui::Action| MenuItem::action(label, action, cx);
        match target {
            Selection::Project(project) => {
                let expanded = store.project(project).is_some_and(|p| p.expanded);
                let mut items = vec![if expanded {
                    action("Collapse", &Collapse)
                } else {
                    action("Expand", &Expand)
                }];
                if expanded {
                    items.push(action("Fetch", &Fetch));
                    if let Some(target) = store.workspace(project).and_then(|ws| ws.target.as_ref())
                    {
                        items.push(MenuItem::action(
                            format!("Update from {}", target.name),
                            &UpdateFromTarget,
                            cx,
                        ));
                    }
                }
                if let Some(path) = store.path(project) {
                    items.push(MenuItem::new("Reveal in Finder", move |_, _, cx| {
                        cx.reveal_path(&path)
                    }));
                }
                items
            }
            Selection::Uncommitted(project) => {
                if store
                    .workspace(project)
                    .is_none_or(|ws| ws.uncommitted.is_empty())
                {
                    return Vec::new();
                }
                vec![
                    action("Commit…", &StartCommit),
                    action("Discard checked changes…", &DeleteSelection).danger(),
                ]
            }
            Selection::Branch(_, ref_name) => {
                let branch = store.branch_for(target);
                let mut items = Vec::new();
                if branch.is_some_and(|b| b.push.can_push()) {
                    items.push(action("Push", &Push));
                }
                if branch.is_some_and(|b| b.review.as_ref().is_some_and(|r| r.url.is_some())) {
                    items.push(action("Open pull request", &OpenPullRequest));
                }
                if !ref_name.is_empty() {
                    items.extend([
                        action("New branch on top…", &NewBranchOnTop),
                        action("Rename…", &RenameSelection),
                        action("Copy name", &CopySelection),
                        action("Delete branch…", &DeleteSelection).danger(),
                    ]);
                }
                items
            }
            Selection::Commit(..) => vec![
                action("Cut", &CutSelection),
                action("Uncommit", &UncommitSelection),
                action("Edit message…", &RenameSelection),
                action("Copy commit id", &CopySelection),
                action("Discard commit…", &DeleteSelection).danger(),
            ],
        }
    }
}

impl Focusable for Navigator {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Navigator {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rows = self.build_rows(cx);
        self.sync_list(cx);
        let theme = Theme::get(cx).clone();
        let store = self.store.read(cx);
        let project_count = store.projects.len();
        let empty = store.projects_loaded && project_count == 0;

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
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(Refresh), cx)),
            )
            .child(
                icon_button("add-project", Icon::Plus)
                    .tooltip("Add project  ⌘O")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(AddProject), cx)),
            );

        v_flex()
            .id("navigator")
            .key_context("Navigator")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::expand))
            .on_action(cx.listener(Self::collapse))
            .on_action(cx.listener(Self::confirm))
            .size_full()
            .relative()
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
            .map(|el| {
                if empty {
                    el.child(
                        v_flex()
                            .p(px(20.))
                            .gap(px(6.))
                            .text_color(theme.text_2)
                            .child("No projects yet.")
                            .child("Add a Git repository with ⌘O."),
                    )
                } else {
                    el.child(
                        list(
                            self.list.clone(),
                            cx.processor(|this, ix: usize, window, cx| {
                                this.render_row(ix, window, cx)
                            }),
                        )
                        .flex_1()
                        .pt(px(2.))
                        .pb(px(12.)),
                    )
                }
            })
            .child(resize_handle(Pane::Navigator, self.store.clone(), &theme))
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
        popover(theme)
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .h(px(28.))
            .max_w(px(320.))
            .border_color(theme.pop)
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
