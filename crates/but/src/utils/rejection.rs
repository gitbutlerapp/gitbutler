//! Explain why a commit or amend was refused.
//!
//! GitButler allows several independent branches to be checked out at once, and
//! a change that depends on a commit in another branch cannot be committed
//! elsewhere on its own. When that happens the whole transaction rolls back,
//! but the raw outcome only carries a coarse [`RejectionReason`] per change —
//! it does not say which commit or branch a change depends on.
//!
//! This module joins the rejected changes with the workspace hunk dependencies
//! (the same data the desktop app uses to render its "commit failed" modal) so
//! the CLI can tell the user precisely which hunk depends on which branch, and
//! suggest stacking the branches to resolve it.

use bstr::ByteSlice;
use but_core::{DiffSpec, HunkHeader, sync::RepoExclusive, tree::create_tree::RejectionReason};
use but_ctx::Context;
use but_hunk_dependency::rejection::{
    RejectedChange, branch_of_commit, dependency_branch, explain_rejections,
};

use crate::{
    id::CommitId,
    theme::{self, Paint},
};

/// Aborts a transaction because some of the selected changes could not be
/// applied.
///
/// Returned from inside the transaction closure so the whole operation rolls
/// back. The command catches it with [`explain_after_rollback`] to report which
/// change depends on which branch; the `Display` impl keeps the message
/// meaningful if it ever escapes unexplained.
#[derive(Debug)]
pub struct RejectedChanges(pub Vec<(RejectionReason, DiffSpec)>);

impl std::fmt::Display for RejectedChanges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let noun = if self.0.len() == 1 {
            "change"
        } else {
            "changes"
        };
        write!(f, "{} {noun} could not be applied", self.0.len())
    }
}

impl std::error::Error for RejectedChanges {}

/// A rejected operation with its user-facing explanation attached.
#[derive(Debug)]
pub(crate) struct ExplainedRejection(pub(crate) String);

impl std::fmt::Display for ExplainedRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ExplainedRejection {}

/// What the aborted operation was targeting; determines the recovery hint.
pub enum Target {
    /// A commit already in the workspace (amend, `commit --above/--below`).
    Commit(CommitId),
    /// The tip of this existing branch.
    Branch(gix::refs::FullName),
    /// A branch that would have been created; the rollback removed it again.
    /// `None` when the name would have been generated.
    NewBranch(Option<String>),
}

/// If `err` is a [`RejectedChanges`] abort, expand it into a report naming the
/// branch/commit each rejected change depends on, plus a recovery hint. Any
/// other error passes through untouched.
///
/// Must be called after the transaction rolled back: the workspace is then
/// unchanged, which is exactly the state the dependencies are computed against.
pub fn explain_after_rollback(
    ctx: &Context,
    perm: &mut RepoExclusive,
    verb: &str,
    target: Target,
    err: anyhow::Error,
) -> anyhow::Error {
    let rejected = match err.downcast::<RejectedChanges>() {
        Ok(rejected) => rejected,
        Err(other) => return other,
    };

    // Explaining a failure must never mask it: without a workspace projection,
    // fall back to the plain rejection count.

    // The failed operation may have consumed the workspace cache. Invalidate it so that we can
    // re-obtain it.
    if let Err(err) = ctx.invalidate_workspace_cache() {
        tracing::warn!(?err, "Failed to load workspace to explain rejections");
        return anyhow::Error::new(ExplainedRejection(format!("Cannot {verb}: {rejected}")));
    }

    let (repo, ws, _db) = match ctx.workspace_and_db_with_perm(perm.read_permission()) {
        Ok(loaded) => loaded,
        Err(load_err) => {
            tracing::warn!(?load_err, "Failed to load workspace to explain rejections");
            return anyhow::Error::new(ExplainedRejection(format!("Cannot {verb}: {rejected}")));
        }
    };
    let (repo, ws) = (&repo, &ws);

    let target_branch = match &target {
        Target::Commit(commit) => branch_of_commit(ws, commit.commit_id, None),
        Target::Branch(name) => Some(name.clone()),
        Target::NewBranch(_) => None,
    };

    let changes = explain_rejections(
        repo,
        ws,
        &rejected.0,
        target_branch.as_ref().map(|name| name.as_ref()),
    );
    // Stacking on the dependency resolves the rejection, but only when every
    // dependency is in one stack — so frame it as a hint, not a directive.
    let dependency = dependency_branch(ws, &changes).map(|name| name.shorten().to_string());
    let target_branch = target_branch.map(|name| name.shorten().to_string());

    let mut message = format!("Cannot {verb}: {rejected}:\n");
    // Writing to a String cannot fail.
    let _ = write_report(
        &mut message,
        repo,
        &changes,
        &target,
        target_branch.as_deref(),
        dependency.as_deref(),
    );
    anyhow::Error::new(ExplainedRejection(message.trim_end().to_string()))
}

/// The per-change lines and, when there is a `dependency` branch other than
/// the target, a copy-pasteable hint for stacking the target on top of it.
fn write_report<W: std::fmt::Write + ?Sized>(
    out: &mut W,
    repo: &gix::Repository,
    changes: &[RejectedChange],
    target: &Target,
    target_branch: Option<&str>,
    dependency: Option<&str>,
) -> std::fmt::Result {
    let t = theme::get();
    for change in changes {
        writeln!(out, "  {}", change.path.to_str_lossy())?;
        if change.dependencies.is_empty() {
            if change.suspected_branches.is_empty() {
                writeln!(out, "    {}", reason_summary(change.reason))?;
            } else {
                let branches = change
                    .suspected_branches
                    .iter()
                    .map(|branch| {
                        t.local_branch
                            .paint(branch.shorten().to_string())
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    out,
                    "    conflicts with commits on {branches} (edits touch the same file)",
                )?;
            }
            continue;
        }
        for dependency in &change.dependencies {
            let range = hunk_range_label(&dependency.hunk);
            for commit in &dependency.commits {
                let id = match CommitId::try_from_commit_id(commit.commit_id, repo) {
                    Ok(id) => theme::Commit(&id).to_string(),
                    Err(_) => theme::Commit(commit.commit_id).to_string(),
                };
                match &commit.branch {
                    Some(branch) => writeln!(
                        out,
                        "    {range} depends on {} ({id})",
                        t.local_branch.paint(branch.shorten().to_string()),
                    )?,
                    None => writeln!(out, "    {range} depends on commit {id}")?,
                }
            }
        }
    }

    let Some(dependency) = dependency else {
        return Ok(());
    };
    let hint = match (target_branch, target) {
        (Some(target_name), _) if target_name != dependency => Some((
            format!(
                "to apply these changes, stack {} on top of {} and try again — commits already on the branch move with it:",
                t.local_branch.paint(target_name),
                t.local_branch.paint(dependency),
            ),
            format!(
                "but move {} --above {}",
                shell_quote(target_name),
                shell_quote(dependency)
            ),
        )),
        (None, Target::NewBranch(Some(name))) => Some((
            format!(
                "to apply these changes, create {} stacked on top of {} and try again:",
                t.local_branch.paint(name),
                t.local_branch.paint(dependency),
            ),
            format!(
                "but branch new {} --above {}",
                shell_quote(name),
                shell_quote(dependency)
            ),
        )),
        // No hint when the dependency is the target branch itself, or when the
        // target branch's name is unknown (a rolled-back generated name) — a
        // suggested command would silently retarget what the user asked for.
        _ => None,
    };
    if let Some((explanation, command)) = hint {
        writeln!(out)?;
        writeln!(out, "{} {explanation}", t.hint.paint("Hint:"))?;
        writeln!(out, "  {}", t.command_suggestion.paint(command))?;
    }
    Ok(())
}

/// Quote `name` for the suggested shell command if it contains anything a shell
/// would treat specially. Git permits characters like `;` and `&` in ref names,
/// so an unquoted name could otherwise break the copy-pasteable command.
pub(crate) fn shell_quote(name: &str) -> std::borrow::Cow<'_, str> {
    let safe = !name.is_empty()
        && name.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'-' | b'_' | b'/' | b'.' | b'@' | b'+' | b'=' | b':' | b','
                )
        });
    if safe {
        std::borrow::Cow::Borrowed(name)
    } else {
        std::borrow::Cow::Owned(format!("'{}'", name.replace('\'', r"'\''")))
    }
}

/// A short, new-side line-range label for a hunk, e.g. `line 5` or `lines 5–9`.
fn hunk_range_label(hunk: &HunkHeader) -> String {
    match hunk.new_lines {
        0 => format!("around line {}", hunk.new_start),
        1 => format!("line {}", hunk.new_start),
        lines => format!(
            "lines {}–{}",
            hunk.new_start,
            hunk.new_start.saturating_add(lines - 1)
        ),
    }
}

/// A plain-language summary for rejections that are not dependency-related.
fn reason_summary(reason: RejectionReason) -> &'static str {
    match reason {
        RejectionReason::NoEffectiveChanges => "no effective change to commit",
        RejectionReason::CherryPickMergeConflict | RejectionReason::WorkspaceMergeConflict => {
            "depends on changes in another branch"
        }
        RejectionReason::WorkspaceMergeConflictOfUnrelatedFile => {
            "conflicts with another change in the workspace"
        }
        RejectionReason::WorktreeFileMissingForObjectConversion => {
            "the file went missing while committing"
        }
        RejectionReason::FileToLargeOrBinary => "the file is too large or binary",
        RejectionReason::PathNotFoundInBaseTree => "the change was not found in the base tree",
        RejectionReason::UnsupportedDirectoryEntry => "unsupported directory entry",
        RejectionReason::UnsupportedTreeEntry => "unsupported file type",
        RejectionReason::MissingDiffSpecAssociation => {
            "the selected hunks no longer match the worktree"
        }
    }
}
