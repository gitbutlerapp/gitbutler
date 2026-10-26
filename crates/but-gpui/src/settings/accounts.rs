//! Accounts: the GitButler account, on General, and on Integrations the forge accounts pull
//! requests and CI go through, with ways to add one: GitHub's browser sign-in, or a token,
//! for GitHub Enterprise and self-hosted GitLab with the server's URL.

use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, Entity, Focusable as _, SharedString, Window,
    prelude::*,
};

use super::{Card, Row, SettingsView};
use crate::backend::Forge;
use crate::backend::accounts::{self, Account};
use crate::backend::run;
use crate::overlays::MenuItem;
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{ButtonStyle, button, dp, field, h_flex, v_flex};

/// Where GitHub's browser sign-in takes the code.
const GITHUB_DEVICE_PAGE: &str = "https://github.com/login/device";

/// How long to wait for the browser sign-in, asking every few seconds.
const SIGN_IN_WAIT: Duration = Duration::from_secs(15 * 60);
const SIGN_IN_POLL: Duration = Duration::from_secs(5);

/// The accounts as last read, and any sign-in under way.
#[derive(Default)]
pub(super) struct Accounts {
    list: Vec<Account>,
    adding: Option<Adding>,
    /// GitHub's browser sign-in, waiting: the code the user enters there.
    github_code: Option<SharedString>,
    /// GitButler's sign-in, waiting for the token its page shows.
    login: Option<Entity<TextInput>>,
}

/// A token being added, with its fields.
struct Adding {
    forge: Forge,
    /// GitHub Enterprise or self-hosted GitLab, whose URL `other` takes.
    self_hosted: bool,
    token: Entity<TextInput>,
    /// The field before the token, when it's asked for: the server's URL, or the account's
    /// email on Bitbucket.
    other: Entity<TextInput>,
}

impl Adding {
    fn asks_other(&self) -> bool {
        self.self_hosted || self.forge == Forge::Bitbucket
    }

    fn title(&self) -> String {
        match (self.forge, self.self_hosted) {
            (Forge::GitHub, true) => "New GitHub Enterprise token".into(),
            (forge, true) => format!("New self-hosted {} token", forge.name()),
            (forge, false) => format!("New {} token", forge.name()),
        }
    }

    /// Where to make the token, and what it needs.
    fn help(&self) -> &'static str {
        match (self.forge, self.self_hosted) {
            (Forge::GitHub, false) => {
                "Make a classic token with the repo scope at github.com/settings/tokens."
            }
            (Forge::GitHub, true) => {
                "Give the server's API URL, ending in /api/v3, and a classic token with the repo \
                 scope."
            }
            (Forge::GitLab, false) => {
                "Make a token with the api scope at gitlab.com/-/user_settings/personal_access_tokens."
            }
            (Forge::GitLab, true) => "Give the server's URL and a token with the api scope.",
            (Forge::Bitbucket, _) => {
                "Make an API token with scopes at id.atlassian.com/manage-profile/security/api-tokens, \
                 and give the account's email."
            }
            // Nothing offers to add an Azure token.
            (Forge::Azure, _) => "",
        }
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

    fn start_adding(
        &mut self,
        forge: Forge,
        self_hosted: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let token = cx.new(|cx| TextInput::new("Token", false, cx).masked());
        let placeholder = match (forge, self_hosted) {
            (Forge::GitHub, true) => "https://github.example.com/api/v3",
            (_, true) => "https://gitlab.example.com",
            (_, false) => "Email",
        };
        let other = cx.new(|cx| TextInput::new(placeholder, false, cx));
        // Enter adds it and Esc cancels from either field, as the URL or email comes first.
        for field in [&token, &other] {
            cx.subscribe_in(field, window, |this, _, event, window, cx| match event {
                InputEvent::Submit => this.add_token(window, cx),
                InputEvent::Cancel => {
                    this.accounts.adding = None;
                    cx.notify();
                }
                InputEvent::Changed => {}
            })
            .detach();
        }
        let adding = Adding {
            forge,
            self_hosted,
            token,
            other,
        };
        let first = if adding.asks_other() {
            &adding.other
        } else {
            &adding.token
        };
        window.focus(&first.focus_handle(cx), cx);
        self.accounts.adding = Some(adding);
        cx.notify();
    }

    fn add_token(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(adding) = &self.accounts.adding else {
            return;
        };
        // Enter in the URL or email goes on to the token, while there's none, and back to
        // them while they're empty.
        for (field, asked) in [(&adding.token, true), (&adding.other, adding.asks_other())] {
            if asked && field.read(cx).text().trim().is_empty() {
                return window.focus(&field.focus_handle(cx), cx);
            }
        }
        let (forge, self_hosted) = (adding.forge, adding.self_hosted);
        let token = adding.token.read(cx).text().to_owned();
        let other = adding.other.read(cx).text().to_owned();
        self.accounts.adding = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let added = run(move || {
                if self_hosted {
                    accounts::add_self_hosted_token(forge, other, token)
                } else {
                    accounts::add_token(forge, token, other)
                }
            })
            .await;
            this.update(cx, |this, cx| {
                match added {
                    Ok(who) => this.store.update(cx, |store, cx| {
                        store.retry_forges();
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
                Ok(who) => {
                    store.retry_forges();
                    store.snack(format!("Signed in to GitHub as {who}"), cx)
                }
                Err(err) => store.failed("Couldn't sign in to GitHub", err, cx),
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
                if token.trim().is_empty() {
                    return;
                }
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
            (_, Some(input)) => token_field(input, None, "Sign in", window, theme, cx),
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

    /// Integrations: the forge accounts connected, then a way to add one.
    pub(super) fn integration_cards(&self, window: &Window, cx: &Context<Self>) -> Vec<Card> {
        let theme = Theme::get(cx);
        let mut connected = Vec::new();
        for account in &self.accounts.list {
            let id = account.id.clone();
            // By forge too, as a GitHub and a GitLab account can share a username, and a shared
            // id takes the second's clicks as the first's.
            let key = format!("forget-{}-{}", account.forge.name(), account.label);
            let forget = button(SharedString::from(key), "Forget")
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
                    .gap(dp(6.))
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
            Some(adding) => rows.push(Row::new(
                adding.title(),
                adding.help(),
                token_field(
                    &adding.token,
                    adding.asks_other().then_some(&adding.other),
                    "Add",
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
                    "For pull requests and CI on GitHub, GitHub Enterprise, GitLab, self-hosted \
                     GitLab or Bitbucket. GitButler's other apps share them.",
                    button("add-account", "Add…")
                        .small()
                        .on_click(move |event, _, cx| {
                            // A token for a forge, on a server of the user's own when the bool
                            // says so, or GitHub's browser sign-in.
                            let item = |label: &'static str, token: Option<(Forge, bool)>| {
                                let this = this.clone();
                                MenuItem::new(label, move |_, window, cx| {
                                    this.update(cx, |this, cx| match token {
                                        Some((forge, self_hosted)) => {
                                            this.start_adding(forge, self_hosted, window, cx)
                                        }
                                        None => this.sign_in_to_github(cx),
                                    })
                                    .ok();
                                })
                            };
                            let items = vec![
                                item("Sign in with GitHub", None),
                                item("GitHub token…", Some((Forge::GitHub, false))),
                                item("GitHub Enterprise token…", Some((Forge::GitHub, true))),
                                item("GitLab token…", Some((Forge::GitLab, false))),
                                item("Self-hosted GitLab token…", Some((Forge::GitLab, true))),
                                item("Bitbucket token…", Some((Forge::Bitbucket, false))),
                            ];
                            (menu.update(cx, |this, cx| {
                                this.open_menu(event, move |_, _| items, cx)
                            }))
                            .ok();
                        }),
                ));
            }
        }
        vec![Card::new(None, connected), Card::new(None, rows)]
    }
}

/// A masked token field, with another field (a URL or an email) before it when given, and
/// `submit`, which sends it.
fn token_field(
    token: &Entity<TextInput>,
    other: Option<&Entity<TextInput>>,
    submit: &'static str,
    window: &Window,
    theme: &Theme,
    cx: &Context<SettingsView>,
) -> AnyElement {
    let focused = |input: &Entity<TextInput>| input.focus_handle(cx).is_focused(window);
    let buttons = h_flex()
        .gap(dp(6.))
        .child({
            let token = token.clone();
            button("cancel-token", "Cancel")
                .small()
                .style(ButtonStyle::Ghost)
                .on_click(move |_, _, cx| token.update(cx, |_, cx| cx.emit(InputEvent::Cancel)))
        })
        .child({
            let token = token.clone();
            button("add-token", submit)
                .small()
                .style(ButtonStyle::Gray)
                .on_click(move |_, _, cx| token.update(cx, |_, cx| cx.emit(InputEvent::Submit)))
        });
    let token = field(token, focused(token), theme);
    match other {
        // Two fields stack, with the buttons under them, so the row's text keeps its room.
        Some(other) => v_flex()
            .gap(dp(6.))
            .items_end()
            .child(field(other, focused(other), theme).w(dp(240.)))
            .child(token.w(dp(240.)))
            .child(buttons),
        None => h_flex().gap(dp(6.)).child(token.w(dp(180.))).child(buttons),
    }
    .into_any_element()
}
