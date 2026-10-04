//! Signing in to GitButler from the terminal: the sign-in page opens in the browser and shows an
//! access token, which is pasted here and stored where Lite and the CLI read it.

use std::{sync::mpsc, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::tui::{CrosstermTerminalGuard, TerminalGuard};

enum Progress {
    Link(String),
    Done,
    Failed(String),
}

/// Sign in, showing progress until it's done; `false` if the person quit instead.
pub(super) fn run(terminal: &mut CrosstermTerminalGuard, server: &str) -> anyhow::Result<bool> {
    let (tx, rx) = mpsc::channel();
    let link_tx = tx.clone();
    std::thread::spawn(move || {
        let progress = match but_api::legacy::users::get_login_token() {
            Ok(token) => {
                but_api::open::open_url(token.url.clone()).ok();
                Progress::Link(token.url)
            }
            Err(err) => Progress::Failed(format!("{err:#}")),
        };
        link_tx.send(progress).ok();
    });

    let mut link = None;
    let mut failed = None;
    let mut token = String::new();
    let mut checking = false;
    loop {
        for progress in rx.try_iter() {
            match progress {
                Progress::Link(url) => link = Some(url),
                Progress::Done => return Ok(true),
                Progress::Failed(err) => {
                    failed = Some(err);
                    checking = false;
                }
            }
        }
        terminal.terminal_mut().draw(|frame| {
            let bold = Style::default().add_modifier(Modifier::BOLD);
            let dim = Style::default().add_modifier(Modifier::DIM);
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("but mesh", bold),
                    Span::styled(format!(" · {server}"), dim),
                ]),
                Line::default(),
                Line::from("Sign in to GitButler to see your machines."),
            ];
            match &link {
                Some(url) => {
                    lines.push(Line::from(
                        "Opened the sign-in page in your browser. Paste the access token it shows:",
                    ));
                    lines.push(Line::styled(url.clone(), dim));
                }
                None if failed.is_none() => lines.push(Line::from("Getting a sign-in link…")),
                None => {}
            }
            lines.push(Line::default());
            // The token is a secret, so only its length shows.
            lines.push(Line::from(vec![
                Span::styled("Token: ", bold),
                Span::raw("•".repeat(token.chars().count().min(40))),
                Span::styled("▏", dim),
            ]));
            if checking {
                lines.push(Line::from("Checking…"));
            } else if let Some(err) = &failed {
                lines.push(Line::from(format!("Signing in failed: {err}")));
            }
            lines.push(Line::default());
            lines.push(Line::styled("enter sign in · esc quit", dim));
            frame.render_widget(
                Paragraph::new(lines).wrap(Wrap { trim: false }),
                frame.area().inner(ratatui::layout::Margin::new(2, 1)),
            );
        })?;
        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        match event::read()? {
            Event::Paste(text) => token.push_str(text.trim()),
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc => return Ok(false),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(false);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    token.clear()
                }
                KeyCode::Char(c) if !c.is_whitespace() => token.push(c),
                KeyCode::Backspace => {
                    token.pop();
                }
                KeyCode::Enter if !token.is_empty() && !checking => {
                    checking = true;
                    failed = None;
                    let tx = tx.clone();
                    let token = token.clone();
                    std::thread::spawn(move || {
                        let progress = match but_api::legacy::users::login_and_persist(token) {
                            Ok(_) => Progress::Done,
                            Err(err) => Progress::Failed(format!("{err:#}")),
                        };
                        tx.send(progress).ok();
                    });
                }
                _ => {}
            },
            _ => {}
        }
    }
}
