//! The terminals open on each worktree, listed under its heading in the order they opened.
//! Nothing saves them: they end with the app, but for Build and restart, which hands them to
//! the new build.

use std::time::Instant;

use gpui::{App, Context, Entity, SharedString, Subscription};

use super::{ProjectId, Selection, Store, StoreEvent};
use crate::backend::{self, Checkout};
use crate::terminal::{Terminal, TerminalEvent, TerminalId};
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

    /// The user is looking at terminal `id`, so a result it showed is no news.
    pub(super) fn seen(&mut self, id: TerminalId, cx: &mut App) {
        if let Some(open) = self.open.iter_mut().find(|open| open.id == id) {
            open.status = open.terminal.update(cx, |terminal, _| terminal.seen());
        }
    }

    /// Those open on `project`'s `checkout`, in the order they opened.
    pub fn on<'a>(
        &'a self,
        project: &'a ProjectId,
        checkout: &'a Checkout,
    ) -> impl Iterator<Item = (TerminalId, &'a Entity<Terminal>)> + 'a {
        (self.open.iter())
            .filter(move |open| open.project == *project && open.checkout == *checkout)
            .map(|open| (open.id, &open.terminal))
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
    /// Open a terminal on `project`'s `checkout` and select it, once its shell has started.
    pub fn open_terminal(
        &mut self,
        project: ProjectId,
        checkout: Checkout,
        cx: &mut Context<Self>,
    ) {
        let cwd = match &checkout {
            Checkout::Main => self.path(&project),
            Checkout::Linked(name) => (self.workspace(&project))
                .and_then(|ws| ws.find_worktree(name.as_ref()))
                .map(|wt| wt.path.clone()),
        };
        let Some(cwd) = cwd else {
            return;
        };
        let asked = Instant::now();
        cx.spawn(async move |this, cx| {
            let started =
                backend::run(move || Ok((Shell::spawn(&cwd, Terminal::OPENING)?, Ghostty::load())))
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
                        store.add_terminal(project, checkout, terminal, cx);
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
        cx: &mut Context<Self>,
    ) {
        let id = TerminalId(self.terminals.opened + 1);
        self.insert_terminal(id, project.clone(), checkout, terminal, None, cx);
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
        if matches!(&self.selection, Some(Selection::Terminal(_, shown)) if *shown == id) {
            self.terminals.seen(id, cx);
        }
        cx.notify();
    }

    /// Hands each terminal's shell over, for the build exec runs next to adopt. One that can't
    /// be is hung up, as the exec closes its master.
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

    /// Opens the terminals the process this one was before exec handed over, where they were.
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

    /// Tells each terminal whether it's on screen: the one selected, while the window shows.
    /// Those out of sight read and answer their shells but don't draw.
    pub(super) fn show_terminals(&mut self, cx: &mut Context<Self>) {
        let on_screen = match &self.selection {
            Some(Selection::Terminal(_, id)) if !self.hidden() => Some(*id),
            _ => None,
        };
        for open in &self.terminals.open {
            let shown = on_screen == Some(open.id);
            open.terminal
                .update(cx, |terminal, _| terminal.set_shown(shown));
        }
    }

    /// Close terminal `id`, hanging up its shell.
    pub fn close_terminal(&mut self, id: TerminalId, cx: &mut Context<Self>) {
        self.close_terminals(&[id], cx);
    }

    /// Close terminals `ids`, hanging up their shells. While one's selected, another terminal
    /// on its worktree is, as the TUI goes on after removing something: the next one, else the
    /// one above. With none, the worktree's first branch, the row below them, else the worktree.
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
        let before = self.terminals.open.len();
        (self.terminals.open).retain(|open| !ids.contains(&open.id));
        if self.terminals.open.len() == before {
            return;
        }
        if let (Some(selected), Some((project, checkout, listed))) = (selected, place) {
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
                .unwrap_or_else(|| Selection::uncommitted_in(project.clone(), checkout));
            self.select(Some(instead), cx);
        }
        cx.notify();
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
