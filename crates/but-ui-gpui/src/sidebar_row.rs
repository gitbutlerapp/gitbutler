//! The rows of the sidebar's trees, as but-ui-react's `SidebarRow`: one row drawn on one line,
//! [`sidebar_row`], and the kinds the Workspace tree builds from it, as `RepoItem`, `BranchItem`
//! and the rest, each setting the row's glyph and words as React's does.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Div, ElementId, Hsla, SharedString, Stateful, Window, div, prelude::*,
    relative,
};

use crate::theme::Theme;
use crate::*;

/// Whether a row folds, as React's `folded`: a chevron in a step of its own before its glyph,
/// pointing down while `Open`, or the dot of a row that could hold children and has none yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fold {
    Open,
    Folded,
    Empty,
}

type Toggle = Rc<dyn Fn(&mut Window, &mut App)>;

/// The one line every sidebar item is drawn on, as React's `Row`: the name takes the line, and
/// the state, the next step and a split's map sit at the end, where they stay. Build it with
/// [`sidebar_row`] or one of the kinds, then [`SidebarRow::build`] it and add the host's own
/// handlers to what that returns.
pub struct SidebarRow {
    id: ElementId,
    depth: usize,
    guides: Vec<Guide>,
    fold: Option<Fold>,
    on_toggle: Option<Toggle>,
    row_folds: bool,
    selected: bool,
    solid: bool,
    linked: bool,
    busy: bool,
    busy_color: Option<Hsla>,
    unread: bool,
    quiet: bool,
    glyph: Option<AnyElement>,
    label: Vec<AnyElement>,
    meta: Vec<AnyElement>,
    actions: Vec<AnyElement>,
    split: Option<AnyElement>,
    split_room: bool,
}

/// A row with nothing on it yet: give it a glyph and its words.
pub fn sidebar_row(id: impl Into<ElementId>) -> SidebarRow {
    SidebarRow {
        id: id.into(),
        depth: 0,
        guides: Vec::new(),
        fold: None,
        on_toggle: None,
        row_folds: false,
        selected: false,
        solid: false,
        linked: false,
        busy: false,
        busy_color: None,
        unread: false,
        quiet: false,
        glyph: None,
        label: Vec::new(),
        meta: Vec::new(),
        actions: Vec::new(),
        split: None,
        split_room: false,
    }
}

impl SidebarRow {
    /// How many levels the row sits below the top of its card, one guide per level, drawn as
    /// `guides` says ([`tree_guides`]); a level left out is a `Line`.
    pub fn depth(mut self, depth: usize, guides: Vec<Guide>) -> Self {
        self.depth = depth;
        self.guides = guides;
        self
    }

    /// Gives the row a chevron, or the dot of a row with nothing to unfold.
    pub fn fold(mut self, fold: Fold) -> Self {
        self.fold = Some(fold);
        self
    }

    /// What the chevron's own click does, which takes no part in the row's click.
    pub fn on_toggle(mut self, toggle: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(toggle));
        self
    }

    /// The row itself is the button that folds, so the chevron only says which way it is.
    pub fn row_folds(mut self) -> Self {
        self.row_folds = true;
        self
    }

    /// Selected: the quiet fill, or with `solid` the solid one while the list holds the keys.
    pub fn selected(mut self, selected: bool, solid: bool) -> Self {
        self.selected = selected;
        self.solid = selected && solid;
        self
    }

    /// Part of something the pointer is on elsewhere in the tree: the hover's ground.
    pub fn linked(mut self, linked: bool) -> Self {
        self.linked = linked;
        self
    }

    /// An action on the row is running: a spinner stands in for its glyph, in `color` where the
    /// row colours its own.
    pub fn busy(mut self, busy: bool, color: Option<Hsla>) -> Self {
        self.busy = busy;
        self.busy_color = color;
        self
    }

    /// Something changed since the row was last opened: a dot in its start padding.
    pub fn unread(mut self, unread: bool) -> Self {
        self.unread = unread;
        self
    }

    /// A row that names what follows and doesn't open, as a stack's caption: no ground under the
    /// pointer.
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }

    /// The row's glyph, coloured by its caller, in a 14px box.
    pub fn glyph(mut self, glyph: impl IntoElement) -> Self {
        self.glyph = Some(glyph.into_any_element());
        self
    }

    /// The row's glyph from the icon set, in the row's muted ink.
    pub fn icon(self, icon: Icon, theme: &Theme) -> Self {
        let color = if self.solid {
            theme.inverse_muted_foreground
        } else {
            theme.muted_foreground
        };
        self.glyph(icon_at(icon, 14.).text_color(color))
    }

    /// What the row says, its name and any badge before it, laid out 6px apart.
    pub fn label(mut self, part: impl IntoElement) -> Self {
        self.label.push(part.into_any_element());
        self
    }

    /// `MetaCount`s and a `MetaStatus`, in the order the row should read them.
    pub fn meta(mut self, count: impl IntoElement) -> Self {
        self.meta.push(count.into_any_element());
        self
    }

    /// The row's next step, a small outline button, after its state and always shown.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// The row's split map, in a cell of its own at its very end.
    pub fn split(mut self, map: impl IntoElement) -> Self {
        self.split = Some(map.into_any_element());
        self
    }

    /// In a tree that shows splits: without one, the row keeps the cell's room, so the counts
    /// line up down the list.
    pub fn split_room(mut self) -> Self {
        self.split_room = true;
        self
    }

    pub fn build(self, theme: &Theme) -> Stateful<Div> {
        let muted = if self.solid {
            theme.inverse_muted_foreground
        } else {
            theme.muted_foreground
        };
        let guide_color = if self.solid {
            theme.derived.inverse_guide
        } else {
            theme.border_subtle
        };
        let has_glyph = self.glyph.is_some() || self.busy;
        // In a tree that shows splits, a row without one keeps the cell's room: the end's gap,
        // the cell's own step and its 16px.
        let split_room = self.split_room && self.split.is_none();
        let steps = (self.depth > 0 || self.fold.is_some()).then(|| {
            let fold_step = (self.fold)
                .map(|fold| fold_step(fold, self.on_toggle.clone(), self.row_folds, theme));
            let levels = self.depth;
            let last_step = fold_step.is_none();
            h_flex()
                .flex_none()
                .self_stretch()
                .gap(dp(4.))
                // The glyph sits 6px after the last step; a row without one keeps a pixel.
                .when(has_glyph, |el| el.mr(dp(6. - 8.)))
                .when(!has_glyph && self.fold.is_none(), |el| el.mr(dp(1.)))
                .children((0..levels).map(|level| {
                    let guide = self.guides.get(level).copied().unwrap_or(Guide::Line);
                    // Into the glyph on the last step, 3px short of it; before a chevron, a pixel on.
                    crate::file_list::guide(
                        guide,
                        last_step && level + 1 == levels,
                        theme.shape.control,
                        guide_color,
                    )
                }))
                .children(fold_step)
        });
        let top_level = steps.is_none();
        let glyph = match (self.busy, self.glyph) {
            (true, _) => Some(spinner(14., self.busy_color.unwrap_or(muted)).into_any_element()),
            (false, glyph) => glyph,
        }
        .map(|glyph| {
            div()
                .flex_none()
                .size(dp(14.))
                .flex()
                .items_center()
                .justify_center()
                // A top-level row that doesn't fold starts its glyph in a chevron's column.
                .when(top_level, |el| el.ml(dp(-1.)))
                .child(glyph)
        });
        let end = (!self.meta.is_empty() || !self.actions.is_empty() || self.split.is_some()).then(
            || {
                h_flex()
                    .flex_none()
                    .gap(dp(4.))
                    .when(!self.meta.is_empty(), |el| {
                        el.child(h_flex().flex_none().children(self.meta))
                    })
                    .when(!self.actions.is_empty(), |el| {
                        el.child(h_flex().flex_none().gap(dp(4.)).children(self.actions))
                    })
                    .children(self.split.map(|map| {
                        h_flex()
                            .flex_none()
                            .w(dp(16.))
                            .ml(dp(4.))
                            .justify_center()
                            .child(map)
                    }))
            },
        );
        h_flex()
            .id(self.id)
            .relative()
            .flex_none()
            .h(dp(theme.sizes.control))
            .px(dp(8.))
            .gap(dp(8.))
            .rounded(dp(theme.shape.control))
            .text_color(if self.solid {
                theme.inverse_foreground
            } else {
                theme.foreground
            })
            .map(|el| match (self.solid, self.selected, self.linked) {
                (true, _, _) => el.bg(theme.inverse),
                (false, true, _) => el.bg(theme.selected_inactive),
                (false, false, true) => el.bg(theme.hover),
                (false, false, false) if self.quiet => el,
                (false, false, false) => el.hover(|el| el.bg(theme.hover)),
            })
            .when(split_room, |el| el.pr(dp(8. + 16. + 4. + 4.)))
            .children(steps)
            .children(glyph)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .gap(dp(6.))
                    .children(self.label),
            )
            .children(end)
            .when(self.unread, |el| {
                el.child(
                    div()
                        .absolute()
                        .size(dp(5.))
                        .left(dp(3.))
                        .top(relative(0.5))
                        .mt(dp(-2.5))
                        .rounded_full()
                        .bg(theme.derived.unread_dot),
                )
            })
    }
}

/// The chevron's step, or the dot of a row with nothing to unfold. The chevron is faded, so the
/// glyph after it reads first; one that folds on its own click takes a ground under the pointer,
/// wider than its step.
fn fold_step(fold: Fold, toggle: Option<Toggle>, row_folds: bool, theme: &Theme) -> Div {
    let step = h_flex()
        .relative()
        .flex_none()
        .w(dp(12.))
        .self_stretch()
        .justify_center();
    match fold {
        Fold::Empty => step.child(
            icon_at(Icon::BulletDisc, 12.)
                .text_color(theme.subtle_foreground)
                .opacity(0.5),
        ),
        Fold::Open | Fold::Folded => {
            let glyph = match fold {
                Fold::Folded => Icon::ChevronRight,
                _ => Icon::ChevronDown,
            };
            let chevron = icon_at(glyph, 12.).text_color(theme.subtle_foreground).opacity(0.6);
            match (toggle, row_folds) {
                (Some(toggle), false) => step.child(
                    h_flex()
                        .id("fold")
                        .absolute()
                        .left(dp(-3.))
                        .w(dp(18.))
                        .h(dp(18.))
                        .top(relative(0.5))
                        .mt(dp(-9.))
                        .justify_center()
                        .rounded(dp(theme.shape.xs))
                        .hover(|el| el.bg(theme.hover))
                        .child(chevron)
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            toggle(window, cx);
                        }),
                ),
                _ => step.child(chevron),
            }
        }
    }
}

/// A row's name: 14px semibold, cut with an ellipsis once its row runs out of room.
fn name(text: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_14()
        .semibold()
        .line_height(dp(17.))
        .child(text.into())
}

/// Where a pull request stands, as a [`branch_item`] draws it: React's `PullRequestState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestState {
    Draft,
    Open,
    Closed,
    Merged,
}

/// Where a commit stands, as a [`commit_item`] colours its glyph from the commit tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitPlace {
    /// Only here: the hollow glyph in `commit_local`.
    Local,
    /// Pushed: the solid glyph in `commit_remote`.
    Pushed,
    /// On the branch's remote, not here yet: the solid glyph in `commit_upstream`.
    Upstream,
}

/// A machine: the card's header when the sidebar is grouped by machine. Its picture is the host's
/// icon, and its status follows its name.
pub fn machine_item(
    id: impl Into<ElementId>,
    machine: impl Into<SharedString>,
    picture: impl IntoElement,
    status: Option<AnyElement>,
) -> SidebarRow {
    let row = sidebar_row(id).glyph(picture).label(name(machine));
    match status {
        Some(status) => row.label(status),
        None => row,
    }
}

/// A repository, under its machine or as a card of its own.
pub fn repo_item(
    id: impl Into<ElementId>,
    repo: impl Into<SharedString>,
    theme: &Theme,
) -> SidebarRow {
    sidebar_row(id).icon(Icon::Repo, theme).label(name(repo))
}

/// A worktree: the main one's folder, a linked one's doubled folder, or a fork's, which says
/// "Fork" before its name.
pub fn worktree_item(
    id: impl Into<ElementId>,
    worktree: impl Into<SharedString>,
    main: bool,
    fork: bool,
    theme: &Theme,
) -> SidebarRow {
    let glyph = match (fork, main) {
        (true, _) => Icon::FolderFork,
        (false, true) => Icon::Folder,
        (false, false) => Icon::FolderCopy,
    };
    let row = sidebar_row(id).icon(glyph, theme);
    let row = match fork {
        true => row.label(badge(BadgeVariant::LightGray, BadgeSize::Regular, theme).child("Fork")),
        false => row,
    };
    row.label(name(worktree))
}

/// A branch: its glyph, and with a pull request, the request's glyph in its state's colour and
/// its number in a badge before the title. A merged one's title recedes until it's picked.
pub fn branch_item(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    pr: Option<(u64, PullRequestState)>,
    merged: bool,
    selected: bool,
    theme: &Theme,
) -> SidebarRow {
    let state = pr
        .map(|(_, state)| state)
        .or(merged.then_some(PullRequestState::Merged));
    let (glyph, color) = match pr.map(|(_, state)| state) {
        Some(PullRequestState::Draft) => (Icon::PrDraft, theme.muted_foreground),
        Some(PullRequestState::Open) => (Icon::Pr, theme.success_mark),
        Some(PullRequestState::Closed) => (Icon::PrClose, theme.danger_mark),
        Some(PullRequestState::Merged) => (Icon::Pr, theme.done_mark),
        None if merged => (Icon::Branch, theme.done_mark),
        None => (Icon::Branch, theme.muted_foreground),
    };
    let row = sidebar_row(id).glyph(icon_at(glyph, 14.).text_color(color));
    let row = match (pr, state) {
        (Some((number, state)), _) => {
            let variant = match state {
                PullRequestState::Draft => BadgeVariant::LightGray,
                PullRequestState::Open => BadgeVariant::Safe,
                PullRequestState::Closed => BadgeVariant::Danger,
                PullRequestState::Merged => BadgeVariant::Purple,
            };
            row.label(badge(variant, BadgeSize::Regular, theme).child(format!("#{number}")))
        }
        (None, Some(_)) => {
            row.label(badge(BadgeVariant::Purple, BadgeSize::Regular, theme).child("Merged"))
        }
        (None, None) => row,
    };
    let title = name(title);
    let title = match state == Some(PullRequestState::Merged) && !selected {
        true => title.text_color(theme.done_text).opacity(0.6),
        false => title,
    };
    row.label(title)
}

/// A commit under its branch: the hollow glyph while it's only here, the solid one once pushed,
/// each in its commit token; a merge's glyph; or the warning in amber when it's conflicted.
pub fn commit_item(
    id: impl Into<ElementId>,
    message: impl Into<SharedString>,
    place: CommitPlace,
    merge: bool,
    conflicted: bool,
    theme: &Theme,
) -> SidebarRow {
    let glyph = match (conflicted, merge, place) {
        (true, _, _) => icon_at(Icon::Warning, 14.).text_color(theme.warning_mark),
        (false, true, CommitPlace::Upstream) => {
            icon_at(Icon::BranchMerge, 14.).text_color(theme.commit_upstream)
        }
        (false, true, _) => icon_at(Icon::BranchMerge, 14.).text_color(theme.muted_foreground),
        (false, false, CommitPlace::Local) => {
            icon_at(Icon::Commit, 14.).text_color(theme.commit_local)
        }
        (false, false, CommitPlace::Pushed) => {
            icon_at(Icon::CommitFill, 14.).text_color(theme.commit_remote)
        }
        (false, false, CommitPlace::Upstream) => {
            icon_at(Icon::CommitFill, 14.).text_color(theme.commit_upstream)
        }
    };
    sidebar_row(id).glyph(glyph).label(
        div()
            .min_w_0()
            .truncate()
            .text_13()
            .line_height(dp(17.))
            .child(message.into()),
    )
}

/// The heading over the commits a branch's remote has that it doesn't: "New on" the remote, and
/// how many, behind.
pub fn upstream_item(
    id: impl Into<ElementId>,
    remote: &str,
    count: usize,
    theme: &Theme,
) -> SidebarRow {
    sidebar_row(id)
        .glyph(icon_at(Icon::ArrowInBox, 14.).text_color(theme.commit_upstream))
        .label(name(format!("New on {remote}")))
        .meta(meta_count(
            MetaCountType::Behind,
            Some(count.to_string().into()),
            theme,
        ))
}

/// A worktree's uncommitted changes, and how many files, in amber while there are any.
pub fn uncommitted_item(id: impl Into<ElementId>, count: usize, theme: &Theme) -> SidebarRow {
    let kind = match count {
        0 => MetaCountType::Clean,
        _ => MetaCountType::Uncommitted,
    };
    sidebar_row(id)
        .icon(Icon::Diff, theme)
        .label(name("Uncommitted changes"))
        .meta(meta_count(kind, Some(count.to_string().into()), theme))
}

/// The caption over a stack of branches: how many and "Stack", and while it's folded, the branch
/// at its tip. It doesn't open, so alone it takes no ground under the pointer.
pub fn stack_caption(
    id: impl Into<ElementId>,
    count: usize,
    tip: impl Into<SharedString>,
    folded: Option<bool>,
    theme: &Theme,
) -> SidebarRow {
    let row = sidebar_row(id)
        .glyph(icon_at(Icon::Stack, 14.).text_color(theme.subtle_foreground))
        .label(badge(BadgeVariant::LightGray, BadgeSize::Regular, theme).child(count.to_string()))
        .label(name("Stack").flex_none().text_color(theme.muted_foreground));
    match folded {
        Some(true) => row.fold(Fold::Folded).label(name(tip)),
        Some(false) => row.fold(Fold::Open),
        None => row.quiet(),
    }
}

/// The last row of a long list: "12 more", or "Show fewer" once everything shows.
pub fn more_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    expanded: bool,
    theme: &Theme,
) -> SidebarRow {
    let glyph = match expanded {
        true => Icon::ChevronUp,
        false => Icon::Kebab,
    };
    sidebar_row(id)
        .glyph(icon_at(glyph, 12.).text_color(theme.muted_foreground))
        .label(
            div()
                .min_w_0()
                .truncate()
                .text_13()
                .text_color(theme.muted_foreground)
                .child(label.into()),
        )
}

/// The label over a group of pages: its name, small and quiet, so the pages under it read first.
pub fn group_label(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: Option<Icon>,
    folded: Option<bool>,
    theme: &Theme,
) -> SidebarRow {
    let row = sidebar_row(id).row_folds();
    let row = match folded {
        Some(true) => row.fold(Fold::Folded),
        Some(false) => row.fold(Fold::Open),
        None => row,
    };
    let row = match icon {
        Some(icon) => row.icon(icon, theme),
        None => row,
    };
    row.label(
        div()
            .min_w_0()
            .truncate()
            .text_12()
            .semibold()
            .text_color(theme.subtle_foreground)
            .child(label.into()),
    )
}

/// A terminal, by its name, and where it's open after it, quieter, giving way first.
pub fn terminal_item(
    id: impl Into<ElementId>,
    terminal: impl Into<SharedString>,
    place: Option<SharedString>,
    theme: &Theme,
) -> SidebarRow {
    let row = sidebar_row(id)
        .icon(Icon::Terminal, theme)
        .label(name(terminal));
    match place {
        Some(place) => row.label(
            div()
                .min_w_0()
                .flex_shrink(100.)
                .truncate()
                .text_13()
                .text_color(theme.muted_foreground)
                .child(SharedString::from(format!("/ {place}"))),
        ),
        None => row,
    }
}

/// Where rows should be and can't show: what failed, in grey, under the warning glyph, and the
/// way out after it as a link.
pub fn notice_item(
    id: impl Into<ElementId>,
    message: impl Into<SharedString>,
    action: Option<SharedString>,
    theme: &Theme,
) -> SidebarRow {
    let words = h_flex()
        .min_w_0()
        .gap(dp(4.))
        .text_13()
        .text_color(theme.muted_foreground)
        .child(div().min_w_0().truncate().child(message.into()))
        .children(action.map(|action| div().flex_none().underline().child(action)));
    sidebar_row(id).icon(Icon::Warning, theme).label(words)
}

const DRAWER: &str = "drawer";

/// A drawer at the foot of the sidebar, as React's `Drawer`: its row, `DrawerItem`'s name, how
/// many it holds (shown at zero too) and a chevron after them, and while it's open, what it holds
/// under it, such as a card of the other projects. A press anywhere on the row opens or closes it.
/// The host keeps whether it's `folded`.
///
/// Its buttons sit at the row's end and take their own clicks: [`Drawer::action`]s always show,
/// [`Drawer::hover_action`]s only while the pointer is on the row, before the others, so those
/// keep their place. A ghost icon-only button each.
///
/// It sits on the panel's ground under the cards, with its name muted so the cards read first.
/// A hairline rules it off from what's above; under an open drawer its card is the rule, so the
/// host passes `ruled(false)` to the next one. The drawer at the panel's foot takes `last`, which
/// rules it off from the foot too.
pub struct Drawer {
    id: ElementId,
    name: SharedString,
    count: Option<usize>,
    folded: bool,
    on_toggle: Option<Toggle>,
    actions: Vec<AnyElement>,
    hover_actions: Vec<AnyElement>,
    content: Option<AnyElement>,
    ruled: bool,
    last: bool,
}

/// A drawer, folded or open, named `name`, holding `count`.
pub fn drawer(
    id: impl Into<ElementId>,
    name: impl Into<SharedString>,
    count: Option<usize>,
    folded: bool,
) -> Drawer {
    Drawer {
        id: id.into(),
        name: name.into(),
        count,
        folded,
        on_toggle: None,
        actions: Vec::new(),
        hover_actions: Vec::new(),
        content: None,
        ruled: true,
        last: false,
    }
}

impl Drawer {
    /// What a press on the row does: open or close the drawer.
    pub fn on_toggle(mut self, toggle: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(toggle));
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    pub fn hover_action(mut self, action: impl IntoElement) -> Self {
        self.hover_actions.push(action.into_any_element());
        self
    }

    /// What it holds, shown under it while it's open.
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// Whether a hairline rules it off from what's above: not under an open drawer.
    pub fn ruled(mut self, ruled: bool) -> Self {
        self.ruled = ruled;
        self
    }

    pub fn last(mut self) -> Self {
        self.last = true;
        self
    }

    /// The drawer's row alone, as React's `DrawerItem`.
    pub fn item(&mut self, theme: &Theme) -> Stateful<Div> {
        let chevron = match self.folded {
            true => Icon::ChevronRight,
            false => Icon::ChevronDown,
        };
        let label = h_flex()
            .flex_grow(1.)
            .min_w_0()
            .overflow_hidden()
            .gap(dp(6.))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_12()
                    .semibold()
                    .child(self.name.clone()),
            )
            .children(
                self.count
                    .map(|count| count_badge(count, theme).flex_none()),
            )
            .child(
                icon_at(chevron, 12.)
                    .flex_none()
                    .text_color(theme.subtle_foreground)
                    .opacity(0.6)
                    .group_hover(DRAWER, |s| s.text_color(theme.muted_foreground).opacity(1.)),
            );
        let actions = std::mem::take(&mut self.actions);
        let hover_actions = std::mem::take(&mut self.hover_actions);
        let has_end = !actions.is_empty() || !hover_actions.is_empty();
        h_flex()
            .id(self.id.clone())
            .group(DRAWER)
            .h(dp(theme.sizes.control))
            .px(dp(8.))
            .gap(dp(8.))
            .rounded(dp(theme.shape.control))
            .text_color(theme.muted_foreground)
            .role(gpui::Role::Button)
            .aria_label(self.name.clone())
            .aria_expanded(!self.folded)
            .child(label)
            .when(has_end, |el| {
                el.child(
                    h_flex()
                        .flex_none()
                        .gap(dp(4.))
                        .when(!hover_actions.is_empty(), |el| {
                            el.child(
                                h_flex()
                                    .gap(dp(4.))
                                    .hidden()
                                    .group_hover(DRAWER, |s| s.flex())
                                    .children(hover_actions),
                            )
                        })
                        .children(actions)
                        // The buttons take their own clicks, so a press on one doesn't fold.
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                )
            })
            .when_some(self.on_toggle.clone(), |el, toggle| {
                el.on_click(move |_, window, cx| toggle(window, cx))
            })
    }

    /// The drawer: its row, and what it holds under it while it's open.
    pub fn build(mut self, theme: &Theme) -> Div {
        let item = self.item(theme);
        let content = (!self.folded).then_some(self.content).flatten();
        v_flex()
            .py(dp(10.))
            .when(self.ruled, |el| {
                el.border_t_1().border_color(theme.border_subtle)
            })
            .when(self.last, |el| {
                el.border_b_1().border_color(theme.border_subtle)
            })
            .child(item)
            .children(content.map(|content| div().mt(dp(8.)).child(content)))
    }
}
