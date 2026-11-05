//! Where a Brief tab stands, along the row of the page's tabs: what's asked, what's written and not
//! sent yet, the answers and who they reach. So it's always in view, without a bar of its own.

use gpui::{AnyElement, Context, Div, div, prelude::*};

use super::head::agent_name;
use super::keys::GoBack;
use super::notes::target_words;
use super::{Approve, Hold, NextBrief, SendNotes, briefs};
use crate::backend::briefs::{Ask, BriefPage, CommentTo};
use crate::detail::Detail;
use crate::detail::page::{BriefState, Writing};
use crate::overlays::MenuItem;
use crate::state::{Axis, RowKey, Selection, Tile};
use crate::terminal::{Terminal, TerminalId};
use crate::theme::Theme;
use crate::ui::{ButtonStyle, Icon, button, dp, h_flex, plural, v_flex};

/// How wide the tab has to be for the row of its tabs to say where the brief stands too.
const STATUS_NEEDS: f32 = 760.;

/// How wide Approve is, with the words it can have.
const APPROVE_WIDTH: f32 = 104.;

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
        let turned_off = (state.tests.iter()).flat_map(|tests| tests.turned_off.first());
        let turned_off = turned_off.map(|off| {
            let name = name_of(&off.path);
            (
                "Turn the test back on".to_owned(),
                format!("Turn the test in {name} back on, or say why it's off."),
            )
        });
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
        turned_off.into_iter().chain(unnamed).chain(stale).collect()
    }

    /// Why this is being held, in a card under the Hold button that came and went without moving
    /// anything: the field, and what's quickest to say.
    pub(super) fn brief_hold_card(&self, cx: &Context<Self>) -> Option<Div> {
        let theme = Theme::get(cx);
        let state = self.page.brief()?;
        // Its field doesn't outlive leaving the page, which the page itself does.
        let (input, _) = self.brief_input.as_ref()?;
        if !matches!(state.writing, Some(Writing::WhyHold)) {
            return None;
        }
        let page = state.page()?;
        let written = input.read(cx).text().to_owned();
        let reasons = self.brief_reasons_to_hold(state, page);
        Some(
            v_flex()
                .absolute()
                .top(dp(8.))
                .right(dp(24.))
                .w(dp(380.))
                .gap(dp(6.))
                .p(dp(10.))
                .rounded(dp(8.))
                .border_1()
                .border_color(theme.border_2)
                .bg(theme.bg_1)
                .shadow_lg()
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_x(dp(8.))
                        .text_size(dp(12.))
                        .child(
                            div()
                                .text_color(theme.text_1)
                                .child(format!("Holding version {}. Why?", page.version)),
                        )
                        .child(
                            div()
                                .text_color(theme.text_3)
                                .child(format!("Goes to {}", self.brief_reaches(state))),
                        ),
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
                    h_flex().flex_wrap().gap(dp(6.)).children(
                        (reasons.into_iter())
                            .filter(|(_, said)| !written.contains(said.as_str()))
                            .enumerate()
                            .map(|(ix, (label, said))| {
                                button(("brief-reason", ix), label)
                                    .small()
                                    .style(ButtonStyle::Outline)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.add_to_writing(&said, window, cx)
                                    }))
                            }),
                    ),
                )
                .child(
                    h_flex()
                        .justify_end()
                        .gap(dp(4.))
                        .child(
                            button("brief-hold-cancel", "Cancel")
                                .small()
                                .style(ButtonStyle::Ghost)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.finish_writing(None, window, cx)
                                })),
                        )
                        .child(
                            button("brief-hold-send", "Hold")
                                .small()
                                .style(ButtonStyle::Gray)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.submit_writing(window, cx)
                                })),
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
                .style(ButtonStyle::Ghost)
                .tooltip("Types it in that terminal")
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
                .style(ButtonStyle::Ghost)
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

    /// What goes along the row of the page's tabs: where the brief stands, what's not sent yet
    /// and the way to send it, and the answers.
    pub(in crate::detail) fn brief_tab_actions(&self, cx: &Context<Self>) -> Option<Div> {
        let theme = Theme::get(cx);
        let state = self.page.brief()?;
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
        let listening = state.activity.listeners.first().map(|listener| {
            agent_name(listener.agent.as_deref().unwrap_or(&listener.name)).to_owned()
        });
        let words = match mine {
            _ if newer.is_some() => format!("Version {} is out", newer.unwrap_or_default()),
            _ if state.unheard => "No agent is reading this".to_owned(),
            Some(mine) => answered_words(mine, page),
            None if answering => match (&state.answered, listening) {
                (Some((version, answer)), _) if *version != page.version => format!(
                    "You {} version {version}. This one asks afresh",
                    if answer == "hold" { "held" } else { "answered" },
                ),
                (_, Some(agent)) => format!("{}. {agent} is waiting", asked_words(page)),
                (_, None) => asked_words(page),
            },
            None => listening.map_or_else(String::new, |agent| {
                format!("{agent} reads what you send here")
            }),
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
        let mut buttons: Vec<AnyElement> = Vec::new();
        if state.unheard && !and_send {
            buttons.insert(0, self.tell_agent_button(page, state, cx));
        }
        buttons.extend(read_newer);
        buttons.splice(0..0, back);
        if and_send {
            if self.brief_terminal(cx).is_some() {
                buttons.insert(0, self.tell_agent_button(page, state, cx));
            }
            let first = (state.notes.iter()).find_map(|note| self.row_of_note(&note.to));
            buttons.push(
                button(
                    "brief-unsent",
                    format!("{} not sent", plural(notes, "comment")),
                )
                .small()
                .icon(Icon::ArrowUp)
                .style(ButtonStyle::Ghost)
                .tint(theme.blue)
                .tooltip("Go to the first")
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(row) = first {
                        this.brief_list.scroll_to_reveal_item(row);
                        cx.notify();
                    }
                }))
                .into_any_element(),
            );
            buttons.push(
                button("brief-send", if notes == 1 { "Send" } else { "Send all" })
                    .small()
                    .style(ButtonStyle::Outline)
                    .tooltip_for(format!("Send to {}", self.brief_reaches(state)), &SendNotes)
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
                    // Not a reason to refuse, but what's approved should be plain. So holding is
                    // what's raised then, and approving otherwise: one or the other.
                    let stale = page.out_of_date() || self.brief_has_alerts(state, cx);
                    buttons.push(
                        button("brief-hold", "Hold")
                            .small()
                            .style(match stale {
                                true => ButtonStyle::Gray,
                                false => ButtonStyle::Ghost,
                            })
                            .tooltip_for(
                                match and_send {
                                    true => "Also sends your comments",
                                    false => "Asks why first",
                                },
                                &Hold,
                            )
                            .disabled_because(sending.then_some("Sending"))
                            .on_click(cx.listener(move |this, _, window, cx| match and_send {
                                true => this.send_brief_talk(Some(Some("hold".to_owned())), cx),
                                false => this.ask_why_hold(window, cx),
                            }))
                            .into_any_element(),
                    );
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
                        // One width either way, so Hold stays put from brief to brief.
                        .min_width(dp(APPROVE_WIDTH))
                        .tooltip_for(
                            match (and_send, stale) {
                                (true, _) => "Also sends your comments",
                                (false, true) => "Despite what's flagged",
                                (false, false) => "This version",
                            },
                            &Approve,
                        )
                        .into_any_element(),
                    );
                }
                (Some(Ask::HeadsUp), None) => buttons
                    .push(answer_button("brief-ack", "Seen it", "acknowledge").into_any_element()),
                // In a menu, as what can be chosen can be long and the row's short.
                (Some(Ask::Decision { options }), None) => {
                    let options = options.clone();
                    buttons.push(
                        button("brief-decide", "Decide")
                            .small()
                            .style(ButtonStyle::Gray)
                            .disabled_because(sending.then_some("Sending"))
                            .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                                let (options, this_page) = (options.clone(), cx.entity());
                                this.open_menu(
                                    event.position(),
                                    move |_, _| {
                                        (options.iter().enumerate())
                                            .map(|(ix, option)| {
                                                let page = this_page.clone();
                                                MenuItem::new(option.clone(), move |_, _, cx| {
                                                    let answer = format!("option:{ix}");
                                                    page.update(cx, |this, cx| {
                                                        this.send_brief_talk(Some(Some(answer)), cx)
                                                    });
                                                })
                                            })
                                            .collect()
                                    },
                                    cx,
                                )
                            }))
                            .into_any_element(),
                    );
                }
                _ => {}
            }
        }
        Some(
            h_flex()
                .flex_1()
                .min_w_0()
                .pl(dp(16.))
                .pb(dp(11.))
                .gap(dp(8.))
                .justify_end()
                .text_size(dp(12.))
                .text_color(theme.text_2)
                // Left out where it would only be cut down to a letter or two.
                .when(self.brief_room.get() >= STATUS_NEEDS, |el| {
                    el.child(div().min_w_0().truncate().child(words))
                })
                .children(buttons.into_iter().map(|el| div().flex_none().child(el))),
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
