//! Read-only terminal UI for observing `but link` coordination state.

mod app;
mod render;
mod terminal;

use std::io::{self, IsTerminal as _};

/// Run the read-only `but link` terminal UI.
///
/// # Errors
///
/// Returns an error when a TTY is unavailable or the link database cannot be opened.
pub(crate) fn run(ctx: &but_ctx::Context) -> anyhow::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        anyhow::bail!("but link tui requires an interactive terminal (TTY)");
    }

    let db_path = crate::db::db_path(&ctx.project_data_dir());
    if !db_path.is_file() {
        anyhow::bail!(
            "no link database found at {} (run a `but link` command first)",
            db_path.display()
        );
    }

    let mut guard = terminal::TerminalGuard::new()?;
    app::run_event_loop(guard.terminal_mut(), &db_path)
}
