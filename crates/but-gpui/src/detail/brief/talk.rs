//! What's said about a brief, kept current while its tab shows, and what's written going out.

use gpui::Context;

use super::{briefs, run};
use crate::backend::briefs::{CommentTo, Note};
use crate::detail::Detail;
use crate::ui::plural;

/// How often what's said is asked for again while a brief shows, as an agent can be replying.
const TALK_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

/// How often, while an agent's waiting on it too.
const TALK_EVERY_WAITED_ON: std::time::Duration = std::time::Duration::from_secs(8);

/// How long until a row that's changed has been drawn and measured.
pub(super) const ROW_MEASURED: std::time::Duration = std::time::Duration::from_millis(50);

impl Detail {
    /// What the version whose tab shows is, by its id and number.
    pub(super) fn shown_version(&self) -> Option<(String, u32)> {
        let brief = (self.page.brief()).filter(|_| self.page.brief_tab())?;
        let page = brief.page()?;
        Some((page.id.clone(), page.version))
    }

    /// Keep what's been said about the brief whose tab shows current: what's kept first, and then
    /// what Briefs says, every little while it shows. Nothing asks once the tab's left.
    pub(super) fn keep_talk_current(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.shown_version() else {
            self.brief_talk = None;
            return;
        };
        if (self.brief_talk.as_ref()).is_some_and(|(talking, _)| *talking == key) {
            return;
        }
        let (id, version) = key.clone();
        let task = cx.spawn(async move |this, cx| {
            let kept = {
                let id = id.clone();
                run(move || briefs::kept_talk(&id, version)).await
            };
            this.update(cx, |this, cx| {
                if let (Ok(talk), Some(brief)) = (kept, this.page.brief_mut()) {
                    brief.activity = talk.activity;
                    brief.notes = talk.notes;
                    this.brief_list_changed(cx);
                }
            })
            .ok();
            loop {
                let hidden =
                    (this.update(cx, |this, cx| this.store.read(cx).hidden())).unwrap_or(true);
                if !hidden {
                    let asked = {
                        let id = id.clone();
                        run(move || briefs::ask_talk(&id, version)).await
                    };
                    let landed = this.update(cx, |this, cx| match asked {
                        Ok(activity) => {
                            if let Some(brief) = this.page.brief_mut()
                                && brief.activity != activity
                            {
                                brief.activity = activity;
                                this.brief_list_changed(cx);
                            }
                        }
                        Err(err) => tracing::debug!("couldn't ask what's said: {err:#}"),
                    });
                    if landed.is_err() {
                        return;
                    }
                }
                // Sooner with an agent at the other end, as a reply can be seconds away.
                let waiting = this.update(cx, |this, _| {
                    (this.page.brief()).is_some_and(|brief| !brief.activity.listeners.is_empty())
                });
                let every = match waiting {
                    Ok(true) => TALK_EVERY_WAITED_ON,
                    _ => TALK_EVERY,
                };
                cx.background_executor().timer(every).await;
            }
        });
        self.brief_talk = Some((key, task));
    }

    /// Keep a note on the whole brief, as written for them.
    pub(super) fn keep_note_on_brief(&mut self, body: String, cx: &mut Context<Self>) {
        if let Some((id, version)) = self.shown_version() {
            self.keep_brief_note(id, version, CommentTo::Brief, body, cx);
        }
    }

    pub(super) fn keep_brief_note(
        &mut self,
        id: String,
        version: u32,
        to: CommentTo,
        body: String,
        cx: &mut Context<Self>,
    ) {
        let key = (id.clone(), version);
        cx.spawn(async move |this, cx| {
            let notes = run(move || briefs::add_note(&id, version, &to, &body)).await;
            this.update(cx, |this, cx| match notes {
                // Kept either way, which that brief's page reads when it next shows.
                Ok(_) if this.shown_version().as_ref() != Some(&key) => {}
                Ok(notes) => this.took_brief_notes(notes, cx),
                Err(err) => (this.store).update(cx, |store, cx| {
                    store.snack(format!("Couldn't keep that comment: {err:#}"), cx)
                }),
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn took_brief_notes(&mut self, notes: Vec<Note>, cx: &mut Context<Self>) {
        if let Some(brief) = self.page.brief_mut() {
            brief.notes = notes;
        }
        self.brief_list_changed(cx);
    }

    pub(super) fn remove_brief_note(&mut self, note: i64, cx: &mut Context<Self>) {
        let Some((id, version)) = self.shown_version() else {
            return;
        };
        let key = (id.clone(), version);
        cx.spawn(async move |this, cx| {
            let notes = run(move || briefs::remove_note(&id, version, note)).await;
            this.update(cx, |this, cx| {
                if let Ok(notes) = notes
                    && this.shown_version().as_ref() == Some(&key)
                {
                    this.took_brief_notes(notes, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Send what's written.
    /// A failure says so and keeps what wasn't sent.
    pub(super) fn send_brief_talk(&mut self, cx: &mut Context<Self>) {
        let Some((id, version)) = self.shown_version() else {
            return;
        };
        if self.brief_sending.is_some() {
            return;
        }
        let Some(brief) = self.page.brief() else {
            return;
        };
        let sent = brief.notes.len();
        let waiting = (brief.activity.listeners.first()).map(|listener| {
            listener
                .agent
                .clone()
                .unwrap_or_else(|| listener.name.clone())
        });
        self.brief_sending = Some((id.clone(), version));
        cx.notify();
        let key = (id.clone(), version);
        cx.spawn(async move |this, cx| {
            let (keep_id, keep_version) = (id.clone(), version);
            let talk = run(move || briefs::send_talk(&id, version)).await;
            let kept = match &talk {
                Ok(_) => None,
                Err(_) => Some(run(move || briefs::kept_talk(&keep_id, keep_version)).await),
            };
            this.update(cx, |this, cx| {
                this.brief_sending = None;
                // What's shown now may be another's by now, whose page this doesn't belong on.
                let here = this.shown_version().as_ref() == Some(&key);
                let words = match talk {
                    Ok(talk) => {
                        if let Some(brief) = this.page.brief_mut().filter(|_| here) {
                            brief.unheard = sent > 0 && talk.activity.listeners.is_empty();
                            brief.activity = talk.activity;
                            brief.notes = talk.notes;
                        }
                        sent_words(sent, waiting.as_deref())
                    }
                    Err(err) => {
                        tracing::warn!("couldn't send what's written on a brief: {err:#}");
                        if let Some(Ok(talk)) = kept
                            && let Some(brief) = this.page.brief_mut().filter(|_| here)
                        {
                            brief.notes = talk.notes;
                        }
                        format!("{err}. What wasn't sent is kept here.")
                    }
                };
                this.brief_list_changed(cx);
                (this.store).update(cx, |store, cx| store.snack(words, cx));
            })
            .ok();
        })
        .detach();
    }
}

/// What to say once it's sent, naming who it reaches when someone's waiting.
fn sent_words(notes: usize, waiting: Option<&str>) -> String {
    let done = match notes {
        0 => "Nothing to send".to_owned(),
        notes => format!("Sent {}", plural(notes, "comment")),
    };
    match waiting {
        Some(waiting) => format!("{done}, {waiting} is waiting on it"),
        None => done,
    }
}
