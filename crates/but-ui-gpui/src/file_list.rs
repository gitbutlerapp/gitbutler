//! A panel of changed files and its rows, as but-ui-react's `FileList` and `FileListItem`.
//!
//! [`file_list`] draws the panel: a header naming the files, with their count and line totals,
//! over the rows, inset from its edges and a pixel apart. [`file_list_item`] draws a row: a
//! file's icon, its name and path, its line counts and what happened to it, or a directory's,
//! stepped in under the directory it sits in.

use std::ops::Range;

use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, Hsla, Role, SharedString, Toggled, Window, div,
    prelude::*, relative,
};

use crate::file_icons::{file_icon, folder_icon};
use crate::theme::{Theme, tokens};
use crate::*;

type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// What happened to a file, as but-ui-react's `FileStatusType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Addition,
    Deletion,
    Modification,
    Rename,
}

impl FileStatus {
    /// The letter the row's last cell shows.
    pub fn letter(self) -> &'static str {
        match self {
            FileStatus::Addition => "A",
            FileStatus::Deletion => "D",
            FileStatus::Modification => "M",
            FileStatus::Rename => "R",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            FileStatus::Addition => "Addition",
            FileStatus::Deletion => "Deletion",
            FileStatus::Modification => "Modification",
            FileStatus::Rename => "Rename",
        }
    }

    pub fn color(self, theme: &Theme) -> Hsla {
        let [added, modified, deleted, renamed] = theme.change_status;
        match self {
            FileStatus::Addition => added,
            FileStatus::Deletion => deleted,
            FileStatus::Modification => modified,
            FileStatus::Rename => renamed,
        }
    }
}

/// How one level's guide draws on a tree's row, as but-ui-react's `Guides` has them: `Line` runs
/// on through the row; `Last`, on the last row under its directory, turns into the row's glyph
/// and ends there; `None` on the rows under a `Last` one, where that level has ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guide {
    Line,
    Last,
    None,
}

/// Each row's guides, outermost first, from how deep each row of a tree sits, in the order they
/// draw, as but-ui-react's `treeGuides`. A row can't see the rows after it, so the list works its
/// guides out here from the rows it shows.
pub fn tree_guides(depths: &[usize]) -> Vec<Vec<Guide>> {
    let mut last = vec![false; depths.len()];
    let mut followed: Vec<bool> = Vec::new();
    for (ix, &depth) in depths.iter().enumerate().rev() {
        if let Some(slot) = last.get_mut(ix) {
            *slot = !followed.get(depth).copied().unwrap_or(false);
        }
        followed.resize(depth + 1, false);
        if let Some(slot) = followed.get_mut(depth) {
            *slot = true;
        }
    }
    let mut ancestor_last: Vec<bool> = Vec::new();
    (depths.iter().zip(last))
        .map(|(&depth, is_last)| {
            let guides = (0..depth)
                .map(|level| match (level + 1 < depth, is_last) {
                    (true, _) if ancestor_last.get(level + 1).copied().unwrap_or(false) => {
                        Guide::None
                    }
                    (true, _) => Guide::Line,
                    (false, true) => Guide::Last,
                    (false, false) => Guide::Line,
                })
                .collect();
            ancestor_last.resize(depth + 1, false);
            if let Some(slot) = ancestor_last.get_mut(depth) {
                *slot = is_last;
            }
            guides
        })
        .collect()
}

/// A panel of changed files: `title`, with how many there are (`count`) and the lines they add
/// and remove, over `rows`. `actions` sit at the header's end.
pub fn file_list(
    title: impl Into<SharedString>,
    count: usize,
    (added, removed): (u32, u32),
    actions: Option<AnyElement>,
    rows: impl IntoIterator<Item = AnyElement>,
    theme: &Theme,
) -> Div {
    v_flex()
        .min_h_0()
        .child(
            h_flex()
                .flex_none()
                .gap(dp(tokens::GAP_ICON_TEXT))
                // The title starts 16px in, as the view header's above it; the rows start 10px
                // under the header.
                .pl(dp(tokens::SPACE_3XL))
                .pr(dp(tokens::PADDING_PANEL))
                .pt(dp(tokens::PADDING_PANEL))
                .pb(dp(tokens::SPACE_XL))
                .child(
                    h_flex()
                        .min_w_0()
                        .gap(dp(tokens::GAP_ICON_TEXT))
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_14()
                                .bold()
                                .text_color(theme.text_1)
                                .child(title.into()),
                        )
                        .child(
                            h_flex()
                                .flex_none()
                                .gap(dp(6.))
                                .child(count_badge(count, theme))
                                .when(added + removed > 0, |el| {
                                    el.child(diff_stats(added, removed, false, theme))
                                }),
                        ),
                )
                .children(actions.map(|actions| h_flex().flex_none().ml_auto().child(actions))),
        )
        .child(
            v_flex()
                .gap(fixed(1.))
                .px(dp(tokens::PADDING_PANEL))
                .pb(dp(tokens::PADDING_PANEL))
                .children(rows),
        )
}

/// One changed file, or a directory of them, in a [`file_list`]. Start from `name` and add what
/// the row shows:
///
/// - `directory` is the path to the file, faded after its name, or before it with
///   [`FileListItem::directory_lead`]; leave it out where a tree already says where it is.
/// - `depth` steps the row in one step per directory it sits in, each drawing the guide that ties
///   it to its directory, and `guides`, from [`tree_guides`], end each directory's guide with a
///   tail into its last row. A directory row sets `folded`, which gives it a folder and a chevron
///   in a step of its own before it; `on_toggle_folded` answers the chevron.
/// - `icon` stands in for the file's type mark, or a directory's folder.
/// - `checkbox` shares the icon's cell and takes its place once the pointer is over it or it is
///   checked; a directory's stands for every file under it, mixed while only some are.
/// - `inert` while an action on the file runs, or while it can't be acted on: the row fades and
///   takes no clicks. It shows no spinner of its own: one snackbar says what's running, once for
///   all the files it touches (but-ui's Toasts and snackbars note).
/// - The last cell says what happened: the conflict mark if `conflicted`, else a tick if
///   `reviewed`, which also fades the name, else the letter for its `status`, else a folded
///   directory's `count` of files; `marks` show before it.
/// - The row has no ⋯: its menu opens with a right-click, which the host handles on the row, so
///   the last cell always says what happened and the letters read down the list.
/// - `selected` grounds the row; `focused`, while its list holds the selection focus, makes that
///   solid, with the row's contents inverted.
#[derive(IntoElement)]
pub struct FileListItem {
    id: ElementId,
    name: SharedString,
    directory: Option<SharedString>,
    directory_lead: bool,
    name_matches: Vec<Range<usize>>,
    directory_matches: Vec<Range<usize>>,
    icon: Option<AnyElement>,
    checkbox: Option<(Toggled, OnClick)>,
    depth: usize,
    guides: Vec<Guide>,
    folded: Option<bool>,
    on_toggle_folded: Option<OnClick>,
    status: Option<FileStatus>,
    count: Option<usize>,
    reviewed: bool,
    conflicted: bool,
    selected: bool,
    joined: (bool, bool),
    focused: bool,
    inert: bool,
    marks: Option<AnyElement>,
    added: u32,
    removed: u32,
    on_click: Option<OnClick>,
}

pub fn file_list_item(id: impl Into<ElementId>, name: impl Into<SharedString>) -> FileListItem {
    FileListItem {
        id: id.into(),
        name: name.into(),
        directory: None,
        directory_lead: false,
        name_matches: Vec::new(),
        directory_matches: Vec::new(),
        icon: None,
        checkbox: None,
        depth: 0,
        guides: Vec::new(),
        folded: None,
        on_toggle_folded: None,
        status: None,
        count: None,
        reviewed: false,
        conflicted: false,
        selected: false,
        joined: (false, false),
        focused: false,
        inert: false,
        marks: None,
        added: 0,
        removed: 0,
        on_click: None,
    }
}

impl FileListItem {
    pub fn directory(mut self, directory: impl Into<SharedString>) -> Self {
        self.directory = Some(directory.into());
        self
    }

    /// The path before the name rather than after it.
    pub fn directory_lead(mut self) -> Self {
        self.directory_lead = true;
        self
    }

    /// What a filter found, as byte ranges of the name and of the directory, underlined.
    pub fn matches(
        mut self,
        name: impl IntoIterator<Item = Range<usize>>,
        directory: impl IntoIterator<Item = Range<usize>>,
    ) -> Self {
        self.name_matches = name.into_iter().collect();
        self.directory_matches = directory.into_iter().collect();
        self
    }

    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.icon = Some(icon.into_any_element());
        self
    }

    pub fn checkbox(
        mut self,
        check: Toggled,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.checkbox = Some((check, Box::new(on_click)));
        self
    }

    pub fn depth(mut self, depth: usize, guides: Vec<Guide>) -> Self {
        self.depth = depth;
        self.guides = guides;
        self
    }

    pub fn folded(mut self, folded: bool) -> Self {
        self.folded = Some(folded);
        self
    }

    pub fn on_toggle_folded(
        mut self,
        on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_folded = Some(Box::new(on_toggle));
        self
    }

    pub fn status(mut self, status: FileStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    pub fn reviewed(mut self, reviewed: bool) -> Self {
        self.reviewed = reviewed;
        self
    }

    pub fn conflicted(mut self, conflicted: bool) -> Self {
        self.conflicted = conflicted;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Selected with the rows `above` and `below` it, which GPUI can't see from here: a run of
    /// them reads as one block, as Finder draws a picked run, rounded only where it starts and
    /// ends, with a faint line between two of its rows.
    pub fn joined(mut self, above: bool, below: bool) -> Self {
        self.joined = (above, below);
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Can't be acted on right now, or an action on it is running: faded, and it takes no clicks.
    pub fn inert(mut self, inert: bool) -> Self {
        self.inert = inert;
        self
    }

    pub fn marks(mut self, marks: impl IntoElement) -> Self {
        self.marks = Some(marks.into_any_element());
        self
    }

    pub fn line_counts(mut self, added: u32, removed: u32) -> Self {
        (self.added, self.removed) = (added, removed);
        self
    }

    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(on_click));
        self
    }
}

/// The hover group the icon's cell shows the checkbox for.
const LEAD: &str = "file-list-item-lead";

/// How wide a step of the indent is, and the room between steps.
const STEP: f32 = 12.;
const STEP_GAP: f32 = 4.;

impl RenderOnce for FileListItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let solid = self.selected && self.focused;
        // On the solid ground the row's quieter parts read in the inverted muted colour.
        let muted = match solid {
            true => theme.muted_on_solid,
            false => theme.text_2,
        };
        // React mixes the muted foreground in at 20%, 40% on the solid ground, which isn't a token
        // yet: the subtle border is what the app's trees draw their guides in.
        let guide_color = match solid {
            true => theme.inverse_soft,
            false => theme.border_3,
        };
        let faded = |el: Div| el.when(self.reviewed, |el| el.opacity(0.5));

        let steps = (self.depth > 0 || self.folded.is_some()).then(|| {
            let guides = (0..self.depth).map(|level| {
                let kind = self.guides.get(level).copied().unwrap_or(Guide::Line);
                // A tail turning into the icon ends 3px short of it: further off than a chevron.
                let into_icon = level + 1 == self.depth && self.folded.is_none();
                guide(kind, into_icon, guide_color)
            });
            let chevron = self
                .folded
                .map(|folded| toggle(folded, self.on_toggle_folded, &self.name, solid, &theme));
            h_flex()
                .flex_none()
                .self_stretch()
                .gap(dp(STEP_GAP))
                .children(guides)
                .children(chevron)
        });
        let has_steps = steps.is_some();

        // The icon in the flow, so the cell keeps its 14px; the checkbox over it.
        let lead = h_flex()
            .flex_none()
            .relative()
            .size(dp(14.))
            .min_w(dp(14.))
            .justify_center()
            .group(LEAD)
            .map(|el| {
                let icon = self.icon.unwrap_or_else(|| match self.folded {
                    Some(_) => folder_icon(&theme, cx).into_any_element(),
                    None => file_icon(&self.name, cx).into_any_element(),
                });
                // GPUI's empty checkbox has no ground of its own, as the web's has, so the
                // icon gives way while the checkbox shows rather than showing through it.
                let checked = self
                    .checkbox
                    .as_ref()
                    .map(|(check, _)| *check != Toggled::False);
                let icon = faded(h_flex().flex_none())
                    .text_color(theme.text_3)
                    .map(|el| match checked {
                        Some(true) => el.opacity(0.),
                        Some(false) => el.group_hover(LEAD, |s| s.opacity(0.)),
                        None => el,
                    })
                    .child(icon);
                el.child(icon)
                    .children(self.checkbox.map(|(check, on_click)| {
                        let shown = check != Toggled::False;
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(!shown, |el| {
                                el.opacity(0.).group_hover(LEAD, |s| s.opacity(1.))
                            })
                            .child(checkbox("check", check, &theme).on_click(
                                move |event, window, cx| {
                                    cx.stop_propagation();
                                    on_click(event, window, cx);
                                },
                            ))
                    }))
            });

        let directory = self.directory.map(|directory| {
            let text = match self.directory_lead {
                true => format!("{directory}/"),
                false => directory.to_string(),
            };
            // Subtle, so the name reads first; on the solid ground, the inverted muted colour.
            let color = match solid {
                true => muted,
                false => theme.text_3,
            };
            // Cut in the middle, so the end that tells two folders apart stays.
            truncated(matched(text, self.directory_matches))
                .text_ellipsis_middle()
                .flex_shrink(1000.)
                .text_color(color)
        });
        let name = truncated(matched(self.name, self.name_matches)).flex_shrink(1.);
        let label = faded(
            h_flex()
                .flex_grow(1.)
                .min_w_0()
                .gap(dp(match self.directory_lead {
                    true => 0.,
                    false => 6.,
                }))
                .text_13()
                .line_height(dp(16.)),
        )
        .map(|el| match (directory, self.directory_lead) {
            (Some(directory), true) => el.child(directory).child(name),
            (Some(directory), false) => el.child(name).child(directory),
            (None, _) => el.child(name),
        });

        let status = if self.conflicted {
            Some(
                div()
                    .id("status")
                    .aria_label("Conflicted")
                    .child(conflict_icon(14., &theme, cx)),
            )
        } else if self.reviewed {
            Some(
                div()
                    .w(dp(12.))
                    .flex()
                    .justify_center()
                    .id("status")
                    .aria_label("Reviewed")
                    .child(icon_at(Icon::Tick, 11.).text_color(theme.pop)),
            )
        } else if let Some(status) = self.status {
            Some(
                div()
                    .w(dp(12.))
                    .flex()
                    .justify_center()
                    .text_11()
                    .semibold()
                    .text_color(status.color(&theme))
                    .id("status")
                    .aria_label(status.word())
                    .child(status.letter()),
            )
        } else {
            self.count.map(|count| {
                div()
                    .id("status")
                    .text_11()
                    .text_color(muted)
                    .child(count.to_string())
            })
        };
        // Reserved on every row, so what comes before it lines up down the list.
        let end = div()
            .flex_none()
            .relative()
            .w(dp(22.))
            .h(dp(theme.sizes.control))
            .children(status.map(|status| centred(div().child(status))));

        h_flex()
            .id(self.id)
            .role(Role::ListItem)
            .aria_selected(self.selected)
            .min_h(dp(theme.sizes.control))
            .gap(dp(tokens::GAP_ICON_TEXT))
            // A top-level file has no folder to line up under: its icon starts a pixel into the
            // row's padding, in the chevrons' column.
            .pl(dp(tokens::SPACE_LG - if has_steps { 0. } else { 1. }))
            .pr(dp(tokens::SPACE_SM))
            .rounded(dp(theme.shape.control))
            .text_color(theme.text_1)
            .map(|el| match (self.selected, solid) {
                (true, true) => el.bg(theme.fill_gray).text_color(theme.text_inverted),
                (true, false) => el.bg(theme.selected_unfocused),
                (false, _) => el.hover(|el| el.bg(theme.hover)),
            })
            .when(self.selected && self.joined.1, |el| el.rounded_b_none())
            // The pixel between this row and the one above, filled with the selection, and the
            // faint line across it, set in from the ends as the row's contents are.
            .when(self.selected && self.joined.0, |el| {
                let (fill, line) = match solid {
                    true => (theme.fill_gray, theme.inverse_soft),
                    false => (theme.selected_unfocused, theme.bg_1),
                };
                el.relative()
                    .rounded_t_none()
                    .child(
                        div()
                            .absolute()
                            .top(dp(-1.))
                            .left_0()
                            .right_0()
                            .h(fixed(1.))
                            .bg(fill),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(dp(-1.))
                            .left(dp(tokens::SPACE_LG))
                            .right(dp(tokens::SPACE_LG))
                            .h(fixed(1.))
                            .bg(line),
                    )
            })
            .when(self.inert, |el| el.opacity(0.5))
            .child(
                // The icon sits 6px after the last step, closer than the name after it.
                h_flex()
                    .flex_none()
                    .self_stretch()
                    .gap(dp(6.))
                    .children(steps)
                    .child(lead),
            )
            .child(label)
            .when(self.added + self.removed > 0, |el| {
                el.child(faded(diff_stats(self.added, self.removed, solid, &theme)))
            })
            .children(
                self.marks
                    .map(|marks| h_flex().flex_none().text_color(muted).child(marks)),
            )
            .child(end)
            .when_some(self.on_click.filter(|_| !self.inert), |el, on_click| {
                el.on_click(on_click)
            })
    }
}

/// Text cut short at its end where it doesn't fit.
fn truncated(text: impl IntoElement) -> Div {
    div()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .child(text)
}

fn centred(el: Div) -> Div {
    el.absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
}

/// One level of the indent: the line that joins the rows under one directory, or on its last row
/// the tail that turns into the row's glyph, `into_icon` when that is a file's icon rather than a
/// chevron.
fn guide(kind: Guide, into_icon: bool, color: Hsla) -> Div {
    let step = div().relative().flex_none().w(dp(STEP)).h_full();
    match kind {
        Guide::None => step,
        // Through the pixel between rows too, so the guides of the rows above and below join.
        Guide::Line => step.child(
            div()
                .absolute()
                .top_0()
                .bottom(dp(-1.))
                .left(dp(STEP / 2. - 0.5))
                .w(fixed(1.))
                .bg(color),
        ),
        // Down to the row's middle, then round a corner towards the glyph.
        Guide::Last => step.child(
            div()
                .absolute()
                .top_0()
                .bottom(relative(0.5))
                .left(dp(STEP / 2. - 0.5))
                .right(dp(match into_icon {
                    true => -3.,
                    false => -1.,
                }))
                .border_l_1()
                .border_b_1()
                .border_color(color)
                .rounded_bl(dp(5.)),
        ),
    }
}

/// A directory's chevron, in a step of its own: faded, so the glyph after it reads first. One that
/// folds on its own click takes a ground under the pointer, wider than its step.
fn toggle(folded: bool, on_toggle: Option<OnClick>, name: &str, solid: bool, theme: &Theme) -> Div {
    let glyph = match folded {
        true => Icon::ChevronRight,
        false => Icon::ChevronDown,
    };
    let (rest, hover, ground) = match solid {
        true => (
            theme.muted_on_solid,
            theme.muted_on_solid,
            theme.hover_on_solid,
        ),
        false => (theme.text_3, theme.text_2, theme.hover),
    };
    let chevron = small_icon(glyph).text_color(rest).opacity(0.6);
    let step = div().flex_none().w(dp(STEP)).h_full().flex().items_center();
    let Some(on_toggle) = on_toggle else {
        return step.justify_center().child(chevron);
    };
    let label = match folded {
        true => format!("Expand {name}"),
        false => format!("Collapse {name}"),
    };
    step.child(
        div()
            .id("fold")
            .role(Role::Button)
            .aria_label(SharedString::from(label))
            .aria_expanded(!folded)
            .group("file-list-toggle")
            .flex_none()
            .ml(dp(-3.))
            .w(dp(STEP + 6.))
            .h(dp(18.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(dp(tokens::RADIUS_SM))
            .hover(|el| el.bg(ground))
            .child(chevron.group_hover("file-list-toggle", |s| s.opacity(1.).text_color(hover)))
            .on_click(move |event, window, cx| {
                cx.stop_propagation();
                on_toggle(event, window, cx);
            }),
    )
}

#[cfg(test)]
mod tests {
    use super::{Guide, tree_guides};

    #[test]
    fn guides_end_at_each_directorys_last_row() {
        // src/{components/{Button, Tooltip}, index.ts}, package.json
        let guides = tree_guides(&[0, 1, 2, 2, 1, 0]);
        assert_eq!(
            guides,
            vec![
                vec![],
                vec![Guide::Line],
                vec![Guide::Line, Guide::Line],
                vec![Guide::Line, Guide::Last],
                vec![Guide::Last],
                vec![],
            ],
            "as but-ui-react's treeGuides works them out"
        );
    }

    #[test]
    fn a_level_that_ended_draws_nothing_further_down() {
        // a/{b/{c}}: b is a's last, so under it a's level has ended.
        let guides = tree_guides(&[0, 1, 2]);
        assert_eq!(
            guides,
            vec![vec![], vec![Guide::Last], vec![Guide::None, Guide::Last]],
            "a's guide stops at b"
        );
    }
}
