//! Signing in to GitButler as the desktop app does: a link opened in the browser, then polling
//! until the account is there, stored where Lite and the CLI read it.

use std::{sync::mpsc, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
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
    std::thread::spawn(move || {
        let token = match but_api::legacy::users::get_login_token() {
            Ok(token) => token,
            Err(err) => return tx.send(Progress::Failed(format!("{err:#}"))),
        };
        tx.send(Progress::Link(token.url.clone()))?;
        but_api::open::open_url(token.url).ok();
        // The login page hands the account over once it's done there; until then this fails.
        for _ in 0..300 {
            if but_api::legacy::users::login_and_persist(token.token.clone()).is_ok() {
                return tx.send(Progress::Done);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        tx.send(Progress::Failed("Signing in timed out".into()))
    });

    let mut link = None;
    let mut failed = None;
    loop {
        for progress in rx.try_iter() {
            match progress {
                Progress::Link(url) => link = Some(url),
                Progress::Done => return Ok(true),
                Progress::Failed(err) => failed = Some(err),
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
            match (&failed, &link) {
                (Some(err), _) => lines.push(Line::from(format!("Signing in failed: {err}"))),
                (None, Some(url)) => {
                    lines.push(Line::from("Opened the sign-in page in your browser:"));
                    lines.push(Line::styled(url.clone(), dim));
                    lines.push(Line::default());
                    lines.push(Line::from("Waiting for you to finish…"));
                }
                (None, None) => lines.push(Line::from("Getting a sign-in link…")),
            }
            lines.push(Line::default());
            lines.push(Line::styled("q quit", dim));
            frame.render_widget(
                Paragraph::new(lines).wrap(Wrap { trim: false }),
                frame.area().inner(ratatui::layout::Margin::new(2, 1)),
            );
        })?;
        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            return Ok(false);
        }
    }
}
