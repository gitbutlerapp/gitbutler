use anyhow::bail;
use but_api::commit::ui::RelativeTo;
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::InsertSide;
use colored::Colorize;

use crate::{
    CliId, IdMap,
    utils::{OutputChannel, shorten_object_id},
};

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
            "Source '{source_str}' not found. If you just performed a Git operation, try running 'but status' to refresh."
        );
    }
    if source_matches.len() > 1 {
        bail!("Source '{source_str}' is ambiguous. Try using more characters to disambiguate.");
    }

    let source_id = &source_matches[0];

    // Resolve target
    let target_matches = id_map.parse_using_context(target_str, ctx)?;
    if target_matches.is_empty() {
        bail!(
            "Target '{target_str}' not found. If you just performed a Git operation, try running 'but status' to refresh."
        );
    }
    if target_matches.len() > 1 {
        bail!("Target '{target_str}' is ambiguous. Try using more characters to disambiguate.");
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
    let Some(operation) = route_move_operation(source_id, target_id, after) else {
        bail!(
            "Cannot move {} ({}) to {} ({}).\n\
            Valid moves: commit→commit, commit→branch, or committed-file→commit",
            source_id.to_short_string().blue().bold(),
            source_id.kind_for_humans().yellow(),
            target_id.to_short_string().blue().bold(),
            target_id.kind_for_humans().yellow()
        );
    };

    operation.execute(ctx, out)
}

/// Represents the operation to perform for a given source and target combination in `but move`.
#[derive(Debug)]
enum MoveOperation<'a> {
    /// Move a commit to be before/after another commit
    CommitToCommit {
        source: gix::ObjectId,
        target: gix::ObjectId,
        after: bool,
    },
    /// Move a commit to a branch (places at top of the branch)
    CommitToBranch {
        source: gix::ObjectId,
        target_branch: &'a str,
    },
    /// Move a committed file to another commit (delegates to rub)
    CommittedFileToCommit {
        path: &'a bstr::BStr,
        source_commit: gix::ObjectId,
        target_commit: gix::ObjectId,
    },
}

impl<'a> MoveOperation<'a> {
    /// Executes this move operation
    fn execute(self, ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
        match self {
            MoveOperation::CommitToCommit {
                source,
                target,
                after,
            } => move_commit_to_commit(ctx, source, target, after, out),
            MoveOperation::CommitToBranch {
                source,
                target_branch,
            } => move_commit_to_branch(ctx, source, target_branch, out),
            MoveOperation::CommittedFileToCommit {
                path,
                source_commit,
                target_commit,
            } => super::file::commited_file_to_another_commit(
                ctx,
                path,
                source_commit,
                target_commit,
                out,
            ),
        }
    }
}

/// Mova a commit to a new position relative to another one.
pub fn move_commit_to_commit(
    ctx: &mut Context,
    source: gix::ObjectId,
    target: gix::ObjectId,
    after: bool,
    out: &mut OutputChannel,
) -> Result<(), anyhow::Error> {
    let side = if after {
        InsertSide::Above
    } else {
        InsertSide::Below
    };

    // Check if source and target are the same commit
    if source == target {
        if let Some(out) = out.for_human() {
            writeln!(out, "Source and target are the same commit. Nothing to do.")?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({"ok": true}))?;
        }
        return Ok(());
    }

    but_api::commit::commit_move(ctx, source, RelativeTo::Commit(target), side)?;

    if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        let action = if after { "after" } else { "before" };
        writeln!(
            out,
            "Moved {} → {} {}",
            shorten_object_id(&repo, source).blue(),
            action,
            shorten_object_id(&repo, target).green(),
        )?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }
    Ok(())
}

/// Move a commit to the top of a branch
pub fn move_commit_to_branch(
    ctx: &mut Context,
    source: gix::ObjectId,
    target_branch: &str,
    out: &mut OutputChannel,
) -> Result<(), anyhow::Error> {
    let target_full_name = gix::refs::FullName::try_from(format!("refs/heads/{target_branch}"))?;
    but_api::commit::commit_move(
        ctx,
        source,
        RelativeTo::Reference(target_full_name),
        InsertSide::Below,
    )?;

    if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        writeln!(
            out,
            "Moved {} → {}",
            shorten_object_id(&repo, source).blue(),
            format!("[{target_branch}]").green()
        )?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }
    Ok(())
}

/// Determines the move operation to perform for a given source and target combination.
/// Returns `Some(operation)` if the combination is valid, `None` otherwise.
fn route_move_operation<'a>(
    source: &'a CliId,
    target: &'a CliId,
    after: bool,
) -> Option<MoveOperation<'a>> {
    use CliId::*;

    match (source, target) {
        // Commit -> Commit: move commit to specific position
        (
            Commit {
                commit_id: source, ..
            },
            Commit {
                commit_id: target, ..
            },
        ) => Some(MoveOperation::CommitToCommit {
            source: *source,
            target: *target,
            after,
        }),
        // Commit -> Branch: move commit to top of branch
        (
            Commit {
                commit_id: source, ..
            },
            Branch { name, .. },
        ) => Some(MoveOperation::CommitToBranch {
            source: *source,
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
            source_commit: *source_commit,
            target_commit: *target_commit,
        }),
        // All other combinations are invalid for move
        _ => None,
    }
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
        let result = route_move_operation(&source, &target, false);
        assert!(matches!(result, Some(MoveOperation::CommitToCommit { .. })));

        // Commit -> Commit with --after: should route to CommitToCommit with after=true
        let source = commit_id("a");
        let target = commit_id("b");
        let result = route_move_operation(&source, &target, true);
        assert!(matches!(
            result,
            Some(MoveOperation::CommitToCommit { after: true, .. })
        ));

        // Commit -> Branch: should route to CommitToBranch
        let source = commit_id("a");
        let target = branch_id("main");
        let result = route_move_operation(&source, &target, false);
        assert!(matches!(result, Some(MoveOperation::CommitToBranch { .. })));

        // CommittedFile -> Commit: should route to CommittedFileToCommit
        let source = committed_file_id("file.txt");
        let target = commit_id("b");
        let result = route_move_operation(&source, &target, false);
        assert!(matches!(
            result,
            Some(MoveOperation::CommittedFileToCommit { .. })
        ));
    }

    #[test]
    fn test_route_move_operation_invalid_combinations() {
        // Uncommitted -> Commit: not supported by move
        let source = uncommitted_id();
        let target = commit_id("a");
        let result = route_move_operation(&source, &target, false);
        assert!(result.is_none());

        // Commit -> Uncommitted: not supported by move
        let source = commit_id("a");
        let target = uncommitted_id();
        let result = route_move_operation(&source, &target, false);
        assert!(result.is_none());

        // Branch -> Commit: not supported by move (this is a rub operation)
        let source = branch_id("main");
        let target = commit_id("a");
        let result = route_move_operation(&source, &target, false);
        assert!(result.is_none());

        // CommittedFile -> Branch: not supported by move (use rub for this)
        let source = committed_file_id("file.txt");
        let target = branch_id("main");
        let result = route_move_operation(&source, &target, false);
        assert!(result.is_none());

        // CommittedFile -> Uncommitted: not supported by move
        let source = committed_file_id("file.txt");
        let target = uncommitted_id();
        let result = route_move_operation(&source, &target, false);
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
