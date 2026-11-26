use but_core::{
    RefMetadata as _, WORKSPACE_REF_NAME, git_config::open_repo_local_config_for_reading,
    ref_metadata::StackKind,
};
use but_testsupport::{CommandExt as _, visualize_commit_graph_all};

use crate::{
    ref_info::with_workspace_commit::utils::{
        StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
    },
    utils::r,
};

#[test]
fn renaming_a_lane_keeps_it_in_the_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;

    but_testsupport::git(&repo)
        .args(["config", "branch.A.description", "Why A"])
        .run();

    let mut ws = graph.into_workspace()?;
    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    but_workspace::branch::rename_reference(editor, r("refs/heads/A"), r("refs/heads/C"))?
        .materialize(Default::default())?;

    // `A` is `C` now, on the same commit and still merged by the workspace commit.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   e16ce30 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* | 09d8e52 (C) A
|/  
* 85efbe4 (origin/main, main, B) M

"#]]
    );

    let ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    assert!(
        ws_md
            .find_branch(r("refs/heads/C"), StackKind::Applied)
            .is_some(),
        "the stacks are written from the graph, which has the new name"
    );
    assert!(
        ws_md
            .find_branch(r("refs/heads/A"), StackKind::AppliedAndUnapplied)
            .is_none(),
        "the old name leaves the metadata as a deleted branch does"
    );
    let config = open_repo_local_config_for_reading(&repo)?;
    assert_eq!(
        (
            config.string("branch.A.description"),
            config.string("branch.C.description")
        ),
        (None, None),
        "the old name's configuration goes with it and doesn't follow the rename"
    );
    Ok(())
}

#[test]
fn renaming_into_and_out_of_its_own_directory() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;

    let mut ws = graph.into_workspace()?;
    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    but_workspace::branch::rename_reference(editor, r("refs/heads/A"), r("refs/heads/A/inner"))?
        .materialize(Default::default())?;

    // `A` goes before `A/inner` is made, as Git can't hold both.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   e16ce30 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* | 09d8e52 (A/inner) A
|/  
* 85efbe4 (origin/main, main, B) M

"#]]
    );

    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    but_workspace::branch::rename_reference(editor, r("refs/heads/A/inner"), r("refs/heads/A"))?
        .materialize(Default::default())?;

    // And back, `A/inner` going before its directory becomes `A`.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   e16ce30 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* | 09d8e52 (A) A
|/  
* 85efbe4 (origin/main, main, B) M

"#]]
    );
    Ok(())
}

#[test]
fn renaming_onto_a_name_that_is_taken_is_refused() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;

    let mut ws = graph.into_workspace()?;
    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let Err(err) =
        but_workspace::branch::rename_reference(editor, r("refs/heads/A"), r("refs/heads/B"))
    else {
        panic!("`B` exists");
    };
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );

    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let with_c = but_workspace::branch::create_reference(editor, r("refs/heads/C"), None, None)?;
    let Err(err) = but_workspace::branch::rename_reference(
        with_c.into_editor(),
        r("refs/heads/A"),
        r("refs/heads/C"),
    ) else {
        panic!("`C` exists in the editor, if not yet in the repository");
    };
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn renaming_a_branch_that_does_not_exist_is_refused() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;

    let mut ws = graph.into_workspace()?;
    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let Err(err) =
        but_workspace::branch::rename_reference(editor, r("refs/heads/missing"), r("refs/heads/C"))
    else {
        panic!("there is no `missing` to rename");
    };
    assert!(
        err.to_string().contains("does not exist"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn renaming_what_the_edit_may_not_change_is_refused() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    let mut ws = graph.into_workspace()?;

    for (from, to, why) in [
        (
            "refs/heads/A",
            "refs/remotes/origin/C",
            "Can only rename local branches",
        ),
        (
            "refs/heads/A",
            "refs/heads/gitbutler/edit",
            "is a GitButler technical branch",
        ),
        (
            "refs/heads/gitbutler/workspace",
            "refs/heads/C",
            "is a GitButler technical branch",
        ),
    ] {
        let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
        let Err(err) = but_workspace::branch::rename_reference(editor, r(from), r(to)) else {
            panic!("renaming {from} to {to} must be refused");
        };
        assert!(
            err.to_string().contains(why),
            "{from} to {to}: unexpected error: {err}"
        );
    }

    let tree = repo.head_commit()?.tree_id()?;
    repo.commit(
        "refs/heads/outside",
        "outside",
        tree,
        gix::commit::NO_PARENT_IDS,
    )?;
    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let Err(err) =
        but_workspace::branch::rename_reference(editor, r("refs/heads/outside"), r("refs/heads/C"))
    else {
        panic!("the editor doesn't reach `outside`");
    };
    assert!(
        err.to_string()
            .contains("is outside of what this edit may change"),
        "unexpected error: {err}"
    );

    ws.graph.options.extra_refs = vec![r("refs/heads/outside").to_owned()];
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta, &mut db)?;
    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    assert!(
        editor
            .try_select_reference(r("refs/heads/outside"))
            .is_some(),
        "an extra ref joins the editor, but as a step it may not change"
    );
    let Err(err) =
        but_workspace::branch::rename_reference(editor, r("refs/heads/outside"), r("refs/heads/C"))
    else {
        panic!("renaming an immutable step would copy the branch rather than rename it");
    };
    assert!(
        err.to_string()
            .contains("is outside of what this edit may change"),
        "unexpected error: {err}"
    );
    Ok(())
}
