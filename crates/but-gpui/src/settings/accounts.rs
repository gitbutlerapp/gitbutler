//! The Accounts section: the GitButler account, and the forge accounts pull requests and
//! CI go through, with ways to add one: GitHub's browser sign-in, or a token.

use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, Entity, Focusable as _, SharedString, Window,
    prelude::*, px,
};

use super::{Row, Section, SettingsView};
use crate::app::MenuItem;
use crate::backend::accounts::{self, Account, Forge};
use crate::backend::run;
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{ButtonStyle, button, field, h_flex};

/// Where GitHub's browser sign-in takes the code.
const GITHUB_DEVICE_PAGE: &str = "https://github.com/login/device";

/// How long to wait for the browser sign-in, asking every few seconds.
const SIGN_IN_WAIT: Duration = Duration::from_secs(15 * 60);
const SIGN_IN_POLL: Duration = Duration::from_secs(5);

/// The accounts as last read, and any sign-in under way.
#[derive(Default)]
pub(super) struct Accounts {
    list: Vec<Account>,
    user: Option<String>,
    /// A token being added: for which forge, with its fields.
    adding: Option<(Forge, Entity<TextInput>, Entity<TextInput>)>,
    /// GitHub's browser sign-in, waiting: the code the user enters there.
    github_code: Option<SharedString>,
    /// GitButler's sign-in, waiting for the token its page shows.
    login: Option<Entity<TextInput>>,
}

/// Where to make a token, and what it needs.
fn token_help(forge: Forge) -> &'static str {
    match forge {
        Forge::GitHub => "Make a classic token with the repo scope at github.com/settings/tokens.",
        Forge::GitLab => {
            "Make a token with the api scope at gitlab.com/-/user_settings/personal_access_tokens."
        }
        Forge::Bitbucket => {
            "Make an API token with scopes at id.atlassian.com/manage-profile/security/api-tokens, \
             and give the account's email."
        }
    }
}

impl SettingsView {
    /// Read the accounts again, as after a change.
    pub(super) fn load_accounts(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let read = run(|| Ok((accounts::accounts(), accounts::gitbutler_user()))).await;
            if let Ok((list, user)) = read {
                this.update(cx, |this, cx| {
                    this.accounts.list = list;
                    this.accounts.user = user;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn start_adding(&mut self, forge: Forge, window: &mut Window, cx: &mut Context<Self>) {
        let token = cx.new(|cx| TextInput::new("Token", false, cx).masked());
        let email = cx.new(|cx| TextInput::new("Email", false, cx));
        cx.subscribe(&token, |this, _, event, cx| match event {
            InputEvent::Submit => this.add_token(cx),
            InputEvent::Cancel => {
                this.accounts.adding = None;
                cx.notify();
            }
            InputEvent::Changed => {}
        })
        .detach();
        let first = if forge == Forge::Bitbucket {
            &email
        } else {
            &token
        };
        window.focus(&first.focus_handle(cx), cx);
        self.accounts.adding = Some((forge, token, email));
        cx.notify();
    }

    fn add_token(&mut self, cx: &mut Context<Self>) {
        let Some((forge, token, email)) = &self.accounts.adding else {
            return;
        };
        let (forge, token, email) = (
            *forge,
            token.read(cx).text().to_owned(),
            email.read(cx).text().to_owned(),
        );
        if token.trim().is_empty() {
            return;
        }
        self.accounts.adding = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let added = run(move || accounts::add_token(forge, token, Some(email))).await;
            this.update(cx, |this, cx| {
                match added {
                    Ok(who) => this.store.update(cx, |store, cx| {
                        store.snack(format!("Added {who} on {}", forge.name()), cx)
                    }),
                    Err(err) => this.error("Couldn't add the account", err, cx),
                }
                this.load_accounts(cx);
            })
            .ok();
        })
        .detach();
    }

    /// GitHub's browser sign-in: open the page, put the code on the clipboard, and wait
    /// for the user to finish there. The wait outlives Settings, so finishing in the
    /// browser after closing it still adds the account.
    fn sign_in_to_github(&mut self, cx: &mut Context<Self>) {
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let started = run(accounts::start_github_sign_in).await;
            let (code, device) = match started {
                Ok(started) => started,
                Err(err) => {
                    this.update(cx, |this, cx| {
                        this.error("Couldn't start signing in", err, cx)
                    })
                    .ok();
                    return;
                }
            };
            this.update(cx, |this, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
                cx.open_url(GITHUB_DEVICE_PAGE);
                this.accounts.github_code = Some(code.into());
                cx.notify();
            })
            .ok();
            let waited = std::time::Instant::now();
            let outcome = loop {
                cx.background_executor().timer(SIGN_IN_POLL).await;
                // Cancel clears the code; a closed Settings doesn't stop the wait.
                let cancelled = this
                    .update(cx, |this, _| this.accounts.github_code.is_none())
                    .unwrap_or(false);
                if cancelled {
                    return;
                }
                let device = device.clone();
                match run(move || accounts::github_signed_in(&device)).await {
                    Ok(None) if waited.elapsed() < SIGN_IN_WAIT => continue,
                    Ok(None) => break Err(anyhow::anyhow!("The sign-in wasn't finished in time")),
                    Ok(Some(who)) => break Ok(who),
                    Err(err) => break Err(err),
                }
            };
            store.update(cx, |store, cx| match &outcome {
                Ok(who) => store.snack(format!("Signed in to GitHub as {who}"), cx),
                Err(err) => store.error("Couldn't sign in to GitHub", format!("{err:#}"), cx),
            });
            this.update(cx, |this, cx| {
                this.accounts.github_code = None;
                this.load_accounts(cx);
            })
            .ok();
        })
        .detach();
    }

    /// GitButler's sign-in: its page shows a token to paste back here.
    fn log_in_to_gitbutler(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| TextInput::new("Paste the token", false, cx).masked());
        cx.subscribe(&input, |this, input, event, cx| match event {
            InputEvent::Submit => {
                let token = input.read(cx).text().to_owned();
                this.accounts.login = None;
                cx.spawn(async move |this, cx| {
                    let logged_in = run(move || accounts::gitbutler_log_in(&token)).await;
                    this.update(cx, |this, cx| {
                        match logged_in {
                            Ok(who) => this.store.update(cx, |store, cx| {
                                store.snack(format!("Signed in to GitButler as {who}"), cx)
                            }),
                            Err(err) => this.error("Couldn't sign in", err, cx),
                        }
                        this.load_accounts(cx);
                    })
                    .ok();
                })
                .detach();
            }
            InputEvent::Cancel => {
                this.accounts.login = None;
                cx.notify();
            }
            InputEvent::Changed => {}
        })
        .detach();
        window.focus(&input.focus_handle(cx), cx);
        self.accounts.login = Some(input);
        cx.notify();
        cx.spawn(async move |this, cx| {
            match run(accounts::gitbutler_login_url).await {
                Ok(url) => this.update(cx, |_, cx| cx.open_url(&url)).ok(),
                Err(err) => this
                    .update(cx, |this, cx| {
                        this.accounts.login = None;
                        this.error("Couldn't reach GitButler", err, cx)
                    })
                    .ok(),
            };
        })
        .detach();
    }

    fn run_and_reload(
        &mut self,
        failed: &'static str,
        op: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = run(op).await;
            this.update(cx, |this, cx| {
                if let Err(err) = result {
                    this.error(failed, err, cx);
                }
                this.load_accounts(cx);
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn account_rows(&self, window: &Window, cx: &Context<Self>) -> Vec<Row> {
        let theme = Theme::get(cx);
        let mut rows = Vec::new();
        let gitbutler: AnyElement = match (&self.accounts.user, &self.accounts.login) {
            (_, Some(input)) => token_field(input, None, window, theme, cx),
            (Some(_), None) => button("log-out", "Log out")
                .small()
                .style(ButtonStyle::Ghost)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.run_and_reload("Couldn't log out", accounts::gitbutler_log_out, cx)
                }))
                .into_any_element(),
            (None, None) => button("log-in", "Log in…")
                .small()
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.log_in_to_gitbutler(window, cx)
                }))
                .into_any_element(),
        };
        rows.push(Row {
            section: Section::Accounts,
            title: "GitButler".into(),
            description: match (&self.accounts.user, &self.accounts.login) {
                (_, Some(_)) => {
                    "Sign in on the page that opened, then paste the token it shows.".into()
                }
                (Some(user), None) => {
                    format!("Signed in as {user}. AI can use GitButler's key.").into()
                }
                (None, None) => {
                    "Sign in to use GitButler's AI key, with no key of your own.".into()
                }
            },
            control: gitbutler,
        });
        for account in &self.accounts.list {
            let id = account.id.clone();
            rows.push(Row {
                section: Section::Accounts,
                title: account.forge.name().into(),
                description: account.label.clone().into(),
                control: button(
                    SharedString::from(format!("forget-{}", account.label)),
                    "Forget",
                )
                .small()
                .style(ButtonStyle::Ghost)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let id = id.clone();
                    this.run_and_reload(
                        "Couldn't forget the account",
                        move || accounts::forget(id),
                        cx,
                    )
                }))
                .into_any_element(),
            });
        }
        if let Some(code) = &self.accounts.github_code {
            let copied = code.clone();
            rows.push(Row {
                section: Section::Accounts,
                title: "Signing in to GitHub".into(),
                description: format!(
                    "Enter {code} on the page that opened. It's on your clipboard."
                )
                .into(),
                control: h_flex()
                    .gap(px(6.))
                    .child(
                        button("copy-code", "Copy code")
                            .small()
                            .style(ButtonStyle::Ghost)
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copied.to_string()))
                            }),
                    )
                    .child(
                        button("cancel-sign-in", "Cancel")
                            .small()
                            .style(ButtonStyle::Ghost)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.accounts.github_code = None;
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            });
        }
        match &self.accounts.adding {
            Some((forge, token, email)) => rows.push(Row {
                section: Section::Accounts,
                title: format!("New {} token", forge.name()).into(),
                description: token_help(*forge).into(),
                control: token_field(
                    token,
                    (*forge == Forge::Bitbucket).then_some(email),
                    window,
                    theme,
                    cx,
                ),
            }),
            None => {
                let this = cx.entity().downgrade();
                let menu = cx.entity().downgrade();
                rows.push(Row {
                    section: Section::Accounts,
                    title: "Add an account".into(),
                    description: "For pull requests and CI on GitHub, GitLab or Bitbucket. \
                                  GitButler's other apps share them."
                        .into(),
                    control: button("add-account", "Add…")
                        .small()
                        .on_click(move |event, _, cx| {
                            let item = |label: &'static str, forge: Option<Forge>| {
                                let this = this.clone();
                                MenuItem::new(label, move |_, window, cx| {
                                    this.update(cx, |this, cx| match forge {
                                        Some(forge) => this.start_adding(forge, window, cx),
                                        None => this.sign_in_to_github(cx),
                                    })
                                    .ok();
                                })
                            };
                            let items = vec![
                                item("Sign in with GitHub", None),
                                item("GitHub token…", Some(Forge::GitHub)),
                                item("GitLab token…", Some(Forge::GitLab)),
                                item("Bitbucket token…", Some(Forge::Bitbucket)),
                            ];
                            menu.update(cx, |this, cx| this.open_menu(event, items, cx))
                                .ok();
                        })
                        .into_any_element(),
                });
            }
        }
        rows
    }
}

/// A masked token field, with an email field before it when given, and Add.
fn token_field(
    token: &Entity<TextInput>,
    email: Option<&Entity<TextInput>>,
    window: &Window,
    theme: &Theme,
    cx: &Context<SettingsView>,
) -> AnyElement {
    let focused = |input: &Entity<TextInput>| input.focus_handle(cx).is_focused(window);
    h_flex()
        .gap(px(6.))
        .children(email.map(|email| field(email, focused(email), theme).w(px(150.))))
        .child(field(token, focused(token), theme).w(px(180.)))
        .child({
            let token = token.clone();
            button("cancel-token", "Cancel")
                .small()
                .style(ButtonStyle::Ghost)
                .on_click(move |_, _, cx| token.update(cx, |_, cx| cx.emit(InputEvent::Cancel)))
        })
        .child({
            let token = token.clone();
            button("add-token", "Add")
                .small()
                .style(ButtonStyle::Gray)
                .on_click(move |_, _, cx| token.update(cx, |_, cx| cx.emit(InputEvent::Submit)))
        })
        .into_any_element()
}
