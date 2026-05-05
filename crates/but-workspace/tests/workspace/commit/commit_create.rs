use anyhow::Result;
use but_core::DiffSpec;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _,
    mutate::{InsertSide, RelativeToRef},
};
use but_workspace::commit::commit_create;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments,
    named_writable_scenario_with_description_and_graph as writable_scenario,
};

fn worktree_changes_as_specs(repo: &gix::Repository) -> Result<Vec<DiffSpec>> {
    Ok(but_core::diff::worktree_changes(repo)?
        .changes
        .into_iter()
        .map(DiffSpec::from)
        .collect())
}

#[test]
fn commit_above_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    std::fs::write(
        repo.workdir_path("inserted-above-commit.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeToRef::Commit(two_id),
        InsertSide::Above,
        "insert above commit",
        0,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let selector = outcome
        .commit_selector
        .expect("a selector for the new commit");
    let materialized = outcome.rebase.materialize()?;
    let new_commit_id = materialized.lookup_pick(selector)?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert above commit");
    assert_eq!(
        new_commit.parent_ids().next().expect("one parent").detach(),
        two_id,
        "new commit should be based on the target commit when inserted above"
    );
    let mut two_ref = repo.find_reference("two")?;
    assert_eq!(
        two_ref.peel_to_id()?.detach(),
        new_commit_id,
        "the two reference should now point to the inserted commit"
    );

    Ok(())
}

#[test]
fn commit_below_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let one_id = repo.rev_parse_single("one")?.detach();
    let two_id = repo.rev_parse_single("two")?.detach();
    std::fs::write(
        repo.workdir_path("inserted-below-commit.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeToRef::Commit(two_id),
        InsertSide::Below,
        "insert below commit",
        0,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let selector = outcome
        .commit_selector
        .expect("a selector for the new commit");
    let materialized = outcome.rebase.materialize()?;
    let new_commit_id = materialized.lookup_pick(selector)?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert below commit");
    assert_eq!(
        new_commit.parent_ids().next().expect("one parent").detach(),
        one_id,
        "new commit should be based on the target's first parent when inserted below"
    );

    Ok(())
}

#[test]
fn commit_above_reference() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    let reference = repo.find_reference("two")?;
    std::fs::write(
        repo.workdir_path("inserted-above-reference.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeToRef::Reference(reference.name()),
        InsertSide::Above,
        "insert above reference",
        0,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let selector = outcome
        .commit_selector
        .expect("a selector for the new commit");
    let materialized = outcome.rebase.materialize()?;
    let new_commit_id = materialized.lookup_pick(selector)?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert above reference");
    assert_eq!(
        new_commit.parent_ids().next().expect("one parent").detach(),
        two_id,
        "new commit should be based on the referenced commit"
    );
    let mut two_ref = repo.find_reference("two")?;
    assert_eq!(
        two_ref.peel_to_id()?.detach(),
        two_id,
        "when inserting above a reference, the reference keeps pointing to the original commit"
    );

    Ok(())
}

#[test]
fn commit_below_merge_commit_uses_first_parent() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("merge-with-two-branches-line-offset", |_| {})?;
    let merge_id = repo.rev_parse_single("HEAD")?.detach();
    let merge_commit = repo.find_commit(merge_id)?;
    let first_parent_id = merge_commit
        .parent_ids()
        .next()
        .expect("merge commit has parent")
        .detach();
    std::fs::write(
        repo.workdir_path("inserted-below-merge.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeToRef::Commit(merge_id),
        InsertSide::Below,
        "insert below merge",
        0,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let selector = outcome
        .commit_selector
        .expect("a selector for the new commit");
    let materialized = outcome.rebase.materialize()?;
    let new_commit_id = materialized.lookup_pick(selector)?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert below merge");
    assert_eq!(
        new_commit
            .parent_ids()
            .next()
            .expect("has a parent")
            .detach(),
        first_parent_id,
        "for below merge commits, we base creation on first parent"
    );

    Ok(())
}

#[test]
fn commit_all_rejected_is_noop() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;

    let outcome = commit_create(
        editor,
        vec![DiffSpec {
            previous_path: None,
            path: "does-not-exist".into(),
            hunk_headers: vec![],
        }],
        RelativeToRef::Commit(two_id),
        InsertSide::Above,
        "no-op commit",
        0,
    )?;

    assert!(
        outcome.commit_selector.is_none(),
        "no selector if there is no new commit"
    );
    assert_eq!(
        outcome.rejected_specs.len(),
        1,
        "the invalid spec should be rejected"
    );
    assert_eq!(outcome.rejected_specs[0].1.path, "does-not-exist");

    Ok(())
}

/// When stacks have different bases (e.g. one forked before an upstream
/// change and another after), the workspace merge tree (`HEAD^{tree}`)
/// contains the upstream version of a file while the older stack's tree
/// does not.
///
/// The cherry-pick in `create_tree` correctly detects this as a conflict:
/// base (`HEAD^{tree}`) has the upstream content, ours (old stack parent)
/// has the pre-upstream content, and theirs has the worktree edit — a
/// genuine three-way conflict.
#[test]
fn commit_to_wrong_base_rejects_conflicting_file() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("two-stacks-modifying-shared-file", |meta| {
            add_stack_with_segments(meta, 0, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 1, "B", StackState::InWorkspace, &[]);
        })?;

    // Stack A has the OLD base (before upstream changed file.txt).
    // Committing a worktree change to file.txt on A should be rejected.
    let a_ref = repo.find_reference("A")?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeToRef::Reference(a_ref.name()),
        InsertSide::Above,
        "commit worktree change to stack A",
        0,
    )?;

    assert!(
        !outcome.rejected_specs.is_empty(),
        "file.txt change should be rejected on the old-base stack"
    );
    assert!(
        outcome.rejected_specs.iter().any(|(reason, spec)| {
            matches!(
                reason,
                but_core::tree::create_tree::RejectionReason::IncompatibleBase
            ) && spec.path == "file.txt"
        }),
        "expected IncompatibleBase for file.txt, got: {:?}",
        outcome.rejected_specs
    );

    Ok(())
}
