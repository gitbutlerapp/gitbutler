//! Agent skill formats, coding-agent detection, and the steering policy blocks
//! that GitButler writes into agent instruction files.
//!
//! This crate holds everything `but agent setup` and `but skill` need that is
//! not tied to a terminal, so the desktop app can offer the same install,
//! uninstall, and customization surface as the CLI. The interactive wizard and
//! all themed output stay in the `but` binary.

pub mod cleanup;
pub mod cli_link;
pub mod detect;
pub mod files;
pub mod format;
#[cfg(test)]
mod format_tests;
pub mod framework;
pub mod freshness;
pub mod install;
pub mod plan;
pub mod policy;
#[cfg(test)]
mod setup_tests;
pub mod status;
pub mod target;

pub use plan::{RepoInfo, Scope};

/// The version stamped into an installed `SKILL.md`, and the version installed
/// skills are compared against. Set at build time by the release scripts.
pub fn cli_version() -> &'static str {
    option_env!("VERSION").unwrap_or("dev")
}
