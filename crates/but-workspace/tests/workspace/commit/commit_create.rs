use anyhow::Result;
use but_core::DiffSpec;
use but_rebase::graph_rebase::{
    GraphExt as _, LookupStep as _,
    mutate::{InsertSide, RelativeTo},
};
use but_workspace::commit::commit_create;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

fn worktree_changes_as_specs(repo: &gix::Repository) -> Result<Vec<DiffSpec>> {
    Ok(but_core::diff::worktree_changes(repo)?
        .changes
        .into_iter()
        .map(DiffSpec::from)
        .collect())
}

#[test]
fn commit_above_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    std::fs::write(
        repo.workdir_path("inserted-above-commit.txt").expect("non-bare"),
        "inserted\n",
    )?;

    let editor = graph.to_editor(&repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeTo::Commit(two_id),
        InsertSide::Above,
        "insert above commit",
        0,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let rebase = outcome.rebase.expect("a new commit was created");
    let selector = outcome.commit_selector.expect("a selector for the new commit");
    let materialized = rebase.materialize()?;
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
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
    let one_id = repo.rev_parse_single("one")?.detach();
    let two_id = repo.rev_parse_single("two")?.detach();
    std::fs::write(
        repo.workdir_path("inserted-below-commit.txt").expect("non-bare"),
        "inserted\n",
    )?;

    let editor = graph.to_editor(&repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeTo::Commit(two_id),
        InsertSide::Below,
        "insert below commit",
        0,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let rebase = outcome.rebase.expect("a new commit was created");
    let selector = outcome.commit_selector.expect("a selector for the new commit");
    let materialized = rebase.materialize()?;
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
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    let reference = repo.find_reference("two")?;
    std::fs::write(
        repo.workdir_path("inserted-above-reference.txt").expect("non-bare"),
        "inserted\n",
    )?;

    let editor = graph.to_editor(&repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeTo::Reference(reference.name()),
        InsertSide::Above,
        "insert above reference",
        0,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let rebase = outcome.rebase.expect("a new commit was created");
    let selector = outcome.commit_selector.expect("a selector for the new commit");
    let materialized = rebase.materialize()?;
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
        repo.workdir_path("inserted-below-merge.txt").expect("non-bare"),
        "inserted\n",
    )?;

    let editor = graph.to_editor(&repo)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeTo::Commit(merge_id),
        InsertSide::Below,
        "insert below merge",
        0,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let rebase = outcome.rebase.expect("a new commit was created");
    let selector = outcome.commit_selector.expect("a selector for the new commit");
    let materialized = rebase.materialize()?;
    let new_commit_id = materialized.lookup_pick(selector)?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert below merge");
    assert_eq!(
        new_commit.parent_ids().next().expect("has a parent").detach(),
        first_parent_id,
        "for below merge commits, we base creation on first parent"
    );

    Ok(())
}

#[test]
fn commit_all_rejected_is_noop() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    let editor = graph.to_editor(&repo)?;

    let outcome = commit_create(
        editor,
        vec![DiffSpec {
            previous_path: None,
            path: "does-not-exist".into(),
            hunk_headers: vec![],
        }],
        RelativeTo::Commit(two_id),
        InsertSide::Above,
        "no-op commit",
        0,
    )?;

    assert!(
        outcome.rebase.is_none(),
        "no rebase should happen if nothing was committed"
    );
    assert!(
        outcome.commit_selector.is_none(),
        "no selector if there is no new commit"
    );
    assert_eq!(outcome.rejected_specs.len(), 1, "the invalid spec should be rejected");
    assert_eq!(outcome.rejected_specs[0].1.path, "does-not-exist");

    Ok(())
}
