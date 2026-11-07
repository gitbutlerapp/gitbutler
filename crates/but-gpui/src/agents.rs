//! What agents ask the app over its socket ([`but_path::app`]), and what the user tells them
//! back. An agent opens a place, with a step of a walkthrough beside it to read, and reads what
//! the user looks at. The user asks agents about what they look at, and steps through a
//! walkthrough, which the agents waiting hear, as `but _wait` does.

use std::collections::VecDeque;
use std::time::Duration;

use but_path::app::{self, Event, EventKind, Open, Opened, Place, View, Wait, Waited, method};
use gpui::{App, AppContext as _, Context, SharedString, Window, actions};

use crate::app::AppView;
use crate::backend::links::{Reply, Request};
use crate::detail::{PageTab, has_tabs, shows_its_diff};
use crate::links::{Tab, place_of};
use crate::state::{Landed, LinkLanding, Selection};
use crate::ui::text_input::TextInput;

actions!(
    agents,
    [
        /// Ask the agents waiting about what's selected, with a note.
        AskAgents,
        /// Tell the agent showing a walkthrough to go on to its next step.
        NextStep,
        /// End the walkthrough an agent is showing.
        EndWalkthrough,
    ]
);

/// How many of what the user told agents are kept for one waiting after an earlier one.
const KEPT: usize = 100;

/// What the prompt asking agents says of who hears it.
const ASK_DETAIL: &str = "Agents waiting with `but _wait` get your note and where you are.";

/// What the user told agents, and the agents waiting to hear the next of it.
#[derive(Default)]
pub(crate) struct Agents {
    events: VecDeque<Event>,
    /// The last event's number, from 1 at the app's start.
    last: u64,
    waiting: Vec<Waiting>,
    next_waiting: u64,
    /// The step of a walkthrough an agent shows, while it's on screen.
    pub(crate) guide: Option<Guide>,
}

/// An agent waiting for the events after `after`.
struct Waiting {
    id: u64,
    after: u64,
    reply: Reply,
}

/// A step of a walkthrough: what an agent says of the place it opened.
pub(crate) struct Guide {
    pub(crate) text: SharedString,
    /// The user asked for the next step, which the agent hasn't shown yet.
    pub(crate) asked_next: bool,
}

impl Agents {
    /// Answer `reply` with the events after `after`, or after the last one if `None`, once
    /// there are any. The id it waits by, while it waits.
    fn wait(&mut self, after: Option<u64>, reply: Reply) -> Option<u64> {
        let after = after.unwrap_or(self.last);
        let events = self.after(after);
        if !events.is_empty() {
            reply.ok(Waited {
                events,
                last: self.last,
            });
            return None;
        }
        let id = self.next_waiting;
        self.next_waiting += 1;
        self.waiting.push(Waiting { id, after, reply });
        Some(id)
    }

    fn after(&self, after: u64) -> Vec<Event> {
        (self.events.iter())
            .filter(|event| event.seq > after)
            .cloned()
            .collect()
    }

    /// Tell agents `kind`, the user being at `view`, answering those waiting. How many heard.
    pub(crate) fn tell(&mut self, kind: EventKind, view: View) -> usize {
        self.last += 1;
        let seq = self.last;
        self.events.push_back(Event { seq, kind, view });
        if self.events.len() > KEPT {
            self.events.pop_front();
        }
        let mut heard = 0;
        for waiting in std::mem::take(&mut self.waiting) {
            // Its connection closed, as when its `but _wait` was stopped.
            if !waiting.reply.is_waited_on() {
                continue;
            }
            waiting.reply.ok(Waited {
                events: self.after(waiting.after),
                last: self.last,
            });
            heard += 1;
        }
        heard
    }

    /// Stop `id` waiting, answering it with no events.
    fn time_out(&mut self, id: u64) {
        if let Some(ix) = self.waiting.iter().position(|waiting| waiting.id == id) {
            let waiting = self.waiting.remove(ix);
            waiting.reply.ok(Waited {
                events: Vec::new(),
                last: self.last,
            });
        }
    }
}

/// Answer `request`, and say whether the window comes forward for it, as for what it shows.
pub(crate) fn answer(request: Request, app: &mut AppView, cx: &mut Context<AppView>) -> bool {
    let Request {
        method,
        params,
        reply,
    } = request;
    match method.as_str() {
        method::OPEN => match serde_json::from_value::<Open>(params) {
            Ok(open) => {
                open_at(open, reply, app, cx);
                true
            }
            Err(err) => {
                let message = format!("`open` takes a `path`, and a place `at` in it: {err}");
                reply.fail(app::code::INVALID_PARAMS, message);
                false
            }
        },
        method::SHOW => {
            reply.ok(serde_json::json!({}));
            true
        }
        method::VIEW => {
            reply.ok(view(app, cx));
            false
        }
        method::WAIT => match serde_json::from_value::<Wait>(params) {
            Ok(wait) => {
                wait_for(wait, reply, app, cx);
                false
            }
            Err(err) => {
                let message = format!("`wait` takes `after` and `timeout_secs`: {err}");
                reply.fail(app::code::INVALID_PARAMS, message);
                false
            }
        },
        other => {
            let message = format!(
                "GitButler has no method `{other}`: it has `open`, `view`, `wait` and `show`"
            );
            reply.fail(app::code::METHOD_NOT_FOUND, message);
            false
        }
    }
}

/// Open the project and place `open` names, answering `reply` once it lands with what the
/// window shows then. The step of a walkthrough it says shows beside it.
fn open_at(open: Open, reply: Reply, app: &mut AppView, cx: &mut Context<AppView>) {
    let Open { path, at, say, .. } = open;
    let (landing, landed) = LinkLanding::new();
    (app.store).update(cx, |store, cx| {
        store.open_from_link(path, at.0, Some(landing), cx)
    });
    cx.spawn(async move |this, cx| {
        let Landed { landed, warning } = landed.await.unwrap_or_else(|_| Landed {
            landed: false,
            warning: Some("Another link took the window elsewhere before this one landed".into()),
        });
        this.update(cx, |app, cx| {
            if let Some(say) = say {
                app.agents.guide = Some(Guide {
                    text: say.into(),
                    asked_next: false,
                });
                cx.notify();
            }
            reply.ok(Opened {
                landed,
                warning,
                view: view(app, cx),
            });
        })
        .ok();
    })
    .detach();
}

/// Answer `reply` with what the user tells agents after the event `wait` names, once there's
/// any, or with none once it times out.
fn wait_for(wait: Wait, reply: Reply, app: &mut AppView, cx: &mut Context<AppView>) {
    let Some(id) = app.agents.wait(wait.after, reply) else {
        return;
    };
    if let Some(secs) = wait.timeout_secs {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(secs))
                .await;
            this.update(cx, |app, _| app.agents.time_out(id)).ok();
        })
        .detach();
    }
}

/// What the user looks at, as a link names it, with its project's folder.
pub(crate) fn view(app: &AppView, cx: &App) -> View {
    let store = app.store.read(cx);
    let Some(selection) = store.selection.clone() else {
        return View::default();
    };
    let detail = app.detail.read(cx);
    let tab = match &selection {
        Selection::Project(_) => Some(Tab::Project(detail.project_tab())),
        selection if has_tabs(selection) => Some(Tab::Page(detail.shown_tab(cx))),
        _ => None,
    };
    // A file opens in the diff, which the page's other tabs show in place of.
    let in_diff = matches!(tab, None | Some(Tab::Page(PageTab::Changes)));
    let place = (in_diff && shows_its_diff(&selection))
        .then(|| detail.link_place(&selection))
        .flatten();
    View {
        path: (store.project(selection.project())).map(|entry| entry.project.path.clone()),
        at: Place(place_of(&selection, tab, place.as_ref())),
        title: Some(store.place(&selection).to_string()),
    }
}

impl AppView {
    /// Ask the agents waiting about what the user looks at, with a note the prompt asks for.
    pub(crate) fn ask_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.store.read(cx).selection.clone() else {
            return;
        };
        let at = view(self, cx);
        let place = self.store.read(cx).place(&selection);
        let input = cx.new(|cx| TextInput::new("What to ask", false, cx));
        let check = |_: &AppView, note: &str, _: &App| {
            note.trim().is_empty().then(|| "Say what to ask".into())
        };
        self.open_prompt(
            Some(place),
            "Ask agents",
            Some(ASK_DETAIL.into()),
            "Ask",
            input,
            None,
            Some(Box::new(check)),
            window,
            cx,
            move |app, note, _, cx| {
                let note = note.trim().to_owned();
                let heard = app.agents.tell(EventKind::Asked { note }, at);
                let said = match heard {
                    0 => "Kept for the next agent that waits",
                    1 => "Sent to the agent waiting",
                    _ => "Sent to the agents waiting",
                };
                app.store.update(cx, |store, cx| store.snack(said, cx));
            },
        );
    }

    /// Tell the agent showing a walkthrough the user's ready for its next step.
    pub(crate) fn next_step(&mut self, cx: &mut Context<Self>) {
        let at = view(self, cx);
        let Some(guide) = &mut self.agents.guide else {
            return;
        };
        guide.asked_next = true;
        self.agents.tell(EventKind::Next, at);
        cx.notify();
    }

    /// End the walkthrough an agent is showing, telling it so.
    pub(crate) fn end_walkthrough(&mut self, cx: &mut Context<Self>) {
        if self.agents.guide.take().is_none() {
            return;
        }
        let at = view(self, cx);
        self.agents.tell(EventKind::Done, at);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use futures::channel::oneshot;

    use super::*;

    /// A reply waited on, as a connection waits, and the answer it gets.
    fn waited() -> (Reply, oneshot::Receiver<but_path::app::Response>) {
        let (reply, answer) = Reply::new(Some(1));
        (reply, answer.expect("a request is waited on"))
    }

    fn events(answer: oneshot::Receiver<but_path::app::Response>) -> Option<Waited> {
        let response = futures::executor::block_on(answer).ok()?;
        serde_json::from_value(response.result?).ok()
    }

    fn asked(note: &str) -> EventKind {
        EventKind::Asked { note: note.into() }
    }

    #[test]
    fn an_agent_waiting_hears_what_the_user_says_next() {
        let mut agents = Agents::default();
        agents.tell(asked("before anyone waited"), View::default());
        let (reply, answer) = waited();
        assert!(
            agents.wait(None, reply).is_some(),
            "with no `after`, it waits past what was said before"
        );
        assert_eq!(agents.tell(asked("why?"), View::default()), 1, "it heard");
        let heard = events(answer).expect("an answer");
        assert_eq!(
            (
                heard.events.iter().map(|e| e.seq).collect::<Vec<_>>(),
                heard.last
            ),
            (vec![2], 2),
            "only what was said while it waited, numbered after the first"
        );
    }

    #[test]
    fn an_agent_waiting_after_an_earlier_event_gets_what_it_missed_at_once() {
        let mut agents = Agents::default();
        agents.tell(asked("one"), View::default());
        agents.tell(EventKind::Next, View::default());
        let (reply, answer) = waited();
        assert_eq!(agents.wait(Some(0), reply), None, "nothing to wait for");
        let heard = events(answer).expect("an answer");
        assert_eq!(
            heard
                .events
                .iter()
                .map(|e| e.kind.clone())
                .collect::<Vec<_>>(),
            vec![asked("one"), EventKind::Next],
            "both, in order"
        );
    }

    #[test]
    fn a_wait_that_times_out_hears_nothing_and_a_closed_one_isnt_counted() {
        let mut agents = Agents::default();
        let (reply, answer) = waited();
        let id = agents.wait(None, reply).expect("it waits");
        agents.time_out(id);
        let heard = events(answer).expect("an answer");
        assert!(heard.events.is_empty(), "it timed out with nothing");

        let (reply, answer) = waited();
        agents.wait(None, reply).expect("it waits");
        drop(answer);
        assert_eq!(
            agents.tell(asked("anyone?"), View::default()),
            0,
            "an agent that stopped waiting didn't hear it"
        );
    }

    #[test]
    fn only_the_last_hundred_events_are_kept() {
        let mut agents = Agents::default();
        for n in 0..KEPT + 5 {
            agents.tell(asked(&n.to_string()), View::default());
        }
        let (reply, answer) = waited();
        agents.wait(Some(0), reply);
        let heard = events(answer).expect("an answer");
        assert_eq!(
            (heard.events.len(), heard.events.first().map(|e| e.seq)),
            (KEPT, Some(6)),
            "the oldest go, numbers kept"
        );
    }
}
