//! The left column: every project as a card, each listing its uncommitted changes,
//! stacks, branches and commits as one tree of rows.

use std::collections::HashSet;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, Entity, FocusHandle, Focusable, Hsla,
    KeyBinding, ListAlignment, ListState, MouseButton, MouseDownEvent, Pixels, Point, SharedString,
    Stateful, Subscription, WeakEntity, Window, actions, div, list, prelude::*, px,
};

use crate::app::{
    AddProject, AppView, Collapse, Confirm, CopySelection, CutSelection, DeleteSelection, Expand,
    Fetch, MenuItem, NewBranchOnTop, OpenPullRequest, Push, Refresh, RenameSelection, SelectNext,
    SelectPrevious, StartCommit, UncommitSelection, UpdateFromTarget,
};
use crate::backend::{CommitId, CommitState, PushState};
use crate::detail::{ChecksChanged, Detail};
use crate::edits::{
    AddEmptyCommitAbove, AddEmptyCommitBelow, CherryPick, CopyChangeId, CopyMessage, MoveDown,
    MoveToNewBranch, MoveUp, NewBranchBelow, NewBranchHere, PullRemote, RewrapMessage, SquashAll,
    SquashPicked, TakeRemote, UnapplyStack, Unstack, UpdateStack,
};
use crate::state::{Cut, Pane, ProjectId, Selection, Store, StoreEvent};
use crate::theme::Theme;
use crate::transfer::Transfer;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, ROW_HEIGHT, Tooltip, button, count, dot, h_flex, icon,
    icon_button, just_now, list_row, plural, popover, relative_time, resize_handle, review,
    small_icon, v_flex,
};

/// Commits shown per branch before folding the rest into "N more".
const FOLD_AFTER: usize = 8;

actions!(
    navigator,
    [SelectFirst, SelectLast, PreviousSection, NextSection]
);

/// Jumps through the tree, as in Lite: to either end, and between branches and projects.
pub fn bind_keys(cx: &mut App) {
    let nav = Some("Navigator && !TextInput");
    cx.bind_keys([
        KeyBinding::new("g g", SelectFirst, nav),
        KeyBinding::new("home", SelectFirst, nav),
        KeyBinding::new("shift-g", SelectLast, nav),
        KeyBinding::new("end", SelectLast, nav),
        KeyBinding::new("shift-up", PreviousSection, nav),
        KeyBinding::new("shift-down", NextSection, nav),
    ]);
}

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
    /// Where a commit of the project being dragged lands in a new branch of its own.
    NewBranch(ProjectId),
}

/// How an item row draws: `solid` is the selection while the navigator has focus.
#[derive(Clone, Copy)]
struct RowStyle<'a> {
    theme: &'a Theme,
    selected: bool,
    solid: bool,
}

impl RowStyle<'_> {
    fn row(&self, id: String, indent: f32) -> Stateful<Div> {
        let theme = self.theme;
        list_row(ElementId::Name(id.into()), self.selected, theme)
            .mx(px(3.))
            .pl(px(8. + indent))
            .when(self.solid, |el| {
                el.bg(theme.fill_gray).text_color(theme.text_inverted)
            })
    }

    fn muted(&self) -> Hsla {
        if self.solid {
            self.theme.text_inverted.opacity(0.7)
        } else {
            self.theme.text_2
        }
    }

    /// Meaningful colours wash out on the dark selection ground; keep them legible.
    fn meta(&self, color: Hsla) -> Hsla {
        if self.solid {
            self.theme.text_inverted
        } else {
            color
        }
    }
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
    /// Branches showing none of their commits, unless one is selected.
    closed_branches: HashSet<(ProjectId, String)>,
    _subscriptions: Vec<Subscription>,
}

impl Navigator {
    pub fn new(
        app: WeakEntity<AppView>,
        store: Entity<Store>,
        detail: Entity<Detail>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.subscribe(&store, |this, _, event, _| {
                if let StoreEvent::SelectionChanged = event {
                    this.reveal_selection = true;
                }
            }),
            // Dragging the uncommitted changes carries the checked ones.
            cx.subscribe(&detail, |_, _, _: &ChecksChanged, cx| cx.notify()),
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
            closed_branches: HashSet::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Rows of every card, with a gap between two cards. Expanded projects get a card
    /// each; the collapsed ones share one compact card at the end.
    fn build_rows(&self, cx: &App) -> Vec<Row> {
        let store = self.store.read(cx);
        let mut rows = Vec::new();
        for entry in store.projects.iter().filter(|entry| entry.expanded) {
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
            // Nothing but the edit goes on while a commit is checked out; its page is the
            // project's.
            if workspace.editing.is_some() {
                rows.push(Row::Note("Editing a commit".into(), false));
                continue;
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
                    let closed = self.closed_branches.contains(&key)
                        && !matches!(&store.selection, Some(Selection::Commit(p, c))
                            if p == id && branch.commits.iter().any(|commit| commit.id == *c));
                    rows.push(Row::Item(Selection::Branch(key.0, key.1)));
                    if closed {
                        continue;
                    }
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
            if matches!(&store.dragging, Some(Cut::Commit(project, _)) if project == id) {
                rows.push(Row::NewBranch(id.clone()));
            }
            rows.push(Row::Item(Selection::Branches(id.clone())));
            rows.push(Row::Item(Selection::History(id.clone())));
        }
        let mut collapsed = store
            .projects
            .iter()
            .filter(|entry| !entry.expanded)
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

    /// The rows that can be selected, in order.
    fn items(&self) -> Vec<&Selection> {
        (self.rows.iter())
            .filter_map(|row| match row {
                Row::Item(item) => Some(item),
                _ => None,
            })
            .collect()
    }

    fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.items();
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

    /// Select the first item before (`forward` false) or after the selection that starts a
    /// section: a project or a branch.
    fn jump_section(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.items();
        let current = self.store.read(cx).selection.as_ref();
        let at = current.and_then(|s| items.iter().position(|item| *item == s));
        let heads =
            |item: &&&Selection| matches!(item, Selection::Project(_) | Selection::Branch(..));
        let next = match (at, forward) {
            (Some(at), true) => items[at + 1..].iter().find(heads),
            (Some(at), false) => items[..at].iter().rev().find(heads),
            (None, _) => items.first(),
        };
        if let Some(next) = next.map(|item| (*item).clone()) {
            self.select(next, window, cx);
        }
    }

    fn select_previous(&mut self, _: &SelectPrevious, window: &mut Window, cx: &mut Context<Self>) {
        self.step(-1, window, cx);
    }

    fn select_next(&mut self, _: &SelectNext, window: &mut Window, cx: &mut Context<Self>) {
        self.step(1, window, cx);
    }

    fn expand(&mut self, _: &Expand, _: &mut Window, cx: &mut Context<Self>) {
        match self.store.read(cx).selection.clone() {
            Some(Selection::Project(id)) => {
                self.store
                    .update(cx, |store, cx| store.set_expanded(&id, true, cx));
            }
            Some(Selection::Branch(project, ref_name)) => {
                self.closed_branches.remove(&(project, ref_name));
                cx.notify();
            }
            _ => {}
        }
    }

    /// Close a branch to its row or open it again, as double-clicking a folder does.
    fn toggle_branch(&mut self, key: (ProjectId, String), cx: &mut Context<Self>) {
        if !self.closed_branches.remove(&key) {
            self.closed_branches.insert(key);
        }
        cx.notify();
    }

    /// Step out the way a tree does: a commit to its branch, an open branch closed, and
    /// anything else up to its project.
    fn collapse(&mut self, _: &Collapse, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let selection = store.selection.clone();
        let closes = matches!(&selection, Some(item @ Selection::Branch(project, ref_name))
            if store.branch_for(item).is_some_and(|branch| !branch.commits.is_empty())
                && !self.closed_branches.contains(&(project.clone(), ref_name.clone())));
        match selection {
            Some(Selection::Project(id)) => {
                self.store
                    .update(cx, |store, cx| store.set_expanded(&id, false, cx));
            }
            Some(Selection::Branch(project, ref_name)) if closes => {
                self.closed_branches.insert((project, ref_name));
                cx.notify();
            }
            Some(Selection::Commit(project, id)) => {
                let branch = store
                    .workspace(&project)
                    .and_then(|ws| ws.find_commit(id))
                    .map(|(branch, _)| branch.ref_name.clone());
                if let Some(ref_name) = branch {
                    self.select(Selection::Branch(project, ref_name), window, cx);
                }
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
        let el = match self.rows.get(ix) {
            None | Some(Row::Gap) => return div().h(px(10.)).into_any_element(),
            Some(Row::Item(item)) => self.render_item(item, window, theme, cx),
            Some(Row::Note(text, is_error)) => h_flex()
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
                .when(*is_error, |el| {
                    el.child(small_icon(Icon::Warning).text_color(theme.danger))
                })
                .child(div().flex_1().min_w(px(0.)).child(text.clone()))
                .into_any_element(),
            Some(Row::Pending(title)) => h_flex()
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
            Some(Row::Section(title)) => h_flex()
                .h(px(26.))
                .px(px(12.))
                .text_size(px(11.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text_3)
                .child(*title)
                .into_any_element(),
            Some(Row::StackCaption(n)) => h_flex()
                .h(px(22.))
                .pl(px(14.))
                .gap(px(6.))
                .text_size(px(11.))
                .text_color(theme.text_3)
                .child(small_icon(Icon::Stack).text_color(theme.text_3))
                .child(format!("Stack of {n}"))
                .into_any_element(),
            Some(Row::NewBranch(project)) => {
                let (highlight, project) = (theme.pop, project.clone());
                h_flex()
                    .id(("new-branch-drop", ix))
                    .h(px(30.))
                    .mx(px(6.))
                    .my(px(2.))
                    .justify_center()
                    .gap(px(6.))
                    .rounded(px(4.))
                    .border_1()
                    .border_dashed()
                    .border_color(theme.border_2)
                    .text_size(px(12.))
                    .text_color(theme.text_2)
                    .child(small_icon(Icon::Branch).text_color(theme.text_2))
                    .child("New branch")
                    .drag_over::<Cut>(move |style, _, _, _| {
                        style
                            .bg(highlight.opacity(0.14))
                            .border_color(highlight)
                            .text_color(highlight)
                    })
                    .on_drop(cx.listener(move |this, payload: &Cut, _, cx| {
                        if let Cut::Commit(from, id) = payload
                            && *from == project
                        {
                            this.store
                                .update(cx, |store, cx| store.move_to_new_branch(from, *id, cx));
                        }
                    }))
                    .into_any_element()
            }
            Some(Row::More(project, ref_name, hidden)) => {
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
        let selected = store.selection.as_ref() == Some(item)
            || matches!(item, Selection::Commit(_, id) if store.picked.contains(id));
        let style = RowStyle {
            theme,
            selected,
            solid: selected && self.focus.contains_focused(window, cx),
        };
        let el = match item {
            Selection::Project(id) => self.project_row(id, style, cx),
            Selection::Uncommitted(id) => Some(self.uncommitted_row(item, id, style, cx)),
            Selection::Branch(id, ref_name) => self.branch_row(item, id, ref_name, style, cx),
            Selection::Commit(id, commit_id) => self.commit_row(item, id, commit_id, style, cx),
            Selection::Branches(id) => Some(self.page_row(
                item,
                format!("branches-{id}"),
                Icon::Branch,
                "Branches",
                style,
                cx,
            )),
            Selection::History(id) => Some(self.page_row(
                item,
                format!("history-{id}"),
                Icon::Clock,
                "History",
                style,
                cx,
            )),
        };
        match el {
            Some(el) => self.drop_target(el, item.clone(), window, theme, cx),
            None => div().into_any_element(),
        }
    }

    /// Select `item` on press, as native lists do: a click waits for the release. ⌘ or ⇧
    /// picks commits along with the selected one.
    fn select_on_press(
        &self,
        item: &Selection,
        cx: &Context<Self>,
    ) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
        let item = item.clone();
        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
            let (platform, shift) = (event.modifiers.platform, event.modifiers.shift);
            match &item {
                Selection::Commit(project, id) if platform || shift => {
                    // A run of commits takes the ones shown, not those folded away.
                    let range = shift.then(|| {
                        (this.rows.iter())
                            .filter_map(|row| match row {
                                Row::Item(Selection::Commit(p, commit)) if p == project => {
                                    Some(*commit)
                                }
                                _ => None,
                            })
                            .collect()
                    });
                    this.store
                        .update(cx, |store, cx| store.pick(project, *id, range, cx));
                    window.focus(&this.focus, cx);
                }
                _ => this.select(item.clone(), window, cx),
            }
        })
    }

    fn project_row(
        &self,
        id: &ProjectId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let theme = style.theme;
        let entry = store.project(id)?;
        let toggle_id = id.clone();
        let expanded = entry.expanded;
        let summary = (entry.workspace.as_ref())
            .filter(|ws| ws.editing.is_none())
            .map(|ws| {
                (
                    ws.uncommitted.len(),
                    ws.unpushed_commits(),
                    ws.target.as_ref().map_or(0, |t| t.behind),
                )
            });
        let unread = entry
            .workspace
            .as_ref()
            .is_some_and(|ws| ws.commits().any(|commit| store.is_unread(id, commit)));
        let activity = entry.workspace.as_ref().and_then(|ws| ws.last_activity());
        Some(
            style
                .row(format!("project-{id}"), 0.)
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
                            .text_color(style.muted()),
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
                .when(unread, |el| el.child(dot(style.meta(theme.pop))))
                .when_some(summary, |el, (uncommitted, unpushed, behind)| {
                    el.when(uncommitted > 0, |el| {
                        el.child(count(Some(Icon::File), uncommitted, style.meta(theme.warn)))
                    })
                    .when(unpushed > 0, |el| {
                        el.child(count(Some(Icon::ArrowUp), unpushed, style.meta(theme.blue)))
                    })
                    .when(behind > 0, |el| {
                        el.child(count(Some(Icon::ArrowDown), behind, style.meta(theme.warn)))
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
                                .text_color(style.meta(theme.safe))
                                .child(dot(style.meta(theme.safe)))
                                .child("active")
                                .tooltip(|_, cx| {
                                    Tooltip::with_key(
                                        "Changed in the last 2 minutes".into(),
                                        None,
                                        cx,
                                    )
                                }),
                        )
                    } else {
                        el.child(
                            div()
                                .text_size(px(11.))
                                .text_color(style.muted())
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
                                .inverted(style.solid)
                                .tooltip("Rebase branches onto the latest target")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let project = project.clone();
                                    this.app
                                        .update(cx, |app, cx| {
                                            app.update_from_target(project, None, window, cx)
                                        })
                                        .ok();
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
                                this.store
                                    .update(cx, |store, cx| store.set_expanded(&id, !expanded, cx));
                            }
                            this.select(Selection::Project(id.clone()), window, cx);
                        }
                    }),
                )
                .into_any_element(),
        )
    }

    /// A quiet row at the end of a project that opens one of its pages.
    fn page_row(
        &self,
        item: &Selection,
        id: String,
        glyph: Icon,
        label: &'static str,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> AnyElement {
        style
            .row(id, 6.)
            .text_color(style.muted())
            .child(icon(glyph).text_color(style.muted()))
            .child(div().flex_1().child(label))
            .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
            .into_any_element()
    }

    fn uncommitted_row(
        &self,
        item: &Selection,
        id: &ProjectId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let theme = style.theme;
        let n = store.workspace(id).map_or(0, |ws| ws.uncommitted.len());
        style
            .row(format!("uncommitted-{id}"), 6.)
            .aria_label("Uncommitted changes")
            .child(icon(Icon::FileDiff).text_color(if n > 0 {
                style.meta(theme.warn)
            } else {
                style.muted()
            }))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .when(n == 0, |el| el.text_color(style.muted()))
                    .child(if n > 0 {
                        "Uncommitted changes"
                    } else {
                        "No uncommitted changes"
                    }),
            )
            .when(n > 0, |el| {
                let to_review = store.to_review(id);
                if to_review > 0 {
                    el.child(count(Some(Icon::Eye), to_review, style.meta(theme.warn)))
                } else {
                    el.child(count(Some(Icon::Tick), n, style.muted()))
                }
            })
            .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
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

    fn branch_row(
        &self,
        item: &Selection,
        id: &ProjectId,
        ref_name: &str,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let theme = style.theme;
        let branch = store.branch_for(item)?;
        let is_cut = matches!(&store.cut, Some(Cut::Branch(_, cut)) if cut == ref_name);
        let unpushed = branch
            .commits
            .iter()
            .filter(|c| c.state == CommitState::Local)
            .count();
        let (glyph, glyph_color) = match (&branch.review, branch.push) {
            (Some(review), _) => review::state_glyph(review.state, theme),
            (None, PushState::Integrated) => (Icon::Tick, theme.safe),
            (None, _) => (Icon::Branch, style.muted()),
        };
        let key = (id.clone(), ref_name.to_owned());
        let closed = self.closed_branches.contains(&key) && !branch.commits.is_empty();
        Some(
            style
                .row(format!("branch-{id}-{ref_name}"), 6.)
                .aria_label(format!("Branch {}", branch.name))
                .child(icon(glyph).text_color(style.meta(glyph_color)))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .font_family(theme.mono_font.clone())
                        .text_size(px(12.))
                        .text_color(style.meta(theme.blue))
                        .child(branch.name.clone()),
                )
                // As in t3code: the number in the state's colour, then one glyph for CI.
                .when_some(branch.review.as_ref(), |el, review| {
                    el.child(
                        div()
                            .text_size(px(11.))
                            .text_color(style.meta(glyph_color))
                            .child(format!("#{}", review.number)),
                    )
                    .when_some(review.checks, |el, checks| {
                        let (glyph, color, text) = review::checks_summary(checks, theme);
                        let tip = format!("{text}. Click to check again");
                        let (store, project) = (self.store.clone(), id.clone());
                        el.child(
                            div()
                                .id(ElementId::Name(format!("checks-{ref_name}").into()))
                                .child(small_icon(glyph).text_color(style.meta(color)))
                                .tooltip(move |_, cx| {
                                    Tooltip::with_key(tip.clone().into(), None, cx)
                                })
                                .on_click(move |_, _, cx| {
                                    store.update(cx, |store, cx| {
                                        store.refresh_forge(&project);
                                        store.snack("Checking CI again", cx)
                                    })
                                }),
                        )
                    })
                })
                .when(!branch.incoming.is_empty(), |el| {
                    el.child(count(
                        Some(Icon::ArrowDown),
                        branch.incoming.len(),
                        style.meta(theme.warn),
                    ))
                })
                .when(unpushed > 0, |el| {
                    el.child(count(Some(Icon::ArrowUp), unpushed, style.meta(theme.blue)))
                })
                .when(branch.commits.is_empty(), |el| {
                    el.child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.text_3)
                            .child("empty"),
                    )
                })
                .when(closed, |el| {
                    el.child(count(
                        Some(Icon::Commit),
                        branch.commits.len(),
                        style.muted(),
                    ))
                })
                .when(
                    branch
                        .commits
                        .iter()
                        .any(|commit| store.is_unread(id, commit)),
                    |el| el.child(dot(style.meta(theme.pop))),
                )
                .when(is_cut, |el| {
                    el.opacity(0.5)
                        .border_1()
                        .border_dashed()
                        .border_color(theme.pop)
                })
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    if event.click_count() == 2 {
                        this.toggle_branch(key.clone(), cx)
                    }
                }))
                .when(!ref_name.is_empty(), |el| {
                    let payload = Cut::Branch(id.clone(), ref_name.to_owned());
                    el.on_drag(
                        payload,
                        drag_preview(self.store.clone(), branch.name.clone().into()),
                    )
                })
                .into_any_element(),
        )
    }

    fn commit_row(
        &self,
        item: &Selection,
        id: &ProjectId,
        commit_id: &CommitId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let theme = style.theme;
        let (_, commit) = store.workspace(id)?.find_commit(*commit_id)?;
        let cut = store.cut.as_ref();
        let is_cut =
            matches!(cut, Some(Cut::Commit(_, cut) | Cut::Pick(_, cut)) if cut == commit_id);
        let (glyph, color) = if commit.conflicted {
            (Icon::Warning, theme.danger)
        } else {
            match commit.state {
                CommitState::Local => (Icon::Commit, theme.blue),
                CommitState::Pushed => (Icon::CommitFill, style.muted()),
                CommitState::Integrated => (Icon::Tick, theme.safe),
            }
        };
        let payload = Cut::Commit(id.clone(), *commit_id);
        let label: SharedString = commit.title.clone().into();
        let unread = store.is_unread(id, commit);
        Some(
            style
                .row(format!("commit-{id}-{}", commit_id.short()), 20.)
                .aria_label(label.clone())
                .child(small_icon(glyph).text_color(style.meta(color)))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .when(commit.title.is_empty(), |el| {
                            el.italic().text_color(style.muted())
                        })
                        .when(unread, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                        .child(if commit.title.is_empty() {
                            "No message".to_owned()
                        } else {
                            commit.title.clone()
                        }),
                )
                .when(unread, |el| el.child(dot(style.meta(theme.pop))))
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .text_color(style.muted())
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
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .on_drag(payload, drag_preview(self.store.clone(), label))
                .into_any_element(),
        )
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
        let held = window.modifiers();
        let verdict = pending
            .map(|payload| Transfer::resolve(payload, &target, held, store).map(|t| t.verb()));
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
                    let held = window.modifiers();
                    Transfer::resolve(payload, &can_drop_target, held, store.read(cx)).is_some()
                })
            })
            .drag_over::<Cut>(move |style, _, _, _| {
                style
                    .bg(highlight.opacity(0.14))
                    .border_1()
                    .border_color(highlight)
            })
            .on_drop(cx.listener(move |this, payload: &Cut, window, cx| {
                let held = window.modifiers();
                this.app
                    .update(cx, |app, cx| {
                        app.transfer(payload, &target, held, window, cx)
                    })
                    .ok();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    let items = this.menu_items(&menu_target, cx);
                    let picked = matches!(&menu_target,
                        Selection::Commit(_, id) if this.store.read(cx).picked.contains(id));
                    if !items.is_empty() {
                        if !picked {
                            this.select(menu_target.clone(), window, cx);
                        }
                        this.app
                            .update(cx, |app, cx| app.open_menu(event.position, items, cx))
                            .ok();
                    }
                }),
            )
            .into_any_element()
    }

    /// What right-clicking `target` offers. Right-clicking selects the row first, so the
    /// items run the same actions as their keys. A branch with commits starts with hiding
    /// or showing them.
    fn menu_items(&self, target: &Selection, cx: &App) -> Vec<MenuItem> {
        let mut items = self.row_actions(target, cx);
        if let Selection::Branch(project, ref_name) = target
            && let Some(first) = items.first_mut()
            && (self.store.read(cx).branch_for(target)).is_some_and(|b| !b.commits.is_empty())
        {
            *first = first.clone().separated();
            let key = (project.clone(), ref_name.clone());
            items.insert(
                0,
                match self.closed_branches.contains(&key) {
                    true => MenuItem::action("Show commits", &Expand, cx),
                    false => MenuItem::action("Hide commits", &Collapse, cx),
                },
            );
        }
        items
    }

    fn row_actions(&self, target: &Selection, cx: &App) -> Vec<MenuItem> {
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
                    let label = self
                        .app
                        .upgrade()
                        .map(|app| app.read(cx).open_in_editor_label(cx))
                        .unwrap_or_default();
                    let (editor_project, terminal_project) = (project.clone(), project.clone());
                    items.extend([
                        MenuItem::new(label, move |this, window, cx| {
                            let position = window.mouse_position();
                            this.open_in_editor(&editor_project, None, None, position, cx)
                        })
                        .separated(),
                        MenuItem::new("Open in terminal", move |this, _, cx| {
                            this.open_in_terminal(&terminal_project, cx)
                        }),
                        MenuItem::new("Reveal in Finder", move |_, _, cx| cx.reveal_path(&path)),
                    ]);
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
            Selection::Branch(project, ref_name) => {
                let mut items = Vec::new();
                let (Some(branch), Some(workspace)) =
                    (store.branch_for(target), store.workspace(project))
                else {
                    return items;
                };
                if branch.push.can_push() {
                    items.push(action("Push", &Push));
                }
                if !branch.incoming.is_empty() && !workspace.has_branches_above(ref_name) {
                    items.extend([
                        MenuItem::action(
                            format!(
                                "Pull {} from the remote",
                                plural(branch.incoming.len(), "commit")
                            ),
                            &PullRemote,
                            cx,
                        ),
                        action("Take the remote's version…", &TakeRemote),
                    ]);
                }
                if branch.review.as_ref().is_some_and(|r| r.url.is_some()) {
                    items.push(action("Open pull request", &OpenPullRequest));
                }
                if crate::pr::on_forge(store, target) && !ref_name.is_empty() {
                    use crate::backend::ReviewState as S;
                    use crate::pr::*;
                    match branch.review.as_ref().map(|r| r.state) {
                        None if !branch.commits.is_empty() => {
                            items.push(action("Create pull request…", &CreatePullRequest))
                        }
                        Some(state @ (S::Open | S::Draft)) => {
                            items.extend([
                                action("Edit pull request…", &EditPullRequest),
                                action(
                                    if state == S::Draft {
                                        "Mark ready for review"
                                    } else {
                                        "Convert to draft"
                                    },
                                    &ToggleDraft,
                                ),
                                action("Merge pull request…", &MergePullRequest),
                            ]);
                            // The forges GitButler can ask to merge on their own.
                            let auto_merges =
                                matches!(workspace.forge.as_deref(), Some("GitHub" | "GitLab"));
                            if state == S::Open && auto_merges {
                                let on = branch.review.as_ref().is_some_and(|r| r.auto_merge);
                                items.push(action(
                                    if on {
                                        "Don't merge automatically"
                                    } else {
                                        "Merge when checks pass"
                                    },
                                    &ToggleAutoMerge,
                                ));
                            }
                            items.push(action("Close pull request…", &ClosePullRequest));
                        }
                        Some(S::Closed) => {
                            items.push(action("Reopen pull request", &ClosePullRequest))
                        }
                        _ => {}
                    }
                }
                if ref_name.is_empty() {
                    return items;
                }
                items.push(action("New branch on top…", &NewBranchOnTop).separated());
                if !branch.commits.is_empty() {
                    items.push(action("New branch below…", &NewBranchBelow));
                }
                items.push(action("Add empty commit", &AddEmptyCommitAbove));
                if branch.commits.len() > 1 {
                    items.push(action("Squash all commits", &SquashAll));
                }
                let stacked = workspace
                    .stack_of(ref_name)
                    .is_some_and(|(stack, _)| stack.branches.len() > 1);
                items.push(action("Cut", &CutSelection).separated());
                if stacked {
                    items.push(action("Unstack", &Unstack));
                }
                if let Some(target) = workspace.target.as_ref().filter(|t| t.needs_update) {
                    items.push(MenuItem::action(
                        format!("Update stack from {}", target.name),
                        &UpdateStack,
                        cx,
                    ));
                }
                if let (Some(target), Some(whole)) = (
                    workspace.target.as_ref(),
                    crate::edits::land_scope(workspace, ref_name),
                ) {
                    let label = if whole {
                        format!("Land the stack on {}…", target.name)
                    } else {
                        format!("Land on {}…", target.name)
                    };
                    items.push(MenuItem::action(label, &crate::edits::LandBranch, cx));
                }
                items.push(action(
                    if stacked {
                        "Unapply stack"
                    } else {
                        "Unapply branch"
                    },
                    &UnapplyStack,
                ));
                items.extend([
                    action("Rename…", &RenameSelection).separated(),
                    action("Copy name", &CopySelection),
                    action("Delete branch…", &DeleteSelection).danger(),
                ]);
                items
            }
            Selection::Commit(_, id) if store.picked.contains(id) => {
                let n = store.picked.len();
                vec![
                    MenuItem::action(format!("Squash {n} commits"), &SquashPicked, cx),
                    MenuItem::action(format!("Uncommit {n} commits"), &UncommitSelection, cx),
                ]
            }
            Selection::Commit(project, id) => {
                let Some(workspace) = store.workspace(project) else {
                    return Vec::new();
                };
                let Some((branch, commit)) = workspace.find_commit(*id) else {
                    return Vec::new();
                };
                let mut items = Vec::new();
                if commit.conflicted {
                    items.extend([
                        action("Resolve conflicts…", &crate::resolve::ResolveConflicts),
                        action("Resolve by hand", &crate::resolve::EditByHand),
                    ]);
                }
                items.extend([
                    action("Cut", &CutSelection).separated(),
                    action("Cherry-pick", &CherryPick),
                    action("Edit message…", &RenameSelection).separated(),
                ]);
                if crate::edits::needs_rewrap(&commit.message) {
                    items.push(action("Rewrap message", &RewrapMessage));
                }
                items.extend([
                    action("Add empty commit above", &AddEmptyCommitAbove),
                    action("Add empty commit below", &AddEmptyCommitBelow),
                ]);
                if !branch.ref_name.is_empty() && branch.commits.first().map(|c| c.id) != Some(*id)
                {
                    items.push(action("New branch ending here…", &NewBranchHere));
                }
                items.extend([
                    action("Move up", &MoveUp).separated(),
                    action("Move down", &MoveDown),
                    action("Move to a new branch", &MoveToNewBranch),
                    action("Copy commit ID", &CopySelection).separated(),
                ]);
                if commit.has_change_id() {
                    items.push(action("Copy change ID", &CopyChangeId));
                }
                items.extend([
                    action("Copy message", &CopyMessage),
                    action("Uncommit", &UncommitSelection).separated(),
                    action("Discard commit…", &DeleteSelection).danger(),
                ]);
                items
            }
            Selection::Branches(_) | Selection::History(_) => Vec::new(),
        }
    }
}

impl Focusable for Navigator {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Navigator {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .on_action(cx.listener(|this, _: &SelectFirst, window, cx| {
                this.step(isize::MIN / 2, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &SelectLast, window, cx| {
                    this.step(isize::MAX / 2, window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &PreviousSection, window, cx| {
                this.jump_section(false, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &NextSection, window, cx| {
                    this.jump_section(true, window, cx)
                }),
            )
            .on_action(cx.listener(Self::expand))
            .on_action(cx.listener(Self::collapse))
            .on_action(cx.listener(Self::confirm))
            .size_full()
            .relative()
            .bg(theme.bg_2)
            .border_r_1()
            .border_color(theme.border_3)
            .child(header)
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
                    let rows = list(
                        self.list.clone(),
                        cx.processor(|this, ix: usize, window, cx| this.render_row(ix, window, cx)),
                    )
                    .size_full()
                    .pt(px(2.))
                    .pb(px(12.));
                    let scroll = Scroll::List(self.list.clone());
                    el.child(scrolled(rows, "navigator-scrollbar", scroll, &theme))
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
        let held = window.modifiers();
        let hint = match &self.payload {
            Cut::Commit(..) if held.alt => Some("Squash"),
            Cut::Commit(..) | Cut::Pick(..) if held.shift => Some("Below"),
            Cut::Commit(..) => Some("⌥ squash · ⇧ below"),
            Cut::Pick(..) => Some("⇧ below"),
            Cut::Branch(..) | Cut::Files { .. } => None,
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
                    Cut::Commit(..) | Cut::Pick(..) => Icon::Commit,
                    Cut::Branch(..) => Icon::Branch,
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
