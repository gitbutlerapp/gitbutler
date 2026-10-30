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
}

struct Open {
    id: TerminalId,
    project: ProjectId,
    checkout: Checkout,
    terminal: Entity<Terminal>,
    title: SharedString,
    status: Status,
    _events: Subscription,
}

impl Terminals {
    pub fn get(&self, id: TerminalId) -> Option<&Entity<Terminal>> {
        (self.open.iter())
            .find(|open| open.id == id)
            .map(|open| &open.terminal)
    }

    /// Kept here so rows can show a terminal without reading it, which would have the window
    /// draw on all it prints, shown or not.
    pub fn title(&self, id: TerminalId) -> Option<&SharedString> {
        (self.open.iter())
            .find(|open| open.id == id)
            .map(|open| &open.title)
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
        self.insert_terminal(id, project.clone(), checkout, terminal, cx);
        self.select(Some(Selection::Terminal(project, id)), cx);
        cx.emit(StoreEvent::TerminalOpened);
    }

    fn insert_terminal(
        &mut self,
        id: TerminalId,
        project: ProjectId,
        checkout: Checkout,
        terminal: Entity<Terminal>,
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
            status,
            _events: events,
        });
    }

    fn retitle_terminal(&mut self, id: TerminalId, title: SharedString, cx: &mut Context<Self>) {
        if let Some(open) = self.terminals.open.iter_mut().find(|open| open.id == id)
            && open.title != title
        {
            open.title = title;
            cx.notify();
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
            } = handed;
            let store = cx.weak_entity();
            match Terminal::adopt(detached, store, cx) {
                Ok(terminal) => {
                    self.insert_terminal(TerminalId(id), project.into(), checkout, terminal, cx)
                }
                Err(err) => tracing::error!("couldn't adopt a terminal: {err:#}"),
            }
        }
    }

    /// Close terminal `id`, hanging up its shell; while it's selected, its worktree is.
    pub fn close_terminal(&mut self, id: TerminalId, cx: &mut Context<Self>) {
        let Some(at) = self.terminals.open.iter().position(|open| open.id == id) else {
            return;
        };
        let open = self.terminals.open.remove(at);
        if let Some(Selection::Terminal(_, selected)) = &self.selection
            && *selected == id
        {
            self.select(
                Some(Selection::uncommitted_in(open.project, open.checkout)),
                cx,
            );
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
        for id in gone {
            self.close_terminal(id, cx);
        }
    }

    /// Show the app's terminals' text at `size`, or with `None` at Ghostty's.
    pub fn set_terminal_text(&mut self, size: Option<u8>, cx: &mut Context<Self>) {
        self.set_prefs(|prefs| prefs.terminal_text = size, cx);
        for open in &self.terminals.open {
            open.terminal.update(cx, |_, cx| cx.notify());
        }
    }
}
