use bstr::ByteSlice;
use but_graph::init::Options;
use but_workspace::WorkspaceCommit;

use crate::ref_info::with_workspace_commit::utils::{
    named_read_only_in_memory_scenario, named_writable_scenario_with_description,
};

mod utils {
    use but_core::ref_metadata::{
        StackId, WorkspaceCommitRelation::Merged, WorkspaceStack, WorkspaceStackBranch,
    };
    use but_meta::VirtualBranchesTomlMetadata;
    use gix::refs::Category;

    use crate::ref_info::with_workspace_commit::utils::{StackState, add_stack_with_segments};

    pub fn add_stacks(
        meta: &mut VirtualBranchesTomlMetadata,
        short_stack_names: impl IntoIterator<Item = &'static str>,
    ) {
        for (idx, stack_name) in short_stack_names.into_iter().enumerate() {
            add_stack_with_segments(
                meta,
                idx as u128 + 1,
                stack_name,
                StackState::InWorkspace,
                &[],
            );
        }
    }

    pub fn to_stacks(
        short_stack_names: impl IntoIterator<Item = &'static str>,
    ) -> Vec<WorkspaceStack> {
        short_stack_names
            .into_iter()
            .map(|short_name| WorkspaceStack {
                id: StackId::generate(),
                workspacecommit_relation: Merged,
                branches: vec![WorkspaceStackBranch {
                    ref_name: Category::LocalBranch
                        .to_full_name(short_name)
                        .expect("known good short ref name"),
                    archived: false,
                }],
            })
            .collect()
    }
}
use utils::{add_stacks, to_stacks};

/// Two branches making non-overlapping edits to the same file.
/// Myers diff produces split hunks with empty insertions that falsely collide.
/// See: https://github.com/GitoxideLabs/gitoxide/issues/2475
///
/// Reproduction:
///   1. A config file has three sections: `[alpha]`, `[bravo]`, `[charlie]`.
///   2. User creates branch A and deletes the `[alpha]` section.
///   3. User creates branch B and deletes the `[bravo]` section.
///   4. Both branches are applied in the workspace.
///
/// Outcome: when the workspace commit is rebuilt, the merge falsely detects a
/// conflict between the two branches. Branch B is unapplied from the workspace
/// even though the edits are completely independent.
#[test]
fn two_branches_non_overlapping_same_file_edits_should_merge_cleanly() -> anyhow::Result<()> {
    let (repo, mut meta) =
        named_read_only_in_memory_scenario("myers-false-conflict-same-file", "")?;
    let stacks = ["delete-alpha", "delete-bravo"];
    add_stacks(&mut meta, stacks);
    let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;

    let out = WorkspaceCommit::from_new_merge_with_metadata(
        &to_stacks(stacks),
        None,
        &graph,
        &repo,
        None,
    )?;

    // These are non-overlapping changes. There should be NO conflicting stacks.
    // With unpatched gitoxide (Myers), this will falsely report a conflict.
    assert!(
        out.conflicting_stacks.is_empty(),
        "Expected no conflicts for non-overlapping edits, but got: {:?}",
        out.conflicting_stacks
    );
    assert_eq!(out.stacks.len(), 2, "Both stacks should be merged");

    Ok(())
}

/// Same as above but with a hero stack — the hero stack's changes must
/// always be present even if a false conflict is detected.
///
/// Outcome: the hero stack is preserved, but the non-hero stack is falsely
/// unapplied from the workspace despite its edits being independent.
#[test]
fn two_branches_non_overlapping_with_hero_stack() -> anyhow::Result<()> {
    let (repo, mut meta) =
        named_read_only_in_memory_scenario("myers-false-conflict-same-file", "")?;

    let stacks = ["delete-alpha", "delete-bravo"];
    add_stacks(&mut meta, stacks);
    let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;

    // With delete-alpha as hero
    let out = WorkspaceCommit::from_new_merge_with_metadata(
        &to_stacks(stacks),
        None,
        &graph,
        &repo,
        Some("refs/heads/delete-alpha".try_into()?),
    )?;

    // The hero stack must always be present in the merged stacks.
    let has_alpha = out.stacks.iter().any(|s| {
        s.name
            .as_ref()
            .is_some_and(|n| n.as_bstr() == b"delete-alpha".as_bstr())
    });
    assert!(
        has_alpha,
        "Hero stack delete-alpha must always be present in merged stacks, got: {:?}",
        out.stacks
    );

    // With unpatched gitoxide (Myers), the non-hero stack is falsely conflicting.
    // Ideally both stacks should merge cleanly since the edits don't overlap.
    assert!(
        out.conflicting_stacks.is_empty(),
        "Non-hero stack falsely marked as conflicting due to Myers diff bug: {:?}",
        out.conflicting_stacks
    );

    Ok(())
}

/// Two branches with multiple sequential commits each, editing the same file.
/// Tests that workspace merge works with the multi-commit variant.
///
/// Reproduction:
///   1. A shared config file has sections `[alpha]`, `[bravo]`, `[charlie]`.
///   2. User creates branch A with two commits: first adds a header comment,
///      then deletes `[alpha]`.
///   3. User creates branch B with two commits: first adds a footer comment,
///      then deletes `[bravo]`.
///   4. Both branches are applied in the workspace.
///
/// Outcome: the workspace merge falsely detects a conflict and unapplies
/// branch B. The extra commit depth makes no difference — the workspace merge
/// only compares the branch tip trees against the merge base, so the same
/// Myers false conflict triggers.
#[test]
fn multi_commit_branches_non_overlapping_same_file() -> anyhow::Result<()> {
    let (repo, mut meta) =
        named_read_only_in_memory_scenario("myers-false-conflict-multi-commit", "")?;
    let stacks = ["edit-alpha", "edit-bravo"];
    add_stacks(&mut meta, stacks);
    let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;

    let out = WorkspaceCommit::from_new_merge_with_metadata(
        &to_stacks(stacks),
        None,
        &graph,
        &repo,
        None,
    )?;

    assert!(
        out.conflicting_stacks.is_empty(),
        "Expected no conflicts for non-overlapping multi-commit edits, but got: {:?}",
        out.conflicting_stacks
    );
    assert_eq!(out.stacks.len(), 2, "Both stacks should be merged");

    Ok(())
}

/// After squashing two sequential commits in one branch, the workspace merge
/// should still work. This tests the rebase + merge pipeline.
///
/// Reproduction:
///   1. Same setup as `multi_commit_branches_non_overlapping_same_file`.
///   2. User squashes the two commits in branch A into a single commit
///      (e.g. via `but rub` or the GUI).
///   3. The squash itself succeeds — it's a single-branch rebase that doesn't
///      involve cross-branch merging.
///   4. After the squash, `update_workspace_commit` rebuilds the workspace by
///      merging all applied branch tips together.
///
/// Outcome: the workspace rebuild falsely detects a conflict between the
/// (now-squashed) branch A and branch B. Branch B is unapplied. The squash
/// didn't change branch A's tree at all — it only collapsed its history —
/// so the same Myers false conflict triggers during the post-squash workspace
/// rebuild.
#[test]
fn squash_then_workspace_merge() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("myers-false-conflict-multi-commit")?;

    // Squash the two commits in edit-alpha via manual cherry-pick approach.
    // We cherry-pick the tip onto main directly, producing the same tree as
    // the tip but with main as the only parent (effectively squashing).
    let alpha_tip = repo.rev_parse_single("edit-alpha")?.detach();
    let base = repo.rev_parse_single("main")?.detach();

    let tip_commit = repo.find_commit(alpha_tip)?.decode()?.to_owned()?;
    let squashed = gix::objs::Commit {
        tree: tip_commit.tree,
        parents: [base].into(),
        author: tip_commit.author,
        committer: tip_commit.committer,
        encoding: tip_commit.encoding,
        message: "squashed: header + delete alpha".into(),
        extra_headers: Default::default(),
    };
    let squashed_id = repo.write_object(&squashed)?.detach();

    // Update the edit-alpha ref to point to the squashed commit
    repo.reference(
        "refs/heads/edit-alpha",
        squashed_id,
        gix::refs::transaction::PreviousValue::Any,
        "squash test",
    )?;

    // Now rebuild the graph and try merging with edit-bravo
    let stacks = ["edit-alpha", "edit-bravo"];
    add_stacks(&mut meta, stacks);
    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;

    let merge_out = WorkspaceCommit::from_new_merge_with_metadata(
        &to_stacks(stacks),
        None,
        &graph,
        &repo,
        None,
    )?;

    assert!(
        merge_out.conflicting_stacks.is_empty(),
        "Squashed branch should still merge cleanly, but got conflicts: {:?}",
        merge_out.conflicting_stacks
    );
    assert_eq!(merge_out.stacks.len(), 2);

    Ok(())
}

/// Amending a commit in one branch (adding an unrelated file), then checking
/// that the workspace merge still works.
///
/// Reproduction:
///   1. Same setup as `multi_commit_branches_non_overlapping_same_file`.
///   2. User realizes they forgot to include `extra-file.txt` in branch A's
///      tip commit and amends it in (e.g. via `but absorb` or the GUI).
///   3. The amend succeeds — it rewrites the tip commit with the additional
///      file but does not change the shared-file content.
///   4. After the amend, `update_workspace_commit` rebuilds the workspace.
///
/// Outcome: the workspace rebuild falsely detects a conflict between branch A
/// and branch B. Branch B is unapplied. The amend only added an unrelated
/// file — the shared-file diff that triggers the Myers bug is unchanged.
#[test]
fn amend_then_workspace_merge() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("myers-false-conflict-multi-commit")?;

    // Amend the tip of edit-alpha by adding an extra file (unrelated change).
    let alpha_tip = repo.rev_parse_single("edit-alpha")?.detach();
    let alpha_commit = repo.find_commit(alpha_tip)?.decode()?.to_owned()?;
    let alpha_tree_id = alpha_commit.tree;

    // Create a new tree with an additional file
    let new_blob = repo.write_blob("extra content\n")?.detach();
    let mut tree_editor = repo.edit_tree(alpha_tree_id)?;
    tree_editor.upsert("extra-file.txt", gix::objs::tree::EntryKind::Blob, new_blob)?;
    let new_tree_id = tree_editor.write()?;

    // Create the amended commit (same parents, new tree)
    let amended_commit = gix::objs::Commit {
        tree: new_tree_id.detach(),
        parents: alpha_commit.parents,
        author: alpha_commit.author,
        committer: alpha_commit.committer,
        encoding: alpha_commit.encoding,
        message: "delete alpha_x from shared-file (amended with extra file)".into(),
        extra_headers: Default::default(),
    };
    let amended_id = repo.write_object(&amended_commit)?.detach();

    // Update the ref
    repo.reference(
        "refs/heads/edit-alpha",
        amended_id,
        gix::refs::transaction::PreviousValue::Any,
        "amend test",
    )?;

    // Rebuild graph and merge
    let stacks = ["edit-alpha", "edit-bravo"];
    add_stacks(&mut meta, stacks);
    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;

    let merge_out = WorkspaceCommit::from_new_merge_with_metadata(
        &to_stacks(stacks),
        None,
        &graph,
        &repo,
        None,
    )?;

    assert!(
        merge_out.conflicting_stacks.is_empty(),
        "Amended branch should still merge cleanly, but got conflicts: {:?}",
        merge_out.conflicting_stacks
    );
    assert_eq!(merge_out.stacks.len(), 2);

    // TODO: once the gitoxide fix lands, add an inline snapshot here verifying
    // that extra-file.txt appears in the merged tree alongside shared-file.

    Ok(())
}

/// Reverse the order of stacks to check if merge order affects conflict detection.
/// With Myers false conflicts, one direction may conflict while the other doesn't.
///
/// Outcome: both orderings produce the same (false) conflict — the second
/// stack in either order is unapplied. The bug is symmetric.
#[test]
fn stack_order_should_not_affect_conflict_detection() -> anyhow::Result<()> {
    let (repo, mut meta) =
        named_read_only_in_memory_scenario("myers-false-conflict-same-file", "")?;

    // Order A: delete-alpha first
    let stacks_a = ["delete-alpha", "delete-bravo"];
    add_stacks(&mut meta, stacks_a);
    let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
    let out_a = WorkspaceCommit::from_new_merge_with_metadata(
        &to_stacks(stacks_a),
        None,
        &graph,
        &repo,
        None,
    )?;

    // Order B: delete-bravo first (need fresh meta)
    let (repo_b, mut meta_b) =
        named_read_only_in_memory_scenario("myers-false-conflict-same-file", "")?;
    let stacks_b = ["delete-bravo", "delete-alpha"];
    add_stacks(&mut meta_b, stacks_b);
    let graph_b = but_graph::Graph::from_head(&repo_b, &*meta_b, Options::limited())?;
    let out_b = WorkspaceCommit::from_new_merge_with_metadata(
        &to_stacks(stacks_b),
        None,
        &graph_b,
        &repo_b,
        None,
    )?;

    // Both orderings should produce the same conflict status
    assert_eq!(
        out_a.conflicting_stacks.is_empty(),
        out_b.conflicting_stacks.is_empty(),
        "Stack ordering should not affect conflict detection.\n\
         Order A conflicts: {:?}\n\
         Order B conflicts: {:?}",
        out_a.conflicting_stacks,
        out_b.conflicting_stacks,
    );

    Ok(())
}
