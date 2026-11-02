//! The terminals open on each worktree, listed under its heading in the order they opened.
//! Nothing saves them: they end with the app, but for Build and restart, which hands them to
//! the new build.

use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Instant;

use gpui::{Context, Entity, SharedString, Subscription};

use super::{Axis, ProjectId, RowKey, Selection, Store, StoreEvent, Tile, Tiling};
use crate::backend::{self, Checkout};
use crate::terminal::{Terminal, TerminalEvent, TerminalId, shell_quoted};
#[cfg(unix)]
use but_tty::Detached;
use but_tty::{Ghostty, Shell, Status};

#[derive(Default)]
pub struct Terminals {
    opened: u64,
    open: Vec<Open>,
}

/// Where a terminal handed to the next build was open.
#[cfg(unix)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Handed {
    project: String,
    checkout: Checkout,
    id: u64,
    /// Missing from what a build from before names handed over.
    #[serde(default)]
    named: Option<String>,
}

struct Open {
    id: TerminalId,
    project: ProjectId,
    checkout: Checkout,
    terminal: Entity<Terminal>,
    /// What the shell titled it, kept while the user's name shows in its place.
    title: SharedString,
    /// The name the user gave it, if they did.
    named: Option<SharedString>,
    status: Status,
    /// It was opened in a tile, which it lives in alone: the navigator doesn't list it, and it
    /// closes with the tile.
    tiled: bool,
    _events: Subscription,
}

impl Terminals {
    pub fn get(&self, id: TerminalId) -> Option<&Entity<Terminal>> {
        (self.open.iter())
            .find(|open| open.id == id)
            .map(|open| &open.terminal)
    }

    /// The name the user gave it, else what its shell titled it. Kept here so rows can show a
    /// terminal without reading it, which would have the window draw on all it prints, shown
    /// or not.
    pub fn title(&self, id: TerminalId) -> Option<&SharedString> {
        (self.open.iter())
            .find(|open| open.id == id)
            .map(|open| open.named.as_ref().unwrap_or(&open.title))
    }

    /// How many are open.
    pub fn count(&self) -> usize {
        self.open.len()
    }

    /// Kept here as `title` is.
    pub fn status(&self, id: TerminalId) -> Option<&Status> {
        (self.open.iter())
            .find(|open| open.id == id)
            .map(|open| &open.status)
    }

    /// Those open on `project`'s `checkout`, in the order they opened, but for those opened in
    /// a tile, which live there alone.
    pub fn on<'a>(
        &'a self,
        project: &'a ProjectId,
        checkout: &'a Checkout,
    ) -> impl Iterator<Item = (TerminalId, &'a Entity<Terminal>)> + 'a {
        (self.open.iter())
            .filter(move |open| open.project == *project && open.checkout == *checkout)
            .filter(|open| !open.tiled)
            .map(|open| (open.id, &open.terminal))
    }

    /// Whether terminal `id` was opened in a tile, which it lives in alone.
    pub fn tiled(&self, id: TerminalId) -> bool {
        self.open.iter().any(|open| open.id == id && open.tiled)
    }

    /// The checkout `selection` is a terminal on, if it is one.
    pub fn checkout_of(&self, selection: &Selection) -> Option<&Checkout> {
        let Selection::Terminal(_, id) = selection else {
            return None;
        };
        (self.open.iter())
            .find(|open| open.id == *id)
            .map(|open| &open.checkout)
    }
}

impl Store {
    /// What a terminal pastes of what's being dragged: a file, hunk or lines from the diff as
    /// their patch, as it was when the drag began ([`Store::dragged_patch`]), else the row
    /// ([`Cut::pasted`](super::Cut::pasted)) as the store followed it, as an agent may have
    /// rewritten it, quoted for the shell.
    pub fn pasted(&self) -> Option<Cow<'_, str>> {
        if let Some(patch) = &self.dragged_patch {
            return Some(Cow::Borrowed(patch));
        }
        let cut = self.dragging.as_ref()?;
        let project = cut.project();
        let words = cut.pasted(&self.path(project)?, self.workspace(project)?)?;
        Some(Cow::Owned(shell_quoted(words)))
    }

    /// Open a terminal on `project`'s `checkout` and select it, once its shell has started.
    pub fn open_terminal(
        &mut self,
        project: ProjectId,
        checkout: Checkout,
        cx: &mut Context<Self>,
    ) {
        self.open_terminal_then(project, checkout, None, cx);
    }

    /// Open a terminal on `project`'s `checkout`. Once its shell has started, select it, or pin
    /// it as `pin` says.
    pub(super) fn open_terminal_then(
        &mut self,
        project: ProjectId,
        checkout: Checkout,
        pin: Option<(RowKey, Tile, Axis)>,
        cx: &mut Context<Self>,
    ) {
        let Some(cwd) = self.checkout_path(&project, &checkout) else {
            return;
        };
        let run = self.prefs.terminal_run().map(str::to_owned);
        let asked = Instant::now();
        cx.spawn(async move |this, cx| {
            let started = backend::run(move || {
                let shell = Shell::spawn(&cwd, Terminal::OPENING, run.as_deref())?;
                Ok((shell, Ghostty::load()))
            })
            .await;
            this.update(cx, |store, cx| {
                let store_handle = cx.weak_entity();
                let opened = started
                    .and_then(|(shell, ghostty)| Terminal::new(shell, ghostty, store_handle, cx));
                match opened {
                    Ok(terminal) => {
                        backend::metrics::operation(
                            "New terminal",
                            asked.elapsed(),
                            None,
                            store.workspace(&project),
                        );
                        store.add_terminal(project, checkout, terminal, pin, cx);
                    }
                    Err(err) => store.failed("Couldn't open a terminal", &err, cx),
                }
            })
            .ok();
        })
        .detach();
    }

    fn add_terminal(
        &mut self,
        project: ProjectId,
        checkout: Checkout,
        terminal: Entity<Terminal>,
        pin: Option<(RowKey, Tile, Axis)>,
        cx: &mut Context<Self>,
    ) {
        let id = TerminalId(self.terminals.opened + 1);
        self.insert_terminal(id, project.clone(), checkout, terminal, None, cx);
        // With the tile it was to go beside gone, it opens as a terminal of its own does.
        if let Some(pin) = pin {
            self.set_tiled(id, true);
            let row = RowKey::Row(Selection::Terminal(project.clone(), id));
            if self.pin_into(pin, row, cx) {
                return;
            }
            self.set_tiled(id, false);
        }
        self.select(Some(Selection::Terminal(project, id)), cx);
        // A selection that stays put leaves the new one out of sight.
        self.show_terminals(cx);
        cx.emit(StoreEvent::TerminalOpened);
    }

    fn insert_terminal(
        &mut self,
        id: TerminalId,
        project: ProjectId,
        checkout: Checkout,
        terminal: Entity<Terminal>,
        named: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.terminals.opened = self.terminals.opened.max(id.0);
        let events = cx.subscribe(&terminal, move |store, _, event, cx| match event {
            TerminalEvent::Retitled(title) => store.retitle_terminal(id, title.clone(), cx),
            TerminalEvent::Restatused(status) => store.restatus_terminal(id, status.clone(), cx),
            TerminalEvent::Exited => store.close_terminal(id, cx),
        });
        let (title, status) = (terminal.read(cx).title(), terminal.read(cx).status());
        self.terminals.open.push(Open {
            id,
            project,
            checkout,
            terminal,
            title,
            named,
            status,
            tiled: false,
            _events: events,
        });
    }

    /// List `project`'s terminals as `lists` has them: each under its checkout, in that order,
    /// in the places they take among the others. One moved to another checkout is listed there,
    /// while its shell stays where it started. One opened since that it doesn't have stays where
    /// it is.
    pub fn order_terminals(
        &mut self,
        project: &ProjectId,
        lists: &[(Checkout, Vec<TerminalId>)],
        cx: &mut Context<Self>,
    ) {
        for open in (self.terminals.open.iter_mut()).filter(|open| open.project == *project) {
            if let Some((on, _)) = lists.iter().find(|(_, list)| list.contains(&open.id)) {
                open.checkout = on.clone();
            }
        }
        let order: Vec<TerminalId> = lists.iter().flat_map(|(_, list)| list).copied().collect();
        let at = |open: &Open| {
            let ours = open.project == *project;
            ours.then(|| order.iter().position(|id| *id == open.id))
                .flatten()
        };
        let open = std::mem::take(&mut self.terminals.open);
        let ordered: Vec<bool> = open.iter().map(|open| at(open).is_some()).collect();
        let (mut moved, rest): (Vec<Open>, Vec<Open>) =
            open.into_iter().partition(|open| at(open).is_some());
        moved.sort_by_key(at);
        let (mut moved, mut rest) = (moved.into_iter(), rest.into_iter());
        self.terminals.open = (ordered.into_iter())
            .filter_map(|ordered| match ordered {
                true => moved.next(),
                false => rest.next(),
            })
            .collect();
        cx.notify();
    }

    /// Show `name` for terminal `id` in place of what its shell titles it, or with an empty
    /// one, the shell's title again.
    pub fn name_terminal(&mut self, id: TerminalId, name: &str, cx: &mut Context<Self>) {
        let name = name.trim();
        let named = (!name.is_empty()).then(|| SharedString::from(name.to_owned()));
        if let Some(open) = self.terminals.open.iter_mut().find(|open| open.id == id)
            && open.named != named
        {
            open.named = named;
            cx.notify();
        }
    }

    fn retitle_terminal(&mut self, id: TerminalId, title: SharedString, cx: &mut Context<Self>) {
        if let Some(open) = self.terminals.open.iter_mut().find(|open| open.id == id)
            && open.title != title
        {
            open.title = title;
            // Under the user's name it doesn't show, and agents retitle often.
            if open.named.is_none() {
                cx.notify();
            }
        }
    }

    fn restatus_terminal(&mut self, id: TerminalId, status: Status, cx: &mut Context<Self>) {
        if let Some(open) = self.terminals.open.iter_mut().find(|open| open.id == id) {
            open.status = status;
        }
        cx.notify();
    }

    /// Hands each terminal's shell over, for the build started next to adopt. One that can't be
    /// is hung up, as its master closes with this process.
    #[cfg(unix)]
    pub fn hand_off_terminals(&mut self, cx: &mut Context<Self>) -> Vec<(Handed, Detached)> {
        (self.terminals.open.iter())
            .filter_map(|open| {
                let detached = open.terminal.update(cx, |terminal, _| terminal.detach());
                let detached = (detached)
                    .inspect_err(|err| tracing::error!("couldn't hand a terminal over: {err:#}"))
                    .ok()?;
                let handed = Handed {
                    project: open.project.to_string(),
                    checkout: open.checkout.clone(),
                    id: open.id.0,
                    named: open.named.as_ref().map(ToString::to_string),
                };
                Some((handed, detached))
            })
            .collect()
    }

    /// Opens the terminals the build this one took over from handed over, where they were.
    #[cfg(unix)]
    pub fn adopt_terminals(&mut self, handed: Vec<(Handed, Detached)>, cx: &mut Context<Self>) {
        for (handed, detached) in handed {
            let Handed {
                project,
                checkout,
                id,
                named,
            } = handed;
            let store = cx.weak_entity();
            match Terminal::adopt(detached, store, cx) {
                Ok(terminal) => {
                    let (id, named) = (TerminalId(id), named.map(Into::into));
                    self.insert_terminal(id, project.into(), checkout, terminal, named, cx)
                }
                Err(err) => tracing::error!("couldn't adopt a terminal: {err:#}"),
            }
        }
        self.show_terminals(cx);
    }

    fn set_tiled(&mut self, id: TerminalId, tiled: bool) {
        if let Some(open) = self.terminals.open.iter_mut().find(|open| open.id == id) {
            open.tiled = tiled;
        }
    }

    /// Tells each terminal whether it's on screen: the one selected and those pinned beside it,
    /// while the window shows. Those out of sight read and answer their shells but don't draw.
    pub(super) fn show_terminals(&mut self, cx: &mut Context<Self>) {
        let selected = match &self.selection {
            Some(Selection::Terminal(_, id)) => Some(*id),
            _ => None,
        };
        let pinned = self.pinned().into_iter().filter_map(|row| match row {
            RowKey::Row(Selection::Terminal(_, id)) => Some(id),
            _ => None,
        });
        let on_screen: Vec<TerminalId> = match self.hidden() {
            true => Vec::new(),
            false => selected.into_iter().chain(pinned).collect(),
        };
        for open in &self.terminals.open {
            let shown = on_screen.contains(&open.id);
            open.terminal
                .update(cx, |terminal, _| terminal.set_shown(shown));
        }
    }

    /// Close terminal `id`, hanging up its shell.
    pub fn close_terminal(&mut self, id: TerminalId, cx: &mut Context<Self>) {
        self.close_terminals(&[id], cx);
    }

    /// Close terminals `ids`, hanging up their shells. While one's selected, a terminal pinned
    /// beside it takes its place: see [`Self::hand_home_on`]. With none, another terminal on its
    /// worktree is selected, as the TUI goes on after removing something: the next one, else
    /// the one above. With none, the worktree's first branch, the row below them, else the
    /// worktree.
    fn close_terminals(&mut self, ids: &[TerminalId], cx: &mut Context<Self>) {
        let selected = match &self.selection {
            Some(Selection::Terminal(_, selected)) if ids.contains(selected) => Some(*selected),
            _ => None,
        };
        let open = selected.and_then(|id| self.terminals.open.iter().find(|open| open.id == id));
        let place = open.map(|open| {
            let (project, checkout) = (open.project.clone(), open.checkout.clone());
            let listed: Vec<TerminalId> = (self.terminals.on(&project, &checkout))
                .map(|(id, _)| id)
                .collect();
            (project, checkout, listed)
        });
        let closed: HashMap<TerminalId, Entity<Terminal>> = (self.terminals.open.iter())
            .filter(|open| ids.contains(&open.id))
            .map(|open| (open.id, open.terminal.clone()))
            .collect();
        if closed.is_empty() {
            return;
        }
        (self.terminals.open).retain(|open| !ids.contains(&open.id));
        let heir = selected.and_then(|_| self.hand_home_on(ids, cx));
        self.unpin_terminals(&closed, cx);
        if let Some((heir, before, tile)) = heir {
            self.select(Some(heir), cx);
            // After the selection, which stops a slide, and in the tiling of the row it's on.
            let terminal = selected.and_then(|id| closed.get(&id)).cloned();
            cx.emit(StoreEvent::TileClosed {
                before,
                tile,
                terminal,
            });
        } else if let (Some(selected), Some((project, checkout, listed))) = (selected, place) {
            let at = listed
                .iter()
                .position(|id| *id == selected)
                .unwrap_or_default();
            let staying = |id: &&TerminalId| !ids.contains(id);
            let terminal = |id: &TerminalId| Selection::Terminal(project.clone(), *id);
            let below = listed.iter().skip(at + 1).find(staying).map(terminal);
            let above = listed.iter().take(at).rev().find(staying).map(terminal);
            let branch = (self.workspace(&project))
                .and_then(|ws| ws.branches_in(&checkout).next())
                .map(|branch| Selection::Branch(project.clone(), branch.ref_name.clone()));
            let instead = (below.or(above).or(branch))
                .unwrap_or_else(|| Selection::Checkout(project.clone(), checkout));
            self.select(Some(instead), cx);
        }
        cx.notify();
    }

    /// Give the selected terminal's place in its row's tiling, as it closes with `closing`, to
    /// the terminal pinned there that had the keys last, else the first: the tiling, and what in
    /// it had the keys, go to that terminal's row, which the navigator lists it as from now on,
    /// as terminals are alike. The terminal to select, if there was one to take the place, with
    /// the tiling as it was, for that row, and the tile closing in it.
    fn hand_home_on(
        &mut self,
        closing: &[TerminalId],
        cx: &mut Context<Self>,
    ) -> Option<(Selection, Tiling, RowKey)> {
        let heirs = self.home_heirs(closing);
        let row = self.row_key(self.selection.as_ref()?);
        let recent = self.tiles_with_keys.get(&row);
        let heir = (recent.into_iter().flatten().rev())
            .find(|shown| heirs.contains(shown))
            .or(heirs.first())?
            .clone();
        let mut tiling = self.tilings.remove(&row)?;
        let recent = self.tiles_with_keys.remove(&row).unwrap_or_default();
        let before = tiling.before_promoting(&row, &heir);
        tiling.promote(&heir);
        if !tiling.is_home_only() {
            self.tilings.insert(heir.clone(), tiling);
        }
        self.tiles_with_keys.insert(heir.clone(), recent);
        cx.emit(StoreEvent::TilingChanged);
        let RowKey::Row(selection @ Selection::Terminal(_, id)) = heir else {
            return None;
        };
        self.set_tiled(id, false);
        Some((selection, before, row))
    }

    /// Close the terminals whose project is gone, or whose linked worktree is gone or archived.
    pub(super) fn close_terminals_gone(&mut self, cx: &mut Context<Self>) {
        let gone: Vec<TerminalId> = (self.terminals.open.iter())
            .filter(|open| {
                let Some(entry) = self.project(&open.project) else {
                    return true;
                };
                match (&open.checkout, &entry.workspace) {
                    (Checkout::Linked(name), Some(workspace)) => {
                        workspace.find_worktree(name.as_ref()).is_none()
                            || self.hides_worktree(&open.project, name.as_ref())
                    }
                    _ => false,
                }
            })
            .map(|open| open.id)
            .collect();
        // At once, so a selection on one doesn't step through the others going with it.
        self.close_terminals(&gone, cx);
    }

    /// Show the app's terminals' text at `size`, or with `None` at Ghostty's.
    pub fn set_terminal_text(&mut self, size: Option<u8>, cx: &mut Context<Self>) {
        self.set_prefs(|prefs| prefs.terminal_text = size, cx);
        for open in &self.terminals.open {
            open.terminal.update(cx, |_, cx| cx.notify());
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::Handed;
    use crate::backend::Checkout;

    fn handed(named: Option<&str>) -> Handed {
        Handed {
            project: "project".into(),
            checkout: Checkout::Main,
            id: 1,
            named: named.map(Into::into),
        }
    }

    #[test]
    fn the_next_build_keeps_a_terminals_name() {
        let json = serde_json::to_string(&handed(Some("My agent"))).unwrap();
        let adopted: Handed = serde_json::from_str(&json).unwrap();
        assert_eq!(
            adopted.named.as_deref(),
            Some("My agent"),
            "the name is handed over with the shell"
        );
    }

    #[test]
    fn a_build_from_before_names_hands_over_terminals_without_one() {
        let mut json = serde_json::to_value(handed(None)).unwrap();
        json.as_object_mut().unwrap().remove("named");
        let adopted: Handed = serde_json::from_value(json).unwrap();
        assert_eq!(adopted.named, None, "they show their shell's title");
    }
}
