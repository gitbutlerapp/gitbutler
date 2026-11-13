//! One-op functions for JSON clients (Tauri, `but-server`, the SDK) and for callers that do a single
//! action. Rust clients that group workspace mutations into one undo step compose
//! [`but_transaction::Transaction`] methods instead.
//!
//! ### Coordinating Filesystem Access
//!
//! For them to behave correctly in multi-threaded scenarios, be sure to use an *exclusive or shared* lock
//! on this level.
//! Lower-level crates like `but-workspace` won't use filesystem-based locking beyond what Git offers natively.
#![forbid(unsafe_code)]

#[cfg(feature = "legacy")]
pub mod legacy;

/// Functions for GitHub authentication.
pub mod github;

/// Functions for GitLab authentication.
pub mod gitlab;

/// Functions for Bitbucket authentication.
pub mod bitbucket;

pub mod branch;

pub mod workspace;

/// Listing target-branch history relative to the workspace.
pub mod target_commits;

/// Land a branch directly onto the target ref (the "avoid pull requests" workflow).
///
/// Gated behind `legacy` for now because the real-remote push relies on the legacy `gitbutler-git`
/// push helper; the boundary itself is modern (ref input, [`WorkspaceState`] output).
#[cfg(feature = "legacy")]
pub mod land;

/// Ephemeral comments anchored to lines in diffs, shared between the GUI and the CLI.
pub mod comments;

/// Commit mutations.
///
/// Each runs as one transaction and records one undo step when it lands. With `dry_run` it
/// returns a preview of the resulting workspace and records nothing. A function without `perm`
/// takes the lock from `ctx`, and its `_with_perm` variant runs under the caller's.
pub mod commit;

/// Functions that operate on linked git worktrees (experimental).
pub mod worktrees;

/// Resolve the conflicts of a conflicted commit with an LLM.
pub mod resolve;

/// Functions that show what changed in various Git entities, like trees, commits and the worktree.
pub mod diff;

/// Types meant to be serialised to JSON, without degenerating information despite the need to be UTF-8 encodable.
/// EXPERIMENTAL
pub mod json;

pub mod platform;

pub mod open;

pub mod panic_capture;

pub mod ai;

pub use but_transaction::{WorkspaceState, workspace_state};

#[cfg(test)]
mod tests {
    use but_api_macros::but_api;
    use but_testsupport::{CommandExt, git_at_dir, open_repo};

    #[but_api(refreshes)]
    pub fn probe(_ctx: &but_ctx::Context, succeed: bool) -> anyhow::Result<()> {
        if succeed {
            Ok(())
        } else {
            Err(anyhow::anyhow!("the mutation failed"))
        }
    }

    #[test]
    fn a_refreshing_mutation_writes_the_sentinel_only_on_success() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        git_at_dir(tmp.path()).args(["init"]).run();
        let ctx = but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?;
        let sentinel = ctx.project_data_dir.join("REFRESH");
        // A project in use has its storage directory; a fresh repository doesn't yet.
        std::fs::create_dir_all(&ctx.project_data_dir)?;

        probe(&ctx, false).unwrap_err();
        assert!(!sentinel.exists(), "a failed mutation changed nothing");

        probe(&ctx, true)?;
        assert_eq!(
            std::fs::read_to_string(&sentinel)?,
            but_project_handle::process_sentinel_token(),
            "the writer signs the sentinel so its own watcher can skip it"
        );
        Ok(())
    }
}
