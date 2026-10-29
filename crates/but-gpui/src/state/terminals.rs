//! The terminals open on each worktree, listed under its heading in the order they opened.
//! Nothing saves them: they end with the app.

use gpui::{Context, Entity, Subscription};

use super::{ProjectId, Selection, Store};
use crate::backend::{self, Checkout};
use crate::terminal::{Terminal, TerminalEvent, TerminalId};

#[derive(Default)]
pub struct Terminals {
    opened: u64,
    open: Vec<Open>,
}

struct Open {
    id: TerminalId,
    project: ProjectId,
    checkout: Checkout,
    terminal: Entity<Terminal>,
    _events: Subscription,
}

impl Terminals {
    pub fn get(&self, id: TerminalId) -> Option<&Entity<Terminal>> {
        (self.open.iter())
            .find(|open| open.id == id)
            .map(|open| &open.terminal)
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
        cx.spawn(async move |this, cx| {
            let opened =
                backend::run(move || backend::terminal::open(&cwd, Terminal::OPENING)).await;
            this.update(cx, |store, cx| {
                let store_handle = cx.weak_entity();
                match opened.and_then(|opened| Terminal::new(opened, store_handle, cx)) {
                    Ok(terminal) => store.add_terminal(project, checkout, terminal, cx),
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
        self.terminals.opened += 1;
        let id = TerminalId(self.terminals.opened);
        let events = cx.subscribe(&terminal, move |store, _, event, cx| match event {
            TerminalEvent::Retitled => cx.notify(),
            TerminalEvent::Exited => store.close_terminal(id, cx),
        });
        self.terminals.open.push(Open {
            id,
            project: project.clone(),
            checkout,
            terminal,
            _events: events,
        });
        self.select(Some(Selection::Terminal(project, id)), cx);
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
