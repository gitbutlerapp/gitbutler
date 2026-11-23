//! Where the keys go through the navigator's rows, in its list or in the card under a tab:
//! stepping, and folding and opening the row they're on.

use gpui::{App, Context, InteractiveElement, Window};

use super::rows::{Fold, Moving, Row};
use super::{
    ClickOpened, Navigator, NextSection, PreviousSection, SelectFirst, SelectLast, items, parent,
    tabs,
};
use crate::app::{Collapse, Confirm, Expand, SelectNext, SelectPrevious, ToggleFold};
use crate::backend::Checkout;
use crate::state::{ProjectId, Section, Selection};

/// Where a step of the keys lands.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Step {
    /// The item at this index of those listed.
    Item(usize),
    /// The drawer's bar, between the cards' rows and its own.
    Bar,
    /// Up off the first row onto the tab the rows hang from, across the top.
    Tab,
}

/// What the keys can step onto off the rows that's nothing to select, so the selection and its
/// page stay as they were: see [`Step`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum OffRow {
    /// The drawer's bar, which folds.
    DrawerBar,
    /// This tab, which the card's rows hang from, when it's one with nothing to select: More
    /// projects'.
    Tab(tabs::TabKey),
}

/// What a key does to the rows, which the list and the card under a tab both take: see
/// [`row_actions`].
#[derive(Clone, Copy)]
pub(super) enum RowAction {
    /// By `delta` rows, or to the first or the last with a step past either end.
    Step(isize),
    /// To the next section's heading, or the one before.
    Section {
        forward: bool,
    },
    Expand,
    Collapse,
    ToggleFold,
    Confirm,
}

/// Have `el` take the keys that go through the rows and fold them, each going to `act` with
/// what it does, so the list and the card under a tab take the same ones, and the card works on
/// its own rows with every one of them.
pub(super) fn row_actions<E: InteractiveElement, V: 'static>(
    el: E,
    cx: &mut Context<V>,
    act: fn(&mut V, RowAction, &mut Window, &mut Context<V>),
) -> E {
    el.on_action(cx.listener(move |this, _: &SelectPrevious, window, cx| {
        act(this, RowAction::Step(-1), window, cx)
    }))
    .on_action(cx.listener(move |this, _: &SelectNext, window, cx| {
        act(this, RowAction::Step(1), window, cx)
    }))
    .on_action(
        cx.listener(move |this, _: &Expand, window, cx| act(this, RowAction::Expand, window, cx)),
    )
    .on_action(
        cx.listener(move |this, _: &Collapse, window, cx| {
            act(this, RowAction::Collapse, window, cx)
        }),
    )
    .on_action(cx.listener(move |this, _: &ToggleFold, window, cx| {
        act(this, RowAction::ToggleFold, window, cx)
    }))
    .on_action(cx.listener(move |this, _: &SelectFirst, window, cx| {
        act(this, RowAction::Step(isize::MIN / 2), window, cx)
    }))
    .on_action(cx.listener(move |this, _: &SelectLast, window, cx| {
        act(this, RowAction::Step(isize::MAX / 2), window, cx)
    }))
    .on_action(cx.listener(move |this, _: &PreviousSection, window, cx| {
        act(this, RowAction::Section { forward: false }, window, cx)
    }))
    .on_action(cx.listener(move |this, _: &NextSection, window, cx| {
        act(this, RowAction::Section { forward: true }, window, cx)
    }))
    .on_action(
        cx.listener(move |this, _: &Confirm, window, cx| act(this, RowAction::Confirm, window, cx)),
    )
}

/// What a step of the keys stops on besides the items ([`step_to`]).
#[derive(Clone, Copy, Default)]
pub(super) struct Stops {
    /// The drawer's bar, after this many items, while it's a stop.
    pub(super) bar: Option<usize>,
    /// The keys are on the drawer's bar.
    pub(super) on_bar: bool,
    /// Across the top, the tab the rows hang from, above them.
    pub(super) tab: bool,
}

/// Where a step of `delta` goes from item `at` of `items`, or from what else `stops` has the keys
/// on. From off the list, a
/// step forward starts at the top and one back at the bottom. Past an end it's `None`, rather
/// than picking the row again, which would take its page back to how it opens on every repeat
/// of a held key. Under a tab, off the list is on the tab, the top, so from there it's down
/// into the rows only, and a step up from the first row goes back to it, though not one to the
/// first row, which is there already.
pub(super) fn step_to(at: Option<usize>, items: usize, stops: Stops, delta: isize) -> Option<Step> {
    let Stops { bar, on_bar, tab } = stops;
    let count = (items + usize::from(bar.is_some())) as isize;
    if count == 0 {
        return None;
    }
    let from = match (bar.filter(|_| on_bar), at) {
        (Some(bar), _) => bar as isize,
        (None, Some(ix)) if bar.is_some_and(|bar| ix >= bar) => ix as isize + 1,
        (None, Some(ix)) => ix as isize,
        (None, None) if delta > 0 => -1,
        (None, None) if tab && !on_bar => return None,
        (None, None) => count,
    };
    if tab && from == 0 && delta == -1 {
        return Some(Step::Tab);
    }
    let next = (from + delta).clamp(0, count - 1);
    if next == from {
        return None;
    }
    let next = next as usize;
    Some(match bar {
        Some(bar) if next == bar => Step::Bar,
        Some(bar) if next > bar => Step::Item(next - 1),
        Some(_) | None => Step::Item(next),
    })
}

impl Navigator {
    /// Do what a key does to the rows: see [`row_actions`].
    pub(super) fn act_on_rows(
        &mut self,
        action: RowAction,
        window: &mut Window,
        cx: &mut Context<Navigator>,
    ) {
        match action {
            RowAction::Step(delta) => self.step(delta, window, cx),
            RowAction::Section { forward } => self.jump_section(forward, window, cx),
            RowAction::Expand => self.expand(&Expand, window, cx),
            RowAction::Collapse => self.collapse(&Collapse, window, cx),
            RowAction::ToggleFold => self.toggle_fold(&ToggleFold, window, cx),
            RowAction::Confirm => self.confirm(&Confirm, window, cx),
        }
    }

    pub(super) fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        // While commits move, the keys step through where they can land.
        if self.store.read(cx).moving.is_some() {
            return self.step_move(delta, cx);
        }
        // ↓ onto a fold of commits shows the rest and goes on into them, as a feed loads more:
        // the fold is no stop of its own.
        let unfolds = delta > 0 && !self.off_rows() && self.unfold_after_selection(cx);
        let built = unfolds.then(|| self.build_rows(cx));
        let items = match &built {
            Some(built) => self.staying(&built.rows, &built.moving, &built.drawer.rows),
            None => self.items(),
        };
        if items.is_empty() {
            return;
        }
        let current = self.store.read(cx).selection.clone();
        // On the tab, the keys are off the list, above it.
        let at = (current.as_ref())
            .filter(|_| !self.stepped_up())
            .and_then(|s| items.iter().position(|item| *item == s));
        let bar = self.drawer_toggles(cx).then(|| match &built {
            Some(built) => self.staying(&built.rows, &built.moving, &[]).len(),
            None => self
                .staying(&self.built.rows, &self.built.moving, &[])
                .len(),
        });
        let tab = self.scope.is_some();
        let next = match step_to(
            at,
            items.len(),
            Stops {
                bar,
                on_bar: self.off_row == Some(OffRow::DrawerBar),
                tab,
            },
            delta,
        ) {
            Some(Step::Bar) => return self.enter_drawer_bar(cx),
            // The projects not watched have nothing for their tab to select, so the keys go
            // onto it as they go onto the drawer's bar.
            Some(Step::Tab) => match self.scope.as_ref().and_then(tabs::TabKey::selection) {
                Some(tab) => return self.select(tab, window, cx),
                None => {
                    self.off_row = self.scope.clone().map(OffRow::Tab);
                    return cx.notify();
                }
            },
            Some(Step::Item(next)) => next,
            None => return,
        };
        let Some(next) = items.get(next).map(|item| (*item).clone()) else {
            return;
        };
        // Back on the row it left, whose page stays as it was, not back to how it opens.
        if self.off_rows() && current.as_ref() == Some(&next) {
            return self.leave_off_row(cx);
        }
        self.select(next, window, cx);
    }

    /// Whether the drawer's bar opens and closes it, and so is a stop for the keys: with no
    /// cards it's always open, and while the filter lists what it finds, they show as they
    /// are, as cards don't fold.
    pub(super) fn drawer_toggles(&self, cx: &App) -> bool {
        // Under its tab the card shows it whole, with nothing to fold.
        self.scope.is_none() && (!self.built.rows.is_empty() && !self.filtering(cx))
    }

    /// Whether the keys were stepped up off the rows onto the tab the card being worked on hangs
    /// from.
    pub(super) fn stepped_up(&self) -> bool {
        matches!((&self.off_row, &self.scope), (Some(OffRow::Tab(on)), Some(scope)) if on == scope)
    }

    /// Whether the keys are off the rows of what's being worked on: on the drawer's bar, or
    /// stepped up onto its tab.
    fn off_rows(&self) -> bool {
        self.off_row == Some(OffRow::DrawerBar) || self.stepped_up()
    }

    /// Give the drawer's bar the keys, in view.
    fn enter_drawer_bar(&mut self, cx: &mut Context<Navigator>) {
        // Pinned at the column's foot, it's always in view.
        self.off_row = Some(OffRow::DrawerBar);
        cx.notify();
    }

    /// Give the keys back to the selection's row, if what's off the rows has them.
    pub(super) fn leave_off_row(&mut self, cx: &mut Context<Navigator>) {
        if self.off_row.take().is_some() {
            cx.notify();
        }
    }

    /// Unfold the branch's "N more" right under the selection, saying whether there was one.
    fn unfold_after_selection(&mut self, cx: &App) -> bool {
        let selection = self.store.read(cx).selection.as_ref();
        let at = (self.built.rows.iter())
            .position(|row| matches!(row, Row::Item(item) if Some(item) == selection));
        let Some(Row::More(project, ref_name, _)) = at.and_then(|at| self.built.rows.get(at + 1))
        else {
            return false;
        };
        self.set_unfolded((project.clone(), ref_name.clone()), true);
        true
    }

    /// Select the first item before (`forward` false) or after the selection that starts a
    /// section: a project or a branch.
    pub(super) fn jump_section(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.store.read(cx).moving.is_some() {
            return self.jump_move(forward, cx);
        }
        let items = self.items();
        let current = self.store.read(cx).selection.clone();
        let at = current.and_then(|s| items.iter().position(|item| **item == s));
        let heads =
            |item: &&&Selection| matches!(item, Selection::Project(_) | Selection::Branch(..));
        let next = match (at, forward) {
            (Some(at), true) => items.iter().skip(at + 1).find(heads),
            (Some(at), false) => items.iter().take(at).rev().find(heads),
            (None, true) => items.iter().find(heads),
            (None, false) => items.iter().rev().find(heads),
        };
        if let Some(next) = next.map(|item| (*item).clone()) {
            self.select(next, window, cx);
        }
    }

    /// Select the project after (`forward`) or before the one the selection's in, past what's
    /// left of its card, as the keys go from tile to tile, at the row it was left on. Not while
    /// something's being moved. Whether there was a project that way.
    pub(crate) fn step_project(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let store = self.store.read(cx);
        if store.moving.is_some() {
            return true;
        }
        let items = self.items();
        let projects: Vec<&ProjectId> = (items.iter())
            .filter_map(|item| match item {
                Selection::Project(id) => Some(id),
                _ => None,
            })
            .collect();
        if projects.is_empty() {
            return false;
        }
        let at = (store.selection.as_ref())
            .and_then(|s| projects.iter().position(|project| *project == s.project()));
        let delta = if forward { 1 } else { -1 };
        let Some(Step::Item(next)) = step_to(at, projects.len(), Stops::default(), delta) else {
            return false;
        };
        let Some(next) = projects.get(next).map(|project| (*project).clone()) else {
            return false;
        };
        self.select(self.left_in_project(&next, cx), window, cx);
        true
    }

    /// What going back to `project` selects: the row it was left on, else its own.
    pub(super) fn left_in_project(&self, project: &ProjectId, cx: &App) -> Selection {
        let store = self.store.read(cx);
        // Selecting a row holds its folds open, but a folded card stays folded, as tabs have no
        // card to fold. A project in the drawer has no rows, and the filter lists only what it
        // finds.
        let folded = store.prefs.navigator_place != crate::state::NavigatorPlace::Top
            && store.prefs.folded_projects.contains(project.as_ref());
        let rows =
            !self.filtering(cx) && !folded && store.project(project).is_some_and(|p| p.expanded);
        (store.left_on.get(project))
            .filter(|row| rows && store.still_there(row))
            .cloned()
            .unwrap_or_else(|| Selection::Project(project.clone()))
    }

    /// The rows that can be selected, in order: the cards', then the drawer's, but not
    /// those a fold closing takes away.
    pub(super) fn items(&self) -> Vec<&Selection> {
        self.staying(
            &self.built.rows,
            &self.built.moving,
            &self.built.drawer.rows,
        )
    }

    /// The items of `rows`, which move as `moving` says, then of `drawer`, without those on
    /// their way out.
    fn staying<'a>(
        &self,
        rows: &'a [Row],
        moving: &[Option<Moving>],
        drawer: &'a [Row],
    ) -> Vec<&'a Selection> {
        let closing = |m: &Option<Moving>| m.as_ref().is_some_and(|m| self.folds.closing(&m.fold));
        let cards = (rows.iter().zip(moving)).filter(|(_, m)| !closing(m));
        let drawer = drawer.iter().filter(|_| !self.folds.closing(&Fold::Drawer));
        items(cards.map(|(row, _)| row).chain(drawer))
    }

    fn expand(&mut self, _: &Expand, window: &mut Window, cx: &mut Context<Self>) {
        if self.off_row == Some(OffRow::DrawerBar) {
            return self.set_drawer_folded(false, cx);
        }
        let store = self.store.read(cx);
        match store.selection.clone() {
            // A folded card opens; an unwatched project is watched.
            Some(Selection::Project(id)) => self.store.update(cx, |store, cx| {
                let folded = store.project(&id).is_some_and(|p| p.expanded)
                    && store.prefs.folded_projects.contains(id.as_ref());
                match folded {
                    true => store.set_project_folded(&id, false, cx),
                    false => store.set_expanded(&id, true, cx),
                }
            }),
            Some(Selection::Branch(project, ref_name)) => {
                self.set_open(ClickOpened::Here((project, ref_name)), true, cx)
            }
            Some(heading @ (Selection::Checkout(..) | Selection::Stack(..))) => {
                self.folded.remove(&heading);
                cx.notify();
            }
            Some(Selection::Mesh(project, id, pick)) => self.expand_mesh(project, id, pick, cx),
            Some(Selection::Section(project, section)) => {
                if !self.heading_open(&project, &section, cx) {
                    self.toggle_heading(project, section, false, window, cx)
                }
            }
            // A remote's merge unfolds the commits it brought in.
            Some(Selection::Upstream(project, ref_name, id))
                if !self.open_merges.contains(&id)
                    && (store.workspace(&project))
                        .and_then(|ws| ws.upstream(&ref_name).iter().find(|c| c.id == id))
                        .is_some_and(|commit| commit.merge) =>
            {
                self.toggle_merge(&project, ref_name, id, window, cx)
            }
            _ => {}
        }
    }

    /// Step out the way a tree does: a commit to its branch, an open branch, stack or worktree
    /// closed, a branch up to its stack or worktree, and anything else up to its project.
    /// While the filter lists what it finds, nothing folds, so ⇧← only goes up.
    fn collapse(&mut self, _: &Collapse, window: &mut Window, cx: &mut Context<Self>) {
        if self.off_row == Some(OffRow::DrawerBar) {
            return self.set_drawer_folded(true, cx);
        }
        let filtering = self.filtering(cx);
        let store = self.store.read(cx);
        let selection = store.selection.clone();
        // A commit a remote's merge brought in goes up to the merge.
        if let Some(Selection::Upstream(project, ref_name, id)) = &selection
            && let Some((merge, _)) = store.merged_upstream(project, ref_name, *id)
        {
            let up = Selection::Upstream(project.clone(), ref_name.clone(), merge);
            return self.select(up, window, cx);
        }
        let closes = !filtering
            && matches!(&selection, Some(item @ Selection::Branch(project, ref_name))
                if store.branch_for(item).is_some_and(|branch| !branch.commits.is_empty())
                    && self.is_open(&ClickOpened::Here((project.clone(), ref_name.clone()))));
        let folds = !filtering
            && (self.built.rows.iter())
                .position(|row| matches!(row, Row::Item(item) if Some(item) == selection.as_ref()))
                .is_some_and(|ix| self.section_open(ix));
        match selection {
            Some(Selection::Project(_)) if filtering => {}
            // An open card folds; a folded one stops being watched.
            Some(Selection::Project(id)) => self.store.update(cx, |store, cx| {
                let open = store.project(&id).is_some_and(|p| p.expanded)
                    && !store.prefs.folded_projects.contains(id.as_ref());
                match open {
                    true => store.set_project_folded(&id, true, cx),
                    false => store.set_expanded(&id, false, cx),
                }
            }),
            Some(Selection::Branch(project, ref_name)) if closes => {
                self.set_open(ClickOpened::Here((project, ref_name)), false, cx)
            }
            Some(Selection::Commit(project, id)) => {
                let branch = store
                    .find_commit(&project, id)
                    .and_then(|(b, _)| b.ref_name.clone());
                if let Some(ref_name) = branch {
                    self.select(Selection::Branch(project, Some(ref_name)), window, cx);
                }
            }
            // An open merge folds; a remote's commits go up to their heading.
            Some(Selection::Upstream(project, ref_name, id)) if self.open_merges.contains(&id) => {
                self.toggle_merge(&project, ref_name, id, window, cx)
            }
            Some(Selection::Upstream(project, ref_name, _)) => {
                let heading = Selection::Section(project, Section::Upstream(ref_name));
                self.select(heading, window, cx)
            }
            Some(Selection::Checkout(project, checkout)) if folds => {
                self.toggle_section(&project, checkout, true, window, cx)
            }
            Some(caption @ Selection::Stack(..)) if folds => {
                self.toggle_stack(caption, true, window, cx)
            }
            Some(Selection::Checkout(project, Checkout::Linked(name)))
                if (store.workspace(&project))
                    .and_then(|ws| ws.find_worktree(name.as_ref()))
                    .is_some_and(|wt| wt.is_inactive(store.prefs.inactive_after)) =>
            {
                let more = Selection::Section(project, Section::Inactive);
                self.select(more, window, cx)
            }
            Some(Selection::Mesh(project, id, pick)) => {
                self.collapse_mesh(project, id, pick, window, cx)
            }
            Some(Selection::Section(project, section)) if !filtering => {
                match self.heading_open(&project, &section, cx) {
                    true => self.toggle_heading(project, section, true, window, cx),
                    false => {
                        let up = match section {
                            Section::Upstream(ref_name) => {
                                Selection::Branch(project, Some(ref_name))
                            }
                            Section::Inactive | Section::Mesh => Selection::Project(project),
                        };
                        self.select(up, window, cx)
                    }
                }
            }
            // Jump up, like closing a folder in a tree.
            Some(selection) => {
                let up = parent(store, &selection)
                    .unwrap_or_else(|| Selection::Project(selection.project().clone()));
                self.select(up, window, cx)
            }
            None => {}
        }
    }

    /// Return in GitButler's keymap: as in the TUI's, except that uncommitted changes start a
    /// commit, placed as c places it.
    pub(super) fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        let selection = self.store.read(cx).selection.clone();
        match selection {
            Some(Selection::Uncommitted(..)) if !self.off_rows() => {
                window.dispatch_action(Box::new(crate::app::StartCommit), cx)
            }
            _ => self.toggle_fold(&ToggleFold, window, cx),
        }
    }

    /// Go into the selected terminal, to type in, once the update running now is over.
    pub(super) fn enter_terminal(&self, window: &mut Window, cx: &mut Context<Self>) {
        (self.app).update_later(window, cx, |app, window, cx| {
            app.enter_terminal(window, cx);
            // A row with tiles gives the keys back to the one that had them last.
            app.focus_last_tile(window, cx);
        });
    }

    /// Return in the TUI's keymap: fold or unfold the rows the selected one heads, as its
    /// chevron does, go into a terminal, or start watching a project. Any other row that heads
    /// none stays as it is, as every row does under the filter.
    pub(super) fn toggle_fold(
        &mut self,
        _: &ToggleFold,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.off_row == Some(OffRow::DrawerBar) {
            return self.toggle_drawer(cx);
        }
        let Some(selection) = self.store.read(cx).selection.clone() else {
            return;
        };
        if let Selection::Terminal(..) = selection {
            return self.enter_terminal(window, cx);
        }
        if let Selection::Project(id) = &selection
            && (self.store.read(cx).project(id)).is_some_and(|entry| !entry.expanded)
        {
            return self.toggle_project(id, cx);
        }
        let fold = Fold::Under(selection.clone());
        let Some(open) = (self.build_rows(cx).folds.into_iter())
            .find_map(|(f, open, _)| (f == fold).then_some(open))
        else {
            return;
        };
        match selection {
            Selection::Project(id) => self.toggle_project(&id, cx),
            Selection::Section(project, section) => {
                self.toggle_heading(project, section, open, window, cx)
            }
            Selection::Branch(project, ref_name) => {
                self.set_open(ClickOpened::Here((project, ref_name)), !open, cx)
            }
            Selection::Checkout(project, checkout) => {
                self.toggle_section(&project, checkout, open, window, cx)
            }
            caption @ Selection::Stack(..) => self.toggle_stack(caption, open, window, cx),
            Selection::Mesh(project, id, pick) => {
                self.toggle_mesh(project, id, pick, open, window, cx)
            }
            Selection::Upstream(project, ref_name, id) => {
                self.toggle_merge(&project, ref_name, id, window, cx)
            }
            Selection::Commit(..)
            | Selection::Pending(..)
            | Selection::Uncommitted(..)
            | Selection::Terminal(..) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Step, Stops, step_to};

    const PLAIN: Stops = Stops {
        bar: None,
        on_bar: false,
        tab: false,
    };
    const UNDER_TAB: Stops = Stops { tab: true, ..PLAIN };

    /// The drawer's bar after `items` items, a stop.
    fn bar(items: usize) -> Stops {
        Stops {
            bar: Some(items),
            ..PLAIN
        }
    }

    /// The drawer's bar after `items` items, with the keys on it.
    fn on_bar(items: usize) -> Stops {
        Stops {
            on_bar: true,
            ..bar(items)
        }
    }

    #[test]
    fn a_step_down_from_a_tab_with_no_rows_stays_put() {
        assert_eq!(step_to(None, 0, UNDER_TAB, 1), None);
    }

    #[test]
    fn a_bar_that_stopped_being_a_stop_steps_from_the_selection() {
        assert_eq!(
            step_to(
                Some(1),
                3,
                Stops {
                    on_bar: true,
                    ..PLAIN
                },
                1
            ),
            Some(Step::Item(2)),
            "the filter took the bar away before the rows were drawn"
        );
    }

    #[test]
    fn a_step_down_from_the_folded_bar_stays_put() {
        assert_eq!(
            step_to(Some(2), 3, on_bar(3), 1),
            None,
            "a held key doesn't open the drawer"
        );
    }

    #[test]
    fn a_step_down_from_the_last_card_row_lands_on_the_bar() {
        assert_eq!(
            step_to(Some(2), 3, bar(3), 1),
            Some(Step::Bar),
            "the folded drawer's bar comes after the cards' rows"
        );
    }

    #[test]
    fn a_step_down_from_the_open_bar_goes_to_its_first_project() {
        assert_eq!(
            step_to(Some(2), 5, on_bar(3), 1),
            Some(Step::Item(3)),
            "the drawer's rows follow its bar"
        );
    }

    #[test]
    fn a_step_up_from_the_bar_goes_to_the_last_card_row() {
        assert_eq!(
            step_to(Some(4), 5, on_bar(3), -1),
            Some(Step::Item(2)),
            "from the bar, wherever the selection stayed"
        );
    }

    #[test]
    fn a_step_up_from_the_drawers_first_project_lands_on_the_bar() {
        assert_eq!(
            step_to(Some(3), 5, bar(3), -1),
            Some(Step::Bar),
            "the bar is above the drawer's rows"
        );
    }

    #[test]
    fn a_step_within_the_drawer_counts_the_bar_above_it() {
        assert_eq!(
            step_to(Some(3), 5, bar(3), 1),
            Some(Step::Item(4)),
            "the bar is a stop, not an item"
        );
    }

    #[test]
    fn down_from_a_tab_goes_into_its_first_row() {
        assert_eq!(step_to(None, 3, UNDER_TAB, 1), Some(Step::Item(0)));
    }

    #[test]
    fn down_from_the_last_row_under_a_tab_stays_put() {
        assert_eq!(
            step_to(Some(2), 3, UNDER_TAB, 1),
            None,
            "the card's rows end there, as the navigator's do"
        );
    }

    #[test]
    fn from_off_the_list_a_step_back_starts_at_the_bottom() {
        assert_eq!(
            step_to(None, 3, PLAIN, -1),
            Some(Step::Item(2)),
            "and forward, at the top"
        );
        assert_eq!(step_to(None, 3, PLAIN, 1), Some(Step::Item(0)));
    }

    #[test]
    fn selecting_the_first_leaves_the_bar_for_the_top() {
        assert_eq!(
            step_to(Some(2), 3, on_bar(3), isize::MIN / 2),
            Some(Step::Item(0)),
            "from the bar too"
        );
    }

    #[test]
    fn selecting_the_last_lands_on_the_folded_bar() {
        assert_eq!(
            step_to(Some(0), 3, bar(3), isize::MAX / 2),
            Some(Step::Bar),
            "it's the last stop while the drawer's folded"
        );
    }

    #[test]
    fn selecting_the_last_passes_the_open_bar() {
        assert_eq!(
            step_to(Some(0), 5, bar(3), isize::MAX / 2),
            Some(Step::Item(4)),
            "the drawer's last project is the last stop"
        );
    }

    #[test]
    fn up_from_a_tab_stays_on_it() {
        assert_eq!(
            step_to(None, 3, UNDER_TAB, -1),
            None,
            "the tab is the top, so the keys don't go round to the card's last row"
        );
    }

    #[test]
    fn up_from_the_drawers_bar_under_a_tab_stays_in_the_card() {
        assert_eq!(
            step_to(
                None,
                3,
                Stops {
                    on_bar: true,
                    ..UNDER_TAB
                },
                -1
            ),
            Some(Step::Item(2)),
            "on the drawer's bar the keys are in the card, though nothing's selected there"
        );
    }

    #[test]
    fn selecting_the_first_under_a_tab_from_its_first_row_stays_put() {
        assert_eq!(
            step_to(Some(0), 3, UNDER_TAB, isize::MIN / 2),
            None,
            "Home goes to the first row, not past it onto the tab"
        );
    }

    #[test]
    fn up_off_the_first_row_under_a_tab_goes_to_the_tab() {
        assert_eq!(step_to(Some(0), 3, UNDER_TAB, -1), Some(Step::Tab));
    }

    #[test]
    fn without_a_bar_a_step_stops_at_the_last_item() {
        assert_eq!(
            step_to(Some(2), 3, PLAIN, 1),
            None,
            "a drawer that can't fold is no stop"
        );
    }
}
