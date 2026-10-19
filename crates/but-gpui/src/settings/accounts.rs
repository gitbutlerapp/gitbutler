//! Accounts: the GitButler account and the mesh of the user's machines, on General, and on
//! Integrations the forge accounts pull requests and CI go through, with ways to add one:
//! GitHub's browser sign-in, or a token.

use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, Entity, Focusable as _, SharedString, Window,
    prelude::*, px,
};

use super::{Card, Row, SettingsView};
use crate::backend::Forge;
use crate::backend::accounts::{self, Account};
use crate::backend::run;
use crate::overlays::MenuItem;
use crate::state::MeshState;
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
        // Nothing offers to add an Azure token.
        Forge::Azure => "",
    }
}

impl SettingsView {
    /// Read the accounts again, as after a change.
    pub(super) fn load_accounts(&mut self, cx: &mut Context<Self>) {
        // Another app may have signed in or out meanwhile.
        self.store.update(cx, |store, cx| store.load_account(cx));
        cx.spawn(async move |this, cx| {
            if let Ok(list) = run(|| Ok(accounts::accounts())).await {
                this.update(cx, |this, cx| {
                    this.accounts.list = list;
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
            let code = SharedString::from(code);
            this.update(cx, |this, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()));
                cx.open_url(GITHUB_DEVICE_PAGE);
                this.accounts.github_code = Some(code.clone());
                cx.notify();
            })
            .ok();
            // This sign-in is the one Settings shows: Cancel, or starting another, ends it,
            // while closing Settings doesn't.
            let current =
                |this: &mut SettingsView| this.accounts.github_code.as_ref() == Some(&code);
            let waited = std::time::Instant::now();
            let outcome = loop {
                cx.background_executor().timer(SIGN_IN_POLL).await;
                if !this.update(cx, |this, _| current(this)).unwrap_or(true) {
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
                if current(this) {
                    this.accounts.github_code = None;
                }
                this.load_accounts(cx);
            })
            .ok();
        })
        .detach();
    }

    /// GitButler's sign-in: its page shows a token to paste back here.
    pub(super) fn log_in_to_gitbutler(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                                store.snack(format!("Signed in to GitButler as {}", who.name), cx)
                            }),
                            Err(err) => this.error("Couldn't sign in", err, cx),
                        }
                        // Whether AI can use GitButler's key changed with it.
                        this.forget_ai();
                        this.store.update(cx, |store, cx| store.load_account(cx));
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

    /// The GitButler account, which General leads with, as Lite's does.
    pub(super) fn gitbutler_row(&self, window: &Window, cx: &Context<Self>) -> Row {
        let theme = Theme::get(cx);
        let user = self.store.read(cx).account.user.as_ref();
        let gitbutler: AnyElement = match (user, &self.accounts.login) {
            (_, Some(input)) => token_field(input, None, window, theme, cx),
            (Some(_), None) => button("log-out", "Sign out")
                .small()
                .style(ButtonStyle::Ghost)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.forget_ai();
                    this.store.update(cx, |store, cx| store.sign_out(cx))
                }))
                .into_any_element(),
            (None, None) => button("log-in", "Sign in…")
                .small()
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.log_in_to_gitbutler(window, cx)
                }))
                .into_any_element(),
        };
        let description: SharedString = match (user, &self.accounts.login) {
            (_, Some(_)) => {
                "Sign in on the page that opened, then paste the token it shows.".into()
            }
            (Some(user), None) => {
                format!("Signed in as {}. AI can use GitButler's key.", user.name).into()
            }
            (None, None) => "Sign in to use GitButler's AI key, with no key of your own.".into(),
        };
        Row::new("GitButler", description, gitbutler)
    }

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
                                this.app
                                    .update(cx, |app, cx| {
                                        app.confirm_then(
                                            message,
                                            "Its daemon stops, and your other machines no longer \
                                             see its work. Its repositories stay as they are.",
                                            "Sign out",
                                            window,
                                            cx,
                                            move |_, cx| {
                                                store
                                                    .update(cx, |store, cx| store.mesh_sign_out(cx))
                                            },
                                        )
                                    })
                                    .ok();
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

    /// Integrations: the forge accounts connected, then a way to add one.
    pub(super) fn integration_cards(&self, window: &Window, cx: &Context<Self>) -> Vec<Card> {
        let theme = Theme::get(cx);
        let mut connected = Vec::new();
        for account in &self.accounts.list {
            let id = account.id.clone();
            let forget = button(
                SharedString::from(format!("forget-{}", account.label)),
                "Forget",
            )
            .small()
            .style(ButtonStyle::Ghost)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let id = id.clone();
                this.apply(
                    "Couldn't forget the account",
                    move || accounts::forget(id),
                    Self::load_accounts,
                    cx,
                )
            }));
            connected.push(Row::new(
                account.forge.name(),
                account.label.clone(),
                forget,
            ));
        }
        let mut rows = Vec::new();
        if let Some(code) = &self.accounts.github_code {
            let copied = code.clone();
            rows.push(Row::new(
                "Signing in to GitHub",
                format!("Enter {code} on the page that opened. It's on your clipboard."),
                h_flex()
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
                    ),
            ));
        }
        match &self.accounts.adding {
            Some((forge, token, email)) => rows.push(Row::new(
                format!("New {} token", forge.name()),
                token_help(*forge),
                token_field(
                    token,
                    (*forge == Forge::Bitbucket).then_some(email),
                    window,
                    theme,
                    cx,
                ),
            )),
            None => {
                let this = cx.entity().downgrade();
                let menu = cx.entity().downgrade();
                rows.push(Row::new(
                    "Add an account",
                    "For pull requests and CI on GitHub, GitLab or Bitbucket. GitButler's other \
                     apps share them.",
                    button("add-account", "Add…")
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
                        }),
                ));
            }
        }
        vec![Card::new(None, connected), Card::new(None, rows)]
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
