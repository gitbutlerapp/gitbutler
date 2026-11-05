//! Saying something back: notes written against blocks and passages and sent together, like notes
//! given after a run-through, an answer to what the brief asks, and what others have said.
//!
//! Notes stay on this machine until they're sent, as a comment tells people. They go as ordinary
//! comments, anchored as the web anchors them, so whoever acts on them needs nothing new: an agent
//! told to check the comments finds them there.

use gpui::{AnyElement, Context, Div, Focusable as _, Window, div, prelude::*};

use super::keys::{NextBrief, WriteNote};
use super::{Row, block_place, briefs, run};
use crate::backend::briefs::{BriefBlock, BriefBody, BriefPage, CommentTo, Note, Thread};
use crate::detail::Detail;
use crate::detail::page::{BriefFold, BriefState, Writing};
use crate::theme::Theme;
use crate::ui::person::person_avatar;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{ButtonStyle, Icon, button, dp, h_flex, icon_button, markdown, v_flex};

/// How much of a paragraph a note quotes to say what it's on.
const MOST_QUOTED: usize = 400;

/// What a thread's about in the version's blocks, drawn under `block`, for the version it was
/// said on. Those of others say what became of them at the top.
fn is_about(thread: &Thread, block: &BriefBlock, version: u32) -> bool {
    thread.version.is_none_or(|said_on| said_on == version)
        && (thread.anchor.block()).is_some_and(|position| block.positions.contains(&position))
}

fn note_is_about(note: &Note, block: &BriefBlock) -> bool {
    match &note.to {
        CommentTo::Block(position)
        | CommentTo::Passage {
            block: position, ..
        } => block.positions.contains(position),
        CommentTo::Brief | CommentTo::Thread(_) => false,
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
                Some((block, first)) => CommentTo::Passage {
                    block: *block.positions.start(),
                    quote: first.text.chars().take(MOST_QUOTED).collect(),
                },
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
        self.brief_list.remeasure();
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
            return CommentTo::Passage {
                block: *block.positions.start(),
                quote: paragraph.text.chars().take(MOST_QUOTED).collect(),
            };
        }
        // Only where the keys went: what's on screen may not be what's being read, and a note on
        // the wrong paragraph says more than it meant to.
        let cursor = (self.page.brief()).and_then(|brief| brief.cursor);
        (cursor.and_then(|cursor| self.brief_rows().get(cursor).copied()))
            .map_or(CommentTo::Brief, |row| row_target(row, &page.blocks))
    }

    /// Start writing a note about `to`, in the bar, where it's always in view and nothing moves.
    pub(super) fn write_brief_note(
        &mut self,
        to: CommentTo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let placeholder = match to {
            CommentTo::Thread(_) => "Write a reply",
            _ => "What should change?",
        };
        self.start_writing(Writing::Note(to), placeholder, window, cx);
    }

    /// Ask why this version is being held, as a hold no one explains leaves an agent guessing.
    pub(super) fn ask_why_hold(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_writing(Writing::WhyHold, "What should change?", window, cx);
    }

    fn start_writing(
        &mut self,
        writing: Writing,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| TextInput::new(placeholder, false, cx));
        let listen = cx.subscribe_in(
            &input,
            window,
            |this, input, event, window, cx| match event {
                InputEvent::Submit => {
                    let body = input.read(cx).text().trim().to_owned();
                    this.finish_writing(Some(body), window, cx);
                }
                InputEvent::Cancel => this.finish_writing(None, window, cx),
                InputEvent::Changed => {}
            },
        );
        window.focus(&input.focus_handle(cx), cx);
        self.brief_input = Some((input, listen));
        if let Some(brief) = self.page.brief_mut() {
            brief.writing = Some(writing);
        }
        cx.notify();
    }

    /// Add `said` to what's being written.
    pub(super) fn add_to_writing(
        &mut self,
        said: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((input, _)) = &self.brief_input {
            input.update(cx, |input, cx| {
                let joined = match input.text().trim() {
                    "" => said.to_owned(),
                    written => format!("{written} {said}"),
                };
                input.set_text(joined, cx);
            });
            // The button pressed goes once what it says is in, and the keys would go with it.
            window.focus(&input.focus_handle(cx), cx);
        }
        cx.notify();
    }

    /// Finish what's being written: `None` drops it. An empty note is no note, and an empty
    /// reason holds without one. The keys go back to the brief.
    pub(super) fn finish_writing(
        &mut self,
        body: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.brief_input = None;
        let writing = self.page.brief_mut().and_then(|brief| brief.writing.take());
        window.focus(&self.focus, cx);
        cx.notify();
        let (Some(writing), Some(body), Some((id, version))) =
            (writing, body, self.shown_version())
        else {
            return;
        };
        match writing {
            Writing::Note(_) if body.is_empty() => {}
            Writing::Note(to) => self.keep_brief_note(id, version, to, body, cx),
            Writing::WhyHold => {
                let key = (id.clone(), version);
                cx.spawn(async move |this, cx| {
                    let kept = match body.is_empty() {
                        true => Ok(Vec::new()),
                        false => {
                            run(move || briefs::add_note(&id, version, &CommentTo::Brief, &body))
                                .await
                        }
                    };
                    this.update(cx, |this, cx| {
                        let notes = match kept {
                            Ok(notes) => notes,
                            // A hold without the why that was written says less than was meant.
                            Err(err) => {
                                return (this.store).update(cx, |store, cx| {
                                    store.snack(format!("Couldn't keep that: {err:#}"), cx)
                                });
                            }
                        };
                        if this.shown_version().as_ref() != Some(&key) {
                            return;
                        }
                        if !notes.is_empty() {
                            this.took_brief_notes(notes, cx);
                        }
                        this.send_brief_talk(Some(Some("hold".to_owned())), cx);
                    })
                    .ok();
                })
                .detach()
            }
        }
    }

    /// A block with what's said about it under it, and the way to say something.
    pub(super) fn brief_block(
        &self,
        at: usize,
        block: &BriefBlock,
        state: &BriefState,
        body: AnyElement,
        words: Option<&[markdown::Block]>,
        cx: &Context<Self>,
    ) -> Div {
        let theme = Theme::get(cx);
        let to = CommentTo::Block(*block.positions.start());
        let write = icon_button(("brief-note", at), Icon::Edit)
            .small()
            .tooltip_for("Write a note on this", &WriteNote)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.write_brief_note(to.clone(), window, cx)
            }));
        // A passage's under the row it's in. The rest, and one whose words are gone, go under the
        // last of the block's rows.
        let version = (state.shown.as_ref())
            .and_then(|shown| shown.as_ref().ok())
            .map_or(0, |page| page.version);
        let all = match &block.body {
            BriefBody::Text(paragraphs) => paragraphs.as_slice(),
            _ => &[],
        };
        let has = |words: &[markdown::Block], quote: &str| {
            words.iter().any(|word| word.text.contains(quote))
        };
        let here = |quote: Option<&str>| {
            let last = words.is_none_or(|words| {
                words.last().map(|last| last as *const _) == all.last().map(|last| last as *const _)
            });
            match quote {
                Some(quote) if has(all, quote) => words.is_some_and(|words| has(words, quote)),
                _ => last,
            }
        };
        let threads = (state.activity.threads.iter()).filter(|thread| {
            is_about(thread, block, version)
                && here(match &thread.anchor {
                    briefs::Anchor::Text { quote, .. } => Some(quote),
                    _ => None,
                })
        });
        let notes = (state.notes.iter()).filter(|note| {
            note_is_about(note, block)
                && here(match &note.to {
                    CommentTo::Passage { quote, .. } => Some(quote),
                    _ => None,
                })
        });
        let group = super::row_group(at);
        div()
            .relative()
            .child(body)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right(dp(-22.))
                    .opacity(0.)
                    .group_hover(group, |el| el.opacity(1.))
                    .child(write),
            )
            .children(threads.map(|thread| self.brief_thread(thread, &state.notes, theme, cx)))
            .children(notes.map(|note| self.brief_note(note, cx)))
    }

    /// The end of the brief: that it ends, what's said about the whole of it, and the way to say
    /// something about the whole of it.
    pub(super) fn brief_end(&self, state: &BriefState, cx: &Context<Self>) -> Div {
        let theme = Theme::get(cx);
        let version = (state.shown.as_ref())
            .and_then(|shown| shown.as_ref().ok())
            .map_or(0, |page| page.version);
        let on_no_block = |thread: &&Thread| {
            thread.anchor.block().is_none() && thread.version.is_none_or(|said| said == version)
        };
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
                    .child(
                        button("brief-note-whole", "Note on the whole brief")
                            .small()
                            .style(ButtonStyle::Outline)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.write_brief_note(CommentTo::Brief, window, cx)
                            })),
                    )
                    .children(self.next_brief(true, cx).map(|next| {
                        button("brief-next", format!("Next: {}", next.title))
                            .small()
                            .style(ButtonStyle::Outline)
                            .tooltip_for(format!("Go on to {}", next.name), &NextBrief)
                            .on_click(cx.listener(|this, _, _, cx| this.hop_to_brief(true, cx)))
                    })),
            )
            .children(
                (state.activity.threads.iter())
                    .filter(on_no_block)
                    .map(|thread| self.brief_thread(thread, &state.notes, theme, cx)),
            )
            // What was said on other versions, which has nowhere on this one to go.
            .children(
                (state.activity.threads.iter())
                    .filter(|thread| thread.version.is_some_and(|said| said != version))
                    .map(|thread| {
                        v_flex()
                            .child(super::small_line(
                                &format!("On version {}", thread.version.unwrap_or_default()),
                                theme.text_3,
                            ))
                            .child(self.brief_thread(thread, &state.notes, theme, cx))
                    }),
            )
            .children(
                (state.notes.iter())
                    .filter(|note| matches!(note.to, CommentTo::Brief))
                    .map(|note| self.brief_note(note, cx)),
            )
    }

    /// What someone said, and the answers to it.
    fn brief_thread(
        &self,
        thread: &Thread,
        notes: &[Note],
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Div {
        let said = |name: &str, agent: Option<&str>, body: &str| {
            h_flex()
                .items_start()
                .gap(dp(8.))
                .child(person_avatar(name, 18., theme))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            h_flex()
                                .gap(dp(6.))
                                .text_size(dp(12.))
                                .text_color(theme.text_2)
                                .child(name.to_owned())
                                .children(agent.map(|agent| {
                                    div().text_color(theme.text_3).child(agent.to_owned())
                                })),
                        )
                        .child(
                            div()
                                .text_size(dp(13.))
                                .line_height(dp(19.))
                                .text_color(theme.text_1)
                                .child(body.to_owned()),
                        ),
                )
        };
        let resolved = thread.resolved.is_some();
        v_flex()
            .mt(dp(8.))
            .p(dp(8.))
            .gap(dp(6.))
            .rounded(dp(6.))
            .border_1()
            .border_color(theme.border_3)
            .when(resolved, |el| el.opacity(0.6))
            .child(said(
                &thread.actor.name,
                thread.actor.agent.as_deref(),
                &thread.body,
            ))
            .children(thread.replies.iter().map(|reply| {
                div().pl(dp(26.)).child(said(
                    &reply.actor.name,
                    reply.actor.agent.as_deref(),
                    &reply.body,
                ))
            }))
            .children(
                (notes.iter())
                    .filter(|note| matches!(&note.to, CommentTo::Thread(to) if *to == thread.id))
                    .map(|note| self.brief_note(note, cx)),
            )
            .child(match resolved {
                true => div()
                    .text_size(dp(11.))
                    .text_color(theme.text_3)
                    .child("Resolved")
                    .into_any_element(),
                false => {
                    let parent = thread.id.clone();
                    button(
                        gpui::SharedString::from(format!("brief-reply-{}", thread.id)),
                        "Reply",
                    )
                    .small()
                    .style(ButtonStyle::Ghost)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.write_brief_note(CommentTo::Thread(parent.clone()), window, cx)
                    }))
                    .into_any_element()
                }
            })
    }

    /// A note not sent yet: marked as only here, and easy to throw away.
    fn brief_note(&self, note: &Note, cx: &Context<Self>) -> Div {
        let theme = Theme::get(cx);
        let id = note.id;
        let quote = match &note.to {
            CommentTo::Passage { quote, .. } => Some(quote.clone()),
            _ => None,
        };
        h_flex()
            .mt(dp(8.))
            .pl(dp(10.))
            .pr(dp(4.))
            .py(dp(4.))
            .gap(dp(8.))
            .rounded(dp(4.))
            .border_l_2()
            .border_color(theme.pop)
            .bg(theme.fill_gray.opacity(0.06))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .children(quote.map(|quote| {
                        div()
                            .truncate()
                            .text_size(dp(11.))
                            .text_color(theme.text_3)
                            .child(format!("“{quote}”"))
                    }))
                    .child(
                        div()
                            .text_size(dp(13.))
                            .line_height(dp(19.))
                            .text_color(theme.text_1)
                            .child(note.body.clone()),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(dp(11.))
                    .text_color(theme.text_3)
                    .child("Not sent"),
            )
            .child(
                icon_button(("brief-note-drop", id as u64), Icon::Cross)
                    .small()
                    .tooltip("Throw this note away")
                    .on_click(cx.listener(move |this, _, _, cx| this.remove_brief_note(id, cx))),
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
            only_new: None,
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
