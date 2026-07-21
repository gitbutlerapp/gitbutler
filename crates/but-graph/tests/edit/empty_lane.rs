//! Committing into an empty workspace lane must make the managed workspace
//! commit adopt the lane, so the new commit survives materialization.
use anyhow::{Context, Result};
use but_graph::edit::{MaterializeOptions, Pick, mutate::InsertSide};
use but_testsupport::visualize_commit_graph_all;

use crate::utils::fixture_writable;

#[test]
fn insert_into_empty_lane_lands_in_workspace_merge() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("workspace-with-three-empty-stacks")?;
    crate::add_stack_with_segments(
        &mut meta,
        1,
        "stack-1",
        but_testsupport::StackState::InWorkspace,
        &[],
    );
    crate::add_stack_with_segments(
        &mut meta,
        2,
        "stack-2",
        but_testsupport::StackState::InWorkspace,
        &[],
    );

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut editor = graph.into_mut(&repo)?;

    // Build a commit to insert into the empty lane (reuse init's tree).
    let base_id = repo.rev_parse_single("stack-1")?;
    let mut obj = but_core::Commit::from_id(base_id)?;
    obj.message = "first commit".into();
    obj.parents = vec![base_id.detach()].into();
    let new_commit = repo.write_object(obj.inner)?.detach();

    let lane_ref = gix::refs::FullName::try_from("refs/heads/stack-1")?;
    let lane_selector = editor
        .select_reference(lane_ref.as_ref())
        .context("lane ref must be in the graph")?;
    editor.insert_commit_with(
        lane_selector,
        Pick::new_untracked_pick(new_commit),
        InsertSide::Below,
    )?;

    let rebased = editor.rebase()?;
    let outcome = rebased.materialize_changes(&*meta, MaterializeOptions::default())?;

    // The lane's new commit is a parent of the rewritten workspace commit and
    // the lane ref moved onto it.
    let new_tip = repo
        .find_reference("refs/heads/stack-1")?
        .peel_to_id()?
        .detach();
    let new_tip_commit = but_core::Commit::from_id(new_tip.attach_repo(&repo))?;
    assert_eq!(
        new_tip_commit.message, "first commit",
        "the lane ref stands on the inserted commit"
    );
    let head_commit = repo.head_commit()?;
    assert!(
        head_commit.parent_ids().any(|id| id.detach() == new_tip),
        "the workspace commit adopted the previously empty lane:\n{}",
        visualize_commit_graph_all(&repo)?
    );
    assert!(
        outcome.workspace.contains_commit(new_tip),
        "the re-traversed workspace contains the inserted commit"
    );
    Ok(())
}

trait AttachRepo {
    fn attach_repo<'a>(self, repo: &'a gix::Repository) -> gix::Id<'a>;
}
impl AttachRepo for gix::ObjectId {
    fn attach_repo<'a>(self, repo: &'a gix::Repository) -> gix::Id<'a> {
        use gix::prelude::ObjectIdExt;
        self.attach(repo)
    }
}
