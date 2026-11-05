//! Where a Brief tab stands, along the row of the page's tabs: what's asked, what's written and not
//! sent yet, the answers and who they reach. So it's always in view, without a bar of its own.

use gpui::{AnyElement, Context, Div, div, prelude::*};

use super::head::agent_name;
use super::keys::GoBack;
use super::notes::target_words;
use super::state::BriefState;
use super::{NextBrief, briefs};
use crate::backend::briefs::{Ask, BriefPage, CommentTo};
use crate::detail::Detail;
use crate::state::{Axis, RowKey, Selection, Tile};
use crate::terminal::{Terminal, TerminalId};
use crate::theme::Theme;
use crate::ui::{ButtonStyle, Icon, button, dp, h_flex, plural};

/// How wide the tab has to be for the row of its tabs to say where the brief stands too.
const STATUS_NEEDS: f32 = 760.;

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
        let busy = self.brief_notes_busy();
        match self.brief_terminal(cx) {
            Some((id, terminal, title, tiled)) => button("brief-tell", format!("Tell {title}"))
                .small()
                .style(ButtonStyle::Ghost)
                .tooltip("Types it in that terminal")
                .disabled_because(busy)
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
                .disabled_because(busy)
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
        // Who'd get what's written is worth knowing before it's written.
        let reaches = match listening {
            Some(agent) if answering => Some(format!("{agent} is waiting")),
            Some(agent) => Some(format!("{agent} reads what you send here")),
            None if notes > 0 || state.unheard => Some("No agent is reading this".to_owned()),
            None => None,
        };
        let asks = match mine {
            Some(mine) => Some(answered_words(mine, page)),
            None => answering.then(|| asked_words(page)),
        };
        let words = match newer {
            Some(newer) => format!("Version {newer} is out"),
            None => (asks.into_iter().chain(reaches))
                .collect::<Vec<_>>()
                .join(". "),
        };
        // Where the row is short, what's there says less rather than running over the tabs.
        let short = self.brief.room.get() < STATUS_NEEDS;
        let and_send = notes > 0;
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
            let mut rows: Vec<usize> = (state.notes.iter())
                .filter_map(|note| self.row_of_note(&note.to))
                .collect();
            rows.sort_unstable();
            buttons.push(
                button(
                    "brief-unsent",
                    match short {
                        true => format!("{notes} not sent"),
                        false => format!("{} not sent", plural(notes, "comment")),
                    },
                )
                .small()
                .style(ButtonStyle::Ghost)
                .tint(theme.blue)
                .tooltip("Go to the next comment")
                .on_click(cx.listener(move |this, _, _, cx| {
                    // From where the reading is, and round to the first after the last.
                    let list = &this.brief.list;
                    let here = list.logical_scroll_top().item_ix;
                    let next = (rows.iter().find(|row| **row > here)).or(rows.first());
                    if let Some(row) = next {
                        list.scroll_to(gpui::ListOffset {
                            item_ix: *row,
                            offset_in_item: gpui::Pixels::ZERO,
                        });
                        cx.notify();
                    }
                }))
                .into_any_element(),
            );
            buttons.push(
                button("brief-send", if notes == 1 { "Send" } else { "Send all" })
                    .small()
                    .style(ButtonStyle::Outline)
                    .tooltip(format!("Send to {}", self.brief_reaches(state)))
                    .disabled_because(self.brief_notes_busy())
                    .on_click(cx.listener(|this, _, _, cx| this.send_brief_talk(cx)))
                    .into_any_element(),
            );
        }
        if answering {
            match mine {
                // That's this one seen to, so what's next is what's likely.
                Some(_) => buttons.extend(self.next_brief(true, cx).map(|next| {
                    button("brief-next-answered", "Next brief")
                        .small()
                        .style(ButtonStyle::Gray)
                        .tooltip_for(format!("Go on to {}", next.title), &NextBrief)
                        .on_click(cx.listener(|this, _, _, cx| this.hop_to_brief(true, cx)))
                        .into_any_element()
                })),
                // Briefs takes an answer only from a person in a browser, not from a token, which
                // an agent could hold as well.
                None => {
                    let url = briefs::web_url(&page.id, page.version);
                    buttons.push(
                        button(
                            "brief-answer",
                            if short { "Answer" } else { "Answer on the web" },
                        )
                        .small()
                        .icon(Icon::ArrowUpRight)
                        .style(ButtonStyle::Gray)
                        .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url)))
                        .into_any_element(),
                    );
                }
            }
        }
        Some(
            h_flex()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .pl(dp(16.))
                .pb(dp(11.))
                .gap(dp(8.))
                .justify_end()
                .text_size(dp(12.))
                .text_color(theme.text_2)
                // Left out where it would only be cut down to a letter or two.
                .when(!short, |el| {
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
