//! Warn when a mutation command leaves commits newly conflicted.
//!
//! GitButler operations never stop on conflicts; rebased commits are recorded
//! in a conflicted state instead. Commands that edit history capture a
//! [`ConflictSnapshot`] before mutating and call [`report_newly_conflicted`]
//! afterwards so the user learns about new conflicts directly from the command
//! output instead of the next `but status`.

use std::collections::HashSet;

use but_core::{ChangeId, commit::CommitIdentifiers};
use but_ctx::Context;
use gix::prelude::ObjectIdExt as _;
use itertools::Itertools as _;

use crate::{
    args::OutputFormat,
    id::CommitIdRef,
    theme::{self, Paint},
    utils::OutputChannel,
};

/// The conflicted workspace commits captured before a mutation ran.
///
/// Commits are identified by change-id, which survives rebases for commits
/// carrying the change-id header. Commits without it fall back to an identity
/// derived from their sha, so such a commit that stays conflicted across a
/// rebase is re-reported as new; GitButler writes the header on every commit
/// it creates, so this only affects commits from external sources.
pub(crate) struct ConflictSnapshot {
    /// `None` when the snapshot could not be taken; reporting is skipped then
    /// so pre-existing conflicts aren't misreported as new ones.
    change_ids: Option<HashSet<ChangeId>>,
}

/// Capture the conflicted workspace commits before a mutation. Best-effort;
/// a failure disables reporting instead of failing the command.
pub(crate) fn snapshot(ctx: &Context) -> ConflictSnapshot {
    let commits = conflicted_workspace_commits(ctx);
    // Enumeration cached a workspace projection taken under a shared guard.
    // Drop it so the command that runs next re-projects under its own lock
    // instead of operating on a potentially outdated view. Failure means a
    // workspace borrow is still live, which no caller of this function does;
    // disabling the report is all that can be done for it here, the stale
    // cache entry itself cannot be removed.
    let invalidated = ctx.invalidate_workspace_cache();
    let change_ids = match (commits, invalidated) {
        (Ok(commits), Ok(())) => Some(
            commits
                .into_iter()
                .map(|commit| commit.inner.change_id)
                .collect(),
        ),
        (Err(err), _) | (_, Err(err)) => {
            tracing::warn!(
                ?err,
                "could not capture conflicted commits before the operation"
            );
            None
        }
    };
    ConflictSnapshot { change_ids }
}

/// Warn about checkout conflict.
pub(crate) fn report_checkout_conflict(out: &mut OutputChannel) {
    if let Err(err) = try_report_checkout_conflict(out) {
        tracing::warn!(?err, "could not report checkout_conflict");
    }
}

fn try_report_checkout_conflict(out: &mut OutputChannel) -> anyhow::Result<()> {
    let t = theme::get();
    if let Some(out) = out.for_human() {
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            t.attention.paint(
                "⚠ A conflict occurred during checkout. Run `but status` for more information."
            )
        )?;
    } else if matches!(out.format(), OutputFormat::Json) {
        // JSON outputs are parsed, so the warning goes to stderr.
        eprintln!(
            "warning: A conflict occurred during checkout. Run `but status` for more information.",
        );
    }
    Ok(())
}

/// Warn about commits that are conflicted now but weren't in `before`.
/// Best-effort; the mutation already succeeded, so errors are only logged.
pub(crate) fn report_newly_conflicted(
    ctx: &Context,
    out: &mut OutputChannel,
    before: ConflictSnapshot,
) {
    if let Err(err) = try_report_newly_conflicted(ctx, out, before) {
        tracing::warn!(?err, "could not report newly conflicted commits");
    }
}

fn try_report_newly_conflicted(
    ctx: &Context,
    out: &mut OutputChannel,
    before: ConflictSnapshot,
) -> anyhow::Result<()> {
    let Some(before) = before.change_ids else {
        return Ok(());
    };
    let newly: Vec<_> = conflicted_workspace_commits(ctx)?
        .into_iter()
        .filter(|commit| !before.contains(&commit.inner.change_id))
        .collect();
    if newly.is_empty() {
        return Ok(());
    }

    let t = theme::get();
    if let Some(out) = out.for_human() {
        let commits = match newly.len() {
            1 => "a commit".to_string(),
            n => format!("{n} commits"),
        };
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            t.attention
                .paint(format!("⚠ This operation left {commits} conflicted:"))
        )?;
        for commit in &newly {
            writeln!(
                out,
                "  {} {} {}",
                t.sym().dot.error(),
                theme::Commit(CommitIdRef::from(&commit.inner)),
                commit.message
            )?;
        }
        writeln!(
            out,
            "Resolve with {}, or back out with {}.",
            t.command_suggestion.paint("but resolve"),
            t.command_suggestion.paint("but undo")
        )?;
    } else if matches!(out.format(), OutputFormat::Json) {
        // JSON outputs are parsed, so the warning goes to stderr.
        let ids = newly
            .iter()
            .map(|commit| commit.inner.id.to_hex_with_len(7))
            .join(", ");
        eprintln!(
            "warning: this operation left {} commit(s) conflicted: {ids}. Resolve with `but resolve`, or back out with `but undo`.",
            newly.len()
        );
    }
    Ok(())
}

struct ConflictedCommit {
    inner: CommitIdentifiers,
    /// First line of the commit message, sanitized for terminal output.
    message: String,
}

/// All conflicted commits currently in the workspace, oldest first within each
/// stack so listings match the bottom-up resolution order. Each commit appears
/// once overall: the traversal visits segments shared between stacks once per
/// stack, so their commits are deduplicated by id.
fn conflicted_workspace_commits(ctx: &Context) -> anyhow::Result<Vec<ConflictedCommit>> {
    let guard = ctx.shared_worktree_access();
    let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;

    let mut seen = HashSet::new();
    let mut conflicted = Vec::new();
    for stack_commit in ws
        .stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .flat_map(|segment| &segment.commits)
    {
        if !seen.insert(stack_commit.id) {
            continue;
        }
        let commit = but_core::Commit::from_id(stack_commit.id.attach(&repo))?;
        if commit.is_conflicted() {
            conflicted.push(ConflictedCommit {
                inner: CommitIdentifiers {
                    id: stack_commit.id,
                    change_id: commit.change_id(),
                },
                message: message_excerpt(&commit),
            });
        }
    }
    // The traversal walks each stack newest first; resolution proceeds oldest
    // first, so flip the order. Stacks are independent of each other, so the
    // reversed stack order does not matter.
    conflicted.reverse();
    Ok(conflicted)
}

/// First line of the commit message, stripped of control characters as the
/// message can come from untrusted upstreams, and shortened for display.
fn message_excerpt(commit: &but_core::Commit<'_>) -> String {
    use bstr::ByteSlice as _;
    commit
        .inner
        .message
        .lines()
        .next()
        .map(|line| line.to_str_lossy())
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(50)
        .collect()
}
