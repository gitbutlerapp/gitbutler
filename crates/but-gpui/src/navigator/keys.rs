//! Where the navigator's keys are, beside the page or across the top: the one place drawn as
//! having them. It's found once a frame, before anything draws ([`find_keys`]), and the rows,
//! the tabs and the terminal overview draw from it, rather than each asking what has the keys.

use gpui::{App, Entity, FocusHandle, Window};

use super::cursor::OffRow;
use super::tabs::TabKey;
use super::{Navigator, NavigatorTabs, TerminalOverview};

/// Where the navigator's keys are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NavigatorKeys {
    /// Not in the navigator, as on the page. Each list shows its selection faint.
    Away,
    /// On the selected row of a list.
    Row(List),
    /// On what heads a list and selects nothing: the drawer's bar beside the page, a tab across
    /// the top. No row of the list shows as selected.
    Head(List),
}

/// A list of rows that can have the navigator's keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum List {
    /// The navigator's rows: beside the page its cards, across the top those of the card down
    /// from this tab, which is never the terminals'.
    Rows(Option<TabKey>),
    /// The terminal overview: at the navigator's foot, or in the card down from its tab.
    Terminals,
}

/// What has the keys, of the navigator's parts, with what says where in it they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum KeysIn {
    /// Something else, as the page, or the filter, which types.
    Elsewhere,
    /// The navigator's rows beside the page, or `on_bar`, its drawer's bar.
    Rows { on_bar: bool },
    /// The tabs across the top, on `tab`, whose card of the navigator's rows shows the
    /// selection as one of them while `row_selected`.
    Tab { tab: TabKey, row_selected: bool },
    /// The tabs across the top, on the terminals' tab, whose overview takes them itself.
    TerminalsTab,
    /// The terminal overview in the card down from its tab, which shows the selection as one of
    /// its rows while `row_selected`.
    TerminalsCard { row_selected: bool },
    /// The terminal overview beside the page, which has no head to have them.
    Overview,
}

impl List {
    /// The list in the card down from `tab`.
    pub(super) fn under(tab: &TabKey) -> List {
        match tab {
            TabKey::Terminals => List::Terminals,
            TabKey::Project(_) | TabKey::Checkout(..) | TabKey::More => {
                List::Rows(Some(tab.clone()))
            }
        }
    }

    /// The tab whose card holds it, across the top.
    fn tab(&self) -> Option<TabKey> {
        match self {
            List::Rows(tab) => tab.clone(),
            List::Terminals => Some(TabKey::Terminals),
        }
    }
}

impl NavigatorKeys {
    /// Where the keys are, from what has them: on the row it shows selected, else on what heads
    /// it.
    pub(super) fn of(keys_in: KeysIn) -> NavigatorKeys {
        match keys_in {
            KeysIn::Elsewhere => NavigatorKeys::Away,
            KeysIn::Rows { on_bar: true } => NavigatorKeys::Head(List::Rows(None)),
            KeysIn::Rows { on_bar: false } => NavigatorKeys::Row(List::Rows(None)),
            KeysIn::Tab {
                tab,
                row_selected: true,
            } => NavigatorKeys::Row(List::Rows(Some(tab))),
            KeysIn::Tab {
                tab,
                row_selected: false,
            } => NavigatorKeys::Head(List::Rows(Some(tab))),
            KeysIn::TerminalsTab
            | KeysIn::TerminalsCard {
                row_selected: false,
            } => NavigatorKeys::Head(List::Terminals),
            KeysIn::TerminalsCard { row_selected: true } | KeysIn::Overview => {
                NavigatorKeys::Row(List::Terminals)
            }
        }
    }

    /// The tab the keys are on or in the card down from, across the top.
    pub(super) fn tab(&self) -> Option<TabKey> {
        match self {
            NavigatorKeys::Away => None,
            NavigatorKeys::Row(list) | NavigatorKeys::Head(list) => list.tab(),
        }
    }

    /// The tab the keys are on itself, across the top.
    pub(super) fn head_tab(&self) -> Option<TabKey> {
        match self {
            NavigatorKeys::Head(list) => list.tab(),
            NavigatorKeys::Away | NavigatorKeys::Row(_) => None,
        }
    }

    /// Whether they're on the selected row of `list`.
    pub(super) fn on_row_of(&self, list: &List) -> bool {
        matches!(self, NavigatorKeys::Row(on) if on == list)
    }

    /// Whether they're on what heads `list`, so none of its rows shows as selected.
    pub(super) fn on_head_of(&self, list: &List) -> bool {
        matches!(self, NavigatorKeys::Head(on) if on == list)
    }

    /// Whether they're in `list`, on its head or a row.
    pub(super) fn in_list(&self, list: &List) -> bool {
        self.on_row_of(list) || self.on_head_of(list)
    }
}

/// Find where the navigator's keys are for the frame about to draw, from `keys`, what a list
/// draws as having them, and tell the rows and the terminal overview, which draw from it as the
/// tabs do.
pub(crate) fn find_keys(
    keys: Option<&FocusHandle>,
    navigator: &Entity<Navigator>,
    tabs: &Entity<NavigatorTabs>,
    terminals: &Entity<TerminalOverview>,
    on_top: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let keys_in = match keys {
        None => KeysIn::Elsewhere,
        Some(keys) if terminals.read(cx).holds(keys, window) => match on_top {
            true => KeysIn::TerminalsCard {
                row_selected: overview_lists_selection(terminals, cx),
            },
            false => KeysIn::Overview,
        },
        Some(keys) if on_top => match tabs.read(cx).keys_tab(Some(keys), window, cx) {
            None => KeysIn::Elsewhere,
            Some(TabKey::Terminals) => KeysIn::TerminalsTab,
            Some(tab) => {
                let row_selected = card_lists_selection(navigator, &tab, cx);
                KeysIn::Tab { tab, row_selected }
            }
        },
        Some(keys) => {
            let navigator = navigator.read(cx);
            match navigator.holds(keys, window, cx) {
                true => KeysIn::Rows {
                    on_bar: navigator.off_row == Some(OffRow::DrawerBar),
                },
                false => KeysIn::Elsewhere,
            }
        }
    };
    let keys = NavigatorKeys::of(keys_in);
    let left_unselected = on_top && tabs.read(cx).terminals_left_unselected(keys.tab().as_ref());
    navigator.update(cx, |navigator, cx| {
        navigator.set_keys(keys.clone(), window, cx)
    });
    terminals.update(cx, |terminals, cx| {
        terminals.set_keys(keys, left_unselected, window, cx)
    });
}

/// Whether the card down from `tab` shows the selection as one of its rows. A tab that selects
/// its project or worktree is that row itself; More projects' card lists the projects not
/// watched, unless the keys were stepped up off them onto it.
fn card_lists_selection(navigator: &Entity<Navigator>, tab: &TabKey, cx: &mut App) -> bool {
    match tab.selection() {
        Some(own) => navigator.read(cx).store.read(cx).selection.as_ref() != Some(&own),
        None => navigator.update(cx, |navigator, cx| {
            navigator.in_scope(tab, cx, |navigator, cx| {
                !navigator.stepped_up() && navigator.selected_card_row(cx).is_some()
            })
        }),
    }
}

/// Whether the overview shows the selection as one of its rows, sorted out first as it will be
/// drawn.
fn overview_lists_selection(terminals: &Entity<TerminalOverview>, cx: &mut App) -> bool {
    terminals.update(cx, |terminals, cx| {
        terminals.sort_out(false, cx);
        terminals.lists_selection(cx)
    })
}

#[cfg(test)]
mod tests {
    use super::{KeysIn, List, NavigatorKeys};
    use crate::navigator::tabs::TabKey;
    use crate::state::ProjectId;

    fn project() -> TabKey {
        TabKey::Project(ProjectId::from("ringo"))
    }

    #[test]
    fn nothing_has_them_off_the_navigator() {
        assert_eq!(NavigatorKeys::of(KeysIn::Elsewhere), NavigatorKeys::Away);
    }

    #[test]
    fn the_rows_beside_the_page_have_them_on_the_selected_row() {
        assert_eq!(
            NavigatorKeys::of(KeysIn::Rows { on_bar: false }),
            NavigatorKeys::Row(List::Rows(None))
        );
    }

    #[test]
    fn the_drawers_bar_heads_the_rows_beside_the_page() {
        assert_eq!(
            NavigatorKeys::of(KeysIn::Rows { on_bar: true }),
            NavigatorKeys::Head(List::Rows(None))
        );
    }

    #[test]
    fn a_tab_has_them_while_its_card_doesnt_show_the_selection() {
        let keys_in = KeysIn::Tab {
            tab: project(),
            row_selected: false,
        };
        assert_eq!(
            NavigatorKeys::of(keys_in),
            NavigatorKeys::Head(List::Rows(Some(project())))
        );
    }

    #[test]
    fn a_row_in_a_tabs_card_has_them_when_selected() {
        let keys_in = KeysIn::Tab {
            tab: TabKey::More,
            row_selected: true,
        };
        assert_eq!(
            NavigatorKeys::of(keys_in),
            NavigatorKeys::Row(List::Rows(Some(TabKey::More)))
        );
    }

    #[test]
    fn the_terminals_tab_has_them_with_the_strip() {
        assert_eq!(
            NavigatorKeys::of(KeysIn::TerminalsTab),
            NavigatorKeys::Head(List::Terminals)
        );
    }

    #[test]
    fn the_terminals_tab_has_them_while_its_card_shows_no_selected_row() {
        let keys_in = KeysIn::TerminalsCard {
            row_selected: false,
        };
        assert_eq!(
            NavigatorKeys::of(keys_in),
            NavigatorKeys::Head(List::Terminals)
        );
    }

    #[test]
    fn a_row_of_the_terminals_card_has_them_when_selected() {
        let keys_in = KeysIn::TerminalsCard { row_selected: true };
        assert_eq!(
            NavigatorKeys::of(keys_in),
            NavigatorKeys::Row(List::Terminals)
        );
    }

    #[test]
    fn the_overview_beside_the_page_has_them_on_its_rows() {
        assert_eq!(
            NavigatorKeys::of(KeysIn::Overview),
            NavigatorKeys::Row(List::Terminals)
        );
    }
}
