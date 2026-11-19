use but_testsupport::{CommandExt as _, visualize_commit_graph_all};
use snapbox::IntoData as _;

use crate::{
    ref_info::with_workspace_commit::utils::{
        StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
    },
    utils::r,
};

#[test]
fn dropping_an_empty_lane_leaves_the_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
|/  
* 85efbe4 (origin/main, main, B) M

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    but_workspace::branch::drop_reference(editor, r("refs/heads/B"))?
        .materialize(Default::default())?;

    // `B` is gone, and with only `A` left `HEAD` is on it without a workspace commit.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 09d8e52 (HEAD -> A) A
* 85efbe4 (origin/main, main) M

"#]]
    );
    Ok(())
}

#[test]
fn dropping_a_branch_another_worktree_has_checked_out_is_refused() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    but_testsupport::git(&repo)
        .args(["worktree", "add", "wt-b", "B"])
        .run();

    let mut ws = graph.into_workspace()?;
    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let err = but_workspace::branch::drop_reference(editor, r("refs/heads/B"))?
        .materialize(Default::default())
        .expect_err("the graph was made before the worktree, so the edit can't move it off B");
    assert!(
        err.to_string().contains("checked out"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn dropping_an_empty_lane_takes_it_out_of_the_workspace_commit() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;

    let mut ws = graph.into_workspace()?;
    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let with_c = but_workspace::branch::create_reference(editor, r("refs/heads/C"), None, None)?;
    but_workspace::branch::drop_reference(with_c.into_editor(), r("refs/heads/B"))?
        .materialize(Default::default())?;

    // `B` is gone, and the workspace commit merges `A` and the empty `C` that are left.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   e16ce30 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* | 09d8e52 (A) A
|/  
* 85efbe4 (origin/main, main, C) M

"#]]
    );
    assert_eq!(
        repo.head_commit()?.parent_ids().count(),
        2,
        "the lane of `B` isn't left behind as a parent that only reaches the target"
    );
    Ok(())
}
