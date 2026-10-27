//! A description's image over most of the window, the others a key or a click away, as Quick
//! Look pages through what's selected: ← and → (or h and l) for the one before or after,
//! Space or Escape to close.

use std::sync::Arc;

use gpui::{
    App, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, ImageSource, Window, actions,
    div, img, prelude::*,
};

use super::Image;
use super::figure::fetched;
use crate::app::{AppView, Cancel};
use crate::theme::Theme;
use crate::ui::modal::{ModalSize, modal};
use crate::ui::{Icon, dp, h_flex, icon_button, muted, v_flex};

actions!(image_preview, [PreviousImage, NextImage]);

/// How much of the window the preview takes, across and down, as much as a modal may.
const SHARE: (f32, f32) = (0.92, 0.88);

/// How tall its header is, and the space around the image under it.
const HEADER: f32 = 48.;
const MARGIN: f32 = 16.;

pub(super) struct Preview {
    images: Arc<[Image]>,
    at: usize,
    focus: FocusHandle,
}

/// Preview `images`, the one `at` first. Inside another modal, which this would close, as
/// What's New, the image opens in the browser instead.
pub(super) fn open(images: Arc<[Image]>, at: usize, window: &mut Window, cx: &mut App) {
    let Some(Some(app)) = window.root::<AppView>() else {
        return;
    };
    window.defer(cx, move |window, cx| {
        app.update(cx, |app, cx| {
            if app.modal.is_some() {
                if let Some(image) = images.get(at) {
                    cx.open_url(&image.opens);
                }
                return;
            }
            let preview = cx.new(|cx| Preview {
                images,
                at,
                focus: cx.focus_handle(),
            });
            app.open_modal(preview, window, cx);
        })
    });
}

impl Preview {
    fn previous(&mut self, _: &PreviousImage, _: &mut Window, cx: &mut Context<Self>) {
        if self.at > 0 {
            self.at -= 1;
            cx.notify();
        }
    }

    fn next(&mut self, _: &NextImage, _: &mut Window, cx: &mut Context<Self>) {
        if self.at + 1 < self.images.len() {
            self.at += 1;
            cx.notify();
        }
    }
}

impl EventEmitter<DismissEvent> for Preview {}

impl Focusable for Preview {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(image) = self.images.get(self.at).cloned() else {
            return div();
        };
        let fetched = fetched(&image.src, window, cx);
        let theme = Theme::get(cx);
        let viewport = window.viewport_size();
        let (width, height) = (
            f32::from(viewport.width) / theme.scale * SHARE.0,
            f32::from(viewport.height) / theme.scale * SHARE.1,
        );
        let (count, at) = (self.images.len(), self.at);
        let title = match image.alt.trim() {
            "" => "Image".to_owned(),
            alt => alt.to_owned(),
        };
        let paging = (count > 1).then(|| {
            h_flex()
                .gap(dp(4.))
                .child(
                    icon_button("preview-previous", Icon::ChevronLeft)
                        .tooltip("Previous image")
                        .dispatch(&PreviousImage)
                        .disabled_because((at == 0).then_some("It's the first")),
                )
                .child(muted(format!("{} of {count}", at + 1), theme))
                .child(
                    icon_button("preview-next", Icon::ChevronRight)
                        .tooltip("Next image")
                        .dispatch(&NextImage)
                        .disabled_because((at + 1 == count).then_some("It's the last")),
                )
        });
        let opens = image.opens.clone();
        let header = h_flex()
            .flex_none()
            .h(dp(HEADER))
            .pl(dp(20.))
            .pr(dp(10.))
            .gap(dp(8.))
            .border_b_1()
            .border_color(theme.border_3)
            .child(
                div()
                    .flex_grow(1.)
                    .min_w_0()
                    .truncate()
                    .text_size(dp(13.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .children(paging)
            .child(
                icon_button("preview-open", Icon::ArrowUpRight)
                    .tooltip("Open in browser")
                    .on_click(move |_, _, cx| cx.open_url(&opens)),
            )
            .child(
                icon_button("preview-close", Icon::Cross)
                    .tooltip("Close")
                    .dispatch(&Cancel),
            );
        // As big as it is, or as fits, the whole of it in sight.
        let room = (width - 2. * MARGIN, height - HEADER - 2. * MARGIN);
        let shown = match fetched {
            Some(Some(picture)) => {
                let fit = (room.0 / picture.width)
                    .min(room.1 / picture.height)
                    .min(1.);
                div()
                    .relative()
                    .w(dp(picture.width * fit))
                    .h(dp(picture.height * fit))
                    .child(
                        img(ImageSource::Render(picture.image))
                            .size_full()
                            .rounded(dp(6.)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .rounded(dp(6.))
                            .border_1()
                            .border_color(theme.border_3),
                    )
            }
            None => muted("Loading…", theme),
            Some(None) => muted("Couldn't load this image", theme),
        };
        modal(ModalSize::Large, theme)
            .w(dp(width))
            .h(dp(height))
            .key_context("ImagePreview")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::next))
            .child(header)
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .items_center()
                    .justify_center()
                    .bg(theme.bg_2)
                    .child(shown),
            )
    }
}
