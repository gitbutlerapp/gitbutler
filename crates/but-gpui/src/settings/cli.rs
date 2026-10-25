//! The `but` CLI's row on General and its card on the welcome: install it with a click, then
//! copy the command that gives coding agents GitButler's skill.

use gpui::{App, ClipboardItem, Context, Div, Entity, FontWeight, div, prelude::*};

use super::{Row, SettingsView};
use crate::state::{Store, snack};
use crate::theme::Theme;
use crate::ui::illustration::{Illustration, illustration};
use crate::ui::{Button, ButtonStyle, Icon, button, dp, h_flex, icon_button, v_flex};

const AGENT_SETUP: &str = "but agent setup";

const SETUP: &str = "Run this in a terminal to give your coding agents GitButler's skill, which \
                     they need to use the CLI well.";

fn install_button(
    id: &'static str,
    label: &'static str,
    store: &Entity<Store>,
    cx: &App,
) -> Button {
    let store = store.clone();
    match store.read(cx).installing_cli {
        true => button(id, "Installing…")
            .small()
            .disabled_because(Some("Waiting for the administrator password")),
        false => button(id, label)
            .small()
            .on_click(move |_, _, cx| store.update(cx, |store, cx| store.install_cli(cx))),
    }
}

/// `but agent setup` as a terminal shows it, on code's ground, with a button that copies it.
fn agent_setup(id: &'static str, store: &Entity<Store>, theme: &Theme) -> Div {
    let store = store.clone();
    h_flex()
        .gap(dp(6.))
        .pl(dp(10.))
        .pr(dp(2.))
        .py(dp(2.))
        .rounded(dp(6.))
        .bg(theme.fill_gray.opacity(0.08))
        .font_family(theme.mono_font.clone())
        .text_size(dp(12.))
        .child(div().text_color(theme.text_3).child("$"))
        .child(div().flex_1().text_color(theme.text_1).child(AGENT_SETUP))
        .child(
            icon_button(id, Icon::Copy)
                .small()
                .tooltip("Copy")
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(AGENT_SETUP.to_owned()));
                    snack(&store, "Copied but agent setup", cx);
                }),
        )
}

impl SettingsView {
    /// Nothing until `but` has been looked for, which only macOS does.
    pub(super) fn cli_row(&self, cx: &Context<Self>) -> Option<Row> {
        const TITLE: &str = "Command-line interface";
        let store = &self.store;
        let row = match store.read(cx).cli.as_ref()? {
            Some(path) => Row::new(
                TITLE,
                format!("Installed at {}. {SETUP}", path.display()),
                install_button("settings-reinstall-cli", "Reinstall", store, cx)
                    .style(ButtonStyle::Ghost)
                    .tooltip("Link this app's copy"),
            )
            .with_below(agent_setup("settings-copy-setup", store, Theme::get(cx))),
            None => Row::new(
                TITLE,
                "Lets you and your coding agents use GitButler from a terminal. macOS may ask \
                 for your password to put it in /usr/local/bin.",
                install_button("settings-install-cli", "Install", store, cx),
            ),
        };
        Some(row.with_keywords("cli but install terminal agent skill"))
    }
}

/// The welcome's card: what the CLI is for, installed with a click, then the setup that gives
/// agents GitButler's skill. Nothing until `but` has been looked for, which only macOS does.
pub fn welcome_card(store: &Entity<Store>, theme: &Theme, cx: &App) -> Option<Div> {
    let installed = store.read(cx).cli.as_ref()?.is_some();
    let (title, body) = match installed {
        true => ("Set up your agents", SETUP),
        false => (
            "Let your agents use GitButler",
            "Install the command-line interface, and any coding agent can commit, branch and \
             push with GitButler.",
        ),
    };
    Some(
        h_flex()
            .w(dp(480.))
            .max_w_full()
            .p(dp(16.))
            .gap(dp(16.))
            .rounded(dp(8.))
            .border_1()
            .border_color(theme.border_3)
            .bg(theme.bg_1)
            .child(illustration(Illustration::Terminal, theme, cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(dp(4.))
                    .child(
                        div()
                            .text_size(dp(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_1)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(dp(12.))
                            .line_height(dp(18.))
                            .text_color(theme.text_2)
                            .child(body),
                    )
                    .when(installed, |el| {
                        el.child(agent_setup("welcome-copy-setup", store, theme).mt(dp(6.)))
                    }),
            )
            .when(!installed, |el| {
                el.child(install_button("welcome-install-cli", "Install", store, cx))
            }),
    )
}
