//! A commit message as its title and body, each a field of its own in one box.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, ScrollHandle,
    Subscription, Window, actions, div, prelude::*,
};

use super::text_input::{InputEvent, TextInput};
use super::{dp, field, fixed, v_flex};
use crate::theme::Theme;

actions!(message_editor, [ToBody, ToTitle]);

#[derive(Clone, Copy, PartialEq)]
pub enum MessageSize {
    /// Under the file list, with room left for the AI button in its corner.
    Compact,
    /// In a commit's header, where a message written already is read and edited.
    Large,
}

pub struct MessageEditor {
    title: Entity<TextInput>,
    body: Entity<TextInput>,
    body_scroll: ScrollHandle,
    size: MessageSize,
    _forwarded: [Subscription; 2],
}

impl EventEmitter<InputEvent> for MessageEditor {}

impl MessageEditor {
    pub fn new(size: MessageSize, cx: &mut Context<Self>) -> Self {
        let body_scroll = ScrollHandle::new();
        let title = cx.new(|cx| TextInput::new("Commit title", false, cx));
        let body = cx
            .new(|cx| TextInput::new("Commit message", true, cx).scrolled_by(body_scroll.clone()));
        let forward = |_: &mut Self, _, event: &InputEvent, cx: &mut Context<Self>| cx.emit(*event);
        Self {
            _forwarded: [cx.subscribe(&title, forward), cx.subscribe(&body, forward)],
            title,
            body,
            body_scroll,
            size,
        }
    }

    pub fn text(&self, cx: &App) -> String {
        joined(self.title.read(cx).text(), self.body.read(cx).text())
    }

    /// Whether what's written is `message`, however it spaced its title from its body.
    pub fn holds(&self, message: &str, cx: &App) -> bool {
        let (title, body) = split(message);
        self.text(cx) == joined(title, body)
    }

    pub fn set_text(&mut self, message: impl AsRef<str>, cx: &mut Context<Self>) {
        let (title, body) = split(message.as_ref());
        self.title.update(cx, |input, cx| input.set_text(title, cx));
        self.body.update(cx, |input, cx| input.set_text(body, cx));
    }

    /// Show `message` as a new editor would, with nothing to undo back to.
    pub fn load(&mut self, message: impl AsRef<str>, cx: &mut Context<Self>) {
        let (title, body) = split(message.as_ref());
        self.title.update(cx, |input, cx| input.load(title, cx));
        self.body.update(cx, |input, cx| input.load(body, cx));
    }

    /// Start a new undo step in both fields with the next change.
    pub fn break_undo_run(&mut self, cx: &mut Context<Self>) {
        self.title.update(cx, |input, _| input.break_undo_run());
        self.body.update(cx, |input, _| input.break_undo_run());
    }

    pub fn contains_focus(&self, window: &Window, cx: &App) -> bool {
        self.title.focus_handle(cx).is_focused(window)
            || self.body.focus_handle(cx).is_focused(window)
    }
}

/// Where the editor takes focus: its title.
impl Focusable for MessageEditor {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.title.focus_handle(cx)
    }
}

impl Render for MessageEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let focused = self.contains_focus(window, cx);
        let (min_body, max_body) = match self.size {
            MessageSize::Compact => (56., 200.),
            MessageSize::Large => (240., 420.),
        };
        let part = |input| {
            field(input, false, theme)
                .border_0()
                .rounded(dp(5.))
                .px(dp(10.))
                .py(dp(8.))
                .cursor_text()
        };
        v_flex()
            .rounded(dp(6.))
            .border_1()
            .border_color(if focused {
                theme.focus_border()
            } else {
                theme.border_2
            })
            .bg(theme.bg_1)
            .line_height(dp(19.))
            .child(
                part(&self.title)
                    .key_context("MessageTitle")
                    .font_weight(FontWeight::MEDIUM)
                    .when(self.size == MessageSize::Compact, |el| el.pr(dp(30.)))
                    .on_action(cx.listener(|this, _: &ToBody, window, cx| {
                        window.focus(&this.body.focus_handle(cx), cx)
                    })),
            )
            .child(div().h(fixed(1.)).bg(theme.border_3))
            .child(
                part(&self.body)
                    .key_context("MessageBody")
                    .on_action(cx.listener(|this, _: &ToTitle, window, cx| {
                        window.focus(&this.title.focus_handle(cx), cx)
                    }))
                    .id("message-body")
                    .min_h(dp(min_body))
                    .max_h(dp(max_body))
                    .overflow_y_scroll()
                    .track_scroll(&self.body_scroll),
            )
    }
}

fn joined(title: &str, body: &str) -> String {
    let (title, body) = (title.trim(), body.trim_end());
    match body.is_empty() {
        true => title.to_owned(),
        false => format!("{title}\n\n{body}"),
    }
}

/// The first line of `message`, and all the rest without the blank lines between them. A title
/// left empty stays empty, so a draft that has only a body comes back as it was.
fn split(message: &str) -> (&str, &str) {
    let (title, body) = message.split_once('\n').unwrap_or((message, ""));
    (
        title.trim(),
        body.trim_start_matches(['\r', '\n']).trim_end(),
    )
}

#[cfg(test)]
mod tests {
    use super::{joined, split};

    #[test]
    fn a_message_survives_splitting_and_joining() {
        let again = |message| {
            let (title, body) = split(message);
            joined(title, body)
        };
        assert_eq!(split("title"), ("title", ""), "no body");
        assert_eq!(
            split("title\n\n\nbody\n\n  indented\n"),
            ("title", "body\n\n  indented"),
            "blank lines go from around the body, and stay inside it"
        );
        assert_eq!(
            split("title\nnext line"),
            ("title", "next line"),
            "only the first line is the title, so a message without a blank line loses nothing"
        );
        assert_eq!(
            again("title\n\nbody"),
            "title\n\nbody",
            "what Git writes is unchanged"
        );
        assert_eq!(
            split(&joined("", "only a body")),
            ("", "only a body"),
            "a body doesn't turn into the title of a draft"
        );
        assert_eq!(
            joined("  ", "\n"),
            "",
            "nothing written is an empty message"
        );
    }
}
