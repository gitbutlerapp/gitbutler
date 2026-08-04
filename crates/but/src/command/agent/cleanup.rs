//! Agent-facing notice for the retired-syntax cleanup sweep.
//!
//! The sweep itself lives in `but_skill::cleanup`; this only decides whether to
//! run it and renders the themed message. The agent that triggered the cleanup
//! loaded the stale rules at session start, so the notice restates the current
//! syntax instead of only pointing at the rewritten files.

use std::path::PathBuf;

use but_skill::{cleanup, policy};

use crate::{theme, utils::detect_agent};

/// Rewrite the retired commit bullet in every global instruction file
/// `but agent setup` could have written, and return an agent-facing notice
/// when something changed. Returns `None` when no agent is driving or nothing
/// needed rewriting.
///
/// Infallible by construction: per-file errors are logged and skipped inside
/// `cleanup_files`, and a panic anywhere in the sweep is caught here, so this
/// maintenance pass can never take down the command that triggered it.
pub(crate) fn retired_policy_syntax_notice() -> Option<String> {
    detect_agent::detect()?;
    std::panic::catch_unwind(|| {
        let changed = cleanup::cleanup_files(&cleanup::candidate_files());
        (!changed.is_empty()).then(|| notice(&changed))
    })
    .unwrap_or_else(|_| {
        tracing::debug!("retired policy syntax cleanup panicked");
        None
    })
}

fn notice(changed: &[PathBuf]) -> String {
    let t = theme::get();
    let paths = changed
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} Retired `but commit` syntax was removed from your GitButler workflow rules ({paths}).\n\
         The rule is now: {}\n\
         Follow the updated syntax; the rules text loaded into your context is outdated.",
        t.sym().success,
        policy::FAST_PATH_BULLET,
    )
}
