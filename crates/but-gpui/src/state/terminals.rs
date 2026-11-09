//! The terminals open on each worktree, listed under its heading in the order they opened.
//! Nothing saves them: they end with the app, but for Build and restart, which hands them to
//! the new build, with the tilings they may be tiles of.

use std::borrow::Cow;
use std::collections::HashMap;
#[cfg(unix)]
use std::collections::HashSet;
use std::time::Instant;

use gpui::{Context, Entity, SharedString, Subscription};

use super::{Axis, ProjectId, RowKey, Selection, Store, StoreEvent, Tile, Tiling};
use crate::backend::{self, Checkout};
use crate::terminal::{Terminal, TerminalEvent, TerminalId, shell_quoted};
#[cfg(unix)]
use but_tty::Detached;
use but_tty::{Ghostty, Shell, Status};

/// A project's checkout, which terminals are listed under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectCheckout {
    pub project: ProjectId,
    pub checkout: Checkout,
}

/// A terminal as [`Terminals::all`] lists it.
pub struct ListedTerminal<'a> {
    pub id: TerminalId,
    pub project: &'a ProjectId,
    pub checkout: &'a Checkout,
    pub title: &'a SharedString,
    pub status: &'a Status,
    /// When `status` last changed, or it opened.
    pub since: Instant,
    /// It lives in a tile alone, which the navigator doesn't list.
    pub tiled: bool,
}

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
    /// Missing from what a build from before tilings carried over hands over.
    #[serde(default)]
    tiled: bool,
}

/// How the rows laid their tiles out, and the row selected, for the next build to show the same.
#[cfg(unix)]
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct HandedTilings {
    /// A list, as JSON's keys can only be strings.
    tilings: Vec<(RowKey, Tiling)>,
    selection: Option<Selection>,
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
    /// When `status` last changed, or it opened.
    since: Instant,
    /// It was opened in a tile or dragged into one, which it lives in alone: the navigator
    /// doesn't list it, and it closes with the tile.
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

    /// Those open on `project`'s `checkout`, in the order they opened, but for those living in
    /// a tile alone.
    /// What every terminal open on `project` is at, tiled or not, or only those on `checkout`.
    pub fn statuses_on<'a>(
        &'a self,
        project: &'a ProjectId,
        checkout: Option<&'a Checkout>,
    ) -> impl Iterator<Item = &'a Status> + 'a {
        (self.open.iter())
            .filter(move |open| open.project == *project)
            .filter(move |open| checkout.is_none_or(|checkout| open.checkout == *checkout))
            .map(|open| &open.status)
    }

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

    /// Those open on `project`, on any of its checkouts, with what each is called, and whether
    /// it lives in a tile of its own.
    pub fn of_project<'a>(
        &'a self,
        project: &'a ProjectId,
    ) -> impl Iterator<Item = (TerminalId, &'a Entity<Terminal>, &'a SharedString, bool)> + 'a {
        (self.open.iter())
            .filter(move |open| open.project == *project)
            .map(|open| {
                (
                    open.id,
                    &open.terminal,
                    open.named.as_ref().unwrap_or(&open.title),
                    open.tiled,
                )
            })
    }

    /// Every one open, on every project, in the order they opened, with where it's open, what
    /// it's called and what it's at, tiled or not.
    pub fn all(&self) -> impl Iterator<Item = ListedTerminal<'_>> {
        (self.open.iter()).map(|open| ListedTerminal {
            id: open.id,
            project: &open.project,
            checkout: &open.checkout,
            title: open.named.as_ref().unwrap_or(&open.title),
            status: &open.status,
            since: open.since,
            tiled: open.tiled,
        })
    }

    /// Whether terminal `id` lives in a tile alone.
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
    /// Where terminal `id` shows: its own row, in the home tile, or the row whose tiling it
    /// lives in a tile of, in that tile.
    pub fn terminal_tile(&self, id: TerminalId) -> Option<(Selection, Tile)> {
        let open = self.terminals.open.iter().find(|open| open.id == id)?;
        let own = Selection::Terminal(open.project.clone(), id);
        if !open.tiled {
            return Some((own, Tile::Home));
        }
        let tile = Tile::Pinned(RowKey::Row(own));
        let (row, _) = (self.tilings.iter()).find(|(_, tiling)| tiling.has(&tile))?;
        Some((self.selection_of(row)?, tile))
    }

    /// The terminal in the selected row's tile that had the keys last, or the row's own, if
    /// it's a terminal's: the one that shows as selected.
    pub fn terminal_shown(&self) -> Option<TerminalId> {
        match self.last_tile_with_keys() {
            Some(Tile::Pinned(RowKey::Row(Selection::Terminal(_, id)))) => return Some(id),
            Some(Tile::Pinned(_)) => return None,
            Some(Tile::Home) | None => {}
        }
        match self.selection.as_ref()? {
            Selection::Terminal(_, id) => Some(*id),
            _ => None,
        }
    }

    /// What a terminal pastes of what's being dragged: a file, hunk or lines from the diff as
    /// their patch, as it was when the drag began ([`Store::dragged_patch`]), else the row
    /// ([`Moving::pasted`](crate::transfer::Moving::pasted)) as the store followed it, as an
    /// agent may have rewritten it, quoted for the shell.
    pub fn pasted(&self) -> Option<Cow<'_, str>> {
        if let Some(patch) = &self.dragged_patch {
            return Some(Cow::Borrowed(patch));
        }
        let dragged = self.dragging.as_ref()?;
        let project = &dragged.project;
        let words = (dragged.what).pasted(&self.path(project)?, self.workspace(project)?)?;
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
        let via = backend::metrics::via();
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
                            via,
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
            since: Instant::now(),
            tiled: false,
            _events: events,
        });
    }

    /// List the terminals `lists` has as it has them: each under its checkout, in that order,
    /// in the places they take among the others. One moved to another checkout, of its project
    /// or another's, is listed there, while its shell stays where it started. One opened since
    /// that it doesn't have stays where it is.
    pub fn order_terminals(
        &mut self,
        lists: &[(ProjectCheckout, Vec<TerminalId>)],
        cx: &mut Context<Self>,
    ) {
        let mut rehomed = Vec::new();
        for open in self.terminals.open.iter_mut() {
            if let Some((on, _)) = lists.iter().find(|(_, list)| list.contains(&open.id)) {
                if open.project != on.project {
                    rehomed.push((open.id, open.project.clone(), on.project.clone()));
                }
                open.project = on.project.clone();
                open.checkout = on.checkout.clone();
            }
        }
        let order: Vec<TerminalId> = lists.iter().flat_map(|(_, list)| list).copied().collect();
        let at = |open: &Open| order.iter().position(|id| *id == open.id);
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
        for (id, from, to) in rehomed {
            self.rehome_terminal(id, from, to, cx);
        }
        cx.notify();
    }

    /// Have terminal `id`'s row, moved from project `from` to `to`, keep its selection and
    /// tiles.
    fn rehome_terminal(
        &mut self,
        id: TerminalId,
        from: ProjectId,
        to: ProjectId,
        cx: &mut Context<Self>,
    ) {
        let (from, to) = (Selection::Terminal(from, id), Selection::Terminal(to, id));
        self.rekey_row(&RowKey::Row(from.clone()), &RowKey::Row(to.clone()));
        cx.emit(StoreEvent::TilingChanged);
        if self.selection.as_ref() == Some(&from) {
            self.select(Some(to), cx);
        }
        // The project it left has no row of it to go back to.
        if self.left_on.get(from.project()) == Some(&from) {
            self.left_on.remove(from.project());
        }
        self.left_in.retain(|_, left| *left != from);
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
        if let Some(open) = self.terminals.open.iter_mut().find(|open| open.id == id)
            && open.status != status
        {
            open.status = status;
            open.since = Instant::now();
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
                    tiled: open.tiled,
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
                tiled,
            } = handed;
            let store = cx.weak_entity();
            match Terminal::adopt(detached, store, cx) {
                Ok(terminal) => {
                    let (id, named) = (TerminalId(id), named.map(Into::into));
                    self.insert_terminal(id, project.into(), checkout, terminal, named, cx);
                    self.set_tiled(id, tiled);
                }
                Err(err) => tracing::error!("couldn't adopt a terminal: {err:#}"),
            }
        }
        self.show_terminals(cx);
    }

    /// The tilings and the selection, for the build started next to adopt.
    #[cfg(unix)]
    pub fn hand_off_tilings(&self) -> HandedTilings {
        HandedTilings {
            tilings: self.tilings.clone().into_iter().collect(),
            selection: self.selection.clone(),
        }
    }

    /// Lays the rows out as the build this one took over from did, once its terminals are
    /// adopted, and selects its row with the first listing of the projects. With nothing
    /// handed, as when they couldn't be read, it still lists the terminals left without a tile.
    #[cfg(unix)]
    pub fn adopt_tilings(&mut self, handed: Option<HandedTilings>) {
        let HandedTilings { tilings, selection } = handed.unwrap_or_default();
        self.tilings = surviving(tilings, |id| self.terminals.get(id).is_some());
        self.restored.handed = selection;
        let pinned: HashSet<TerminalId> = (self.tilings.values())
            .flat_map(Tiling::pinned_terminals)
            .collect();
        for open in &mut self.terminals.open {
            open.tiled &= pinned.contains(&open.id);
        }
    }

    fn set_tiled(&mut self, id: TerminalId, tiled: bool) {
        if let Some(open) = self.terminals.open.iter_mut().find(|open| open.id == id) {
            open.tiled = tiled;
        }
    }

    /// Have the terminal in tile `pinned` of the selected row's tiling, dragged there from the
    /// navigator, live there alone: the navigator stops listing it, and it leaves any other
    /// tiling it's in. Not one with tiles of its own, which only its row shows.
    pub(super) fn tile_terminal(&mut self, pinned: &RowKey, cx: &mut Context<Self>) {
        let RowKey::Row(Selection::Terminal(_, id)) = pinned else {
            return;
        };
        let Some(selection) = &self.selection else {
            return;
        };
        if self.tilings.contains_key(pinned) {
            return;
        }
        let row = self.row_key(selection);
        let before = self.tilings.clone();
        leave_other_tilings(&mut self.tilings, &row, pinned);
        self.set_tiled(*id, true);
        if self.tilings != before {
            cx.emit(StoreEvent::TilingChanged);
        }
        cx.notify();
    }

    /// Take tile `pinned` away from the selected row's tiling, as [`Self::close_tile`] does, its
    /// terminal, dragged to the navigator, staying open and listed where its move shows it.
    pub fn list_tiled_terminal(&mut self, pinned: &RowKey, cx: &mut Context<Self>) {
        if let RowKey::Row(Selection::Terminal(_, id)) = pinned {
            self.set_tiled(*id, false);
        }
        if let Some(moving) = self.moving.take()
            && let Some(order) = moving.terminal_order()
        {
            self.order_terminals(&order, cx);
        }
        self.close_tile(pinned, cx);
    }

    /// Tells each terminal whether it's on screen: the one selected and those pinned beside it,
    /// while the window shows. Those out of sight read and answer their shells but don't draw.
    pub(super) fn show_terminals(&mut self, cx: &mut Context<Self>) {
        let selected = match &self.selection {
            Some(Selection::Terminal(_, id)) => Some(*id),
            _ => None,
        };
        let pinned = self.tiling().into_iter().flat_map(Tiling::pinned_terminals);
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
            cx.emit(StoreEvent::TilingMoved {
                from: row.clone(),
                to: heir.clone(),
            });
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

/// Take `pinned` out of `tilings` but for `row`'s: a tiling goes with the last tile beside its
/// home one.
fn leave_other_tilings(tilings: &mut HashMap<RowKey, Tiling>, row: &RowKey, pinned: &RowKey) {
    tilings.retain(|other, tiling| {
        if other != row {
            tiling.retain(&|tile| tile != pinned);
        }
        !tiling.is_home_only()
    });
}

/// `tilings` as they stand with only the terminals `open` says are: a tiling goes with its
/// row's terminal, and with the last tile beside its home one.
#[cfg(unix)]
fn surviving(
    tilings: Vec<(RowKey, Tiling)>,
    open: impl Fn(TerminalId) -> bool,
) -> HashMap<RowKey, Tiling> {
    (tilings.into_iter())
        .filter(|(row, _)| row_open(row, &open))
        .filter_map(|(row, mut tiling)| {
            tiling.retain(&|pinned| row_open(pinned, &open));
            (!tiling.is_home_only()).then_some((row, tiling))
        })
        .collect()
}

/// Whether `row` is there, which only a terminal may not be.
#[cfg(unix)]
fn row_open(row: &RowKey, open: &impl Fn(TerminalId) -> bool) -> bool {
    match row {
        RowKey::Row(Selection::Terminal(_, id)) => open(*id),
        RowKey::Row(_) | RowKey::Commit(..) | RowKey::Tab(..) | RowKey::Changes(_) => true,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::collections::HashMap;

    use super::{Handed, HandedTilings, leave_other_tilings, surviving};
    use crate::backend::Checkout;
    use crate::state::{Axis, MeshPick, ProjectTab, RowKey, Section, Selection, Tile, Tiling};
    use crate::terminal::TerminalId;

    fn handed(named: Option<&str>) -> Handed {
        Handed {
            project: "project".into(),
            checkout: Checkout::Main,
            id: 1,
            named: named.map(Into::into),
            tiled: false,
        }
    }

    fn terminal(id: u64) -> RowKey {
        RowKey::Row(Selection::Terminal("project".into(), TerminalId(id)))
    }

    fn commit() -> RowKey {
        RowKey::Commit("project".into(), "change".to_owned())
    }

    fn tiled(pinned: impl IntoIterator<Item = RowKey>) -> Tiling {
        let mut tiling = Tiling::default();
        for row in pinned {
            tiling.pin(&Tile::Home, Axis::Across, row);
        }
        tiling
    }

    #[test]
    fn a_terminal_dragged_into_a_tile_leaves_the_other_tilings_it_is_in() {
        let other = RowKey::Commit("project".into(), "other".to_owned());
        let mut tilings = HashMap::from([
            (commit(), tiled([terminal(1)])),
            (other.clone(), tiled([terminal(1), terminal(2)])),
        ]);
        leave_other_tilings(&mut tilings, &commit(), &terminal(1));
        assert_eq!(
            tilings.get(&other),
            Some(&tiled([terminal(2)])),
            "it lives only in the tile it was dragged to"
        );
    }

    #[test]
    fn a_terminal_dragged_into_a_tile_stays_in_the_tiling_it_landed_in() {
        let mut tilings = HashMap::from([(commit(), tiled([terminal(1)]))]);
        leave_other_tilings(&mut tilings, &commit(), &terminal(1));
        assert_eq!(
            tilings.get(&commit()),
            Some(&tiled([terminal(1)])),
            "the tile it landed as is its home now"
        );
    }

    #[test]
    fn a_tiling_goes_with_the_last_tile_a_dragged_terminal_leaves() {
        let other = RowKey::Commit("project".into(), "other".to_owned());
        let mut tilings = HashMap::from([
            (commit(), tiled([terminal(1)])),
            (other.clone(), tiled([terminal(1)])),
        ]);
        leave_other_tilings(&mut tilings, &commit(), &terminal(1));
        assert!(
            !tilings.contains_key(&other),
            "only the home tile was left beside it"
        );
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

    #[test]
    fn the_next_build_keeps_a_terminal_in_its_tile() {
        let tiled = Handed {
            tiled: true,
            ..handed(None)
        };
        let json = serde_json::to_string(&tiled).unwrap();
        let adopted: Handed = serde_json::from_str(&json).unwrap();
        assert!(adopted.tiled, "it stays out of the navigator");
    }

    #[test]
    fn a_build_from_before_tilings_carried_over_lists_its_terminals() {
        let mut json = serde_json::to_value(handed(None)).unwrap();
        json.as_object_mut().unwrap().remove("tiled");
        let adopted: Handed = serde_json::from_value(json).unwrap();
        assert!(
            !adopted.tiled,
            "with no tile to go to, the navigator has it"
        );
    }

    #[test]
    fn the_next_build_keeps_the_tilings_and_the_selection() {
        let branch = Selection::Branch(
            "project".into(),
            Some("refs/heads/main".try_into().unwrap()),
        );
        let mesh = Selection::Mesh(
            "project".into(),
            "machine".into(),
            MeshPick::Branch("main".to_owned()),
        );
        let section = Selection::Section(
            "project".into(),
            Section::Upstream("refs/heads/main".try_into().unwrap()),
        );
        let tilings = Vec::from([
            (
                RowKey::Row(branch.clone()),
                tiled([terminal(1), commit(), RowKey::Row(mesh)]),
            ),
            (commit(), tiled([RowKey::Row(section)])),
        ]);
        let json = serde_json::to_string(&HandedTilings {
            tilings: tilings.clone(),
            selection: Some(branch.clone()),
        })
        .unwrap();
        let adopted: HandedTilings = serde_json::from_str(&json).unwrap();
        assert_eq!(
            adopted.tilings, tilings,
            "each row lays its tiles out as it did"
        );
        assert_eq!(adopted.selection, Some(branch), "the same row is selected");
    }

    #[test]
    fn a_project_tab_pinned_comes_over_with_its_tiling() {
        let tab = RowKey::Tab("project".into(), ProjectTab::Branches);
        let tilings = Vec::from([(commit(), tiled([tab.clone()]))]);
        let json = serde_json::to_string(&tilings).unwrap();
        let adopted: Vec<(RowKey, Tiling)> = serde_json::from_str(&json).unwrap();
        let left = surviving(adopted, |_| true);
        assert_eq!(
            left.get(&commit()),
            Some(&tiled([tab])),
            "only a terminal can be gone from the next build"
        );
    }

    #[test]
    fn a_terminal_that_did_not_come_over_leaves_its_tile() {
        let tilings = Vec::from([(commit(), tiled([terminal(1), terminal(2)]))]);
        let left = surviving(tilings, |id| id == TerminalId(2));
        assert_eq!(
            left.get(&commit()),
            Some(&tiled([terminal(2)])),
            "the tile beside it stays"
        );
    }

    #[test]
    fn a_tiling_goes_with_its_rows_terminal() {
        let tilings = Vec::from([(terminal(1), tiled([terminal(2)]))]);
        let left = surviving(tilings, |id| id == TerminalId(2));
        assert!(left.is_empty(), "there's no row left to show it");
    }

    #[test]
    fn a_tiling_goes_with_the_last_tile_beside_its_home_one() {
        let tilings = Vec::from([(commit(), tiled([terminal(1)]))]);
        let left = surviving(tilings, |_| false);
        assert!(left.is_empty(), "a row alone needs no tiling");
    }
}
