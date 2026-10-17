//! The GitButler account at the top of the navigator: its picture and menu, or Sign in.

use gpui::{AnyElement, Context, MouseButton, MouseDownEvent, div, img, prelude::*, px};

use super::Navigator;
use crate::overlays::MenuItem;
use crate::theme::Theme;
use crate::ui::{ButtonStyle, button};

impl Navigator {
    pub(super) fn account(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let account = &self.store.read(cx).account;
        let Some(user) = account.user.clone() else {
            return button("sign-in", "Sign in")
                .small()
                .style(ButtonStyle::Ghost)
                .tooltip("Sign in to GitButler, for its AI key")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.app.update(cx, |app, cx| app.sign_in(window, cx)).ok();
                }))
                .into_any_element();
        };
        let size = px(20.);
        // Without a picture, the first letter of the name stands in for one.
        let face = match account.avatar.clone() {
            Some(picture) => img(picture).size(size).rounded_full().into_any_element(),
            None => div()
                .size(size)
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(theme.fill_gray.opacity(0.2))
                .text_size(px(11.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text_2)
                .children(
                    user.name
                        .chars()
                        .next()
                        .map(|c| c.to_uppercase().to_string()),
                )
                .into_any_element(),
        };
        div()
            .id("account")
            .p(px(2.))
            .rounded_full()
            .cursor_pointer()
            .hover(|el| el.bg(theme.hover()))
            .child(face)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    let who = user.login.clone().unwrap_or_else(|| user.name.clone());
                    // Who it is, leading to the account's settings.
                    let identity = match &user.email {
                        Some(email) => format!("{} · {email}", user.name),
                        None => user.name.clone(),
                    };
                    let items = vec![
                        MenuItem::new(identity, |app, window, cx| app.open_accounts(window, cx)),
                        MenuItem::new(format!("Sign out {who}"), |app, _, cx| {
                            app.store.update(cx, |store, cx| store.sign_out(cx))
                        })
                        .separated(),
                    ];
                    this.app
                        .update(cx, |app, cx| app.open_menu(event.position, items, cx))
                        .ok();
                }),
            )
            .into_any_element()
    }
}
