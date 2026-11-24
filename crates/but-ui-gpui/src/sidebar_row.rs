//! The rows of the sidebar's trees, as but-ui-react's `SidebarRow`: one row drawn on one line,
//! [`sidebar_row`], and the kinds the Workspace tree builds from it, as `RepoItem`, `BranchItem`
//! and the rest, each setting the row's glyph and words as React's does.

use std::rc::Rc;

use gpui::{
    AnimationExt as _, AnyElement, App, Div, ElementId, Hsla, MouseButton, MouseDownEvent,
    SharedString, Stateful, Window, div, prelude::*, relative,
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

/// A colour a row's glyph or words take from the theme as the row is built, so that on a solid
/// selection they take their token's other value, as React's rows flip their scheme there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ink {
    Muted,
    Subtle,
    CommitLocal,
    CommitRemote,
    CommitUpstream,
    CommitIntegrated,
    Warning,
    Success,
    Danger,
    /// The done hue's mark, as a merged pull request's glyph.
    Done,
    /// The done hue's text, as a merged branch's title.
    DoneText,
}

impl Ink {
    pub fn of(self, theme: &Theme) -> Hsla {
        match self {
            Ink::Muted => theme.muted_foreground,
            Ink::Subtle => theme.subtle_foreground,
            Ink::CommitLocal => theme.commit_local,
            Ink::CommitRemote => theme.commit_remote,
            Ink::CommitUpstream => theme.commit_upstream,
            Ink::CommitIntegrated => theme.commit_integrated,
            Ink::Warning => theme.warning_mark,
            Ink::Success => theme.success_mark,
            Ink::Danger => theme.danger_mark,
            Ink::Done => theme.done_mark,
            Ink::DoneText => theme.done_text,
        }
    }
}

/// A part of what a row says, drawn as the row is built.
enum Part {
    Element(AnyElement),
    /// Words in an [`Ink`].
    Inked(Box<Div>, Ink),
    Badge(BadgeVariant, SharedString),
}

/// A part of a row's meta, drawn as the row is built.
enum Meta {
    Element(AnyElement),
    Count {
        kind: MetaCountType,
        value: Option<SharedString>,
        tip: Option<SharedString>,
    },
}
type Press = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>;

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
    on_fold_press: Option<Press>,
    row_folds: bool,
    inset: f32,
    meta_gives_way: bool,
    selected: bool,
    solid: bool,
    linked: bool,
    busy: bool,
    busy_color: Option<Hsla>,
    unread: bool,
    quiet: bool,
    glyph: Option<AnyElement>,
    icon: Option<(Icon, f32, Ink)>,
    fold_room: bool,
    muted_text: bool,
    label: Vec<Part>,
    meta: Vec<Meta>,
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
        on_fold_press: None,
        row_folds: false,
        inset: 0.,
        meta_gives_way: false,
        selected: false,
        solid: false,
        linked: false,
        busy: false,
        busy_color: None,
        unread: false,
        quiet: false,
        glyph: None,
        icon: None,
        fold_room: false,
        muted_text: false,
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

    /// What a press on the chevron does before its click, for a host whose rows take the
    /// press themselves (to select, or to start a drag) and that keeps the keys where they are.
    pub fn on_fold_press(
        mut self,
        press: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_fold_press = Some(Rc::new(press));
        self
    }

    /// Sets the row's contents in by `px` inside its ground, for a host that lays out its tree's
    /// steps and guides itself; the ground stays the row's full width.
    pub fn inset(mut self, px: f32) -> Self {
        self.inset = px;
        self
    }

    /// The meta holds a name, a `MetaCount` `head`: it gives way before the row's own name, down
    /// to its glyph, as React's row does when its meta has a `data-meta-name`. Give that count
    /// `min_w_0` so its value can shrink.
    pub fn meta_gives_way(mut self) -> Self {
        self.meta_gives_way = true;
        self
    }

    /// The field its name is being edited in, in place of what it says.
    pub fn rename(mut self, field: impl IntoElement) -> Self {
        self.label = vec![Part::Element(field.into_any_element())];
        self
    }

    /// The row itself is the button that folds, so the chevron only says which way it is.
    pub fn row_folds(mut self) -> Self {
        self.row_folds = true;
        self
    }

    /// The chevron takes its own click again, after a kind that made the row the button.
    pub fn chevron_folds(mut self) -> Self {
        self.row_folds = false;
        self
    }

    /// Selected: the quiet fill, or with `solid` the solid one while the list holds the keys. A
    /// row that folds on its click ([`Self::row_folds`]) takes the focus ring instead, only
    /// while the list holds the keys.
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

    /// The row's glyph from the icon set, in the row's muted ink, which on a solid selection is
    /// the inverted one, however late the row is selected.
    pub fn icon(mut self, icon: Icon, _theme: &Theme) -> Self {
        self.icon = Some((icon, 14., Ink::Muted));
        self
    }

    /// As [`Self::icon`], 12 points, as a row that only pages a list wears it.
    pub fn small_icon(mut self, icon: Icon) -> Self {
        self.icon = Some((icon, 12., Ink::Muted));
        self
    }

    /// The row's glyph from the icon set in `ink`, `px` points.
    pub fn inked_icon(mut self, icon: Icon, px: f32, ink: Ink) -> Self {
        self.icon = Some((icon, px, ink));
        self
    }

    /// The row's words in its muted ink rather than its own, as a row that only pages a list
    /// says them, still legible on a solid selection.
    pub fn muted_text(mut self) -> Self {
        self.muted_text = true;
        self
    }

    /// Keeps a chevron's room with no chevron in it, for a row that folds while its folds are
    /// set aside, as while a filter lists what it finds, so the row doesn't move.
    pub fn fold_room(mut self) -> Self {
        self.fold_room = true;
        self
    }

    /// What the row says, its name and any badge before it, laid out 6px apart.
    pub fn label(mut self, part: impl IntoElement) -> Self {
        self.label.push(Part::Element(part.into_any_element()));
        self
    }

    /// Words in `ink`, among what the row says.
    pub fn inked_label(mut self, words: Div, ink: Ink) -> Self {
        self.label.push(Part::Inked(Box::new(words), ink));
        self
    }

    /// A badge among what the row says, as a pull request's number.
    pub fn badge(mut self, variant: BadgeVariant, words: impl Into<SharedString>) -> Self {
        self.label.push(Part::Badge(variant, words.into()));
        self
    }

    /// `MetaCount`s and a `MetaStatus`, in the order the row should read them.
    pub fn meta(mut self, count: impl IntoElement) -> Self {
        self.meta.push(Meta::Element(count.into_any_element()));
        self
    }

    /// A `MetaCount` of `kind`, saying `tip` under the pointer, drawn as the row is built so it
    /// takes the solid selection's colours.
    pub fn count(
        mut self,
        kind: MetaCountType,
        value: Option<SharedString>,
        tip: Option<SharedString>,
    ) -> Self {
        self.meta.push(Meta::Count { kind, value, tip });
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

    pub fn build(mut self, theme: &Theme) -> Stateful<Div> {
        // A row that folds on its click is a button, as React's GroupLabel, not an item: it
        // takes no selection's fill, and with the keys on it, the focus ring.
        let ringed = self.row_folds && self.solid;
        if self.row_folds {
            self.selected = false;
            self.solid = false;
        }
        // What the row's parts take their colours from: on a solid selection, each token's
        // other value.
        let flipped;
        let inks = match self.solid {
            true => {
                flipped = theme.for_solid();
                &flipped
            }
            false => theme,
        };
        let muted = Ink::Muted.of(inks);
        let guide_color = if self.solid {
            theme.derived.inverse_guide
        } else {
            theme.border_subtle
        };
        let glyph = (self.glyph).or_else(|| {
            (self.icon).map(|(icon, px, ink)| {
                icon_at(icon, px)
                    .text_color(ink.of(inks))
                    .into_any_element()
            })
        });
        let has_glyph = glyph.is_some() || self.busy;
        let fold_room = self.fold_room && self.fold.is_none();
        let inset = self.inset;
        // In a tree that shows splits, a row without one keeps the cell's room: the end's gap,
        // the cell's own step and its 16px.
        let split_room = self.split_room && self.split.is_none();
        let steps = (self.depth > 0 || self.fold.is_some() || fold_room).then(|| {
            let fold_step = (self.fold).map(|fold| {
                let press = self.on_fold_press.clone();
                fold_step(
                    &self.id,
                    fold,
                    self.on_toggle.clone(),
                    press,
                    self.row_folds,
                    self.solid,
                    inks,
                )
            });
            let fold_step =
                fold_step.or_else(|| fold_room.then(|| h_flex().flex_none().w(dp(12.))));
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
        let unstepped = steps.is_none();
        let glyph = match (self.busy, glyph) {
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
                // A row that doesn't fold starts its glyph in a chevron's column: a pixel into the
                // padding at the top level, 2 on under a row a host steps it in under.
                .when(unstepped && inset == 0., |el| el.ml(dp(-1.)))
                .when(unstepped && inset > 0., |el| el.ml(dp(2.)))
                .child(glyph)
        });
        let gives_way = self.meta_gives_way;
        let solid = self.solid;
        let labels = self.label.into_iter().map(|part| match part {
            Part::Element(el) => el,
            Part::Inked(words, ink) => (*words).text_color(ink.of(inks)).into_any_element(),
            Part::Badge(variant, words) => badge(variant, BadgeSize::Regular, inks)
                .child(words)
                .into_any_element(),
        });
        let labels: Vec<AnyElement> = labels.collect();
        let metas: Vec<AnyElement> = (self.meta.into_iter())
            .map(|meta| match meta {
                Meta::Element(el) => el,
                Meta::Count { kind, value, tip } => {
                    let count = meta_count_on(kind, value, solid, theme);
                    match tip {
                        Some(tip) => count
                            .id(ElementId::Name(tip.clone()))
                            .aria_label(tip.clone())
                            .tooltip(Tooltip::text(tip))
                            .into_any_element(),
                        None => count.into_any_element(),
                    }
                }
            })
            .collect();
        // Counts last on the row, with no button or map after them, have no edge of their own,
        // and their padding sets their text in less than a button's sets its label: they stand
        // as far further in as a split's map does, so they don't look stuck to the row's end. In
        // a tree that keeps a map's room they aren't at the end, and stay put.
        let counts_last = self.actions.is_empty() && self.split.is_none() && !split_room;
        let end =
            (!metas.is_empty() || !self.actions.is_empty() || self.split.is_some()).then(|| {
                h_flex()
                    .gap(dp(4.))
                    .map(|mut el| {
                        // Many times the name's shrink, so the name keeps every pixel while the
                        // meta's name has room to give.
                        match gives_way {
                            true => el.style().flex_shrink = Some(10000.),
                            false => el.style().flex_shrink = Some(0.),
                        }
                        el.when(gives_way, |el| el.min_w_0())
                    })
                    .when(!metas.is_empty(), |el| {
                        el.child(
                            h_flex()
                                .when(!gives_way, |el| el.flex_none())
                                .when(gives_way, |el| el.min_w_0())
                                .when(counts_last, |el| el.mr(dp(4.)))
                                .children(metas),
                        )
                    })
                    .when(!self.actions.is_empty(), |el| {
                        el.child(h_flex().flex_none().gap(dp(4.)).children(self.actions))
                    })
                    .children(self.split.map(|map| {
                        h_flex()
                            .flex_none()
                            .w(dp(16.))
                            .ml(dp(4.))
                            // A mark with no edge or padding of its own, set in as far as a
                            // count's glyph, so it doesn't look stuck to the row's end.
                            .mr(dp(4.))
                            .justify_center()
                            .child(map)
                    }))
            });
        // The end sits as far in from the row's edge as a small button in it does from its top
        // and bottom, as but-ui-react's row pads its end.
        let end_pad = (theme.sizes.control - theme.sizes.control_sm) / 2.;
        h_flex()
            .id(self.id)
            .relative()
            .flex_none()
            .h(dp(theme.sizes.control))
            .pl(dp(8. + self.inset))
            .pr(dp(end_pad))
            .gap(dp(8.))
            .rounded(dp(theme.shape.control))
            .text_color(match (self.solid, self.muted_text) {
                (true, false) => theme.inverse_foreground,
                (false, false) => theme.foreground,
                (_, true) => muted,
            })
            .map(|el| match (self.solid, self.selected, self.linked) {
                (true, _, _) => el.bg(theme.inverse),
                (false, true, _) => el.bg(theme.selected_inactive),
                (false, false, true) => el.bg(theme.hover),
                (false, false, false) if self.quiet => el,
                (false, false, false) => el.hover(|el| el.bg(theme.hover)),
            })
            .when(ringed, |el| el.shadow(vec![focus_ring(theme)]))
            .when(split_room, |el| el.pr(dp(end_pad + 4. + 16. + 4. + 4.)))
            .children(steps)
            .children(glyph)
            .child(
                // Grows from its words' width rather than from nothing, so a meta that gives way
                // has something to give way to.
                h_flex()
                    .flex_grow(1.)
                    .min_w_0()
                    .overflow_hidden()
                    .gap(dp(6.))
                    .children(labels),
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
fn fold_step(
    id: &ElementId,
    fold: Fold,
    toggle: Option<Toggle>,
    press: Option<Press>,
    row_folds: bool,
    solid: bool,
    theme: &Theme,
) -> Div {
    // On a solid selection, the inverted ghost's, as but-ui-react's chevron button there.
    let ground = match solid {
        true => theme.inverse_ghost_hover,
        false => theme.hover,
    };
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
            let open = fold == Fold::Open;
            match (toggle, row_folds) {
                (Some(toggle), false) => step.child(
                    h_flex()
                        .id("fold")
                        .group(FOLD_GROUP)
                        .absolute()
                        .left(dp(-3.))
                        .w(dp(18.))
                        .h(dp(18.))
                        .top(relative(0.5))
                        .mt(dp(-9.))
                        .justify_center()
                        // As but-ui-react's chevron button: `radius-sm`, the hover's ground under
                        // the pointer, and the chevron a step louder.
                        .rounded(dp(theme.shape.sm))
                        .hover(move |el| el.bg(ground))
                        .child(pressed_chevron((id.clone(), "chevron"), open, theme))
                        .when_some(press, |el, press| {
                            el.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                press(event, window, cx)
                            })
                        })
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            toggle(window, cx);
                        }),
                ),
                _ => step.child(chevron(
                    (id.clone(), "chevron"),
                    open,
                    theme.subtle_foreground,
                )),
            }
        }
    }
}

/// A fold's chevron, in `color` and faded so the glyph after it reads first: pointing right, or
/// down while `open`, turning between the two on the spring a switch's knob slides on. One clicked again mid-turn turns back from where it
/// is, and a row drawn for the first time starts where it points.
pub fn chevron(
    id: impl Into<ElementId>,
    open: bool,
    color: Hsla,
) -> gpui::SpringAnimationElement<gpui::Svg> {
    icon_at(Icon::ChevronRight, 12.)
        .text_color(color)
        .opacity(0.6)
        .with_spring(id, switch_spring(open), move |el, phase| {
            el.with_transformation(gpui::Transformation::rotate(gpui::radians(
                phase.interpolate_clamped(0., std::f32::consts::FRAC_PI_2),
            )))
        })
}

/// The group a chevron that folds on its own click is, so it lights up under the pointer.
const FOLD_GROUP: &str = "sidebar-row-fold";

/// [`chevron`] on its own button, in `subtle`, a step louder in `muted` and at full strength
/// while the pointer's on it, as but-ui-react's chevron button.
fn pressed_chevron(
    id: impl Into<ElementId>,
    open: bool,
    theme: &Theme,
) -> gpui::SpringAnimationElement<gpui::Svg> {
    let muted = theme.muted_foreground;
    icon_at(Icon::ChevronRight, 12.)
        .text_color(theme.subtle_foreground)
        .opacity(0.6)
        .group_hover(FOLD_GROUP, move |s| s.text_color(muted).opacity(1.))
        .with_spring(id, switch_spring(open), move |el, phase| {
            el.with_transformation(gpui::Transformation::rotate(gpui::radians(
                phase.interpolate_clamped(0., std::f32::consts::FRAC_PI_2),
            )))
        })
}

/// A row's name: 14px regular, cut with an ellipsis once its row runs out of room. For a row
/// no kind fits, as the kinds' names are.
pub fn item_name(text: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_14()
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
    /// In the target already: the solid glyph in `commit_integrated`.
    Integrated,
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
    let row = sidebar_row(id).glyph(picture).label(item_name(machine));
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
    sidebar_row(id)
        .icon(Icon::Repo, theme)
        .label(item_name(repo))
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
        true => row.badge(BadgeVariant::LightGray, "Fork"),
        false => row,
    };
    row.label(item_name(worktree))
}

/// A branch: its glyph, and with a pull request, the request's glyph in its state's colour and
/// its number in a badge before the title. A merged one's title recedes until it's picked.
pub fn branch_item(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    pr: Option<(u64, PullRequestState)>,
    merged: bool,
    selected: bool,
    _theme: &Theme,
) -> SidebarRow {
    let state = pr
        .map(|(_, state)| state)
        .or(merged.then_some(PullRequestState::Merged));
    let (glyph, ink) = match pr.map(|(_, state)| state) {
        Some(PullRequestState::Draft) => (Icon::PrDraft, Ink::Muted),
        Some(PullRequestState::Open) => (Icon::Pr, Ink::Success),
        Some(PullRequestState::Closed) => (Icon::PrClose, Ink::Danger),
        Some(PullRequestState::Merged) => (Icon::Pr, Ink::Done),
        None if merged => (Icon::Branch, Ink::Done),
        None => (Icon::Branch, Ink::Muted),
    };
    let row = sidebar_row(id).inked_icon(glyph, 14., ink);
    let row = match (pr, state) {
        (Some((number, state)), _) => {
            let variant = match state {
                PullRequestState::Draft => BadgeVariant::LightGray,
                PullRequestState::Open => BadgeVariant::Safe,
                PullRequestState::Closed => BadgeVariant::Danger,
                PullRequestState::Merged => BadgeVariant::Purple,
            };
            row.badge(variant, format!("#{number}"))
        }
        (None, Some(_)) => row.badge(BadgeVariant::Purple, "Merged"),
        (None, None) => row,
    };
    let title = item_name(title);
    match state == Some(PullRequestState::Merged) && !selected {
        true => row.inked_label(title.opacity(0.8), Ink::DoneText),
        false => row.label(title),
    }
}

/// A commit under its branch: the hollow glyph while it's only here, the solid one once pushed,
/// each in its commit token; a merge's glyph; or the warning in amber when it's conflicted.
pub fn commit_item(
    id: impl Into<ElementId>,
    message: impl Into<SharedString>,
    place: CommitPlace,
    merge: bool,
    conflicted: bool,
    _theme: &Theme,
) -> SidebarRow {
    let (glyph, ink) = match (conflicted, merge, place) {
        (true, _, _) => (Icon::Warning, Ink::Warning),
        (false, true, CommitPlace::Upstream) => (Icon::BranchMerge, Ink::CommitUpstream),
        (false, true, _) => (Icon::BranchMerge, Ink::Muted),
        (false, false, CommitPlace::Local) => (Icon::Commit, Ink::CommitLocal),
        (false, false, CommitPlace::Pushed) => (Icon::CommitFill, Ink::CommitRemote),
        (false, false, CommitPlace::Upstream) => (Icon::CommitFill, Ink::CommitUpstream),
        (false, false, CommitPlace::Integrated) => (Icon::CommitFill, Ink::CommitIntegrated),
    };
    sidebar_row(id).inked_icon(glyph, 14., ink).label(
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
    count: Option<usize>,
    _theme: &Theme,
) -> SidebarRow {
    let row = sidebar_row(id)
        .inked_icon(Icon::ArrowInBox, 14., Ink::CommitUpstream)
        .label(item_name(format!("New on {remote}")));
    // Without one, its next step says how many.
    match count {
        Some(count) => row.count(MetaCountType::Behind, Some(count.to_string().into()), None),
        None => row,
    }
}

/// A worktree's uncommitted changes, and how many files, in amber while there are any.
pub fn uncommitted_item(
    id: impl Into<ElementId>,
    count: usize,
    tip: Option<SharedString>,
    theme: &Theme,
) -> SidebarRow {
    let kind = match count {
        0 => MetaCountType::Clean,
        _ => MetaCountType::Uncommitted,
    };
    sidebar_row(id)
        .icon(Icon::Diff, theme)
        .label(item_name("Uncommitted changes"))
        .count(kind, Some(count.to_string().into()), tip)
}

/// The caption over a stack of branches: how many and "Stack", and while it's folded, the branch
/// at its tip. It doesn't open, so alone it takes no ground under the pointer.
pub fn stack_caption(
    id: impl Into<ElementId>,
    count: usize,
    tip: impl Into<SharedString>,
    folded: Option<bool>,
    _theme: &Theme,
) -> SidebarRow {
    let row = sidebar_row(id)
        .inked_icon(Icon::Stack, 14., Ink::Subtle)
        .badge(BadgeVariant::LightGray, count.to_string())
        .inked_label(item_name("Stack").flex_none(), Ink::Muted);
    match folded {
        // The tip in the caption's own subtle ink, a step under "Stack", as React's.
        Some(true) => row
            .fold(Fold::Folded)
            .inked_label(item_name(tip), Ink::Subtle),
        Some(false) => row.fold(Fold::Open),
        None => row.quiet(),
    }
}

/// A row still loading, as but-ui-react's `LoadingItem`: a bone where its glyph goes and one for
/// its name, `width` wide, at a row's height, so the card holds its shape and nothing moves as the
/// rows land. It takes no pointer: nothing's there yet to press.
pub fn loading_item(id: impl Into<ElementId>, width: f32) -> SidebarRow {
    sidebar_row(id)
        .quiet()
        .glyph(skeleton_circle("bone-glyph", 14.))
        .label(skeleton_text("bone-name", TEXT_11, width))
}

/// The last row of a long list: "12 more", or "Show fewer" once everything shows.
pub fn more_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    expanded: bool,
    _theme: &Theme,
) -> SidebarRow {
    let glyph = match expanded {
        true => Icon::ChevronUp,
        false => Icon::Kebab,
    };
    // Its glyph a step quieter than its words, as React's, so the tail reads quieter still
    // than the rows it hides.
    sidebar_row(id)
        .inked_icon(glyph, 12., Ink::Subtle)
        .muted_text()
        .label(div().min_w_0().truncate().text_13().child(label.into()))
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
    row.inked_label(
        div()
            .min_w_0()
            .truncate()
            .text_12()
            .semibold()
            .child(label.into()),
        Ink::Subtle,
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
        .label(item_name(terminal));
    match place {
        Some(place) => row.inked_label(
            div()
                .min_w_0()
                .flex_shrink(100.)
                .truncate()
                .text_13()
                .child(SharedString::from(format!("/ {place}"))),
            Ink::Muted,
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
        .child(div().min_w_0().truncate().child(message.into()))
        .children(action.map(|action| div().flex_none().underline().child(action)));
    sidebar_row(id)
        .icon(Icon::Warning, theme)
        .inked_label(words, Ink::Muted)
}

const DRAWER: &str = "drawer";

/// A drawer at the foot of the sidebar, as React's `Drawer`: its row, `DrawerItem`'s name, how
/// many it holds (shown at zero too) and a chevron after them, and while it's open, what it holds
/// under it, such as a card of the hidden projects. A press anywhere on the row opens or closes it.
/// The host keeps whether it's `folded`.
///
/// Its buttons sit at the row's end and take their own clicks: [`Drawer::action`]s always show,
/// [`Drawer::hover_action`]s only while the pointer is on the row, before the others, so those
/// keep their place. A ghost icon-only button each.
///
/// It sits on the panel's ground under the cards, with its name muted so the cards read first.
/// A hairline rules it off from what's above, an open drawer's card too; a host with nothing
/// above it passes `ruled(false)`. The drawer at the panel's foot takes `last`, which rules it
/// off from the foot too.
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

    /// Whether a hairline rules it off from what's above.
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
                                    // Laid out all along and only painted under the
                                    // pointer: GPUI paints nothing it hasn't laid out, so
                                    // one shown on hover from `hidden` would abort the app.
                                    .invisible()
                                    .group_hover(DRAWER, |s| s.visible())
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
