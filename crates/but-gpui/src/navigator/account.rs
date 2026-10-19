//! The GitButler account at the top of the navigator: its picture and menu, or Sign in.

use gpui::{AnyElement, Context, MouseButton, MouseDownEvent, div, img, prelude::*, px};

use super::Navigator;
use crate::overlays::MenuItem;
use crate::theme::Theme;
use crate::ui::person::person_avatar;
use crate::ui::{ButtonStyle, button};

impl Navigator {
    /// New version, when the desktop app's update feed offers one. Not "Update", which a
    /// project's row says for bringing in its target's new commits.
    pub(super) fn update_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let version = self.store.read(cx).newer_version.clone()?;
        Some(
            button("update-app", "New version")
                .small()
                .style(ButtonStyle::Gray)
                .tooltip(format!("Update to {version}"))
                .on_click(cx.listener(|this, _, window, cx| {
                    (this.app)
                        .update(cx, |app, cx| app.restart_to_update(window, cx))
                        .ok();
                }))
                .into_any_element(),
        )
    }

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
        // Without a picture, the person's pattern stands in, as in GitButler's other apps.
        let face = match account.avatar.clone() {
            Some(picture) => img(picture).size(px(20.)).rounded_full().into_any_element(),
            None => {
                let seed = (user.login.as_ref().or(user.email.as_ref())).unwrap_or(&user.name);
                person_avatar(seed, 20., theme).into_any_element()
            }
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
                        MenuItem::new(identity, |app, window, cx| {
                            app.open_settings(&crate::settings::OpenSettings, window, cx)
                        }),
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
