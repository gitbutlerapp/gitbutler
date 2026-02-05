//! Claude Code hook for workspace awareness and skill activation.
//!
//! Outputs workspace status as JSON plus a skill-loading nudge.
//! Intended to fire on the `Stop` hook so the agent sees what changed
//! and is reminded to use the `but` skill for version control.
//!
//! Design: best-effort, never fail. Always exits 0 — errors propagate
//! to a `catch_unwind` boundary so panics (e.g. broken pipe) cannot escape.
use std::io::Write as _;

use bstr::ByteSlice;
use but_core::TreeStatusKind;
use serde::Serialize;

/// Main entry point for `but eval-hook`.
///
/// Uses `catch_unwind` to enforce the "never fail" contract — even panics
/// (e.g. from broken pipe on stdout/stderr) are caught and silently ignored.
pub fn execute() {
    let _ = std::panic::catch_unwind(output_status);
}

/// Workspace status as returned by the hook.
/// Contains worktree changes (from `worktree_changes_no_renames()`) and
/// branch/stack info (from `head_info()`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HookStatus {
    uncommitted_file_count: usize,
    uncommitted_files: Vec<FileChange>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stacks: Vec<StackInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileChange {
    path: String,
    change_type: &'static str,
}

/// A stack with its branches and commit counts — enough for the agent
/// to understand the workspace shape without the full `but status` payload.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StackInfo {
    branches: Vec<BranchInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BranchInfo {
    name: String,
    commit_count: usize,
    push_status: but_workspace::ui::PushStatus,
}

fn output_status() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let ctx = but_ctx::Context::discover(&cwd)?;
    let repo = ctx.repo.get()?;

    // Worktree changes — skips rename tracking for faster response.
    let worktree_changes = but_core::diff::worktree_changes_no_renames(&repo)?;

    let uncommitted_files: Vec<FileChange> = worktree_changes
        .changes
        .iter()
        .map(|c| FileChange {
            path: c.path.to_str_lossy().into_owned(),
            change_type: match c.status.kind() {
                TreeStatusKind::Addition => "added",
                TreeStatusKind::Deletion => "removed",
                TreeStatusKind::Modification => "modified",
                // Unreachable with no-renames, but kept for exhaustive match.
                TreeStatusKind::Rename => "renamed",
            },
        })
        .collect();

    // Stack/branch info via head_info — graph walk only (expensive_commit_info disabled).
    // Best-effort: if this fails (corrupted refs, etc.), we still report worktree changes.
    let stacks = collect_stacks(&ctx, &repo);

    let status = HookStatus {
        uncommitted_file_count: uncommitted_files.len(),
        uncommitted_files,
        stacks,
    };

    let json = serde_json::to_string(&status)?;

    // Output: JSON status block + human-readable nudge.
    // The JSON gives the agent structured data; the nudge ensures skill loading.
    write!(
        std::io::stdout(),
        "but status: {json}\n\
         → Skill(gitbutler): load the `but` (gitbutler) skill for version control \
         (staging, committing, pushing, branching). Do not use raw git commands."
    )?;
    Ok(())
}

/// Collect stack/branch info from head_info. Returns empty vec on any error.
fn collect_stacks(ctx: &but_ctx::Context, repo: &gix::Repository) -> Vec<StackInfo> {
    let Ok(meta) = ctx.meta() else {
        return Vec::new();
    };
    let info = match but_workspace::head_info(
        repo,
        &meta,
        but_workspace::ref_info::Options {
            expensive_commit_info: false,
            ..Default::default()
        },
    ) {
        Ok(info) => info,
        Err(e) => {
            tracing::debug!(?e, "eval-hook: failed to collect stack info");
            return Vec::new();
        }
    };
    info.stacks
        .iter()
        .map(|stack| StackInfo {
            branches: stack
                .segments
                .iter()
                .filter_map(|seg| {
                    let name = seg.ref_info.as_ref()?.ref_name.shorten().to_string();
                    Some(BranchInfo {
                        name,
                        commit_count: seg.commits.len(),
                        push_status: seg.push_status,
                    })
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_status_serialization_shape() {
        let status = HookStatus {
            uncommitted_file_count: 2,
            uncommitted_files: vec![
                FileChange {
                    path: "foo.rs".into(),
                    change_type: "modified",
                },
                FileChange {
                    path: "bar.rs".into(),
                    change_type: "added",
                },
            ],
            stacks: vec![],
        };
        let json = serde_json::to_string(&status).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["uncommittedFileCount"], 2);
        assert_eq!(v["uncommittedFiles"].as_array().unwrap().len(), 2);
        assert_eq!(v["uncommittedFiles"][0]["changeType"], "modified");
        // Empty stacks are omitted by skip_serializing_if
        assert!(v.get("stacks").is_none());
    }
}
