//! The Mesh row on General: whether this machine is in the mesh of the user's machines, and a
//! way in or out. Everything else the mesh does is in `state/mesh.rs`.

use gpui::{AnyElement, ClickEvent, Context, SharedString, prelude::*};

use super::{Row, SettingsView};
use crate::state::MeshState;
use crate::ui::{ButtonStyle, button};

impl SettingsView {
    /// The mesh of the user's machines: whether this one is in it, and a way in or out.
    pub(super) fn mesh_row(&self, cx: &Context<Self>) -> Row {
        let mesh = &self.store.read(cx).mesh;
        let (description, control): (SharedString, Option<AnyElement>) = match &mesh.state {
            MeshState::Unknown => ("Checking…".into(), None),
            MeshState::Unavailable(why) => (why.clone().into(), None),
            MeshState::SignedOut if mesh.signing_in => (
                "Approve this machine on the page that opened.".into(),
                Some(
                    button("mesh-sign-in", "Signing in…")
                        .small()
                        .disabled_because(Some("Waiting for the browser"))
                        .into_any_element(),
                ),
            ),
            MeshState::SignedOut => (
                "Sign in to see unfinished work on your other machines beside each project, and \
                 show them this one's."
                    .into(),
                Some(
                    button("mesh-sign-in", "Sign in…")
                        .small()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.store.update(cx, |store, cx| store.mesh_sign_in(cx))
                        }))
                        .into_any_element(),
                ),
            ),
            MeshState::Connected(connection) => {
                let reporting = match connection.daemon_running {
                    true => "Projects you watch here show on your other machines.",
                    false => {
                        "Its daemon isn't running, so your other machines don't see this \
                              one's work. Run `but mesh service restart`."
                    }
                };
                let machine = connection.machine.clone();
                (
                    format!(
                        "Signed in as {} on {}. {reporting}",
                        connection.login, connection.machine
                    )
                    .into(),
                    Some(
                        button("mesh-sign-out", "Sign out")
                            .small()
                            .style(ButtonStyle::Ghost)
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                let store = this.store.clone();
                                let message = format!("Take {machine} out of your mesh?");
                                this.app.update_later(window, cx, move |app, window, cx| {
                                    app.confirm_then(
                                        None,
                                        message,
                                        "Its daemon stops, and your other machines no longer \
                                         see its work. Its repositories stay as they are.",
                                        "Sign out",
                                        window,
                                        cx,
                                        move |_, cx| {
                                            store.update(cx, |store, cx| store.mesh_sign_out(cx))
                                        },
                                    )
                                });
                            }))
                            .into_any_element(),
                    ),
                )
            }
        };
        Row::new(
            "Mesh",
            description,
            control.unwrap_or_else(|| gpui::div().into_any_element()),
        )
    }
}
