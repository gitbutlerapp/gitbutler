//! Who is signed in to GitButler, shown at the top of the navigator.

use std::sync::Arc;

use gpui::{Context, Image};

use super::Store;
use crate::backend::{self, accounts::GitButlerUser};

/// The GitButler account this app's other windows and GitButler's other apps share.
#[derive(Default)]
pub struct Account {
    pub user: Option<GitButlerUser>,
    /// Their picture, once it has loaded.
    pub avatar: Option<Arc<Image>>,
}

impl Store {
    /// Read who is signed in again, as after signing in or out here or in another app.
    pub fn load_account(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let user = backend::run(|| Ok(backend::accounts::gitbutler_user()))
                .await
                .ok()
                .flatten();
            let picture = user.as_ref().and_then(|user| user.picture.clone());
            this.update(cx, |store, cx| {
                if store.account.user != user {
                    store.account = Account { user, avatar: None };
                    cx.notify();
                }
            })
            .ok();
            let Some(url) = picture else {
                return;
            };
            // Without its picture, the account still shows by its initial.
            let avatar = backend::run(move || backend::accounts::avatar(&url)).await;
            this.update(cx, |store, cx| {
                store.account.avatar = avatar.ok();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn sign_out(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let signed_out = backend::run(backend::accounts::gitbutler_log_out).await;
            this.update(cx, |store, cx| {
                match signed_out {
                    Ok(()) => store.snack("Signed out of GitButler", cx),
                    Err(err) => store.error("Couldn't sign out", format!("{err:#}"), cx),
                }
                store.load_account(cx);
            })
            .ok();
        })
        .detach();
    }
}
