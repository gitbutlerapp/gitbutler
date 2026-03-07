//! Minimal CLI for `but link` — multi-agent coordination via repo-scoped `SQLite`.

mod claiming;
mod cli;
mod commands;
mod db;
mod payloads;
mod read;
mod repo;
mod text;
mod tui;

pub use cli::Platform;

/// Run the `but link` implementation with already-parsed CLI arguments.
///
/// # Errors
///
/// Returns an error when command execution fails.
pub fn handle(platform: Platform, current_dir: &std::path::Path) -> anyhow::Result<()> {
    commands::run(platform, current_dir).inspect_err(|e| {
        // Print structured JSON error to stdout for machine consumers (agents).
        // The error still propagates for nonzero exit-code and metrics;
        // anyhow's Display goes to stderr which is secondary.
        println!(
            "{}",
            serde_json::json!({ "ok": false, "error": e.to_string() })
        );
    })
}
