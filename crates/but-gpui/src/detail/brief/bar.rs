//! The foot of a Brief tab: what's being written, what's to send and answer, and who it reaches.
//! It's always in view, so what's said is never something to go looking for, and writing there
//! moves nothing on the page.

use gpui::{AnyElement, Context, Div, div, prelude::*};

use super::head::agent_name;
use super::keys::GoBack;
use super::notes::target_words;
use super::{Approve, Hold, NextBrief, SendNotes, WriteNote, briefs};
use crate::backend::briefs::{Ask, BriefPage, CommentTo};
use crate::detail::Detail;
use crate::detail::page::{BriefState, Writing};
use crate::state::{Axis, RowKey, Selection, Tile};
use crate::terminal::{Terminal, TerminalId};
use crate::theme::Theme;
use crate::ui::{ButtonStyle, Icon, button, dp, h_flex, plural, v_flex};

impl Detail {
    /// Who what's sent as comments reaches: the brief's author, and any agent waiting on it.
    fn brief_reaches(&self, state: &BriefState) -> String {
        // On one's own brief, what's said is for whoever reads it next.
        let author = match state.activity.answers.can_answer {
            true => state
                .found
                .as_ref()
                .map_or("the author", |found| found.author.name.as_str()),
            false => "the brief's comments",
        };
        match state.activity.listeners.first() {
            Some(listener) => {
                let agent = agent_name(listener.agent.as_deref().unwrap_or(&listener.name));
                format!("{author} and {agent}")
            }
            None => author.to_owned(),
        }
    }

    /// What the repository gives as reasons to hold this, each as a short name and what it'd say.
    /// They add up in the field, as a branch can have several of them.
    fn brief_reasons_to_hold(&self, state: &BriefState, page: &BriefPage) -> Vec<(String, String)> {
        let name_of = |path: &str| path.rsplit('/').next().unwrap_or(path).to_owned();
        let unnamed = state.unnamed.first().map(|path| {
            let name = name_of(path);
            (
                format!("Explain {name}"),
                format!("Say what {name} does. The brief doesn't mention it."),
            )
        });
        let stale = page.out_of_date().then(|| {
            (
                "Update the brief".to_owned(),
                "Update the brief to match the branch.".to_owned(),
            )
        });
        unnamed.into_iter().chain(stale).collect()
    }

    /// The bar while something's being written: what it's about, the field, and what's quickest
    /// to say.
    fn brief_writing_bar(
        &self,
        writing: &Writing,
        state: &BriefState,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let theme = Theme::get(cx);
        let (input, _) = self.brief_input.as_ref()?;
        let page = state.page()?;
        let (about, keeps): (String, &str) = match writing {
            Writing::Note(to) => (
                match to {
                    CommentTo::Thread(_) => "A reply".to_owned(),
                    to => format!("A note on {}", target_words(to, page)),
                },
                "Enter saves. Esc discards.",
            ),
            Writing::WhyHold => (
                format!("Holding version {}. Why?", page.version),
                "Enter holds and sends. Esc cancels.",
            ),
        };
        let written = input.read(cx).text().to_owned();
        let reasons = match writing {
            Writing::WhyHold => self.brief_reasons_to_hold(state, page),
            Writing::Note(_) => Vec::new(),
        };
        Some(
            v_flex()
                .flex_none()
                .gap(dp(6.))
                .px(dp(24.))
                .py(dp(8.))
                .border_t_1()
                .border_color(theme.border_3)
                .bg(theme.bg_2)
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_x(dp(8.))
                        .text_size(dp(12.))
                        .child(div().text_color(theme.text_1).child(about))
                        .child(div().text_color(theme.text_3).child(match writing {
                            Writing::Note(_) => "It stays here until you send it".to_owned(),
                            Writing::WhyHold => {
                                format!("Goes to {}", self.brief_reaches(state))
                            }
                        })),
                )
                .child(
                    div()
                        .px(dp(8.))
                        .py(dp(5.))
                        .rounded(dp(6.))
                        .border_1()
                        .border_color(theme.focus_border())
                        .bg(theme.bg_1)
                        .text_size(dp(13.))
                        .child(input.clone()),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(dp(4.))
                        .children(
                            (reasons.into_iter().enumerate())
                                .filter(|(_, (_, said))| !written.contains(said.as_str()))
                                .map(|(ix, (label, said))| {
                                    button(("brief-reason", ix), label)
                                        .small()
                                        .style(ButtonStyle::Outline)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.add_to_writing(&said, window, cx)
                                        }))
                                }),
                        )
                        .child(
                            div()
                                .ml(dp(6.))
                                .text_size(dp(11.))
                                .text_color(theme.text_3)
                                .child(keeps.to_owned()),
                        ),
                ),
        )
    }

    /// The project's terminal most likely running an agent: one named for one, and one in a tile
    /// first, as that's likely beside this.
    fn brief_terminal(
        &self,
        cx: &Context<Self>,
    ) -> Option<(TerminalId, gpui::Entity<Terminal>, gpui::SharedString, bool)> {
        let project = self.selected_project()?;
        let store = self.store.read(cx);
        let open: Vec<_> = store.terminals.of_project(&project).collect();
        let named = |title: &str| {
            let title = title.to_lowercase();
            ["claude", "codex", "agent", "gemini", "cursor"]
                .iter()
                .any(|name| title.contains(name))
        };
        let best = (open
            .iter()
            .find(|(_, _, title, tiled)| named(title) && *tiled))
        .or_else(|| open.iter().find(|(_, _, title, _)| named(title)))
        .or_else(|| open.iter().find(|(_, _, _, tiled)| *tiled))
        .or_else(|| open.first())?;
        Some((best.0, best.1.clone(), best.2.clone(), best.3))
    }

    /// The way to tell an agent: typed into the project's terminal that's most likely running
    /// one, for a look and an Enter there, as an agent may be busy. What's written goes that way
    /// and nowhere else, as that may be all it's for. With nothing written, that it should check
    /// the comments. With no terminal open, the words to give it.
    fn tell_agent_button(
        &self,
        page: &BriefPage,
        state: &BriefState,
        cx: &Context<Self>,
    ) -> AnyElement {
        let url = briefs::web_url(&page.id, page.version);
        let words = match state.notes.is_empty() {
            true => format!("Check the comments on my brief and act on them: {url}"),
            false => {
                let notes = (state.notes.iter()).map(|note| match &note.to {
                    CommentTo::Passage { quote, .. } => {
                        let quote: String = quote.chars().take(120).collect();
                        format!("- On \"{quote}\": {}", note.body)
                    }
                    to => format!("- On {}: {}", target_words(to, page), note.body),
                });
                let notes: Vec<String> = notes.collect();
                format!("About the brief {url}\n{}", notes.join("\n"))
            }
        };
        let notes: Vec<i64> = state.notes.iter().map(|note| note.id).collect();
        match self.brief_terminal(cx) {
            Some((id, terminal, title, tiled)) => button("brief-tell", format!("Tell {title}"))
                .small()
                .style(ButtonStyle::Gray)
                .tooltip("Type it in that terminal, for you to look at and send")
                .on_click(cx.listener(move |this, _, window, cx| {
                    let Some(project) = this.selected_project() else {
                        return;
                    };
                    // In a tile, it stays in view: beside this, if it's another branch's so far.
                    (this.store).update(cx, |store, cx| {
                        let selection = Selection::Terminal(project, id);
                        match tiled {
                            true => store.pin(Tile::Home, Axis::Across, RowKey::Row(selection), cx),
                            false => store.select(Some(selection), cx),
                        }
                    });
                    let (terminal, words) = (terminal.clone(), words.clone());
                    // Once it's drawn, as the keys go with it.
                    window.on_next_frame(move |window, cx| {
                        terminal.update(cx, |terminal, cx| terminal.paste(&words, window, cx));
                    });
                    if let Some(brief) = this.page.brief_mut() {
                        brief.unheard = false;
                    }
                    for note in &notes {
                        this.remove_brief_note(*note, cx);
                    }
                }))
                .into_any_element(),
            None => button("brief-tell", "Copy what to tell it")
                .small()
                .style(ButtonStyle::Gray)
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(words.clone()));
                    if let Some(brief) = this.page.brief_mut() {
                        brief.unheard = false;
                    }
                    (this.store).update(cx, |store, cx| store.snack("Copied", cx));
                }))
                .into_any_element(),
        }
    }

    /// Along the tab's foot: what's being written, or what's to send and answer, and the way to
    /// say something, so that's never a thing to go looking for.
    pub(super) fn brief_bar(&self, cx: &Context<Self>) -> Option<Div> {
        let theme = Theme::get(cx);
        let state = self.page.brief()?;
        // Its field doesn't outlive leaving the page, which the page itself does.
        if let Some(writing) = state
            .writing
            .as_ref()
            .filter(|_| self.brief_input.is_some())
        {
            return self.brief_writing_bar(writing, state, cx);
        }
        let page = state.page()?;
        let notes = state.notes.len();
        let answers = &state.activity.answers;
        let answering = page.ask.is_some() && answers.can_answer;
        let mine = answers.mine.as_deref();
        // A newer version, wherever they are in this one: the agent's answered.
        let newer = (state.activity.latest.into_iter())
            .chain(state.found.as_ref().map(|found| found.latest))
            .max()
            .filter(|latest| *latest > page.version);
        let words = match (notes, mine) {
            (0, _) if newer.is_some() => format!("Version {} is out", newer.unwrap_or_default()),
            (0, _) if state.unheard => {
                "Sent as comments. No agent is reading this brief right now.".to_owned()
            }
            (0, Some(mine)) => answered_words(mine, page),
            (0, None) if answering => match (&state.answered, &page.ask) {
                (Some((version, answer)), _) if *version != page.version => format!(
                    "You {} version {version}. This is version {}, asking afresh",
                    if answer == "hold" { "held" } else { "answered" },
                    page.version
                ),
                _ => match state.activity.listeners.first() {
                    Some(listener) => format!(
                        "{}. {} is waiting",
                        asked_words(page),
                        agent_name(listener.agent.as_deref().unwrap_or(&listener.name))
                    ),
                    None => asked_words(page),
                },
            },
            (0, None) => match state.activity.listeners.first() {
                Some(listener) => format!(
                    "{} reads what you send here",
                    agent_name(listener.agent.as_deref().unwrap_or(&listener.name))
                ),
                None => String::new(),
            },
            (n, _) => {
                let reaches = self.brief_reaches(state);
                let notes = plural(n, "note");
                match (self.brief_terminal(cx), answering && mine.is_none()) {
                    (Some((_, _, title, _)), _) => {
                        format!("{notes}, not sent. Tell {title}, or send to {reaches}")
                    }
                    (None, true) => {
                        let goes = if n == 1 { "goes" } else { "go" };
                        format!("{notes} for {reaches} {goes} with any of these")
                    }
                    (None, false) => format!("{notes}, not sent. Send to {reaches}"),
                }
            }
        };
        let sending = self.brief_sending.is_some();
        let and_send = notes > 0;
        let answer_button = |id: &'static str, label: &str, answer: &'static str| {
            button(id, label.to_owned())
                .small()
                .style(ButtonStyle::Gray)
                .disabled_because(sending.then_some("Sending"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.send_brief_talk(Some(Some(answer.to_owned())), cx)
                }))
        };
        let read_newer = newer.map(|latest| {
            button("brief-read-newer", "Read it")
                .small()
                .style(ButtonStyle::Gray)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(brief) = this.page.brief_mut() {
                        brief.version = Some(latest);
                    }
                    this.ask_briefs(cx);
                    cx.notify();
                }))
                .into_any_element()
        });
        let back = state.back.is_some().then(|| {
            button("brief-back", "Back to where you were")
                .small()
                .icon(Icon::ArrowUp)
                .style(ButtonStyle::Ghost)
                .tooltip_for("Go back", &GoBack)
                .on_click(cx.listener(|this, _, window, cx| this.go_back_in_brief(window, cx)))
                .into_any_element()
        });
        let mut buttons: Vec<AnyElement> = vec![
            button("brief-write", "Write a note")
                .small()
                .icon(Icon::Edit)
                .style(ButtonStyle::Ghost)
                .tooltip_for(
                    "Write a note on what's selected, or where you are",
                    &WriteNote,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    let to = this.brief_note_target();
                    this.write_brief_note(to, window, cx)
                }))
                .into_any_element(),
        ];
        if state.unheard && !and_send {
            buttons.insert(0, self.tell_agent_button(page, state, cx));
        }
        buttons.extend(read_newer);
        buttons.splice(0..0, back);
        if and_send {
            if self.brief_terminal(cx).is_some() {
                buttons.insert(0, self.tell_agent_button(page, state, cx));
            }
            buttons.push(
                button("brief-send", "Send as comments")
                    .small()
                    .style(ButtonStyle::Outline)
                    .tooltip_for("Send what's written as comments", &SendNotes)
                    .disabled_because(sending.then_some("Sending"))
                    .on_click(cx.listener(|this, _, _, cx| this.send_brief_talk(None, cx)))
                    .into_any_element(),
            );
        }
        if answering {
            match (&page.ask, mine) {
                (_, Some(_)) => {
                    buttons.push(
                        button("brief-take-back", "Take back")
                            .small()
                            .style(ButtonStyle::Ghost)
                            .disabled_because(sending.then_some("Sending"))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.send_brief_talk(Some(None), cx)),
                            )
                            .into_any_element(),
                    );
                    // That's this one seen to, so what's next is what's likely.
                    buttons.extend(self.next_brief(true, cx).map(|next| {
                        button("brief-next-answered", "Next brief")
                            .small()
                            .style(ButtonStyle::Gray)
                            .tooltip_for(format!("Go on to {}", next.title), &NextBrief)
                            .on_click(cx.listener(|this, _, _, cx| this.hop_to_brief(true, cx)))
                            .into_any_element()
                    }));
                }
                (Some(Ask::SignOff), None) => {
                    buttons.push(
                        button("brief-hold", "Hold")
                            .small()
                            .style(ButtonStyle::Gray)
                            .tooltip_for("Hold this version, and say why", &Hold)
                            .disabled_because(sending.then_some("Sending"))
                            .on_click(cx.listener(move |this, _, window, cx| match and_send {
                                true => this.send_brief_talk(Some(Some("hold".to_owned())), cx),
                                false => this.ask_why_hold(window, cx),
                            }))
                            .into_any_element(),
                    );
                    // Not a reason to refuse, but what's approved should be plain.
                    let stale = page.out_of_date() || self.brief_has_alerts(state, cx);
                    buttons.push(
                        answer_button(
                            "brief-approve",
                            if stale { "Approve anyway" } else { "Approve" },
                            "approve",
                        )
                        .style(match stale {
                            true => ButtonStyle::Ghost,
                            false => ButtonStyle::Pop,
                        })
                        .tooltip_for(
                            match stale {
                                true => "Approve this version, despite what's flagged above it",
                                false => "Approve this version",
                            },
                            &Approve,
                        )
                        .into_any_element(),
                    );
                }
                (Some(Ask::HeadsUp), None) => buttons
                    .push(answer_button("brief-ack", "Seen it", "acknowledge").into_any_element()),
                (Some(Ask::Decision { options }), None) => {
                    for (ix, option) in options.iter().enumerate() {
                        let answer = format!("option:{ix}");
                        buttons.push(
                            button(("brief-option", ix), option.clone())
                                .small()
                                .style(ButtonStyle::Gray)
                                .disabled_because(sending.then_some("Sending"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.send_brief_talk(Some(Some(answer.clone())), cx)
                                }))
                                .into_any_element(),
                        );
                    }
                }
                _ => {}
            }
        }
        Some(
            h_flex()
                .flex_none()
                .flex_wrap()
                .gap(dp(8.))
                .px(dp(24.))
                .py(dp(8.))
                .border_t_1()
                .border_color(theme.border_3)
                .bg(theme.bg_2)
                .text_size(dp(12.))
                .text_color(theme.text_2)
                .child(
                    div()
                        .flex_1()
                        .min_w(dp(120.))
                        .text_color(theme.text_1)
                        .child(words),
                )
                .children(buttons),
        )
    }
}

/// What's asked, to say beside the answers.
fn asked_words(page: &BriefPage) -> String {
    match &page.ask {
        Some(Ask::SignOff) => "Wants your sign-off".to_owned(),
        Some(Ask::HeadsUp) => "Just so you know".to_owned(),
        Some(Ask::Decision { .. }) => "Wants a decision".to_owned(),
        _ => String::new(),
    }
}

fn answered_words(answer: &str, page: &BriefPage) -> String {
    match (answer, &page.ask) {
        ("approve", _) => "You approved this version".to_owned(),
        ("hold", _) => "You're holding this version".to_owned(),
        ("acknowledge", _) => "You've seen this".to_owned(),
        (option, Some(Ask::Decision { options })) => (option.strip_prefix("option:"))
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| options.get(n))
            .map_or_else(
                || "You answered".to_owned(),
                |picked| format!("You picked {picked}"),
            ),
        _ => "You answered".to_owned(),
    }
}
