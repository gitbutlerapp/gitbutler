//! Alerts, as `Window::prompt` asks for them, where the platform has none of its own, as on
//! Linux: drawn as the app's own prompts are. GPUI's stand-in cuts long text off.

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, FontWeight, PromptButton, PromptHandle,
    PromptLevel, PromptResponse, RenderablePromptHandle, SharedString, Window, div, prelude::*,
};

use super::window_frame::inside_frame;
use super::{ButtonStyle, button, dp, h_flex, muted, popover};
use crate::theme::Theme;

/// What `App::set_prompt_builder` takes.
pub fn alert(
    _level: PromptLevel,
    message: &str,
    detail: Option<&str>,
    actions: &[PromptButton],
    handle: PromptHandle,
    window: &mut Window,
    cx: &mut App,
) -> RenderablePromptHandle {
    let alert = cx.new(|cx| Alert {
        message: message.to_owned().into(),
        detail: detail.map(|detail| detail.to_owned().into()),
        actions: actions.to_vec(),
        focus: cx.focus_handle(),
    });
    handle.with_view(alert, window, cx)
}

struct Alert {
    message: SharedString,
    detail: Option<SharedString>,
    actions: Vec<PromptButton>,
    focus: FocusHandle,
}

impl EventEmitter<PromptResponse> for Alert {}

impl Focusable for Alert {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Alert {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        // Enter takes the first answer and Escape the one that cancels, as native alerts do.
        let cancel = (self.actions.iter().position(PromptButton::is_cancel))
            .unwrap_or(self.actions.len().saturating_sub(1));
        // Cancel first and the first answer last, where GNOME puts them.
        let buttons = (self.actions.iter().enumerate().rev()).map(|(ix, action)| {
            button(("alert", ix), action.label().clone())
                .style(match ix {
                    _ if action.is_cancel() => ButtonStyle::Ghost,
                    0 => ButtonStyle::Gray,
                    _ => ButtonStyle::Outline,
                })
                .on_click(cx.listener(move |_, _, _, cx| cx.emit(PromptResponse(ix))))
        });
        let card = popover(theme)
            .w(dp(380.))
            .p(dp(16.))
            .gap(dp(12.))
            .rounded(dp(10.))
            .child(
                div()
                    .text_size(dp(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.message.clone()),
            )
            .when_some(self.detail.clone(), |el, detail| {
                el.child(muted(detail, theme))
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_end()
                    .gap(dp(8.))
                    .children(buttons),
            );
        inside_frame(div().size_full(), window).child(
            div()
                .id("alert")
                .track_focus(&self.focus)
                .on_key_down(cx.listener(move |_, event: &gpui::KeyDownEvent, _, cx| {
                    match event.keystroke.key.as_str() {
                        "enter" => cx.emit(PromptResponse(0)),
                        "escape" => cx.emit(PromptResponse(cancel)),
                        _ => {}
                    }
                }))
                .size_full()
                .occlude()
                .bg(gpui::black().opacity(0.25))
                .flex()
                .justify_center()
                .items_start()
                .pt(dp(120.))
                // A layer of its own, so it takes the app's look itself.
                .font_family(theme.ui_font.clone())
                .text_size(dp(13.))
                .line_height(dp(18.))
                .text_color(theme.text_1)
                .child(card),
        )
    }
}
