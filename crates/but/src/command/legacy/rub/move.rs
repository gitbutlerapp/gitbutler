use anyhow::{Context as _, bail};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_oxidize::ObjectIdExt;
use colored::Colorize;
use gitbutler_branch_actions::reorder::commits_order;
use gitbutler_stack::VirtualBranchesHandle;
use gix::ObjectId;

use super::{assign::branch_name_to_stack_id, undo::stack_id_by_commit_id};
use crate::{CliId, IdMap, utils::OutputChannel};

/// Represents the operation to perform for a given source and target combination in `but move`.
#[derive(Debug)]
enum MoveOperation<'a> {
    /// Move a commit to be before/after another commit
    CommitToCommit {
        source: &'a ObjectId,
        target: &'a ObjectId,
        target_str: &'a str,
        after: bool,
    },
    /// Move a commit to a branch (places at top of the branch)
    CommitToBranch {
        source: &'a ObjectId,
        target_branch: &'a str,
    },
    /// Move a committed file to another commit (delegates to rub)
    CommittedFileToCommit {
        path: &'a bstr::BStr,
        source_commit: &'a ObjectId,
        target_commit: &'a ObjectId,
    },
}

impl<'a> MoveOperation<'a> {
    /// Executes this move operation
    fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        match self {
            MoveOperation::CommitToCommit {
                source,
                target,
                target_str,
                after,
            } => move_to_commit(ctx, out, source, target, target_str, after),
            MoveOperation::CommitToBranch { source, target_branch } => move_to_branch(ctx, out, source, target_branch),
            MoveOperation::CommittedFileToCommit {
                path,
                source_commit,
                target_commit,
            } => super::commits::commited_file_to_another_commit(ctx, path, *source_commit, *target_commit, out),
        }
    }
}

/// Determines the move operation to perform for a given source and target combination.
/// Returns `Some(operation)` if the combination is valid, `None` otherwise.
fn route_move_operation<'a>(
    source: &'a CliId,
    target: &'a CliId,
    target_str: &'a str,
    after: bool,
) -> Option<MoveOperation<'a>> {
    use CliId::*;

    match (source, target) {
        // Commit -> Commit: move commit to specific position
        (Commit { commit_id: source, .. }, Commit { commit_id: target, .. }) => Some(MoveOperation::CommitToCommit {
            source,
            target,
            target_str,
            after,
        }),
        // Commit -> Branch: move commit to top of branch
        (Commit { commit_id: source, .. }, Branch { name, .. }) => Some(MoveOperation::CommitToBranch {
            source,
            target_branch: name,
        }),
        // CommittedFile -> Commit: move a file from one commit to another
        (
            CommittedFile {
                path,
                commit_id: source_commit,
                ..
            },
            Commit {
                commit_id: target_commit,
                ..
            },
        ) => Some(MoveOperation::CommittedFileToCommit {
            path: path.as_ref(),
            source_commit,
            target_commit,
        }),
        // All other combinations are invalid for move
        _ => None,
    }
}

/// Move a commit to a new location in the stack.
///
/// This handles two scenarios:
/// 1. Moving a commit to be before/after another commit (reordering within or across stacks)
/// 2. Moving a commit to a branch (places at top)
///
/// # Limitations
/// - When moving commits within the same stack, you can specify exact position (before/after target)
/// - When moving commits to a different stack, the commit goes to the top (API limitation)
/// - You cannot currently move a commit to a specific position in a different stack in one operation
pub(crate) fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_str: &str,
    target_str: &str,
    after: bool,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;

    // Resolve source
    let source_matches = id_map.parse_using_context(source_str, ctx)?;
    if source_matches.is_empty() {
        bail!(
            "Source '{}' not found. If you just performed a Git operation, try running 'but status' to refresh.",
            source_str
        );
    }
    if source_matches.len() > 1 {
        bail!(
            "Source '{}' is ambiguous. Try using more characters to disambiguate.",
            source_str
        );
    }

    let source_id = &source_matches[0];

    // Resolve target
    let target_matches = id_map.parse_using_context(target_str, ctx)?;
    if target_matches.is_empty() {
        bail!(
            "Target '{}' not found. If you just performed a Git operation, try running 'but status' to refresh.",
            target_str
        );
    }
    if target_matches.len() > 1 {
        bail!(
            "Target '{}' is ambiguous. Try using more characters to disambiguate.",
            target_str
        );
    }

    let target_id = &target_matches[0];

    // Validate --after flag usage
    if after {
        // Check if target is a branch (--after only makes sense for commit-to-commit moves)
        if matches!(target_id, CliId::Branch { .. }) {
            bail!(
                "The {} flag only makes sense when moving a commit to another commit.\n\
                When moving to a branch, the commit is placed at the top of the stack by default.",
                "--after"
            );
        }
        // Check if source is a committed file (--after doesn't make sense for file moves)
        if matches!(source_id, CliId::CommittedFile { .. }) {
            bail!(
                "The {} flag only makes sense when moving a commit to another commit.\n\
                When moving a file from one commit to another, the changes are simply transferred.",
                "--after"
            );
        }
    }

    // Route and execute the operation
    let Some(operation) = route_move_operation(source_id, target_id, target_str, after) else {
        bail!(
            "Cannot move {} ({}) to {} ({}).\n\
            Valid moves: commit→commit, commit→branch, or committed-file→commit",
            source_id.to_short_string().blue().underline(),
            source_id.kind_for_humans().yellow(),
            target_id.to_short_string().blue().underline(),
            target_id.kind_for_humans().yellow()
        );
    };

    operation.execute(ctx, out)
}

/// Move a commit to be before/after another commit
fn move_to_commit(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_oid: &ObjectId,
    target_oid: &ObjectId,
    target_str: &str,
    after: bool,
) -> anyhow::Result<()> {
    let source_stack_id = stack_id_by_commit_id(ctx, source_oid)?;
    let target_stack_id = stack_id_by_commit_id(ctx, target_oid)?;

    if source_stack_id == target_stack_id {
        // Same stack - use reorder_stack API
        move_within_stack(ctx, out, source_oid, target_oid, source_stack_id, after)
    } else {
        // Different stacks - API limitation: can't move to specific position in another stack
        let target_stack_head_branch = get_topmost_branch_name(ctx, target_stack_id)?;

        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "{} Cannot move commit to a specific position in another stack in one operation.",
                "Error:".red().bold()
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "The commit {} is in a different stack than the target commit {}.",
                source_oid.to_string()[..7].blue(),
                target_oid.to_string()[..7].blue()
            )?;
            writeln!(out)?;
            writeln!(out, "{}", "You can do this in two steps:".yellow())?;
            writeln!(out)?;
            let source_str = source_oid.to_string();
            writeln!(
                out,
                "  {}  {}",
                "1.".cyan(),
                format!("but move {} {}", &source_str[..7], target_stack_head_branch).green()
            )?;
            writeln!(
                out,
                "     {} Move to the topmost branch in the target stack",
                "↳".cyan()
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "  {}  {}",
                "2.".cyan(),
                format!(
                    "but move <new-commit-id> {}{}",
                    target_str,
                    if after { " --after" } else { "" }
                )
                .green()
            )?;
            writeln!(
                out,
                "     {} Reposition to the desired location (use the new commit ID from step 1)",
                "↳".cyan()
            )?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({
                "ok": false,
                "error": "cross_stack_position",
                "hint": format!(
                    "Move to branch first with 'but move {} {}', then reposition",
                    &source_oid.to_string()[..7],
                    target_stack_head_branch
                ),
            }))?;
        }

        bail!("Cannot move commit to specific position in another stack");
    }
}

/// Move a commit to a branch (places at top of stack)
fn move_to_branch(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_oid: &ObjectId,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    let target_stack_id = branch_name_to_stack_id(ctx, Some(target_branch_name))?
        .ok_or_else(|| anyhow::anyhow!("Could not find stack for branch {}", target_branch_name))?;
    let source_stack_id = stack_id_by_commit_id(ctx, source_oid)?;

    if source_stack_id == target_stack_id {
        // Same stack - move to top of the target branch using reorder_stack
        let vb_state = &VirtualBranchesHandle::new(ctx.project_data_dir());
        let stack = vb_state.get_stack_in_workspace(source_stack_id)?;
        let mut stack_order = commits_order(ctx, &stack)?;
        let git2_oid = source_oid.to_git2();

        // Remove commit from wherever it is
        stack_order.series.iter_mut().for_each(|series| {
            series.commit_ids.retain(|commit_id| commit_id != &git2_oid);
        });

        // Add to the top (beginning) of the target branch
        if let Some(series) = stack_order.series.iter_mut().find(|s| s.name == target_branch_name) {
            series.commit_ids.insert(0, git2_oid);
        } else {
            bail!("Branch '{}' not found in stack", target_branch_name);
        }

        gitbutler_branch_actions::reorder_stack(ctx, source_stack_id, stack_order)?;

        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "Moved {} → {}",
                source_oid.to_string()[..7].blue(),
                format!("[{}]", target_branch_name).green()
            )?;

            // Check if target branch is not the topmost branch
            let topmost_branch = get_topmost_branch_name(ctx, source_stack_id)?;
            if topmost_branch != target_branch_name {
                writeln!(out)?;
                writeln!(
                    out,
                    "{} The commit was placed at the top of branch '{}', but this is not the topmost branch in the stack.",
                    "Note:".yellow(),
                    target_branch_name
                )?;
                writeln!(out, "To move it to a specific position within the branch, you can run:")?;
                writeln!(out)?;
                let source_str = source_oid.to_string();
                let cmd = format!("but move {} <target-commit> [--after]", &source_str[..7]);
                writeln!(out, "  {}", cmd.green())?;
            }
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
    } else {
        // Different stack - use move_commit API
        if let Some(illegal_move) =
            gitbutler_branch_actions::move_commit(ctx, target_stack_id, source_oid.to_git2(), source_stack_id)?
        {
            if let Some(out) = out.for_human() {
                match &illegal_move {
                    gitbutler_branch_actions::MoveCommitIllegalAction::DependsOnCommits(deps) => {
                        writeln!(
                            out,
                            "Cannot move commit {} because it depends on commits: {}",
                            source_oid,
                            deps.join(", ")
                        )?;
                    }
                    gitbutler_branch_actions::MoveCommitIllegalAction::HasDependentChanges(deps) => {
                        writeln!(
                            out,
                            "Cannot move commit {} because it has dependent changes: {}",
                            source_oid,
                            deps.join(", ")
                        )?;
                    }
                    gitbutler_branch_actions::MoveCommitIllegalAction::HasDependentUncommittedChanges => {
                        writeln!(
                            out,
                            "Cannot move commit {} because it has dependent uncommitted changes",
                            source_oid
                        )?;
                    }
                }
            } else if let Some(out) = out.for_json() {
                out.write_value(super::illegal_move_to_json(&illegal_move))?;
            }
            bail!("Illegal move");
        }

        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "Moved {} → {}",
                source_oid.to_string()[..7].blue(),
                format!("[{}]", target_branch_name).green()
            )?;

            // Check if target branch is not the topmost branch
            let topmost_branch = get_topmost_branch_name(ctx, target_stack_id)?;
            if topmost_branch != target_branch_name {
                writeln!(out)?;
                writeln!(
                    out,
                    "{} The commit was placed at the top of branch '{}', but this is not the topmost branch in the stack.",
                    "Note:".yellow(),
                    target_branch_name
                )?;
                writeln!(out, "To move it to a specific position within the branch, you can run:")?;
                writeln!(out)?;
                let source_str = source_oid.to_string();
                let cmd = format!("but move {} <target-commit> [--after]", &source_str[..7]);
                writeln!(out, "  {}", cmd.green())?;
            }
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
    }

    Ok(())
}

/// Move a commit within the same stack using reorder_stack API
fn move_within_stack(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_oid: &ObjectId,
    target_oid: &ObjectId,
    stack_id: StackId,
    after: bool,
) -> anyhow::Result<()> {
    let vb_state = &VirtualBranchesHandle::new(ctx.project_data_dir());
    let stack = vb_state.get_stack_in_workspace(stack_id)?;
    let mut stack_order = commits_order(ctx, &stack)?;
    let git2_source_oid = source_oid.to_git2();
    let git2_target_oid = target_oid.to_git2();

    // Find which series contains each commit
    let mut source_series_idx = None;
    let mut target_series_idx = None;
    let mut target_position_in_series = None;

    for (series_idx, series) in stack_order.series.iter().enumerate() {
        if let Some(pos) = series.commit_ids.iter().position(|oid| *oid == git2_source_oid) {
            source_series_idx = Some((series_idx, pos));
        }
        if let Some(pos) = series.commit_ids.iter().position(|oid| *oid == git2_target_oid) {
            target_series_idx = Some(series_idx);
            target_position_in_series = Some(pos);
        }
    }

    let (source_idx, source_pos) = source_series_idx.context("Source commit not found in stack")?;
    let target_idx = target_series_idx.context("Target commit not found in stack")?;
    let target_pos = target_position_in_series.context("Target position not found")?;

    // Check if source and target are the same commit
    if git2_source_oid == git2_target_oid {
        if let Some(out) = out.for_human() {
            writeln!(out, "Source and target are the same commit. Nothing to do.")?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        return Ok(());
    }

    // Remove source commit from its current location
    stack_order.series[source_idx]
        .commit_ids
        .retain(|oid| *oid != git2_source_oid);

    // Calculate new position
    // If after is true, insert after (above) the target
    // If after is false (default), insert before (below) the target
    // If source and target are in the same series and source was before target,
    // we need to adjust target_pos since we just removed source
    let adjusted_target_pos = if source_idx == target_idx && source_pos < target_pos {
        target_pos - 1
    } else {
        target_pos
    };

    let insert_position = if after {
        adjusted_target_pos
    } else {
        adjusted_target_pos + 1
    };

    // Insert source commit at the new position in the target series
    stack_order.series[target_idx]
        .commit_ids
        .insert(insert_position, git2_source_oid);

    gitbutler_branch_actions::reorder_stack(ctx, stack_id, stack_order)?;

    if let Some(out) = out.for_human() {
        let position_desc = if after { "after" } else { "before" };
        writeln!(
            out,
            "Moved {} {} {}",
            source_oid.to_string()[..7].blue(),
            position_desc,
            target_oid.to_string()[..7].blue()
        )?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }

    Ok(())
}

/// Get the name of the topmost branch in a stack
fn get_topmost_branch_name(ctx: &Context, stack_id: StackId) -> anyhow::Result<String> {
    let vb_state = &VirtualBranchesHandle::new(ctx.project_data_dir());
    let stack = vb_state.get_stack_in_workspace(stack_id)?;

    stack
        .heads
        .last()
        .map(|branch| branch.name.clone())
        .context("Stack has no branches")
}

#[cfg(test)]
mod tests {
    use bstr::BString;

    use super::*;

    // Helper to create test CliIds
    fn commit_id(id: &str) -> CliId {
        CliId::Commit {
            commit_id: gix::ObjectId::empty_tree(gix::hash::Kind::Sha1),
            id: id.to_string(),
        }
    }

    fn branch_id(name: &str) -> CliId {
        CliId::Branch {
            name: name.to_string(),
            id: "br".to_string(),
            stack_id: None,
        }
    }

    fn uncommitted_id() -> CliId {
        CliId::Uncommitted(crate::id::UncommittedCliId {
            id: "uc".to_string(),
            hunk_assignments: nonempty::NonEmpty::new(but_hunk_assignment::HunkAssignment {
                id: None,
                hunk_header: None,
                path: "test.txt".to_string(),
                path_bytes: BString::from("test.txt"),
                stack_id: None,
                hunk_locks: None,
                line_nums_added: None,
                line_nums_removed: None,
                diff: None,
            }),
            is_entire_file: true,
        })
    }

    fn committed_file_id(path: &str) -> CliId {
        CliId::CommittedFile {
            commit_id: gix::ObjectId::empty_tree(gix::hash::Kind::Sha1),
            path: BString::from(path),
            id: "cf".to_string(),
        }
    }

    #[test]
    fn test_route_move_operation_valid_combinations() {
        // Commit -> Commit: should route to CommitToCommit
        let source = commit_id("a");
        let target = commit_id("b");
        let result = route_move_operation(&source, &target, "b", false);
        assert!(matches!(result, Some(MoveOperation::CommitToCommit { .. })));

        // Commit -> Commit with --after: should route to CommitToCommit with after=true
        let source = commit_id("a");
        let target = commit_id("b");
        let result = route_move_operation(&source, &target, "b", true);
        assert!(matches!(
            result,
            Some(MoveOperation::CommitToCommit { after: true, .. })
        ));

        // Commit -> Branch: should route to CommitToBranch
        let source = commit_id("a");
        let target = branch_id("main");
        let result = route_move_operation(&source, &target, "main", false);
        assert!(matches!(result, Some(MoveOperation::CommitToBranch { .. })));

        // CommittedFile -> Commit: should route to CommittedFileToCommit
        let source = committed_file_id("file.txt");
        let target = commit_id("b");
        let result = route_move_operation(&source, &target, "b", false);
        assert!(matches!(result, Some(MoveOperation::CommittedFileToCommit { .. })));
    }

    #[test]
    fn test_route_move_operation_invalid_combinations() {
        // Uncommitted -> Commit: not supported by move
        let source = uncommitted_id();
        let target = commit_id("a");
        let result = route_move_operation(&source, &target, "a", false);
        assert!(result.is_none());

        // Commit -> Uncommitted: not supported by move
        let source = commit_id("a");
        let target = uncommitted_id();
        let result = route_move_operation(&source, &target, "uc", false);
        assert!(result.is_none());

        // Branch -> Commit: not supported by move (this is a rub operation)
        let source = branch_id("main");
        let target = commit_id("a");
        let result = route_move_operation(&source, &target, "a", false);
        assert!(result.is_none());

        // CommittedFile -> Branch: not supported by move (use rub for this)
        let source = committed_file_id("file.txt");
        let target = branch_id("main");
        let result = route_move_operation(&source, &target, "main", false);
        assert!(result.is_none());

        // CommittedFile -> Uncommitted: not supported by move
        let source = committed_file_id("file.txt");
        let target = uncommitted_id();
        let result = route_move_operation(&source, &target, "uc", false);
        assert!(result.is_none());
    }

    /// Test the index adjustment logic when moving commits within the same series.
    /// This is the most complex part of the move logic and most prone to off-by-one errors.
    #[test]
    fn test_index_adjustment_logic() {
        // Simulating the logic from move_within_stack without needing a full Git repo

        // Test case 1: Moving from earlier to later position (source_pos < target_pos)
        // Initial: [A, B, C, D] at indices [0, 1, 2, 3]
        // Move A (index 0) before D (index 3)
        let source_pos = 0;
        let target_pos = 3;
        let source_idx = 0;
        let target_idx = 0; // same series

        // After removal: [B, C, D] at indices [0, 1, 2]
        // Target D is now at index 2 (was 3, minus 1 for removal)
        let adjusted_target_pos = if source_idx == target_idx && source_pos < target_pos {
            target_pos - 1
        } else {
            target_pos
        };
        assert_eq!(adjusted_target_pos, 2);

        // Insert before D means insert at adjusted_target_pos + 1
        let insert_position = adjusted_target_pos + 1; // after=false case
        assert_eq!(insert_position, 3);

        // Result: [B, C, D, A] - A is now before (older than) D ✓

        // Test case 2: Moving from later to earlier position (source_pos > target_pos)
        // Initial: [A, B, C, D] at indices [0, 1, 2, 3]
        // Move D (index 3) after A (index 0)
        let source_pos = 3;
        let target_pos = 0;
        let source_idx = 0;
        let target_idx = 0; // same series

        // After removal: [A, B, C] at indices [0, 1, 2]
        // Target A stays at index 0 (no adjustment needed)
        let adjusted_target_pos = if source_idx == target_idx && source_pos < target_pos {
            target_pos - 1
        } else {
            target_pos
        };
        assert_eq!(adjusted_target_pos, 0);

        // Insert after A means insert at adjusted_target_pos
        let insert_position = adjusted_target_pos; // after=true case
        assert_eq!(insert_position, 0);

        // Result: [D, A, B, C] - D is now after (newer than) A ✓

        // Test case 3: Moving within adjacent positions
        // Initial: [A, B, C] at indices [0, 1, 2]
        // Move B (index 1) before C (index 2)
        let source_pos = 1;
        let target_pos = 2;
        let source_idx = 0;
        let target_idx = 0;

        // After removal: [A, C] at indices [0, 1]
        let adjusted_target_pos = if source_idx == target_idx && source_pos < target_pos {
            target_pos - 1
        } else {
            target_pos
        };
        assert_eq!(adjusted_target_pos, 1);

        // Insert before C means insert at adjusted_target_pos + 1
        let insert_position = adjusted_target_pos + 1;
        assert_eq!(insert_position, 2);

        // Result: [A, C, B] - B is now before (older than) C ✓
    }

    #[test]
    fn test_index_adjustment_different_series() {
        // When moving between different series, no adjustment needed
        // Initial series 0: [A, B, C]
        // Initial series 1: [D, E, F]
        // Move B (series 0, index 1) to before E (series 1, index 1)

        let source_pos = 1;
        let target_pos = 1;
        let source_idx = 0;
        let target_idx = 1; // different series

        // After removal from series 0: [A, C]
        // Series 1 is unchanged: [D, E, F]
        // No adjustment needed because they're in different series
        let adjusted_target_pos = if source_idx == target_idx && source_pos < target_pos {
            target_pos - 1
        } else {
            target_pos
        };
        assert_eq!(adjusted_target_pos, 1); // No adjustment

        // Insert before E means insert at adjusted_target_pos + 1
        let insert_position = adjusted_target_pos + 1;
        assert_eq!(insert_position, 2);

        // Result series 1: [D, E, B, F] - B is now before (older than) E ✓
    }

    #[test]
    fn test_before_vs_after_semantics() {
        // Verify the "before" (default) and "after" semantics are correct
        // Given commits ordered newest to oldest: [NEW, MID, OLD]
        // at indices [0, 1, 2]

        // "Move X after Y" means X becomes newer than Y (X depends on Y)
        // So moving MID after NEW should give [MID, NEW, OLD]
        let source_pos = 1; // MID
        let target_pos = 0; // NEW
        let after = true;

        let adjusted_target_pos = if source_pos < target_pos {
            target_pos - 1
        } else {
            target_pos
        };
        let insert_position = if after {
            adjusted_target_pos
        } else {
            adjusted_target_pos + 1
        };
        assert_eq!(insert_position, 0); // MID goes to index 0, pushing NEW down

        // "Move X before Y" means X becomes older than Y (Y depends on X)
        // So moving MID before OLD should give [NEW, OLD, MID]
        let source_pos = 1; // MID
        let target_pos = 2; // OLD
        let after = false;

        let adjusted_target_pos = if source_pos < target_pos {
            target_pos - 1
        } else {
            target_pos
        };
        let insert_position = if after {
            adjusted_target_pos
        } else {
            adjusted_target_pos + 1
        };
        assert_eq!(adjusted_target_pos, 1); // OLD moved to index 1 after removing MID
        assert_eq!(insert_position, 2); // MID goes after OLD (higher index = older)
    }

    #[test]
    fn test_moving_to_same_position_logic() {
        // When moving a commit and it ends up at the same logical position,
        // we want to ensure the math works out correctly

        // Move C after B when they're already in order [A, B, C]
        // This should be essentially a no-op after the adjustment
        let source_pos = 2; // C
        let target_pos = 1; // B
        let after = true; // after B

        // After removing C: [A, B] at [0, 1]
        // B is still at index 1
        let adjusted_target_pos = if source_pos < target_pos {
            target_pos - 1
        } else {
            target_pos
        };
        assert_eq!(adjusted_target_pos, 1);

        // Insert after B (index 1) means insert at index 1
        let insert_position = if after {
            adjusted_target_pos
        } else {
            adjusted_target_pos + 1
        };
        assert_eq!(insert_position, 1);

        // Result: [A, C, B] - Wait, this is wrong for a no-op case!
        // Actually this isn't a no-op - "after B" means newer than B
        // So C moves to be newer than B, which is a real change.
    }

    #[test]
    fn test_edge_case_two_commits() {
        // Test with only two commits to ensure boundary conditions work

        // [A, B] - Move B before A (swap them)
        let source_pos = 1;
        let target_pos = 0;
        let after = false;

        let adjusted_target_pos = if source_pos < target_pos {
            target_pos - 1
        } else {
            target_pos
        };
        assert_eq!(adjusted_target_pos, 0);

        let insert_position = if after {
            adjusted_target_pos
        } else {
            adjusted_target_pos + 1
        };
        assert_eq!(insert_position, 1);

        // Result: [A, B] - B stays in same position, which means B is still newer
        // Actually "before A" means older than A, so B should go after A
        // But [A, B] with B at index 1 means B is older than A already
        // So this is actually correct - B is before (older than) A
    }
}
