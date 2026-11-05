//! Saying something back: notes written against blocks and passages and sent together, like notes
//! given after a run-through, an answer to what the brief asks, and what others have said.
//!
//! Notes stay on this machine until they're sent, as a comment tells people. They go as ordinary
//! comments, anchored as the web anchors them, so whoever acts on them needs nothing new: an agent
//! told to check the comments finds them there.

use gpui::{AnyElement, Context, Div, Focusable as _, Window, div, prelude::*};

use super::keys::NextBrief;
use super::state::{BriefFold, BriefInput, BriefState, Writing};
use super::{Row, block_place, briefs};
use crate::backend::briefs::{BriefBlock, BriefBody, BriefPage, CommentTo, Note, Thread};
use crate::detail::Detail;
use crate::detail::pull_request::{card, tag};
use crate::theme::Theme;
use crate::ui::person::person_avatar;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{ButtonStyle, Icon, button, dp, h_flex, icon_button, markdown, v_flex};

/// How much of a paragraph a note quotes to say what it's on.
const MOST_QUOTED: usize = 400;

/// A note on `words` of `block`, by as much of them as it takes to find them again.
pub(super) fn passage(block: &BriefBlock, words: &str) -> CommentTo {
    CommentTo::Passage {
        block: *block.positions.start(),
        quote: words.chars().take(MOST_QUOTED).collect(),
    }
}

/// What a note written with the keys on `row` is about. A section is a row of a block that can be
/// the whole brief, so it's the section, by its first words, not the block.
fn row_target(row: Row, blocks: &[BriefBlock]) -> CommentTo {
    let whole = |at: usize| {
        (blocks.get(at)).map_or(CommentTo::Brief, |block| {
            CommentTo::Block(*block.positions.start())
        })
    };
    match row {
        Row::Text { at, from, .. } => {
            let block = blocks.get(at);
            let first = block.and_then(|block| match &block.body {
                BriefBody::Text(paragraphs) => paragraphs.get(from),
                _ => None,
            });
            match block.zip(first.filter(|first| !first.text.trim().is_empty())) {
                Some((block, first)) => passage(block, &first.text),
                None => whole(at),
            }
        }
        Row::Block(at) => whole(at),
        Row::Fold { first, .. } => whole(first),
        Row::Head | Row::End | Row::Since | Row::Unchanged(_) => CommentTo::Brief,
    }
}

impl Detail {
    /// Open what's shut of `fold`, or shut it, the reading staying where it is if rows come or go.
    pub(super) fn toggle_brief_fold(&mut self, fold: BriefFold, cx: &mut Context<Self>) {
        let place = self.brief_place_after(|brief| brief.toggle(fold));
        if let (BriefFold::Unchanged, Some(brief)) = (fold, self.page.brief()) {
            brief.top.set(place);
        }
        self.brief_list_changed(cx);
    }

    /// The rows' heights may have changed.
    pub(super) fn brief_list_changed(&mut self, cx: &mut Context<Self>) {
        self.brief.list.remeasure();
        cx.notify();
    }

    /// What a note written now would be about: the passage selected, or the block the keys are
    /// on, or the whole brief.
    pub(super) fn brief_note_target(&self) -> CommentTo {
        let page = (self.page.brief()).and_then(|brief| brief.shown.as_ref()?.as_ref().ok());
        let Some(page) = page else {
            return CommentTo::Brief;
        };
        if let (Some(quote), Some((id, _))) = (self.selected_brief_text(), self.note_range()) {
            let block = block_place(id, &page.id)
                .and_then(|(at, _)| page.blocks.get(at))
                .map(|block| *block.positions.start());
            if let Some(block) = block {
                return CommentTo::Passage { block, quote };
            }
        }
        // The paragraph clicked in, in its own words, which comes back to it wherever it moves.
        if let Some((id, (point, _))) = self.note_selection.as_ref()
            && let Some((at, from)) = block_place(id, &page.id)
            && let Some(block) = page.blocks.get(at)
            && let BriefBody::Text(paragraphs) = &block.body
            && let Some(paragraph) = paragraphs.get(from + point.0)
            && !paragraph.text.trim().is_empty()
        {
            return passage(block, &paragraph.text);
        }
        // Only where the keys went: what's on screen may not be what's being read, and a note on
        // the wrong paragraph says more than it meant to.
        let cursor = (self.page.brief()).and_then(|brief| brief.cursor);
        (cursor.and_then(|cursor| self.brief_rows().get(cursor).copied()))
            .map_or(CommentTo::Brief, |row| row_target(row, &page.blocks))
    }

    /// Start writing a note about `to`, in the margin beside it.
    pub(super) fn write_brief_note(
        &mut self,
        to: CommentTo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let placeholder = match to {
            CommentTo::Thread(_) => "Write a reply…",
            _ => "Write a comment…",
        };
        // What it's on is tinted from here, where a selection would hide that.
        self.note_selection = None;
        let row = self.row_of_note(&to);
        self.start_writing(Writing::Note(to), placeholder, window, cx);
        if let Some(row) = row {
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(super::talk::ROW_MEASURED)
                    .await;
                this.update(cx, |this, cx| {
                    // Only from elsewhere: where it's beside what's on screen, nothing moves.
                    let view = this.brief.list.viewport_bounds();
                    let shown = (this.brief.list.bounds_for_item(row))
                        .is_some_and(|row| row.intersects(&view));
                    if !shown {
                        this.brief.list.scroll_to_reveal_item(row);
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
    }

    pub(super) fn start_writing(
        &mut self,
        writing: Writing,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(version) = self.shown_version() else {
            return;
        };
        self.keep_brief_writing();
        let field = cx.new(|cx| TextInput::new(placeholder, true, cx).enter_submits());
        let listen = cx.subscribe_in(
            &field,
            window,
            |this, field, event, window, cx| match event {
                InputEvent::Submit => this.submit_writing(window, cx),
                InputEvent::Cancel => this.finish_writing(None, window, cx),
                InputEvent::Changed => {
                    if let Some(input) = &mut this.brief.input {
                        input.written = field.read(cx).text().to_owned();
                    }
                    // It wraps, so its row can grow as it's written.
                    this.brief.list.remeasure();
                    cx.notify();
                }
            },
        );
        window.focus(&field.focus_handle(cx), cx);
        self.brief.input = Some(BriefInput {
            field,
            writing,
            version,
            written: String::new(),
            _listen: listen,
        });
        self.brief_list_changed(cx);
    }

    /// What's being written, if anything is.
    pub(super) fn brief_writing(&self) -> Option<&Writing> {
        self.brief.input.as_ref().map(|input| &input.writing)
    }

    /// Keep a comment being written as one not sent, as its field is going away without it being
    /// finished: whoever wrote it went elsewhere, and their words shouldn't go with it.
    pub(in crate::detail) fn keep_brief_writing(&mut self) {
        let Some(input) = self.brief.input.take() else {
            return;
        };
        let Writing::Note(to) = &input.writing else {
            return;
        };
        let body = input.written.trim();
        if body.is_empty() {
            return;
        }
        let (id, version) = &input.version;
        match briefs::add_note(id, *version, to, body) {
            Ok(notes) => {
                let here = |page: &BriefPage| page.id == *id && page.version == *version;
                if let Some(brief) =
                    (self.page.brief_mut()).filter(|brief| brief.page().is_some_and(here))
                {
                    brief.notes = notes;
                }
            }
            Err(err) => tracing::warn!("couldn't keep the comment being written: {err:#}"),
        }
    }

    /// Finish what's being written, as Enter does.
    pub(super) fn submit_writing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let body = (self.brief.input.as_ref()).map(|input| input.written.trim().to_owned());
        self.finish_writing(body, window, cx);
    }

    /// Add `said` to what's being written.
    pub(super) fn add_to_writing(
        &mut self,
        said: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = &self.brief.input {
            input.field.update(cx, |input, cx| {
                let joined = match input.text().trim() {
                    "" => said.to_owned(),
                    written => format!("{written} {said}"),
                };
                input.set_text(joined, cx);
            });
            // The button pressed goes once what it says is in, and the keys would go with it.
            window.focus(&input.field.focus_handle(cx), cx);
        }
        cx.notify();
    }

    /// Finish what's being written: `None` drops it. An empty note is no note. The keys go back
    /// to the brief.
    pub(super) fn finish_writing(
        &mut self,
        body: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = self.brief.input.take();
        window.focus(&self.focus, cx);
        self.brief_list_changed(cx);
        let (Some(input), Some(body)) = (input, body) else {
            return;
        };
        let (id, version) = input.version;
        let writing = input.writing;
        match writing {
            Writing::Note(_) if body.is_empty() => {}
            Writing::Note(to) => self.keep_brief_note(id, version, to, body, cx),
            Writing::Shortcut { editing, .. } => self.save_brief_shortcut(editing, body, cx),
        }
    }

    /// The end of the brief: that it ends, what's next, and what was said on other versions.
    pub(super) fn brief_end(&self, state: &BriefState, cx: &Context<Self>) -> Div {
        let theme = Theme::get(cx);
        let version = (state.shown.as_ref())
            .and_then(|shown| shown.as_ref().ok())
            .map_or(0, |page| page.version);
        v_flex()
            .gap(dp(8.))
            .pt(dp(12.))
            .border_t_1()
            .border_color(theme.border_3)
            .child(
                h_flex()
                    .gap(dp(8.))
                    .text_size(dp(12.))
                    .text_color(theme.text_3)
                    .children(self.next_brief(true, cx).map(|next| {
                        button("brief-next", format!("Next: {}", next.title))
                            .small()
                            .style(ButtonStyle::Outline)
                            .tooltip_for(format!("Go on to {}", next.name), &NextBrief)
                            .on_click(cx.listener(|this, _, _, cx| this.hop_to_brief(true, cx)))
                    })),
            )
            // What was said on other versions, which has nowhere on this one to go.
            .children(
                (state.activity.threads.iter())
                    .filter(|thread| !super::margin::said_on(thread, version))
                    .map(|thread| {
                        v_flex()
                            .child(super::small_line(
                                &format!("On version {}", thread.version.unwrap_or_default()),
                                theme.text_3,
                            ))
                            .child(self.brief_thread(thread, state, cx))
                    }),
            )
    }

    /// What someone said, and the answers to it, in a card as a pull request's conversation has.
    pub(super) fn brief_thread(
        &self,
        thread: &Thread,
        state: &BriefState,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        let theme = Theme::get(cx);
        let said =
            |at: usize, name: &str, agent: Option<&str>, body: &str, end: Option<AnyElement>| {
                let id = format!("brief-said-{}-{at}", thread.id);
                v_flex()
                    .px(dp(10.))
                    .py(dp(8.))
                    .gap(dp(2.))
                    .when(at > 0, |el| el.border_t_1().border_color(theme.border_3))
                    .child(
                        h_flex()
                            .min_h(dp(20.))
                            .gap(dp(6.))
                            .text_size(dp(12.))
                            .text_color(theme.text_2)
                            .child(person_avatar(name, 16., theme))
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.text_1)
                                    .child(name.to_owned()),
                            )
                            .children(agent.map(|agent| div().child(agent.to_owned())))
                            .child(div().flex_1())
                            .children(end),
                    )
                    .child(markdown::render(&id, &markdown::parse(body), None, theme))
            };
        let resolved = thread.resolved.is_some();
        let quote = super::margin::quote_of(thread).map(str::to_owned);
        let gone = quote.clone().filter(|quote| {
            (thread.anchor.block()).is_some_and(|at| super::margin::quote_is_gone(state, at, quote))
        });
        let replying = (self.brief_writing())
            .filter(|writing| {
                matches!(writing, Writing::Note(CommentTo::Thread(to)) if *to == thread.id)
            })
            .and_then(|writing| self.talk_composer(writing, true, cx));
        // What's next for it, in the corner: that it's done with, or the way to answer.
        let end = match (resolved, &replying) {
            (true, _) => Some(tag(theme.safe).child("Resolved").into_any_element()),
            (false, Some(_)) => None,
            (false, None) => {
                let parent = thread.id.clone();
                Some(
                    button(
                        gpui::SharedString::from(format!("brief-reply-{}", thread.id)),
                        "Reply",
                    )
                    .small()
                    .style(ButtonStyle::Ghost)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.write_brief_note(CommentTo::Thread(parent.clone()), window, cx)
                    }))
                    .into_any_element(),
                )
            }
        };
        card(theme)
            .id(gpui::SharedString::from(format!(
                "brief-thread-{}",
                thread.id
            )))
            .when(resolved, |el| el.opacity(0.72).hover(|el| el.opacity(1.)))
            .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                this.hover_said(quote.clone(), *hovering, cx)
            }))
            .children(gone.map(|quote| {
                div()
                    .px(dp(10.))
                    .pt(dp(8.))
                    .child(super::margin::quote_line(&quote, theme))
            }))
            .child(said(
                0,
                &thread.actor.name,
                thread.actor.agent.as_deref(),
                &thread.body,
                end,
            ))
            .children(thread.replies.iter().enumerate().map(|(ix, reply)| {
                said(
                    ix + 1,
                    &reply.actor.name,
                    reply.actor.agent.as_deref(),
                    &reply.body,
                    None,
                )
            }))
            .children(
                (state.notes.iter())
                    .filter(|note| matches!(&note.to, CommentTo::Thread(to) if *to == thread.id))
                    .map(|note| self.brief_note(note, true, state, cx)),
            )
            .children(replying)
    }

    /// A comment not sent yet, in the blue of what's not pushed, and easy to throw away. `nested`
    /// in a thread's card, it has no card of its own.
    pub(super) fn brief_note(
        &self,
        note: &Note,
        nested: bool,
        state: &BriefState,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        let theme = Theme::get(cx);
        let id = note.id;
        let quote = match &note.to {
            CommentTo::Passage { quote, .. } => Some(quote.clone()),
            _ => None,
        };
        let gone = match &note.to {
            CommentTo::Passage { block, quote } => {
                super::margin::quote_is_gone(state, *block, quote).then(|| quote.clone())
            }
            _ => None,
        };
        v_flex()
            .id(("brief-note", id as u64))
            .pl(dp(10.))
            .pr(dp(6.))
            .py(dp(6.))
            .gap(dp(2.))
            .map(|el| match nested {
                true => el.border_t_1().border_color(theme.border_3),
                false => el
                    .rounded(dp(6.))
                    .border_1()
                    .border_color(theme.blue.opacity(0.35))
                    .bg(theme.blue.opacity(0.05)),
            })
            .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                this.hover_said(quote.clone(), *hovering, cx)
            }))
            .children(gone.map(|quote| super::margin::quote_line(&quote, theme)))
            // On a line with what it says, until that needs the line to itself.
            .child(
                h_flex()
                    .flex_wrap()
                    .items_start()
                    .gap(dp(6.))
                    .child(
                        div()
                            .flex_auto()
                            .min_w_0()
                            .text_size(dp(13.))
                            .line_height(dp(20.))
                            .text_color(theme.text_1)
                            .child(note.body.clone()),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .ml_auto()
                            .gap(dp(6.))
                            .child(tag(theme.blue).child("Not sent"))
                            .child(
                                icon_button(("brief-note-drop", id as u64), Icon::Cross)
                                    .small()
                                    .tooltip("Discard")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.remove_brief_note(id, cx)
                                    })),
                            ),
                    ),
            )
    }
}

/// What a note is on, as the reader would say it.
pub(super) fn target_words(to: &CommentTo, page: &BriefPage) -> String {
    let shortened = |words: &str| match words.char_indices().nth(60) {
        Some((end, _)) => format!("{}…", words.get(..end).unwrap_or(words)),
        None => words.to_owned(),
    };
    match to {
        CommentTo::Brief => "the whole brief".to_owned(),
        CommentTo::Thread(_) => "that thread".to_owned(),
        CommentTo::Passage { quote, .. } => format!("“{}”", shortened(quote)),
        CommentTo::Block(position) => {
            let block = (page.blocks.iter()).find(|block| block.positions.contains(position));
            match block.map(|block| &block.body) {
                Some(BriefBody::Code(code)) => format!("the code from {}", code.path),
                Some(BriefBody::Pictures(_)) => "these pictures".to_owned(),
                Some(BriefBody::Files { .. }) => "this list of files".to_owned(),
                Some(BriefBody::Text(blocks)) => {
                    let first = blocks
                        .first()
                        .map_or("this part", |block| block.text.as_str());
                    format!("“{}”", shortened(first))
                }
                _ => "this part".to_owned(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Row, row_target};
    use crate::backend::briefs::{BriefBlock, BriefBody, CommentTo};
    use crate::ui::markdown;

    #[test]
    fn a_note_from_a_section_is_on_that_section_not_its_block() {
        let blocks = [BriefBlock {
            positions: 3..=3,
            fresh: false,
            detail: None,
            body: BriefBody::Text(markdown::parse("The opening\n\n## Why 30 days\n\nBecause")),
        }];
        let row = Row::Text {
            at: 0,
            from: 1,
            to: 3,
        };
        assert!(
            matches!(
                row_target(row, &blocks),
                CommentTo::Passage { block: 3, quote } if quote == "Why 30 days"
            ),
            "the section's own first words say where it goes, as the block's would say the top"
        );
    }
}
