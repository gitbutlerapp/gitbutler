//! The top of the navigator's right end: New version, when the app has an update, and the
//! GitButler account's picture and menu, or Sign in.

use gpui::{AnyElement, Context, MouseButton, MouseDownEvent, div, img, prelude::*};

use super::Navigator;
use crate::overlays::MenuItem;
use crate::theme::Theme;
use crate::ui::person::person_avatar;
use crate::ui::{ButtonStyle, button, dp};

impl Navigator {
    /// New version, when the desktop app's update feed offers one, and how far installing it
    /// has got once it installs. Not "Update", which a project's row says for bringing in its
    /// target's new commits.
    pub(super) fn update_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let version = store.newer_version.clone()?;
        let label = match store.updating {
            None => "New version".to_owned(),
            Some(0) => "Downloading…".to_owned(),
            Some(100..) => "Installing…".to_owned(),
            Some(percent) => format!("Downloading {percent}%"),
        };
        let installing = store.updating.map(|_| format!("Installing {version}"));
        Some(
            button("update-app", label)
                .small()
                .style(ButtonStyle::Gray)
                .tooltip(format!("Update to {version}"))
                .disabled_because(installing)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.app.update_later(window, cx, move |app, window, cx| {
                        app.install_update(window, cx)
                    });
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
                    this.app
                        .update_later(window, cx, move |app, window, cx| app.sign_in(window, cx));
                }))
                .into_any_element();
        };
        // Without a picture, the person's pattern stands in, as in GitButler's other apps.
        let face = match account.avatar.clone() {
            Some(picture) => img(picture).size(dp(20.)).rounded_full().into_any_element(),
            None => {
                let seed = (user.login.as_ref().or(user.email.as_ref())).unwrap_or(&user.name);
                person_avatar(seed, 20., theme).into_any_element()
            }
        };
        div()
            .id("account")
            .p(dp(2.))
            .rounded_full()
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
                    let items = move |_: &mut Self, _: &mut Context<Self>| {
                        vec![
                            MenuItem::new(identity, |app, window, cx| {
                                app.open_settings(&crate::settings::OpenSettings, window, cx)
                            }),
                            MenuItem::new(format!("Sign out {who}"), |app, _, cx| {
                                app.store.update(cx, |store, cx| store.sign_out(cx))
                            })
                            .separated(),
                        ]
                    };
                    this.open_menu(event.position, items, cx);
                }),
            )
            .into_any_element()
    }
}
