//! The margin beside a brief's words, where what's said about them goes: comments and threads level
//! with what they're about, the place to write one, and shortcuts to the notes said most. A row is
//! as tall as the taller of its words and its margin, so nothing overlaps. Where the tab hasn't the
//! room, they go under the words instead.

use std::borrow::Cow;
use std::ops::Range;

use gpui::{
    AnyElement, Context, CursorStyle, Div, MouseButton, MouseDownEvent, Pixels, Point, Window, div,
    prelude::*,
};

use super::keys::WriteNote;
use super::{Row, briefs};
use crate::backend::briefs::{BriefBlock, BriefBody, CommentTo, Thread};
use crate::detail::Detail;
use crate::detail::page::{BriefState, Writing};
use crate::overlays::MenuItem;
use crate::theme::Theme;
use crate::ui::markdown::Tint;
use crate::ui::{Button, ButtonStyle, Icon, button, dp, h_flex, icon_button, markdown, v_flex};

/// The notes a click leaves until someone changes them, as what's said most about generated text.
const STOCK_SHORTCUTS: [&str; 3] = ["Cut this", "Shorter", "Why?"];

/// How far the margin stands from the words.
const MARGIN_GAP: f32 = 32.;

/// The least a margin can be and still read: the row's padding either side, the gap and the column.
const MARGIN_NEEDS: f32 = 24. + MARGIN_GAP + 190. + 24.;

/// How much of a shortcut a button shows.
const SHORTCUT_SHOWN: usize = 16;

/// What goes with something drawn of a brief, for what's said about it.
pub(super) struct Beside<'a> {
    /// Tells this one's controls from all the others'.
    pub key: usize,
    /// How wide what's drawn is.
    pub wide: f32,
    pub spot: Spot<'a>,
    /// What a comment from here would be about.
    pub to: CommentTo,
    /// Whether something's selected here, which keeps the way to comment on it in view.
    pub held: bool,
    /// Whether what's said can run on past what's drawn, without moving what's under it.
    pub floats: bool,
}

/// What something said goes beside.
pub(super) enum Spot<'a> {
    /// The brief as a whole, beside its head.
    Brief,
    /// All of a block that isn't words.
    Block(&'a BriefBlock),
    /// A text block's paragraphs, by index into all of them.
    Words(&'a BriefBlock, Range<usize>),
}

impl Spot<'_> {
    /// Whether something said about `block`, by its position, and in `quote`'s words, goes here.
    fn takes(&self, block: Option<usize>, quote: Option<&str>) -> bool {
        match (self, block) {
            (Spot::Brief, None) => true,
            (Spot::Block(on), Some(block)) => on.positions.contains(&block),
            (Spot::Words(on, paragraphs), Some(block)) => {
                on.positions.contains(&block) && paragraphs.contains(&paragraph_of(on, quote))
            }
            _ => false,
        }
    }
}

/// The paragraph of `block` that `quote` starts in. Without one, or once its words are gone, the
/// first, which is where the block starts rather than where a long one ends.
fn paragraph_of(block: &BriefBlock, quote: Option<&str>) -> usize {
    let BriefBody::Text(paragraphs) = &block.body else {
        return 0;
    };
    let first = quote.and_then(|quote| quote.lines().map(str::trim).find(|line| !line.is_empty()));
    first
        .and_then(|first| (paragraphs.iter()).position(|paragraph| paragraph.text.contains(first)))
        .unwrap_or(0)
}

/// Whether `quote`'s words are still in the block at position `at` of what's shown, whose tint then
/// says what something's about without saying it again.
pub(super) fn quote_is_gone(state: &BriefState, at: usize, quote: &str) -> bool {
    let block = (state.page())
        .and_then(|page| (page.blocks.iter()).find(|block| block.positions.contains(&at)));
    let Some(BriefBody::Text(paragraphs)) = block.map(|block| &block.body) else {
        return true;
    };
    !(paragraphs.iter()).any(|paragraph| !quoted_in(&paragraph.text, quote).is_empty())
}

/// What a note is about, as a place: the block, if any, and the words.
fn place_of(to: &CommentTo) -> Option<(Option<usize>, Option<&str>)> {
    match to {
        CommentTo::Brief => Some((None, None)),
        CommentTo::Block(block) => Some((Some(*block), None)),
        CommentTo::Passage { block, quote } => Some((Some(*block), Some(quote))),
        CommentTo::Thread(_) => None,
    }
}

fn quote_of(thread: &Thread) -> Option<&str> {
    match &thread.anchor {
        briefs::Anchor::Text { quote, .. } => Some(quote),
        _ => None,
    }
}

/// Where `quote` shows in `text`, a line of it at a time, as a selection runs across paragraphs.
fn quoted_in(text: &str, quote: &str) -> Vec<Range<usize>> {
    (quote.lines().map(str::trim))
        .filter(|line| !line.is_empty())
        .filter_map(|line| text.find(line).map(|at| at..at + line.len()))
        .collect()
}

/// `words` with what `quotes` say tinted, each with its style.
fn tinted<'a>(
    words: Cow<'a, [markdown::Block]>,
    quotes: &[(&str, markdown::Style)],
) -> Cow<'a, [markdown::Block]> {
    if quotes.is_empty() {
        return words;
    }
    let mut words = words.into_owned();
    for paragraph in &mut words {
        for (quote, style) in quotes {
            let found = quoted_in(&paragraph.text, quote);
            paragraph
                .styles
                .extend(found.into_iter().map(|range| (range, *style)));
        }
    }
    Cow::Owned(words)
}

impl Detail {
    /// Remember how wide the tab is, in design pixels, for its rows to go by. Here, as asking the
    /// list while it's drawing a row can't be done.
    pub(super) fn measure_brief_room(&self, window: &Window) {
        let wide = self.brief_list.viewport_bounds().size.width;
        // Not drawn yet, its rows lay out for the last width rather than moving a frame later.
        if wide <= Pixels::ZERO {
            return;
        }
        let room = wide / dp(1.).to_pixels(window.rem_size());
        if room != self.brief_room.get() {
            self.brief_room.set(room);
            // What the list measured was for another width, which it only knows from drawing.
            self.brief_list.remeasure();
            window.request_animation_frame();
        }
    }

    /// Whether what's `wide` leaves the margin the room to go beside it.
    pub(super) fn margin_beside(&self, wide: f32) -> bool {
        self.brief_room.get() >= wide + MARGIN_NEEDS
    }

    /// `main`, with what's said about it beside it or under it, and the way to say something.
    pub(super) fn with_talk(
        &self,
        main: AnyElement,
        beside: Beside<'_>,
        state: &BriefState,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Beside {
            key,
            wide,
            spot,
            to,
            held,
            floats,
        } = beside;
        let theme = Theme::get(cx);
        let writing = self.brief_writing().is_some();
        let pointed = self.brief_hovered == Some(key) && !writing;
        let shows_controls = !writing && (pointed || held);
        let cards = self.talk_cards(&spot, state, cx);
        // Pointing at it lasts until other words are pointed at, for it to last the way over to
        // what shows beside it, and over what's beside it from the paragraph before.
        let pointing = div()
            .id(("brief-words", key))
            .when(matches!(spot, Spot::Words(..)), |el| {
                el.cursor(CursorStyle::IBeam)
            })
            .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                if *hovering && this.brief_hovered != Some(key) {
                    this.brief_hovered = Some(key);
                    cx.notify();
                }
            }));
        if self.margin_beside(wide) {
            // The tint reaches past the words' edge, so the words themselves stay put.
            let words = pointing
                .mx(dp(-6.))
                .px(dp(6.))
                .rounded(dp(4.))
                .when(pointed && matches!(spot, Spot::Words(..)), |el| {
                    el.bg(theme.hover())
                })
                .child(main);
            return h_flex()
                .relative()
                .w_full()
                .items_start()
                .child(div().flex_none().w(dp(wide)).child(words))
                // In the gap, which nothing beside the words ever covers.
                .children(shows_controls.then(|| {
                    div()
                        .absolute()
                        .top_0()
                        .left(dp(wide + 6.))
                        .child(self.talk_write_button(key, to.clone(), cx))
                }))
                .child(
                    v_flex()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .ml(dp(MARGIN_GAP))
                        .gap(dp(6.))
                        // Taking no height, it runs on past the words without moving them.
                        .when(floats, |el| el.h_0())
                        .children(cards)
                        // With words selected, what's said most is a click away. It takes no
                        // room, so that selecting moves nothing.
                        .children((held && !writing).then(|| {
                            div().relative().h_0().child(
                                self.talk_shortcuts(key, to.clone(), cx)
                                    .absolute()
                                    .left_0()
                                    .right_0(),
                            )
                        })),
                )
                .into_any_element();
        }
        v_flex()
            .relative()
            .child(pointing.child(main))
            .children(shows_controls.then(|| {
                div()
                    .absolute()
                    .top_0()
                    .right(dp(-23.))
                    .child(self.talk_write_button(key, to, cx))
            }))
            .children(cards.into_iter().map(|card| div().mt(dp(8.)).child(card)))
            .into_any_element()
    }

    /// Whether what's beside paragraphs `drawn` of `row` can run on past them. Not into something
    /// else beside the words, nor over what's wider than words, which reaches into the margin.
    pub(super) fn margin_floats(
        &self,
        drawn: Range<usize>,
        row: Row,
        next: Option<Row>,
        state: &BriefState,
    ) -> bool {
        let (Some(page), Row::Text { at, to, .. }) = (state.page(), row) else {
            return false;
        };
        let talk_in = |at: usize, paragraphs: Range<usize>| {
            page.blocks.get(at).is_some_and(|block| {
                let spot = Spot::Words(block, paragraphs);
                let version = page.version;
                (state.activity.threads.iter()).any(|thread| {
                    thread.version.is_none_or(|said_on| said_on == version)
                        && spot.takes(thread.anchor.block(), quote_of(thread))
                }) || (state.notes.iter()).any(|note| {
                    place_of(&note.to).is_some_and(|(block, quote)| spot.takes(block, quote))
                }) || self.brief_writing().is_some_and(|writing| match writing {
                    Writing::Note(to) | Writing::Shortcut { to, .. } => {
                        place_of(to).is_some_and(|(block, quote)| spot.takes(block, quote))
                    }
                })
            })
        };
        let Some(Row::Text {
            at: next_at,
            from: next_from,
            to: next_to,
        }) = next
        else {
            return false;
        };
        !talk_in(at, drawn.end..to) && !talk_in(next_at, next_from..next_to)
    }

    /// What's said about `spot`, and what's being written about it.
    fn talk_cards(
        &self,
        spot: &Spot<'_>,
        state: &BriefState,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let version = state.page().map_or(0, |page| page.version);
        let threads = (state.activity.threads.iter())
            .filter(|thread| {
                thread.version.is_none_or(|said_on| said_on == version)
                    && spot.takes(thread.anchor.block(), quote_of(thread))
            })
            .map(|thread| self.brief_thread(thread, state, cx).into_any_element());
        let notes = (state.notes.iter())
            .filter(|note| {
                place_of(&note.to).is_some_and(|(block, quote)| spot.takes(block, quote))
            })
            .map(|note| self.brief_note(note, false, state, cx).into_any_element());
        let writing = self
            .brief_writing()
            .and_then(|writing| match writing {
                Writing::Note(to) | Writing::Shortcut { to, .. } => {
                    let (block, quote) = place_of(to)?;
                    spot.takes(block, quote).then_some(writing)
                }
            })
            .and_then(|writing| self.talk_composer(writing, false, cx))
            .map(IntoElement::into_any_element);
        threads.chain(notes).chain(writing).collect()
    }

    /// What's written of a note or a shortcut, where it's going: the field, the shortcuts, which
    /// save the writing, and the way out. `nested`, in a thread's card, it has no card of its own.
    pub(super) fn talk_composer(
        &self,
        writing: &Writing,
        nested: bool,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let theme = Theme::get(cx);
        let input = self.brief_input.as_ref()?;
        let written = !input.written.trim().is_empty();
        let title = match writing {
            Writing::Shortcut { editing: None, .. } => Some("New shortcut"),
            Writing::Shortcut { .. } => Some("Edit shortcut"),
            _ => None,
        };
        let shortcuts =
            match writing {
                Writing::Note(to) => Some(
                    h_flex()
                        .flex_wrap()
                        .gap(dp(2.))
                        .children(self.brief_shortcuts(cx).into_iter().enumerate().map(
                            |(ix, said)| {
                                let text = said.clone();
                                self.shortcut_button(
                                    ("brief-composer-shortcut", ix),
                                    ix,
                                    &said,
                                    to.clone(),
                                    // Words of theirs are kept, with this after them.
                                    move |this, window, cx| match written {
                                        true => this.add_to_writing(&text, window, cx),
                                        false => {
                                            this.finish_writing(Some(text.clone()), window, cx)
                                        }
                                    },
                                    cx,
                                )
                            },
                        ))
                        .child(
                            icon_button("brief-composer-add-shortcut", Icon::Plus)
                                .small()
                                .tooltip(match written {
                                    true => "Keep as a shortcut",
                                    false => "Add a shortcut",
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.add_shortcut_from_writing(window, cx)
                                })),
                        ),
                ),
                _ => None,
            };
        let actions = h_flex()
            .justify_end()
            .gap(dp(4.))
            .child(
                button("brief-composer-cancel", "Cancel")
                    .small()
                    .style(ButtonStyle::Ghost)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.finish_writing(None, window, cx)),
                    ),
            )
            .child(
                button(
                    "brief-composer-keep",
                    if title.is_some() { "Save" } else { "Add" },
                )
                .small()
                .style(ButtonStyle::Gray)
                .disabled_because((!written).then_some("Nothing written yet"))
                .on_click(cx.listener(|this, _, window, cx| this.submit_writing(window, cx))),
            );
        Some(
            v_flex()
                .gap(dp(6.))
                .map(|el| match nested {
                    true => el
                        .px(dp(10.))
                        .py(dp(8.))
                        .border_t_1()
                        .border_color(theme.border_3),
                    false => el
                        .p(dp(8.))
                        .rounded(dp(6.))
                        .border_1()
                        .border_color(theme.focus_border())
                        .bg(theme.bg_1),
                })
                .children(title.map(|title| {
                    div()
                        .text_size(dp(12.))
                        .text_color(theme.text_2)
                        .child(title)
                }))
                .child(div().text_size(dp(13.)).child(input.field.clone()))
                .children(shortcuts)
                .child(actions),
        )
    }

    fn talk_write_button(&self, key: usize, to: CommentTo, cx: &Context<Self>) -> Button {
        icon_button(("brief-talk-write", key), Icon::Edit)
            .small()
            .tooltip_for("Write a comment", &WriteNote)
            .on_click(
                cx.listener(move |this, _, window, cx| {
                    this.write_brief_note(to.clone(), window, cx)
                }),
            )
    }

    /// What's said most about what's selected, a click away. It has a ground, as it can show over
    /// what's run on from the paragraph before.
    fn talk_shortcuts(&self, key: usize, to: CommentTo, cx: &Context<Self>) -> Div {
        let theme = Theme::get(cx);
        h_flex()
            .flex_wrap()
            .gap(dp(2.))
            .rounded(dp(6.))
            .bg(theme.bg_1)
            .children(
                self.brief_shortcuts(cx)
                    .into_iter()
                    .enumerate()
                    .map(|(ix, said)| {
                        let (about, text) = (to.clone(), said.clone());
                        self.shortcut_button(
                            ("brief-shortcut", key * 8 + ix),
                            ix,
                            &said,
                            to.clone(),
                            move |this, _, cx| this.leave_shortcut(about.clone(), text.clone(), cx),
                            cx,
                        )
                    }),
            )
    }

    /// A shortcut's button, whose right click changes it.
    fn shortcut_button(
        &self,
        id: impl Into<gpui::ElementId>,
        ix: usize,
        said: &str,
        to: CommentTo,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> Div {
        let cut = said.chars().count() > SHORTCUT_SHOWN;
        let shown: String = match cut {
            true => said.chars().take(SHORTCUT_SHOWN).chain(['…']).collect(),
            false => said.to_owned(),
        };
        div()
            .flex_none()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.shortcut_menu(ix, to.clone(), event.position, cx)
                }),
            )
            .child(
                button(id, shown)
                    .small()
                    .style(ButtonStyle::Ghost)
                    .tooltip(match cut {
                        true => said.to_owned(),
                        false => "Right-click to change".to_owned(),
                    })
                    .on_click(cx.listener(move |this, _, window, cx| on_click(this, window, cx))),
            )
    }

    pub(super) fn brief_shortcuts(&self, cx: &gpui::App) -> Vec<String> {
        (self.store.read(cx).prefs.brief_shortcuts.clone())
            .unwrap_or_else(|| STOCK_SHORTCUTS.map(str::to_owned).to_vec())
    }

    fn set_brief_shortcuts(&mut self, shortcuts: Vec<String>, cx: &mut Context<Self>) {
        (self.store).update(cx, |store, cx| {
            store.set_prefs(|prefs| prefs.brief_shortcuts = Some(shortcuts), cx)
        });
        self.brief_list_changed(cx);
    }

    /// Leave `said` as a note on `to`, right away.
    fn leave_shortcut(&mut self, to: CommentTo, said: String, cx: &mut Context<Self>) {
        if let Some((id, version)) = self.shown_version() {
            self.note_selection = None;
            self.keep_brief_note(id, version, to, said, cx);
        }
    }

    /// Start writing a shortcut, in place of the note it would be left as.
    fn write_shortcut(
        &mut self,
        editing: Option<usize>,
        to: CommentTo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let said = editing.and_then(|ix| self.brief_shortcuts(cx).get(ix).cloned());
        self.start_writing(
            Writing::Shortcut { editing, to },
            "A comment you leave often…",
            window,
            cx,
        );
        if let (Some(said), Some(input)) = (said, &self.brief_input) {
            input.field.update(cx, |input, cx| input.set_text(said, cx));
        }
    }

    /// Keep what's being written as a shortcut, to go on writing it. With nothing written yet,
    /// write one here.
    fn add_shortcut_from_writing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let written = (self.brief_input.as_ref())
            .map(|input| input.written.trim().to_owned())
            .unwrap_or_default();
        if !written.is_empty() {
            return self.save_brief_shortcut(None, written, cx);
        }
        let to = self.brief_writing().and_then(|writing| match writing {
            Writing::Note(to) => Some(to.clone()),
            _ => None,
        });
        if let Some(to) = to {
            self.write_shortcut(None, to, window, cx);
        }
    }

    /// Change shortcut `editing` to `said`, or take it away if that's empty, or add it.
    pub(super) fn save_brief_shortcut(
        &mut self,
        editing: Option<usize>,
        said: String,
        cx: &mut Context<Self>,
    ) {
        let mut shortcuts = self.brief_shortcuts(cx);
        match (editing, said.is_empty()) {
            (Some(ix), true) if ix < shortcuts.len() => {
                shortcuts.remove(ix);
            }
            (Some(ix), false) => {
                if let Some(shortcut) = shortcuts.get_mut(ix) {
                    *shortcut = said;
                }
            }
            (None, false) if !shortcuts.contains(&said) => shortcuts.push(said),
            _ => return,
        }
        self.set_brief_shortcuts(shortcuts, cx);
    }

    fn shortcut_menu(&self, ix: usize, to: CommentTo, at: Point<Pixels>, cx: &mut Context<Self>) {
        let this = cx.entity();
        self.open_menu(
            at,
            move |_, _| {
                let (edit, remove) = (this.clone(), this.clone());
                vec![
                    MenuItem::new("Edit shortcut", move |_, window, cx| {
                        let to = to.clone();
                        edit.update(cx, |this, cx| this.write_shortcut(Some(ix), to, window, cx));
                    }),
                    MenuItem::new("Remove shortcut", move |_, _, cx| {
                        remove.update(cx, |this, cx| {
                            this.save_brief_shortcut(Some(ix), String::new(), cx)
                        });
                    }),
                ]
            },
            cx,
        );
    }

    /// The rows' words with what's said about them tinted: more strongly what's being written
    /// about, or what the comment the pointer's on is about.
    pub(super) fn said_in_words<'a>(
        &self,
        block: &BriefBlock,
        state: &BriefState,
        words: Cow<'a, [markdown::Block]>,
    ) -> Cow<'a, [markdown::Block]> {
        let version = state.page().map_or(0, |page| page.version);
        let style = |tint| markdown::Style {
            tint: Some(tint),
            ..Default::default()
        };
        let on_block = |position: usize| block.positions.contains(&position);
        let hovered = self.brief_said_hovered.as_deref();
        let threads = (state.activity.threads.iter())
            .filter(|thread| thread.resolved.is_none())
            .filter(|thread| thread.version.is_none_or(|said_on| said_on == version))
            .filter(|thread| thread.anchor.block().is_some_and(on_block))
            .filter_map(quote_of)
            .map(|quote| {
                (
                    quote,
                    style(match hovered == Some(quote) {
                        true => Tint::Saying,
                        false => Tint::Said,
                    }),
                )
            });
        let notes = (state.notes.iter()).filter_map(|note| match &note.to {
            CommentTo::Passage { block, quote } if on_block(*block) => {
                let quote = quote.as_str();
                Some((
                    quote,
                    style(match hovered == Some(quote) {
                        true => Tint::SayingDraft,
                        false => Tint::Draft,
                    }),
                ))
            }
            _ => None,
        });
        let writing = self.brief_writing().and_then(|writing| match writing {
            Writing::Note(CommentTo::Passage { block, quote }) if on_block(*block) => {
                Some((quote.as_str(), style(Tint::SayingDraft)))
            }
            _ => None,
        });
        let quotes: Vec<_> = threads.chain(notes).chain(writing).collect();
        tinted(words, &quotes)
    }

    /// Say which words the comment the pointer's on is about, for them to stand out.
    pub(super) fn hover_said(
        &mut self,
        quote: Option<String>,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        let now = quote.filter(|_| hovered);
        if self.brief_said_hovered != now {
            self.brief_said_hovered = now;
            cx.notify();
        }
    }

    /// The row a note on `to` shows in.
    pub(super) fn row_of_note(&self, to: &CommentTo) -> Option<usize> {
        let page = self.page.brief()?.page()?;
        let (block, quote) = place_of(to)?;
        self.brief_rows()
            .iter()
            .position(|row| match (*row, block) {
                (Row::Head, None) => true,
                (Row::Block(at), Some(block)) => {
                    (page.blocks.get(at)).is_some_and(|on| on.positions.contains(&block))
                }
                (Row::Text { at, from, to }, Some(block)) => {
                    (page.blocks.get(at)).is_some_and(|on| {
                        on.positions.contains(&block)
                            && (from..to).contains(&paragraph_of(on, quote))
                    })
                }
                _ => false,
            })
    }
}

/// A line of what something's about, as much as fits.
pub(super) fn quote_line(quote: &str, theme: &Theme) -> Div {
    div()
        .truncate()
        .text_size(dp(12.))
        .text_color(theme.text_3)
        .child(format!("“{}”", quote.lines().next().unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::{BriefBlock, BriefBody, paragraph_of, quoted_in};
    use crate::ui::markdown;

    fn block(words: &str) -> BriefBlock {
        BriefBlock {
            positions: 3..=3,
            fresh: false,
            detail: None,
            body: BriefBody::Text(markdown::parse(words)),
        }
    }

    #[test]
    fn a_comment_goes_beside_the_paragraph_its_quote_starts_in() {
        let block = block("The opening\n\nThe middle\n\nThe end");
        assert_eq!(paragraph_of(&block, Some("middle")), 1, "by its own words");
        assert_eq!(
            paragraph_of(&block, Some("middle\n\nThe end")),
            1,
            "a selection across paragraphs goes where it starts"
        );
    }

    #[test]
    fn a_comment_with_no_words_to_go_by_goes_where_its_block_starts() {
        let block = block("The opening\n\nThe end");
        assert_eq!(
            paragraph_of(&block, None),
            0,
            "not at the end of a long block"
        );
        assert_eq!(paragraph_of(&block, Some("gone")), 0, "its words have gone");
    }

    #[test]
    fn a_quote_tints_what_it_says_in_each_paragraph_it_runs_across() {
        assert_eq!(
            quoted_in("A big one", "big"),
            vec![2..5],
            "just those words"
        );
        assert_eq!(
            quoted_in("The end", "The opening\n\nThe end"),
            vec![0..7],
            "the line of it that's in this paragraph"
        );
        assert!(quoted_in("The end", "gone").is_empty(), "nothing to tint");
    }
}
