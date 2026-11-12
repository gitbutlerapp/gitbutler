//! The panes of a page that take the keys. A page says which it shows, and [`pane_area`] lays
//! them out, so going from one to another is going to what's beside it, as between tiles, but
//! for how a tie is settled: see [`pick`]. A new pane is a [`KeyPane`], what its matches ask for,
//! and a line in [`Detail::key_panes`] saying when it shows. One with rows of its own to step
//! through calls [`Detail::leave`] past their ends.

use gpui::{App, Context, FocusHandle, Window};

use super::Detail;
use crate::state::{Area, Selection, Side, along_edge, nearest_beside};

/// A pane of a page that can have the keys, in reading order, which Tab goes through.
/// Not [`crate::state::Pane`], which are the panes the user can resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum KeyPane {
    /// The tabs to a project's pages, over all the rest.
    ProjectTabs,
    /// A Branches, Operations history or project page's list.
    List,
    /// A page's Changes, pull request and brief tabs, over what they show.
    Tabs,
    /// The file list, or the files beside a list page's list.
    Files,
    /// The diff, or a brief in its place, which the keys read on through.
    Diff,
}

impl KeyPane {
    const ALL: [KeyPane; 5] = [
        KeyPane::ProjectTabs,
        KeyPane::List,
        KeyPane::Tabs,
        KeyPane::Files,
        KeyPane::Diff,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            KeyPane::ProjectTabs => "project-tabs",
            KeyPane::List => "list",
            KeyPane::Tabs => "tabs",
            KeyPane::Files => "files",
            KeyPane::Diff => "diff",
        }
    }

    pub(crate) fn named(name: &str) -> Option<KeyPane> {
        KeyPane::ALL.into_iter().find(|pane| pane.name() == name)
    }

    /// Whether it's a row of tabs, over the rest of the page, rather than what the page shows.
    fn is_tabs(self) -> bool {
        match self {
            KeyPane::ProjectTabs | KeyPane::Tabs => true,
            KeyPane::List | KeyPane::Files | KeyPane::Diff => false,
        }
    }
}

/// How the keys go on from where they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyMove {
    /// To what's on that side, as the arrows and h, j, k and l do.
    Beside(Side),
    /// To the tile on that side, past the panes of the one with the keys, as ⇧⌘H, ⇧⌘J, ⇧⌘K and
    /// ⇧⌘L do. In the navigator, up and down go to the project on that side.
    TileBeside(Side),
    /// To what's next, round again, as Tab does, or going back, as ⇧Tab does.
    Round { forward: bool },
}

/// Where `pane` sits on a page showing `shown`, in columns and rows rather than pixels.
fn pane_area(pane: KeyPane, shown: &[KeyPane]) -> Area {
    let under_tabs = if shown.contains(&KeyPane::Tabs) {
        2.
    } else {
        1.
    };
    let diff_left = if shown.contains(&KeyPane::Files) {
        2.
    } else {
        1.
    };
    let (left, top, right, bottom) = match pane {
        KeyPane::ProjectTabs => (0., 0., 3., 1.),
        KeyPane::List => (0., 1., 1., 3.),
        KeyPane::Tabs => (1., 1., 3., 2.),
        KeyPane::Files => (1., under_tabs, 2., 3.),
        KeyPane::Diff => (diff_left, under_tabs, 3., 3.),
    };
    Area {
        left,
        top,
        width: right - left,
        height: bottom - top,
    }
}

fn pane_areas(shown: &[KeyPane]) -> Vec<(KeyPane, Area)> {
    (shown.iter())
        .map(|pane| (*pane, pane_area(*pane, shown)))
        .collect()
}

/// Of `candidates`, the one the keys `last` left, else what the page shows before its tabs, the
/// first in reading order.
fn pick(candidates: Vec<KeyPane>, last: Option<KeyPane>) -> Option<KeyPane> {
    (last.filter(|last| candidates.contains(last))).or(candidates
        .into_iter()
        .min_by_key(|pane| (pane.is_tabs(), *pane)))
}

/// The pane on `side` of `from`, of `shown`. `from` needn't be shown, as the keys can be on a
/// card in the diff's place.
fn pane_beside(
    shown: &[KeyPane],
    from: KeyPane,
    side: Side,
    last: Option<KeyPane>,
) -> Option<KeyPane> {
    let placed: Vec<KeyPane> = (shown.iter().copied()).chain([from]).collect();
    let areas = pane_areas(&placed);
    let beside = nearest_beside(&areas, &from, side);
    pick(beside.into_iter().map(|(pane, _)| *pane).collect(), last)
}

/// The pane the keys land on going `side` into a page showing `shown`: of those along the edge
/// they come in by, counting what the page shows apart, as tabs reach further than it does and
/// ← on them would change the page.
fn pane_entered(shown: &[KeyPane], side: Side, last: Option<KeyPane>) -> Option<KeyPane> {
    let edge = side.opposite();
    let areas = pane_areas(shown);
    let content: Vec<(KeyPane, Area)> = (areas.iter().copied())
        .filter(|(pane, _)| !pane.is_tabs())
        .collect();
    let reached = (along_edge(&areas, edge).into_iter()).chain(along_edge(&content, edge));
    pick(reached.copied().collect(), last)
}

/// The pane the keys land on coming `side` into a page showing `shown` from another tile: the one
/// they left, wherever it is on the page, as a tile's gone to as a whole, else as [`pane_entered`].
fn pane_returned_to(shown: &[KeyPane], side: Side, last: Option<KeyPane>) -> Option<KeyPane> {
    (last.filter(|last| shown.contains(last))).or_else(|| pane_entered(shown, side, last))
}

/// The pane after `from` going round, or before it going back, of `shown`, in reading order.
fn pane_round(shown: &[KeyPane], from: KeyPane, forward: bool) -> Option<KeyPane> {
    let next = match forward {
        true => shown.iter().find(|pane| **pane > from),
        false => shown.iter().rfind(|pane| **pane < from),
    };
    next.copied()
}

impl Detail {
    /// The panes that show and can take the keys, in reading order. A page that's an overview
    /// alone has none past the tabs, nor have the files or the diff without changes to show.
    pub(crate) fn key_panes(&self, cx: &App) -> Vec<KeyPane> {
        let list_page = self.list_page();
        let changes = !self.without_changes(cx);
        let review = self.showing_review(cx);
        let brief = self.showing_brief();
        let graph = self.showing_graph();
        let project_tabs = matches!(self.selection(), Some(Selection::Project(_)));
        let files = match list_page {
            true => self.beside_files_shown(cx),
            false => self.files_shown(cx),
        };
        [
            (KeyPane::ProjectTabs, project_tabs),
            (KeyPane::List, list_page),
            (KeyPane::Tabs, self.tabs_shown(cx)),
            (KeyPane::Files, changes && !review && !brief && files),
            (
                KeyPane::Diff,
                brief || graph || (changes && !review && !self.overview_only()),
            ),
        ]
        .into_iter()
        .filter_map(|(pane, shown)| shown.then_some(pane))
        .collect()
    }

    pub(super) fn pane_focus(&self, pane: KeyPane) -> &FocusHandle {
        match pane {
            KeyPane::ProjectTabs => &self.project_tabs_focus,
            KeyPane::List => &self.side_focus,
            KeyPane::Tabs => &self.tabs_focus,
            KeyPane::Files => &self.files_focus,
            KeyPane::Diff => &self.focus,
        }
    }

    /// The pane with the keys. A People card counts as the list it's under, and anything else as
    /// the diff, as what merged lately does, drawn in its place.
    pub(crate) fn pane_with_keys(&self, window: &Window, cx: &App) -> KeyPane {
        if self.people_focus.is_focused(window) {
            return KeyPane::List;
        }
        (self.key_panes(cx).into_iter())
            .find(|pane| self.pane_focus(*pane).is_focused(window))
            .unwrap_or(KeyPane::Diff)
    }

    fn last_pane(&self) -> Option<KeyPane> {
        self.page.files().and_then(|files| files.last_pane)
    }

    /// Note that the keys left `pane`, for coming back. Only a page with files has panes to
    /// choose between.
    pub(crate) fn left_pane(&mut self, pane: KeyPane) {
        if let Some(files) = self.page.files_mut() {
            files.last_pane = Some(pane);
        }
    }

    pub(crate) fn key_pane_after(&self, from: KeyPane, by: KeyMove, cx: &App) -> Option<KeyPane> {
        let shown = self.key_panes(cx);
        match by {
            KeyMove::Beside(side) => pane_beside(&shown, from, side, self.last_pane()),
            KeyMove::TileBeside(_) => None,
            KeyMove::Round { forward } => pane_round(&shown, from, forward),
        }
    }

    pub(crate) fn key_pane_entered(&self, by: KeyMove, cx: &App) -> Option<KeyPane> {
        let shown = self.key_panes(cx);
        match by {
            KeyMove::Beside(side) => pane_entered(&shown, side, self.last_pane()),
            KeyMove::TileBeside(side) => pane_returned_to(&shown, side, self.last_pane()),
            KeyMove::Round { forward: true } => shown.first().copied(),
            KeyMove::Round { forward: false } => shown.last().copied(),
        }
    }

    /// Give `pane` the keys. A list takes them through [`Detail::focus_files`], on its first row
    /// while the page shows its overview.
    pub(crate) fn focus_key_pane(
        &mut self,
        pane: KeyPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match pane {
            KeyPane::List => self.focus_files(window, cx),
            KeyPane::ProjectTabs | KeyPane::Tabs | KeyPane::Files | KeyPane::Diff => {
                window.focus(self.pane_focus(pane), cx);
                cx.notify();
            }
        }
    }

    /// With nothing left to step to on `side` of what has the keys, have them go on from it.
    pub(super) fn leave(&self, side: Side, window: &mut Window, cx: &mut Context<Self>) {
        (self.app).update_later(window, cx, move |app, window, cx| {
            app.move_keys(KeyMove::Beside(side), window, cx)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyPane, pane_beside, pane_entered, pane_returned_to, pane_round};
    use crate::state::Side;

    /// A list page with its files showing and a pull request's tabs.
    const LIST_PAGE: [KeyPane; 5] = [
        KeyPane::ProjectTabs,
        KeyPane::List,
        KeyPane::Tabs,
        KeyPane::Files,
        KeyPane::Diff,
    ];

    /// A commit's page, with a pull request's tabs.
    const DIFF_PAGE: [KeyPane; 3] = [KeyPane::Tabs, KeyPane::Files, KeyPane::Diff];

    /// A list page with no files beside its list and no tabs.
    const PLAIN_LIST_PAGE: [KeyPane; 3] = [KeyPane::ProjectTabs, KeyPane::List, KeyPane::Diff];

    #[test]
    fn going_back_to_a_tile_lands_on_the_pane_the_keys_left() {
        assert_eq!(
            pane_returned_to(&DIFF_PAGE, Side::Left, Some(KeyPane::Files)),
            Some(KeyPane::Files),
            "the files aren't along the edge it comes in by, but the keys left them"
        );
    }

    #[test]
    fn a_tile_the_keys_never_left_is_entered_by_its_edge() {
        assert_eq!(
            pane_returned_to(&DIFF_PAGE, Side::Left, None),
            Some(KeyPane::Diff),
            "with nothing to go back to, it's the pane on that side"
        );
    }

    #[test]
    fn a_pane_that_has_gone_is_not_returned_to() {
        assert_eq!(
            pane_returned_to(&DIFF_PAGE, Side::Left, Some(KeyPane::List)),
            Some(KeyPane::Diff),
            "this page shows no list to go back to"
        );
    }

    #[test]
    fn right_of_a_list_is_what_tabs_show_not_the_tabs() {
        assert_eq!(
            pane_beside(&LIST_PAGE, KeyPane::List, Side::Right, None),
            Some(KeyPane::Files),
            "tabs only take the keys from the side when they're asked back"
        );
    }

    #[test]
    fn right_of_a_list_is_the_tabs_the_keys_left() {
        assert_eq!(
            pane_beside(&LIST_PAGE, KeyPane::List, Side::Right, Some(KeyPane::Tabs)),
            Some(KeyPane::Tabs),
            "going left out of tabs and back lands on them, past the files"
        );
    }

    #[test]
    fn left_of_tabs_is_the_list_they_sit_beside() {
        assert_eq!(
            pane_beside(&LIST_PAGE, KeyPane::Tabs, Side::Left, None),
            Some(KeyPane::List),
            "the files are under the tabs, not left of them"
        );
    }

    #[test]
    fn left_of_the_diff_are_the_files_then_the_list() {
        assert_eq!(
            pane_beside(&LIST_PAGE, KeyPane::Diff, Side::Left, None),
            Some(KeyPane::Files),
            "the files are between the diff and the list"
        );
        assert_eq!(
            pane_beside(&PLAIN_LIST_PAGE, KeyPane::Diff, Side::Left, None),
            Some(KeyPane::List),
            "without files the list is next to the diff"
        );
    }

    #[test]
    fn nothing_of_the_page_is_past_its_edges() {
        assert_eq!(
            pane_beside(&DIFF_PAGE, KeyPane::Files, Side::Left, None),
            None,
            "left of the file list is another tile, or the navigator"
        );
        assert_eq!(
            pane_beside(&DIFF_PAGE, KeyPane::Diff, Side::Right, None),
            None,
            "right of the diff is another tile, or the window's edge"
        );
    }

    #[test]
    fn over_what_tabs_show_are_the_tabs() {
        assert_eq!(
            pane_beside(&LIST_PAGE, KeyPane::Diff, Side::Top, None),
            Some(KeyPane::Tabs),
            "the diff reaches the tabs before the project's"
        );
        assert_eq!(
            pane_beside(&DIFF_PAGE, KeyPane::Files, Side::Top, None),
            Some(KeyPane::Tabs),
            "so does the file list"
        );
    }

    #[test]
    fn over_tabs_are_the_projects_tabs_on_a_list_page() {
        assert_eq!(
            pane_beside(&LIST_PAGE, KeyPane::Tabs, Side::Top, None),
            Some(KeyPane::ProjectTabs),
            "a list page's tabs sit under the project's"
        );
        assert_eq!(
            pane_beside(&DIFF_PAGE, KeyPane::Tabs, Side::Top, None),
            None,
            "nothing is over a commit's tabs"
        );
    }

    #[test]
    fn over_the_list_are_the_projects_tabs() {
        assert_eq!(
            pane_beside(&LIST_PAGE, KeyPane::List, Side::Top, None),
            Some(KeyPane::ProjectTabs),
            "↑ past the top of the list reaches them"
        );
    }

    #[test]
    fn over_the_diff_without_tabs_are_the_projects_tabs() {
        assert_eq!(
            pane_beside(&PLAIN_LIST_PAGE, KeyPane::Diff, Side::Top, None),
            Some(KeyPane::ProjectTabs),
            "with no tabs between them they're next to each other"
        );
    }

    #[test]
    fn under_the_projects_tabs_is_the_list() {
        assert_eq!(
            pane_beside(&PLAIN_LIST_PAGE, KeyPane::ProjectTabs, Side::Bottom, None),
            Some(KeyPane::List),
            "with nothing to say otherwise ↓ goes to the page's list"
        );
    }

    #[test]
    fn under_the_projects_tabs_is_what_the_keys_left() {
        assert_eq!(
            pane_beside(
                &PLAIN_LIST_PAGE,
                KeyPane::ProjectTabs,
                Side::Bottom,
                Some(KeyPane::Diff)
            ),
            Some(KeyPane::Diff),
            "↓ goes back to the diff ↑ came from"
        );
        assert_eq!(
            pane_beside(
                &LIST_PAGE,
                KeyPane::ProjectTabs,
                Side::Bottom,
                Some(KeyPane::Tabs)
            ),
            Some(KeyPane::Tabs),
            "↓ goes back to the tabs ↑ came from"
        );
    }

    #[test]
    fn what_the_keys_left_only_counts_if_its_on_that_side() {
        assert_eq!(
            pane_beside(
                &LIST_PAGE,
                KeyPane::List,
                Side::Right,
                Some(KeyPane::ProjectTabs)
            ),
            Some(KeyPane::Files),
            "the project's tabs aren't right of the list"
        );
    }

    #[test]
    fn under_tabs_are_the_files_first() {
        assert_eq!(
            pane_beside(&DIFF_PAGE, KeyPane::Tabs, Side::Bottom, None),
            Some(KeyPane::Files),
            "the file list comes before the diff"
        );
    }

    #[test]
    fn under_tabs_is_the_diff_the_keys_left() {
        assert_eq!(
            pane_beside(&DIFF_PAGE, KeyPane::Tabs, Side::Bottom, Some(KeyPane::Diff)),
            Some(KeyPane::Diff),
            "↓ goes back to the diff ↑ came from"
        );
    }

    #[test]
    fn nothing_is_under_a_pull_requests_tab() {
        let shown = [KeyPane::ProjectTabs, KeyPane::List, KeyPane::Tabs];
        assert_eq!(
            pane_beside(&shown, KeyPane::Tabs, Side::Bottom, None),
            None,
            "the pull request's page takes no keys of its own"
        );
    }

    #[test]
    fn a_card_in_the_diffs_place_still_has_the_list_on_its_left() {
        let shown = [KeyPane::ProjectTabs, KeyPane::List];
        assert_eq!(
            pane_beside(&shown, KeyPane::Diff, Side::Left, None),
            Some(KeyPane::List),
            "the keys go on from where the diff would be"
        );
    }

    #[test]
    fn coming_from_the_left_a_list_page_opens_on_its_list() {
        assert_eq!(
            pane_entered(&LIST_PAGE, Side::Right, None),
            Some(KeyPane::List),
            "the project's tabs reach that edge too, but the list comes first"
        );
    }

    #[test]
    fn coming_from_the_left_the_tabs_the_keys_left_come_back() {
        assert_eq!(
            pane_entered(&LIST_PAGE, Side::Right, Some(KeyPane::ProjectTabs)),
            Some(KeyPane::ProjectTabs),
            "going left out of the project's tabs and back lands on them"
        );
        assert_eq!(
            pane_entered(&DIFF_PAGE, Side::Right, Some(KeyPane::Tabs)),
            Some(KeyPane::Tabs),
            "and out of a commit's tabs"
        );
    }

    #[test]
    fn coming_from_the_left_skips_nothing_to_reach_what_the_keys_left() {
        assert_eq!(
            pane_entered(&DIFF_PAGE, Side::Right, Some(KeyPane::Diff)),
            Some(KeyPane::Files),
            "the diff isn't on that edge, so the file list beside it takes them"
        );
    }

    #[test]
    fn coming_from_the_left_tabs_with_nothing_under_them_take_the_keys() {
        assert_eq!(
            pane_entered(&[KeyPane::Tabs], Side::Right, None),
            Some(KeyPane::Tabs),
            "under a pull request's tab there's nothing else to land on"
        );
        assert_eq!(
            pane_entered(&[KeyPane::ProjectTabs], Side::Right, None),
            Some(KeyPane::ProjectTabs),
            "nor on the Graph"
        );
    }

    #[test]
    fn coming_from_the_right_lands_on_the_diff() {
        assert_eq!(
            pane_entered(&DIFF_PAGE, Side::Left, None),
            Some(KeyPane::Diff),
            "the tabs reach that edge too, but what they show comes first"
        );
    }

    #[test]
    fn coming_from_the_right_a_list_with_nothing_open_takes_the_keys() {
        let shown = [KeyPane::ProjectTabs, KeyPane::List];
        assert_eq!(
            pane_entered(&shown, Side::Left, None),
            Some(KeyPane::List),
            "the project's tabs reach further right, but ← on them would change the page"
        );
    }

    #[test]
    fn going_down_into_a_list_page_passes_the_projects_tabs() {
        assert_eq!(
            pane_entered(&LIST_PAGE, Side::Bottom, None),
            Some(KeyPane::List),
            "what a page shows comes before the tabs over it"
        );
    }

    #[test]
    fn going_down_into_a_list_page_comes_back_to_what_the_keys_left() {
        assert_eq!(
            pane_entered(&PLAIN_LIST_PAGE, Side::Bottom, Some(KeyPane::Diff)),
            Some(KeyPane::Diff),
            "the list and the diff both start under the project's tabs"
        );
    }

    #[test]
    fn nothing_of_the_page_is_right_of_its_tabs() {
        assert_eq!(
            pane_beside(&LIST_PAGE, KeyPane::ProjectTabs, Side::Right, None),
            None,
            "right of the project's tabs is the next tile, past the last tab"
        );
        assert_eq!(
            pane_beside(&LIST_PAGE, KeyPane::Tabs, Side::Right, None),
            None,
            "and of a pull request's"
        );
    }

    #[test]
    fn a_page_with_no_panes_takes_no_keys() {
        assert_eq!(
            pane_entered(&[], Side::Right, None),
            None,
            "an overview alone has nothing to land on"
        );
    }

    #[test]
    fn going_round_takes_every_pane_in_order() {
        assert_eq!(
            pane_round(&LIST_PAGE, KeyPane::List, true),
            Some(KeyPane::Tabs),
            "Tab stops on the tabs too"
        );
        assert_eq!(
            pane_round(&LIST_PAGE, KeyPane::Files, false),
            Some(KeyPane::Tabs),
            "and ⇧Tab goes back through them"
        );
    }

    #[test]
    fn going_round_ends_at_the_ends_of_the_page() {
        assert_eq!(
            pane_round(&LIST_PAGE, KeyPane::Diff, true),
            None,
            "past the last pane is the next tile"
        );
        assert_eq!(
            pane_round(&LIST_PAGE, KeyPane::ProjectTabs, false),
            None,
            "before the first is the one before"
        );
    }

    #[test]
    fn every_pane_is_found_by_its_name() {
        for pane in KeyPane::ALL {
            assert_eq!(
                KeyPane::named(pane.name()),
                Some(pane),
                "the control socket names it"
            );
        }
        assert_eq!(
            KeyPane::named("nowhere"),
            None,
            "a name of no pane finds none"
        );
    }
}
