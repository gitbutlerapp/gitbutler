use crate::{
    ref_info::with_workspace_commit::utils::{
        StackState, add_stack_with_segments, named_read_only_in_memory_scenario,
        named_writable_scenario, named_writable_scenario_with_description_and_graph,
    },
    utils::r,
};
use bstr::ByteSlice;
use but_core::{
    RefMetadata, WORKSPACE_REF_NAME, ref_metadata,
    ref_metadata::{StackId, StackKind, WorkspaceCommitRelation::Outside},
};
use but_graph::{Graph, init::Overlay, workspace::WorkspaceKind};
use but_testsupport::{
    CommandExt, InMemoryRefMetadata, git, graph_workspace, graph_workspace_determinisitcally,
    id_at, id_by_rev, sanitize_uuids_and_timestamps, visualize_commit_graph_all,
    visualize_disk_tree_with_hashes_skip_dot_git,
};
use but_workspace::branch::{
    OnWorkspaceMergeConflict,
    apply::{OutcomeStatus, WorkspaceMerge, WorkspaceReferenceNaming},
    unapply::WorkspaceDisposition,
};
use gix::refs::{Category, transaction::PreviousValue};
use snapbox::prelude::*;

fn project_meta(meta: &impl RefMetadata) -> ref_metadata::ProjectMeta {
    meta.workspace(
        but_core::WORKSPACE_REF_NAME
            .try_into()
            .expect("valid workspace ref"),
    )
    .map(|workspace| workspace.project_meta())
    .unwrap_or_default()
}

fn assert_worktree_files(repo: &gix::Repository, present: &[&str], absent: &[&str]) {
    for path in present {
        let path = repo.workdir_path(path).expect("non-bare");
        assert!(
            path.exists(),
            "{} should exist in the worktree",
            path.display()
        );
    }
    for path in absent {
        let path = repo.workdir_path(path).expect("non-bare");
        assert!(
            !path.exists(),
            "{} should not exist in the worktree",
            path.display()
        );
    }
}

#[test]
fn operation_denied_on_improper_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-one-stack-ws-advanced",
            |_meta| {},
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 0d01196 (HEAD -> gitbutler/workspace) O1
* 4979833 GitButler Workspace Commit
* 3183e43 (main, B, A) M1

"#]]
    );
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on 3183e43
└── ≡:0:anon on 3183e43
    └── :0:anon
        ├── ·0d01196 (🏘️) ►gitbutler/workspace[🌳]
        └── ·4979833 (🏘️)

"#]]
    );

    let branch_b = r("refs/heads/B");
    let err = but_workspace::branch::apply(branch_b, ws.clone(), &repo, &mut meta, apply_options())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to work on workspace whose workspace commit isn't at the top",
        "cannot apply on a workspace that isn't proper"
    );

    let err =
        but_workspace::branch::apply(r("HEAD"), ws.clone(), &repo, &mut meta, apply_options())
            .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to apply symbolic ref 'HEAD' due to potential ambiguity"
    );

    let err = but_workspace::branch::unapply(branch_b, &ws, &repo, &mut meta, unapply_options())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to work on workspace whose workspace commit isn't at the top",
        "cannot unapply on a workspace that isn't proper"
    );

    let err = but_workspace::branch::unapply(r("HEAD"), &ws, &repo, &mut meta, unapply_options())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to unapply symbolic ref 'HEAD' due to potential ambiguity"
    );

    Ok(())
}

#[test]
fn unapply_tip_of_ad_hoc_branch_is_an_error() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-with-3-commits")?;
    // fixture starts with a single local main branch
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 281da94 (HEAD -> main) 3
* 12995d7 2
* 3d57fc1 1

"#]]
    );

    let ws = but_graph::Graph::from_repo(
        &repo,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .into_workspace()?;
    // a normal checked-out branch is still an ad-hoc workspace without workspace metadata
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:3:main <> ✓!
└── ≡👉:3:main[🌳] {1}
    └── 👉:3:main[🌳]
        ├── ·281da94 (🏘️)
        ├── ·12995d7 (🏘️)
        └── ·3d57fc1 (🏘️)

"#]]
    );

    let err = but_workspace::branch::unapply(
        r("refs/heads/main"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )
    .expect_err("unapplied workspace should be empty, but this one can't be");
    assert_eq!(
        err.to_string(),
        "Cannot unapply branch 'main' from an ad-hoc workspace because the workspace cannot be empty"
    );
    Ok(())
}

#[test]
fn ws_ref_no_ws_commit_two_virtual_stacks_on_same_commit_apply_dependent_first()
-> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-no-ws-commit-one-stack-one-branch",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::Inactive, &["B"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    // We know a stack, but nothing is actually in the workspace.
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );

    // Put "B" into the workspace, even though it's the dependent branch of A.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:2:B on e5d0542 {1}
    └── 📙:2:B

"#]]
    );

    // Applying A is always a new stack then.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    // the workspace ref still points to the base e5d0542
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:2:B on e5d0542 {1}
│   └── 📙:2:B
└── ≡📙:1:A on e5d0542 {41}
    └── 📙:1:A

"#]]
    );

    // It's all virtual.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    let ws = out.workspace;
    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // A was removed with metadata only
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:1:B on e5d0542 {1}
    └── 📙:1:B

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    // virtual stacks don't cause checkouts.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    // no stacks are left
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );
    // no workspace commit is present
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );
    Ok(())
}

mod workspace_disposition {
    use super::*;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_testsupport::gix_testtools::tempfile::TempDir;

    #[test]
    fn keep_workspace_commit() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = workspace_with_virtual_base()?;
        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::KeepWorkspaceCommit),
        )?;
        assert!(
            out.workspace.kind.has_managed_commit(),
            "KeepWorkspaceCommit must preserve the checked-out managed workspace commit"
        );
        assert!(
            out.workspace_merge.is_some(),
            "unapply must rebuild the workspace merge instead of collapsing the workspace ref"
        );
        // A was removed
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️:3:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:5:virtual-base on 85efbe4 {1}
│   └── 📙:5:virtual-base
└── ≡📙:7:B on 85efbe4 {3}
    └── 📙:7:B
        └── ·c813d8d (🏘️)

"#]]
        );
        // the workspace commit is still there with two parents (even though there is no need)
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 09d8e52 (A) A
| * 3b77768 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/|
| * c813d8d (B) B
|/
* 85efbe4 (origin/main, virtual-base, main) M

"#]]
        );

        Ok(())
    }

    #[test]
    fn keep_workspace_reference() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = workspace_with_virtual_base()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::KeepWorkspaceReference),
        )?;
        assert!(
            out.workspace_merge.is_some(),
            "KeepWorkspaceReference should keep a workspace commit when a virtual stack remains next to a real stack"
        );
        // There still is a workspace merge commit, we have two stacks, virtual or not
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️:3:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:5:virtual-base on 85efbe4 {1}
│   └── 📙:5:virtual-base
└── ≡📙:7:B on 85efbe4 {3}
    └── 📙:7:B
        └── ·c813d8d (🏘️)

"#]]
        );
        // the virtual stack counts as real stack and it merges the base to account for it
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 09d8e52 (A) A
| * 3b77768 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/|
| * c813d8d (B) B
|/
* 85efbe4 (origin/main, virtual-base, main) M

"#]]
        );

        Ok(())
    }

    #[test]
    fn prevent_unnecessary_workspace_reference_checks_out_last_real_stack() -> anyhow::Result<()> {
        let (_tmp, graph, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "ws-ref-ws-commit-two-stacks",
                |meta| {
                    add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                    add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
                },
            )?;
        let ws = graph.into_workspace()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
        )?;
        // with one real stack left, the workspace ref is deleted and the remaining stack is checked out
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/B",
    ),
}

"#]]
        );
        // the workspace ref is deleted and HEAD points to the last real stack
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 09d8e52 (A) A
| * c813d8d (HEAD -> B) B
|/
* 85efbe4 (origin/main, main) M

"#]]
        );
        // the projection is the checked-out remaining stack
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
⌂:4:B <> ✓refs/remotes/origin/main on 85efbe4
└── ≡👉📙:4:B[🌳] on 85efbe4 {1}
    ├── 👉📙:4:B[🌳]
    │   └── ·c813d8d (🏘️)
    └── :2:main <> origin/main →:3:

"#]]
        );

        Ok(())
    }

    #[test]
    fn prevent_unnecessary_workspace_references_keep_workspace_commit_keeps_merge_when_ref_remains()
    -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = workspace_with_virtual_base()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/virtual-base"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(
                WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
            ),
        )?;
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
        );
        assert!(
            out.workspace.kind.has_managed_commit(),
            "compatibility disposition should preserve the managed workspace commit when the workspace ref remains"
        );
        assert!(
            out.workspace_merge.is_some(),
            "compatibility disposition should rebuild the workspace merge instead of collapsing the ref to the last real stack"
        );
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:4:A on 85efbe4 {2}
│   └── 📙:4:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:8:B on 85efbe4 {3}
    └── 📙:8:B
        └── ·c813d8d (🏘️)

"#]]
        );
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
*   483c8bd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\
| * c813d8d (B) B
* | 09d8e52 (A) A
|/
* 85efbe4 (origin/main, virtual-base, main) M

"#]]
            .raw()
        );

        Ok(())
    }

    #[test]
    fn allow_workspace_reference_deletion() -> anyhow::Result<()> {
        let (_tmp, _, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "no-ws-ref-no-ws-commit-two-branches",
                |_meta| {},
            )?;

        let ws = Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?
            .into_workspace()?;
        let out =
            but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
        let ws = out.workspace;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
└── ≡📙:1:A on e5d0542 {41}
    └── 📙:1:A

"#]]
        );

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
        )?;
        // deleting the workspace reference should switch to the local tracking branch of the target
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/main",
    ),
}

"#]]
        );

        // everything is dissolved, there is no gitbutler/workspace ref either
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
⌂:2:main <> ✓refs/remotes/origin/main on e5d0542
└── ≡👉:2:main[🌳] <> origin/main →:3: on e5d0542 {1}
    └── 👉:2:main[🌳] <> origin/main →:3:

"#]]
        );

        Ok(())
    }

    #[test]
    fn compatibility_mode_deletes_workspace_reference_when_possible() -> anyhow::Result<()> {
        let (_tmp, _, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "no-ws-ref-no-ws-commit-two-branches",
                |_meta| {},
            )?;

        let ws = Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?
            .into_workspace()?;
        let out =
            but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
        let ws = out.workspace;

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(
                WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
            ),
        )?;
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/main",
    ),
}

"#]]
        );

        // the compatibility mode still removes the whole workspace when it can
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
⌂:2:main <> ✓refs/remotes/origin/main on e5d0542
└── ≡👉:2:main[🌳] <> origin/main →:3: on e5d0542 {1}
    └── 👉:2:main[🌳] <> origin/main →:3:

"#]]
        );

        Ok(())
    }

    #[test]
    fn unapply_workspace_ref_requires_disposition_that_allows_switching() -> anyhow::Result<()> {
        let (_tmp, _, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "no-ws-ref-no-ws-commit-two-branches",
                |_meta| {},
            )?;

        let ws = Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?
            .into_workspace()?;
        let out =
            but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
        let ws = out.workspace;
        // the workspace ref is checked out
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
└── ≡📙:1:A on e5d0542 {41}
    └── 📙:1:A

"#]]
        );

        let refs_before = visualize_commit_graph_all(&repo)?;
        let err = but_workspace::branch::unapply(
            r("refs/heads/gitbutler/workspace"),
            &ws,
            &repo,
            &mut meta,
            unapply_options(),
        )
        .expect_err("unapplying the workspace ref requires checking out another ref");
        assert_eq!(
            err.to_string(),
            "Cannot unapply workspace reference 'gitbutler/workspace' without switching away from it"
        );

        // the workspace is unchanged
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
└── ≡📙:1:A on e5d0542 {41}
    └── 📙:1:A

"#]]
        );
        assert_eq!(
            visualize_commit_graph_all(&repo)?,
            refs_before,
            "failing before checkout must leave refs unchanged"
        );

        Ok(())
    }

    #[test]
    fn keep_workspace_commit_with_last_stack_removed() -> anyhow::Result<()> {
        let (_tmp, graph, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "ws-ref-ws-commit-one-stack",
                |meta| {
                    add_stack_with_segments(meta, 1, "B", StackState::InWorkspace, &["A"]);
                },
            )?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* d69fe94 (B) B
* 09d8e52 (A) A
* 85efbe4 (origin/main, main) M

"#]]
        );

        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:8:B on 85efbe4 {1}
    ├── 📙:8:B
    │   └── ·d69fe94 (🏘️)
    └── 📙:4:A
        └── ·09d8e52 (🏘️)

"#]]
        );

        let out = but_workspace::branch::unapply(
            r("refs/heads/B"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::KeepWorkspaceCommit),
        )?;
        assert!(
            out.workspace_merge.is_none(),
            "removing the last stack cannot rebuild a workspace merge commit"
        );
        // the workspace is empty
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4

"#]]
        );
        // the workspace commit is kept as no-op, it's legacy behaviour that will be removed
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* d69fe94 (B) B
* 09d8e52 (A) A
| * bde8ed6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* 85efbe4 (origin/main, main) M

"#]]
        );

        Ok(())
    }

    fn workspace_with_virtual_base() -> anyhow::Result<(
        TempDir,
        gix::Repository,
        VirtualBranchesTomlMetadata,
        but_graph::Workspace,
    )> {
        let (tmp, repo, mut meta) = named_writable_scenario("ws-ref-ws-commit-two-stacks")?;
        let base_id = repo
            .find_reference("refs/heads/main")?
            .peel_to_id()?
            .detach();
        repo.reference(
            r("refs/heads/virtual-base"),
            base_id,
            PreviousValue::Any,
            "create a metadata-only virtual stack on the workspace base",
        )?;
        add_stack_with_segments(&mut meta, 1, "virtual-base", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 2, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 3, "B", StackState::InWorkspace, &[]);
        let ws = Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?
            .into_workspace()?;
        assert!(
            ws.kind.has_managed_commit(),
            "fixture starts with a managed workspace commit"
        );

        let repo_log = visualize_commit_graph_all(&repo)?;

        snapbox::assert_data_eq!(
            repo_log,
            snapbox::str![[r#"
*   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\
| * 09d8e52 (A) A
* | c813d8d (B) B
|/
* 85efbe4 (origin/main, virtual-base, main) M

"#]]
            .raw()
        );
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:6:virtual-base on 85efbe4 {1}
│   └── 📙:6:virtual-base
├── ≡📙:4:A on 85efbe4 {2}
│   └── 📙:4:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:9:B on 85efbe4 {3}
    └── 📙:9:B
        └── ·c813d8d (🏘️)

"#]]
        );

        Ok((tmp, repo, meta, ws))
    }
}

#[test]
fn main_with_advanced_remote_tracking_branch() -> anyhow::Result<()> {
    let (_tmp, _graph, mut repo, vb_version_cannot_have_remotes, _description) =
        named_writable_scenario_with_description_and_graph(
            "main-with-advanced-remote",
            |_meta| {},
        )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 6b40b15 (origin/feature) without-local-tracking
| * 552e7dc (origin/main) only-on-remote
|/
* 3183e43 (HEAD -> main) M1

"#]]
    );

    let mut meta = InMemoryRefMetadata::default();
    meta.workspaces.push((
        "refs/heads/gitbutler/workspace".try_into()?,
        ref_metadata::Workspace::default(),
    ));
    git(&repo)
        .args([
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ])
        .run();
    git(&repo)
        .args(["branch", "--set-upstream-to", "origin/main", "main"])
        .run();
    repo.reload()?;
    let graph = Graph::from_repo(
        &repo,
        &vb_version_cannot_have_remotes,
        ref_metadata::ProjectMeta::default(),
        Overlay::default(),
    )?;
    let ws = graph.into_workspace()?;
    // note how the remote isn't interesting as we have no target configured, nor an extra target.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:2:main <> ✓!
└── ≡👉:2:main[🌳] <> origin/main →:3:⇣1 {1}
    └── 👉:2:main[🌳] <> origin/main →:3:⇣1
        ├── 🟣552e7dc ►origin/main
        └── ❄3183e43 (🏘️)

"#]]
    );

    // We cannot apply remote tracking branches directly, but it resolves automatically to local tracking branches.
    let out = but_workspace::branch::apply(
        r("refs/remotes/origin/main"),
        ws.clone(),
        &repo,
        &mut meta,
        apply_options(),
    )?;
    // nothing was actually applied as the `main` branch is already in the workspace
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    workspace_ref_created: false,
    applied_branches: "[]",
}

"#]]
    );

    // Set up an automatic tracking of origin/feature, as remote tracking branches can't be in the workspace.
    let out = but_workspace::branch::apply(
        r("refs/remotes/origin/feature"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: true,
    applied_branches: "[refs/heads/main, refs/heads/feature]",
}

"#]]
    );
    let ws = out.workspace;
    // both branches, main and feature, are available in the newly created workspace ref.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:6:feature <> origin/feature →:7: {2ec}
│   └── 📙:6:feature <> origin/feature →:7:
│       ├── ❄6b40b15 (🏘️) ►origin/feature
│       └── ❄3183e43 (🏘️) ►main
└── ≡📙:4:main <> origin/main →:5:⇣1 {1a5}
    └── 📙:4:main <> origin/main →:5:⇣1
        ├── 🟣552e7dc ►origin/main
        └── ❄3183e43 (🏘️)

"#]]
    );

    // the new local tracking ref actually exists, and is in the right spot.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   e00cf08 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
| * 6b40b15 (origin/feature, feature) without-local-tracking
|/
| * 552e7dc (origin/main) only-on-remote
|/
* 3183e43 (main) M1

"#]]
    );

    repo.reload()?;
    let config = repo.config_snapshot();
    let section = config.section("branch", Some("feature".into()))?;
    snapbox::assert_data_eq!(
        section.to_bstring().to_string(),
        snapbox::str![[r#"
[branch "feature"]
	remote = origin
	merge = refs/heads/feature

"#]]
    );

    Ok(())
}

#[test]
fn unapply_remotely_tracked_tip_of_multi_segment_stack_can_delete_workspace_ref()
-> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "no-ws-ref-stack-and-dependent-branch",
            |_meta| {},
        )?;
    for branch in ["A", "B", "C"] {
        let local_name = Category::LocalBranch.to_full_name(branch)?;
        let remote_name: gix::refs::FullName = format!("refs/remotes/origin/{branch}")
            .as_str()
            .try_into()?;
        let commit_id = repo
            .find_reference(local_name.as_ref())?
            .peel_to_id()?
            .detach();
        repo.reference(
            remote_name,
            commit_id,
            PreviousValue::Any,
            "create remote-tracking ref for stack branch",
        )?;
    }

    let ws = graph.into_workspace()?;
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:8:B <> origin/B →:11: on 85efbe4 {42}
    └── 📙:8:B <> origin/B →:11:
        ├── ❄f084d61 (🏘️) ►A, ►C, ►origin/A, ►origin/B, ►origin/C
        └── ❄7076dee (🏘️) ►D, ►E

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(
            WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
        ),
    )?;
    // deleting the workspace ref uses the target fallback instead of the unapplied stack
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/main",
    ),
}

"#]]
    );
    // the managed workspace ref is deleted and the target's local tracking branch is checked out
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f084d61 (origin/C, origin/B, origin/A, C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );

    // an ad-hoc workspace remains on the target's local tracking branch
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
⌂:1:main <> ✓refs/remotes/origin/main on 85efbe4
└── ≡👉:1:main[🌳] <> origin/main →:2: on 85efbe4 {1}
    └── 👉:1:main[🌳] <> origin/main →:2:

"#]]
    );
    Ok(())
}

#[test]
fn workspace_with_out_of_ws_ref_and_anon_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "advanced-stack-and-unnamed-stack-in-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "outside", StackState::InWorkspace, &[]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d03b217 (feature) F1
| *   dd3b979 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| |\
| * | d6bdeab missing-name
|/ /
| | * 5121eb9 (outside) advanced-outside
| |/
| * 67c6397 advanced-inside
|/
* 3183e43 (origin/main, main) M1

"#]]
        .raw()
    );

    let ws = graph.into_workspace()?;
    // The workspace commit still points to `advanced-inside`. The `outside` ref has advanced
    // beyond it, so `advanced-outside` is outside the workspace and the physical path remains
    // anonymous until reference-path projection is ported.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡:4:anon on 3183e43
│   └── :4:anon
│       └── ·d6bdeab (🏘️)
└── ≡:3:anon on 3183e43
    └── :3:anon
        └── ·67c6397 (🏘️)

"#]]
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/feature"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/feature]",
}

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:8:outside on 3183e43 {1}
│   └── 📙:8:outside
│       ├── ·5121eb9 (🏘️)
│       └── ·67c6397 (🏘️)
├── ≡📙:9:feature on 3183e43 {2ec}
│   └── 📙:9:feature
│       └── ·d03b217 (🏘️)
└── ≡:5:anon on 3183e43
    └── :5:anon
        └── ·d6bdeab (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn apply_branch_projected_from_outside_merges_advanced_commits() -> anyhow::Result<()> {
    let (_tmp, _graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-one-stack",
            |_meta| {},
        )?;
    git(&repo).args(["checkout", "B"]).run();
    git(&repo)
        .args(["commit", "--allow-empty", "-m", "B outside workspace"])
        .run();
    git(&repo).args(["checkout", "gitbutler/workspace"]).run();
    let graph = Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?;
    let ws = graph.into_workspace()?;
    assert!(
        !ws.stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .any(|segment| segment.ref_name() == Some(r("refs/heads/B"))),
        "the cached workspace does not yet project inactive B"
    );
    let outside_tip = repo.find_reference("refs/heads/B")?.peel_to_id()?.detach();
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(out.status, OutcomeStatus::Applied);
    assert!(
        out.workspace_merge.is_some(),
        "applying outside commits must rebuild the managed workspace"
    );
    assert!(
        out.workspace.stacks.iter().any(|stack| {
            stack
                .segments
                .iter()
                .flat_map(|segment| &segment.commits)
                .any(|commit| commit.id == outside_tip)
        }),
        "the advanced branch tip is visible inside the rebuilt workspace"
    );
    Ok(())
}

#[test]
fn ws_ref_no_ws_commit_two_stacks_on_same_commit() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-no-ws-commit-one-stack-one-branch",
            |_meta| {},
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );

    // Put "A" into the workspace, yielding a single branch.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:1:A on e5d0542 {41}
    └── 📙:1:A

"#]]
    );
    // No commit was created, as it's not enabled by default.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    let branch_b = r("refs/heads/B");
    let out = but_workspace::branch::apply(branch_b, ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );
    // Note how it will create a new stack (to keep it simple),
    // in theory we could also add B as dependent branch.
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:1:A on e5d0542 {41}
│   └── 📙:1:A
└── ≡📙:2:B on e5d0542 {42}
    └── 📙:2:B

"#]]
    );

    // Nothing changed visibly, still, it's all in the metadata.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    let out = but_workspace::branch::unapply(branch_b, &ws, &repo, &mut meta, unapply_options())?;
    let ws = out.workspace.into_owned();
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:1:A on e5d0542 {41}
    └── 📙:1:A

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    Ok(())
}

#[test]
fn unapply_natural_stack_with_partial_workspace_metadata() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-two-stacks",
            |meta| {
                add_stack_with_segments(meta, 1, "B", StackState::InWorkspace, &["not-in-graph"]);
            },
        )?;
    let ws = graph.into_workspace()?;
    // A is naturally visible, and B has metadata that matches it with an extra segment in metadata that's not there
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:7:B on 85efbe4 {1}
│   └── 📙:7:B
│       └── ·c813d8d (🏘️)
└── ≡:8:A on 85efbe4
    └── :8:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;

    // A is removed and B is left
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:4:B on 85efbe4 {1}
    └── 📙:4:B
        └── ·c813d8d (🏘️) ►gitbutler/workspace[🌳]

"#]]
    );

    let ws_md = out.workspace.metadata.as_ref().expect("present");
    // the extra segment is still present in metadata to help with stack identities
    snapbox::assert_data_eq!(
        ws_md.to_debug(),
        snapbox::str![[r#"
Workspace {
    ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
    stacks: [
        WorkspaceStack {
            id: 00000000-0000-0000-0000-000000000001,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/B",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/not-in-graph",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ],
    target_ref: "refs/remotes/origin/main",
    target_commit_id: Sha1(85efbe4d5a663bff0ed8fb5fbc38a72be0592f55),
    push_remote: None,
}

"#]]
    );
    Ok(())
}

#[test]
fn unapply_natural_stack_branch_without_workspace_metadata() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack-files",
            |_meta| {},
        )?;
    let ws = graph.into_workspace()?;
    // a single and multi-branch stack are visible without any workspace metadata
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 893d602
├── ≡:8:C on 893d602
│   ├── :8:C
│   │   └── ·356de85 (🏘️)
│   └── :10:B
│       └── ·f25f65c (🏘️)
└── ≡:9:A on 893d602
    └── :9:A
        └── ·26e45af (🏘️)

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/C"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;

    // C was unapplied, and the workspace commit removed
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 893d602
└── ≡📙:2:A on 893d602 {1}
    └── 📙:2:A
        └── ·26e45af (🏘️) ►gitbutler/workspace[🌳]

"#]]
    );
    // the workspace commit isn't retained due to the workspace disposition
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 26e45af (HEAD -> gitbutler/workspace, A) A
| * 356de85 (C) C
| * f25f65c (B) B
|/
* 893d602 (origin/main, main) M

"#]]
    );

    let ws_md = out.workspace.metadata.as_ref().expect("present");
    // the metadata matches the real workspace now
    snapbox::assert_data_eq!(
        sanitize_uuids_and_timestamps(format!("{ws_md:#?}")),
        snapbox::str![[r#"
Workspace {
    ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
    stacks: [
        WorkspaceStack {
            id: 1,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/C",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/B",
                    archived: false,
                },
            ],
            workspacecommit_relation: Outside,
        },
        WorkspaceStack {
            id: 2,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/A",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ],
    target_ref: "refs/remotes/origin/main",
    target_commit_id: Sha1(893d6025c9de29ed75369de967e52a1154bbf0ee),
    push_remote: None,
}
"#]]
    );
    Ok(())
}

#[test]
fn no_ws_ref_no_ws_commit_two_stacks_on_same_commit_ad_hoc_workspace_without_target_branch()
-> anyhow::Result<()> {
    let (_tmp, _, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "no-ws-ref-no-ws-commit-two-branches",
            |_meta| {},
        )?;

    // Delete the target branch.
    {
        let mut ws_md = meta.workspace("refs/heads/gitbutler/workspace".try_into().unwrap())?;
        let mut project_meta = ws_md.project_meta();
        assert!(project_meta.target_ref.is_some());
        project_meta.target_ref.take();
        ws_md.set_project_meta(project_meta);
        meta.set_workspace(&ws_md)?;
        let ws_md = meta.workspace("refs/heads/gitbutler/workspace".try_into().unwrap())?;
        assert!(
            ws_md.project_meta().target_ref.is_none(),
            "we just deleted it, it should be transferred"
        );
    }
    let graph = but_graph::Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> main, origin/main, B, A) A

"#]]
    );
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:3:main <> ✓!
└── ≡👉:3:main[🌳] <> origin/main →:4: {1}
    └── 👉:3:main[🌳] <> origin/main →:4:
        └── ❄e5d0542 (🏘️) ►A, ►B, ►origin/main

"#]]
    );

    // Put "A" into the workspace, creating the workspace ref, but never put a branch related to the target in as well,
    // which is currently checked out with `main`.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: true,
    applied_branches: "[refs/heads/main, refs/heads/A]",
}

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:4:main <> origin/main →:5: {1a5}
│   └── 📙:4:main <> origin/main →:5:
│       └── ❄e5d0542 (🏘️) ►A, ►B, ►gitbutler/workspace[🌳], ►origin/main
└── ≡📙:1:A {41}
    └── 📙:1:A
        └── ·e5d0542 (🏘️) ►B, ►gitbutler/workspace[🌳], ►main, ►origin/main

"#]]
    );

    // No commit was created, as it's not enabled by default, but a ws-ref was created, and it's checked out.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A

"#]]
    );

    // Thread the post-apply workspace into the next apply instead of reusing the stale projection.
    let ws = out.workspace;
    let out = but_workspace::branch::apply(
        r("refs/heads/B"),
        ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            // Make it appear in place of A, in the center.
            order: Some(1),
            ..apply_options()
        },
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:4:main <> origin/main →:5: {1a5}
│   └── 📙:4:main <> origin/main →:5:
│       └── ❄e5d0542 (🏘️) ►A, ►B, ►gitbutler/workspace[🌳], ►origin/main
├── ≡📙:2:B {42}
│   └── 📙:2:B
│       └── ·e5d0542 (🏘️) ►A, ►gitbutler/workspace[🌳], ►main, ►origin/main
└── ≡📙:1:A {41}
    └── 📙:1:A
        └── ·e5d0542 (🏘️) ►B, ►gitbutler/workspace[🌳], ►main, ►origin/main

"#]]
    );

    // Nothing changed visibly, still, it's all in the metadata.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A

"#]]
    );

    // Reset the workspace to 'unapply', but keep the per-branch metadata.
    let mut ws_md = meta.workspace(ws.ref_name().expect("proper gb workspace"))?;
    for stack in &mut ws_md.stacks {
        stack.workspacecommit_relation = Outside;
    }
    meta.set_workspace(&ws_md)?;

    let ws = Graph::from_repo(
        &repo,
        &meta,
        ws.graph.project_meta().clone(),
        Overlay::default(),
    )?
    .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:4:main <> origin/main →:5:
    └── 📙:4:main <> origin/main →:5:
        └── ❄e5d0542 (🏘️) ►A, ►B, ►gitbutler/workspace[🌳], ►origin/main

"#]]
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            workspace_merge: WorkspaceMerge::AlwaysMerge,
            ..apply_options()
        },
    )?;
    // A workspace commit was created, even though it does nothing.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 6277161 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* e5d0542 (origin/main, main, B, A) A

"#]]
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:2:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:3:A {41}
    └── 📙:3:A
        └── ·e5d0542 (🏘️) ►B, ►main, ►origin/main

"#]]
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/B"),
        ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            workspace_merge: WorkspaceMerge::AlwaysMerge,
            ..apply_options()
        },
    )?;

    // It's idempotent, but has to update the workspace commit nonetheless for the comment, which depends on the stacks.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 0de6159 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* e5d0542 (origin/main, main, B, A) A

"#]]
        .raw()
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:2:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:4:B {42}
    └── 📙:4:B
        └── ·e5d0542 (🏘️) ►A, ►main, ►origin/main

"#]]
    );

    Ok(())
}

#[test]
fn no_ws_ref_no_ws_commit_two_stacks_on_same_commit_ad_hoc_workspace_with_target()
-> anyhow::Result<()> {
    let (_tmp, _, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "no-ws-ref-no-ws-commit-two-branches",
            |_meta| {},
        )?;

    git(&repo)
        .args([
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ])
        .run();
    git(&repo)
        .args(["branch", "--set-upstream-to", "origin/main", "main"])
        .run();
    repo.reload()?;
    let graph = but_graph::Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> main, origin/main, B, A) A

"#]]
    );
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:3:main <> ✓refs/remotes/origin/main on e5d0542
└── ≡👉:3:main[🌳] <> origin/main →:4: on e5d0542 {1}
    └── 👉:3:main[🌳] <> origin/main →:4:

"#]]
    );

    // Put "A" into the workspace, creating the workspace ref, but never put a branch related to the target in as well,
    // which is currently checked out with `main`.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: true,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
└── ≡📙:1:A on e5d0542 {41}
    └── 📙:1:A

"#]]
    );

    // No commit was created, as it's not enabled by default, but a ws-ref was created, and it's checked out.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
├── ≡📙:1:A on e5d0542 {41}
│   └── 📙:1:A
└── ≡📙:2:B on e5d0542 {42}
    └── 📙:2:B

"#]]
    );

    // Nothing changed visibly, still, it's all in the metadata.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A

"#]]
    );

    // Cannot put local tracking branch of target into workspace that has it configured.
    for branch in ["refs/heads/main", "refs/remotes/origin/main"] {
        let err =
            but_workspace::branch::apply(r(branch), ws.clone(), &repo, &mut meta, apply_options())
                .unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("Cannot add the target '{branch}' branch to its own workspace")
        );

        let out =
            but_workspace::branch::unapply(r(branch), &ws, &repo, &mut meta, unapply_options())?;
        assert!(
            !out.workspace_changed(),
            "target refs are never applied, so unapplying them is always fulfilled after the call (i.e. they aren't applied)"
        );
    }

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    let ws = out.workspace.into_owned();
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
└── ≡📙:1:A on e5d0542 {41}
    └── 📙:1:A

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )?;
    // the target's local tracking branch is checked out
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
⌂:2:main <> ✓refs/remotes/origin/main on e5d0542
└── ≡👉:2:main[🌳] <> origin/main →:3: on e5d0542 {1}
    └── 👉:2:main[🌳] <> origin/main →:3:

"#]]
    );
    // no workspace ref is present anymore
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> main, origin/main, B, A) A

"#]]
    );

    Ok(())
}

#[test]
fn apply_after_switching_out_of_workspace_drops_stale_stacks() -> anyhow::Result<()> {
    // A managed workspace exists with `outside` marked in-workspace. The user then `git switch`es
    // onto `feature`, a branch outside the workspace, leaving the metadata stale.
    let (_tmp, _graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "advanced-stack-and-unnamed-stack-in-workspace",
            |meta| {
                add_stack_with_segments(meta, 1, "outside", StackState::InWorkspace, &[]);
            },
        )?;
    git(&repo).args(["branch", "applied", "feature"]).run();
    git(&repo).args(["checkout", "feature"]).run();

    let ws = Graph::from_repo(
        &repo,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Overlay::default(),
    )?
    .into_workspace()?;

    // Apply a different branch from the adhoc `feature` checkout. The new workspace should hold the
    // current branch plus the applied one, while the stale `outside` stack is demoted.
    let out = but_workspace::branch::apply(
        r("refs/heads/applied"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    assert_eq!(out.status, OutcomeStatus::Applied);

    let ws_md = meta.workspace(WORKSPACE_REF_NAME.try_into().unwrap())?;
    let in_workspace = |name: &str| {
        ws_md
            .stacks
            .iter()
            .find(|s| s.name().is_some_and(|n| n.shorten() == name))
            .map(|s| s.is_in_workspace())
    };
    assert_eq!(
        in_workspace("outside"),
        Some(false),
        "the stale stack from the abandoned workspace is demoted to outside"
    );
    assert_eq!(
        in_workspace("feature"),
        Some(true),
        "the foreign branch we switched to joins the new workspace"
    );
    assert!(
        ws_md
            .find_branch(r("refs/heads/applied"), StackKind::Applied)
            .is_some(),
        "the newly applied branch is in the new workspace metadata"
    );
    Ok(())
}

#[test]
fn apply_from_enclosed_adhoc_workspace_rebuilds_around_current_and_applied() -> anyhow::Result<()> {
    // A managed workspace has two live stacks, then HEAD is moved to one of its branches. Applying
    // a third branch from that enclosed AdHoc checkout rebuilds the workspace around the checked-out
    // branch and the newly applied branch.
    let (_tmp, _graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-two-file-stacks",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;
    // initial workspace has A and B applied
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   4fce3a1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\
| * ccf539c (A) A
* | 53c254d (B) B
|/
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["A", "B"], &["C"]);
    // workspace checkout contains A and B files
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── A:100644:f70f10e4db19068f79bc43844b49f3eece45c4e8
├── B:100644:223b7836fb19fdf64ba2d3cd6173c6a283141f78
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );

    git(&repo).args(["checkout", "-b", "C", "main"]).run();
    std::fs::write(repo.workdir_path("C").expect("non-bare"), "C\n")?;
    git(&repo).args(["add", "C"]).run();
    git(&repo).args(["commit", "-m", "add C"]).run();
    git(&repo).args(["checkout", "B"]).run();
    // direct checkout of B leaves C outside the workspace
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 863775d (C) add C
| *   4fce3a1 (gitbutler/workspace) GitButler Workspace Commit
| |\
| | * ccf539c (A) A
| |/
|/|
| * 53c254d (HEAD -> B) B
|/
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["B"], &["A", "C"]);
    // direct checkout of B contains only B's file
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── B:100644:223b7836fb19fdf64ba2d3cd6173c6a283141f78
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );

    let ws = Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?
        .into_workspace()?;
    assert!(
        matches!(ws.kind, WorkspaceKind::Managed { .. }),
        "direct checkout of B can still project as a managed workspace"
    );
    // direct checkout of B is still enclosed by the existing workspace
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace <> ✓refs/remotes/origin/main on 893d602
├── ≡📙:8:A on 893d602 {1}
│   └── 📙:8:A
│       └── ·ccf539c (🏘️)
└── ≡👉📙:5:B[🌳] on 893d602 {2}
    └── 👉📙:5:B[🌳]
        └── ·53c254d (🏘️)

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(out.status, OutcomeStatus::Applied);
    // applying C rebuilds the workspace around B and C
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* ccf539c (A) A
| *   7ed2c6c (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| |\
| | * 863775d (C) add C
| |/
|/|
| * 53c254d (B) B
|/
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["B", "C"], &["A"]);
    // workspace checkout contains B and C files after apply
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── B:100644:223b7836fb19fdf64ba2d3cd6173c6a283141f78
├── C:100644:3cc58df83752123644fef39faab2393af643b1d2
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );
    assert_eq!(
        repo.head_name()?.as_ref().map(|rn| rn.as_bstr()),
        Some(r("refs/heads/gitbutler/workspace").as_bstr()),
        "applying from an ad-hoc checkout should switch back to the managed workspace"
    );

    let ws_md = meta.workspace(WORKSPACE_REF_NAME.try_into().unwrap())?;
    let in_workspace = |name: &str| {
        ws_md
            .stacks
            .iter()
            .find(|s| s.name().is_some_and(|n| n.shorten() == name))
            .map(|s| s.is_in_workspace())
    };
    assert_eq!(
        in_workspace("A"),
        Some(false),
        "previously visible stack A should be left behind"
    );
    assert_eq!(
        in_workspace("B"),
        Some(true),
        "checked-out enclosed stack B should stay in the workspace"
    );
    assert_eq!(
        in_workspace("C"),
        Some(true),
        "the newly applied branch joins the workspace"
    );
    Ok(())
}

#[test]
fn apply_from_adhoc_checkout_rebuilds_around_current_and_applied() -> anyhow::Result<()> {
    // A managed workspace has two live stacks, then HEAD is moved to a third branch. Applying one
    // of the previously applied branches rebuilds the workspace around the checked-out branch and
    // the branch being applied.
    let (_tmp, _graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-two-file-stacks",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;
    // initial workspace has A and B applied
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   4fce3a1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\
| * ccf539c (A) A
* | 53c254d (B) B
|/
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["A", "B"], &["C"]);
    // workspace checkout contains A and B files
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── A:100644:f70f10e4db19068f79bc43844b49f3eece45c4e8
├── B:100644:223b7836fb19fdf64ba2d3cd6173c6a283141f78
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );

    git(&repo).args(["checkout", "-b", "C", "main"]).run();
    std::fs::write(repo.workdir_path("C").expect("non-bare"), "C\n")?;
    git(&repo).args(["add", "C"]).run();
    git(&repo).args(["commit", "-m", "add C"]).run();
    // direct checkout of C leaves A and B in the old workspace
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 863775d (HEAD -> C) add C
| *   4fce3a1 (gitbutler/workspace) GitButler Workspace Commit
| |\
| | * ccf539c (A) A
| |/
|/|
| * 53c254d (B) B
|/
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["C"], &["A", "B"]);
    // direct checkout of C contains only C's file
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── C:100644:3cc58df83752123644fef39faab2393af643b1d2
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );

    let ws = Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?
        .into_workspace()?;
    // direct checkout of C is an ad-hoc workspace next to the existing managed workspace
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:7:C <> ✓refs/remotes/origin/main on 893d602
└── ≡👉:7:C[🌳] on 893d602 {1}
    └── 👉:7:C[🌳]
        └── ·863775d (🏘️)

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(out.status, OutcomeStatus::Applied);
    // applying A rebuilds the workspace around C and A
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 53c254d (B) B
| *   dbb9910 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| |\
| | * 863775d (C) add C
| |/
|/|
| * ccf539c (A) A
|/
* 893d602 (origin/main, main) M

"#]]
        .raw()
    );
    assert_worktree_files(&repo, &["A", "C"], &["B"]);
    // workspace checkout contains C and A files after apply
    snapbox::assert_data_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── A:100644:f70f10e4db19068f79bc43844b49f3eece45c4e8
├── C:100644:3cc58df83752123644fef39faab2393af643b1d2
└── base:100644:df967b96a579e45a18b8251732d16804b2e56a55

"#]]
    );
    assert_eq!(
        repo.head_name()?.as_ref().map(|rn| rn.as_bstr()),
        Some(r("refs/heads/gitbutler/workspace").as_bstr()),
        "applying from an ad-hoc checkout should switch back to the managed workspace"
    );

    let ws_md = meta.workspace(WORKSPACE_REF_NAME.try_into().unwrap())?;
    let in_workspace = |name: &str| {
        ws_md
            .stacks
            .iter()
            .find(|s| s.name().is_some_and(|n| n.shorten() == name))
            .map(|s| s.is_in_workspace())
    };
    assert_eq!(
        in_workspace("A"),
        Some(true),
        "the newly applied branch joins the workspace"
    );
    assert_eq!(
        in_workspace("B"),
        Some(false),
        "previously visible stack B should be left behind"
    );
    assert_eq!(
        in_workspace("C"),
        Some(true),
        "the checked-out branch joins the workspace"
    );
    Ok(())
}

#[test]
fn apply_already_applied_branch_from_adhoc_checkout_excludes_other_applied_stacks()
-> anyhow::Result<()> {
    let (_tmp, _graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-three-file-stacks",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 3, "C", StackState::InWorkspace, &[]);
            },
        )?;
    assert_worktree_files(&repo, &["A", "B", "C"], &[]);

    let a_tip_before_apply = id_by_rev(&repo, "A");
    git(&repo).args(["checkout", "A"]).run();
    assert_worktree_files(&repo, &["A"], &["B", "C"]);

    let ws = Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?
        .into_workspace()?;
    let out =
        but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options())?;

    assert_eq!(out.status, OutcomeStatus::Applied);
    assert_eq!(
        id_by_rev(&repo, "A"),
        a_tip_before_apply,
        "applying from A must not move A to the workspace commit"
    );
    assert_eq!(
        repo.head_name()?.as_ref().map(|rn| rn.as_bstr()),
        Some(r("refs/heads/gitbutler/workspace").as_bstr()),
        "applying from an ad-hoc checkout should switch back to the managed workspace"
    );
    assert_worktree_files(&repo, &["A", "C"], &["B"]);

    let ws_md = meta.workspace(WORKSPACE_REF_NAME.try_into().unwrap())?;
    let in_workspace = |name: &str| {
        ws_md
            .stacks
            .iter()
            .find(|s| s.name().is_some_and(|n| n.shorten() == name))
            .map(|s| s.is_in_workspace())
    };
    assert_eq!(in_workspace("A"), Some(true));
    assert_eq!(in_workspace("B"), Some(false));
    assert_eq!(in_workspace("C"), Some(true));
    Ok(())
}

#[test]
fn new_workspace_exists_elsewhere_and_to_be_applied_branch_exists_there() -> anyhow::Result<()> {
    let (_tmp, ws_graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-no-ws-commit-one-stack-one-branch",
            |_meta| {},
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );
    // The default workspace, it's empty as target is set to `main`.
    snapbox::assert_data_eq!(
        graph_workspace(&ws_graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );

    // Pretend "B" is checked out (it's at the right state independently of that)
    let (b_id, b_ref) = id_at(&repo, "B");
    let graph = but_graph::Graph::from_repo(
        &repo,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Overlay::default().with_entrypoint(b_id.detach(), Some(b_ref)),
    )?;
    let ws = graph.into_workspace()?;
    // Note how the existing `gitbutler/workspace` disappears, which is expected here.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );

    // Put "A" into the workspace, hence we want "A" and "B" in it.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );

    let ws = out.workspace;
    // This apply brings both branches into the existing workspace, and it's where HEAD points to.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:1:A on e5d0542 {41}
    └── 📙:1:A

"#]]
    );

    // HEAD must now point to the workspace (that already existed)
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    Ok(())
}

mod unapply_checked_out {
    use but_testsupport::gix_testtools::tempfile::TempDir;
    use snapbox::prelude::*;

    use super::*;

    type Scenario = (
        TempDir,
        gix::Repository,
        but_meta::VirtualBranchesTomlMetadata,
        but_graph::Workspace,
    );

    fn virtual_stack_tip_checked_out() -> anyhow::Result<Scenario> {
        let (tmp, _graph, repo, meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "ws-ref-no-ws-commit-one-stack-one-branch",
                |meta| {
                    add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                    add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
                },
            )?;
        let initial_graph = visualize_commit_graph_all(&repo)?;

        snapbox::assert_data_eq!(
            initial_graph,
            snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
        );

        git(&repo).args(["checkout", "B"]).run();
        let ws = Graph::from_repo(
            &repo,
            &meta,
            but_core::ref_metadata::ProjectMeta::default(),
            Overlay::default(),
        )?
        .into_workspace()?;

        // B is checked out directly, and its stack is virtual
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace <> ✓! on e5d0542
├── ≡📙:1:A on e5d0542 {1}
│   └── 📙:1:A
└── ≡👉📙:2:B[🌳] on e5d0542 {2}
    └── 👉📙:2:B[🌳]

"#]]
        );

        Ok((tmp, repo, meta, ws))
    }

    fn real_stack_tip_checked_out() -> anyhow::Result<Scenario> {
        let (tmp, graph, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "detached-with-multiple-branches",
                |_meta| {},
            )?;
        let initial_graph = visualize_commit_graph_all(&repo)?;

        // the initial checkout is detached, so the workspace has no target ref to fall back to
        snapbox::assert_data_eq!(
            initial_graph,
            snapbox::str![[r#"
* 49d4b34 (A) A1
| * f57c528 (B) B1
|/
| * aaa195b (HEAD, C) C1
|/
* 3183e43 (main) M1

"#]]
        );

        let mut ws = graph.into_workspace()?;
        for branch_to_apply in ["C", "B"] {
            let out = but_workspace::branch::apply(
                Category::LocalBranch
                    .to_full_name(branch_to_apply)?
                    .as_ref(),
                ws,
                &repo,
                &mut meta,
                apply_options(),
            )?;
            ws = out.workspace;
        }

        // C and B are real stacks in the managed workspace
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:5:C on 3183e43 {43}
│   └── 📙:5:C
│       └── ·aaa195b (🏘️)
└── ≡📙:6:B on 3183e43 {42}
    └── 📙:6:B
        └── ·f57c528 (🏘️)

"#]]
        );

        git(&repo).args(["checkout", "B"]).run();
        let ws = Graph::from_repo(
            &repo,
            &meta,
            but_core::ref_metadata::ProjectMeta::default(),
            Overlay::default(),
        )?
        .into_workspace()?;

        // B is checked out directly, and its stack has a real commit
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace <> ✓! on 3183e43
├── ≡📙:4:C on 3183e43 {43}
│   └── 📙:4:C
│       └── ·aaa195b (🏘️)
└── ≡👉📙:5:B[🌳] on 3183e43 {42}
    └── 👉📙:5:B[🌳]
        └── ·f57c528 (🏘️)

"#]]
        );

        Ok((tmp, repo, meta, ws))
    }

    #[test]
    fn virtual_stack_tip() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = virtual_stack_tip_checked_out()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/B"),
            &ws,
            &repo,
            &mut meta,
            unapply_options(),
        )?;
        // the workspace is checked out as the current brnach was unapplied
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/gitbutler/workspace",
    ),
}

"#]]
        );

        // the returned workspace is projected from the managed workspace ref
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:1:A on e5d0542 {1}
    └── 📙:1:A

"#]]
        );
        // HEAD switches back to the managed workspace when the checked-out branch is unapplied
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
        );
        Ok(())
    }

    #[test]
    fn virtual_stack_tip_switches_from_workspace_to_last_stack_when_allowed() -> anyhow::Result<()>
    {
        let (_tmp, repo, mut meta, ws) = virtual_stack_tip_checked_out()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/B"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
        )?;
        // the checked-out stack is unapplied, then the remaining stack is checked out directly
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/A",
    ),
}

"#]]
        );
        // the returned workspace is projected from the remaining stack
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
⌂:1:A <> ✓!
└── ≡👉📙:1:A[🌳] {1}
    └── 👉📙:1:A[🌳]
        └── ·e5d0542 (🏘️) ►main

"#]]
        );
        // HEAD briefly returned to the workspace ref internally, then dissolved it and checked out A (previously virtual)
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* e5d0542 (HEAD -> A, main, B) A

"#]]
        );
        Ok(())
    }

    #[test]
    fn virtual_stack_tip_with_indirect_entrypoint() -> anyhow::Result<()> {
        let (_tmp, _graph, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "ws-ref-no-ws-commit-one-stack-one-branch",
                |meta| {
                    add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &["A"]);
                },
            )?;

        git(&repo).args(["checkout", "A"]).run();
        let ws = Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?
            .into_workspace()?;
        // A is checked out as a lower segment of the B stack
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace <> ✓! on e5d0542
└── ≡📙:2:B on e5d0542 {2}
    ├── 📙:2:B
    └── 👉📙:1:A[🌳]

"#]]
        );

        let out = but_workspace::branch::unapply(
            r("refs/heads/B"),
            &ws,
            &repo,
            &mut meta,
            unapply_options(),
        )?;
        // the workspace is checked out as the current branch's stack was unapplied
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/gitbutler/workspace",
    ),
}

"#]]
        );
        // the returned workspace is projected from the managed workspace ref
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
        );
        // HEAD switches back to the managed workspace when the checked-out stack is unapplied
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
        );
        Ok(())
    }

    #[test]
    fn virtual_unrelated_stack_tip() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = virtual_stack_tip_checked_out()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options(),
        )?;
        // no change in what's checked out
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
        );

        // the returned workspace preserves the unrelated checked-out entrypoint
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace <> ✓! on e5d0542
└── ≡👉📙:1:B[🌳] on e5d0542 {2}
    └── 👉📙:1:B[🌳]

"#]]
        );
        // HEAD remains on B while the managed workspace also contains B
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* e5d0542 (HEAD -> B, main, gitbutler/workspace, A) A

"#]]
        );
        Ok(())
    }

    #[test]
    fn real_stack_tip() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = real_stack_tip_checked_out()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/B"),
            &ws,
            &repo,
            &mut meta,
            unapply_options(),
        )?;
        // the workspace ref is checked out
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/gitbutler/workspace",
    ),
}

"#]]
        );

        // the returned workspace is projected from the managed workspace ref
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on 3183e43
└── ≡📙:2:C on 3183e43 {43}
    ├── 📙:2:C
    │   └── ·aaa195b (🏘️) ►gitbutler/workspace[🌳]
    └── :4:main

"#]]
        );
        // HEAD switches back to the managed workspace when the checked-out branch is unapplied, without forcing a workspace commit (this is fine, as this is outside of what legacy can do anyway)
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 49d4b34 (A) A1
| * f57c528 (B) B1
|/
| * aaa195b (HEAD -> gitbutler/workspace, C) C1
|/
* 3183e43 (main) M1

"#]]
        );
        Ok(())
    }

    #[test]
    fn real_unrelated_stack_tip() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, ws) = real_stack_tip_checked_out()?;

        let out = but_workspace::branch::unapply(
            r("refs/heads/C"),
            &ws,
            &repo,
            &mut meta,
            unapply_options(),
        )?;
        // the checked out branch isn't changed
        snapbox::assert_data_eq!(
            out.to_debug(),
            snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
        );

        // the returned workspace preserves the unrelated checked-out entrypoint
        snapbox::assert_data_eq!(
            graph_workspace(&out.workspace).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace <> ✓! on 3183e43
└── ≡👉📙:2:B[🌳] on 3183e43 {42}
    ├── 👉📙:2:B[🌳]
    │   └── ·f57c528 (🏘️) ►gitbutler/workspace
    └── :4:main

"#]]
        );
        // HEAD remains on B while the managed workspace also contains B
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 49d4b34 (A) A1
| * f57c528 (HEAD -> B, gitbutler/workspace) B1
|/
| * aaa195b (C) C1
|/
* 3183e43 (main) M1

"#]]
        );
        Ok(())
    }
}

#[test]
fn apply_multiple_without_target_or_metadata_or_base() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("one-fork", |meta| {
            meta.data_mut().default_target = None;
        })?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bf53300 (A) add A
| * b1540e5 (HEAD -> main) M
|/
| * 0e391b2 (origin/B) add B
|/
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
    );

    let mut project_meta = graph.project_meta().clone();
    project_meta.target_commit_id = None;
    let graph = Graph::from_repo(&repo, &meta, project_meta, Overlay::default())?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:2:main <> ✓!
└── ≡👉:2:main[🌳] <> origin/main →:3:⇡1 {1}
    ├── 👉:2:main[🌳] <> origin/main →:3:⇡1
    │   └── ·b1540e5 (🏘️)
    └── :3:origin/main →:2:
        └── ❄e31e6ca (🏘️)

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: true,
    applied_branches: "[refs/heads/main, refs/heads/A]",
}

"#]]
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓! on e31e6ca
├── ≡📙:4:main <> origin/main →:7:⇡1 {1a5}
│   └── 📙:4:main <> origin/main →:7:⇡1
│       ├── ·b1540e5 (🏘️)
│       └── ❄e31e6ca (🏘️) ►origin/main
└── ≡📙:5:A {41}
    └── 📙:5:A
        ├── ·bf53300 (🏘️)
        └── ·e31e6ca (🏘️) ►origin/main

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   d87b903 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\
| * bf53300 (A) add A
* | b1540e5 (main) M
|/
| * 0e391b2 (origin/B) add B
|/
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
        .raw()
    );

    let branch_b_rt = r("refs/remotes/origin/B");
    let out = but_workspace::branch::apply(branch_b_rt, ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓! on e31e6ca
├── ≡📙:8:main <> origin/main →:10:⇡1 {1a5}
│   └── 📙:8:main <> origin/main →:10:⇡1
│       ├── ·b1540e5 (🏘️)
│       └── ❄e31e6ca (🏘️) ►origin/main
├── ≡📙:9:A {41}
│   └── 📙:9:A
│       ├── ·bf53300 (🏘️)
│       └── ·e31e6ca (🏘️) ►origin/main
└── ≡📙:5:B <> origin/B →:6: {42}
    └── 📙:5:B <> origin/B →:6:
        ├── ❄0e391b2 (🏘️) ►origin/B
        └── ❄e31e6ca (🏘️) ►origin/main

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   7bcf528 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \
| | * 0e391b2 (origin/B, B) add B
| * | bf53300 (A) add A
| |/
* / b1540e5 (main) M
|/
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
        .raw()
    );

    let out =
        but_workspace::branch::unapply(branch_b_rt, &ws, &repo, &mut meta, unapply_options())?;
    // remote tracking branches can't be unapplied, as they aren't applied in the first place
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    checked_out: None,
}

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    // the local tracking branch, howeer, will be unapplied
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓! on e31e6ca
├── ≡📙:4:main <> origin/main →:7:⇡1 {1a5}
│   └── 📙:4:main <> origin/main →:7:⇡1
│       ├── ·b1540e5 (🏘️)
│       └── ❄e31e6ca (🏘️) ►origin/main
└── ≡📙:5:A {41}
    └── 📙:5:A
        ├── ·bf53300 (🏘️)
        └── ·e31e6ca (🏘️) ►origin/main

"#]]
    );
    assert!(
        !repo.workdir_path("B").expect("non-bare").exists(),
        "unapplying B updates the checked-out workspace tree"
    );
    assert_eq!(
        std::fs::read_to_string(repo.workdir_path("A").expect("non-bare"))?,
        "A\n"
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    // A is removed and main is checked out
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // By default, we keep the workspace, and `main` is in there as a target is missing
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓! on b1540e5
└── ≡📙:3:main <> origin/main →:4:⇡1 {1a5}
    ├── 📙:3:main <> origin/main →:4:⇡1
    │   └── ·b1540e5 (🏘️) ►gitbutler/workspace[🌳]
    └── :4:origin/main →:3:
        └── ❄e31e6ca (🏘️)

"#]]
    );
    assert!(
        !repo.workdir_path("A").expect("non-bare").exists(),
        "unapplying A updates the checked-out workspace tree"
    );
    assert_eq!(
        std::fs::read_to_string(repo.workdir_path("init").expect("non-bare"))?,
        "init\n"
    );
    Ok(())
}

#[test]
fn unapply_dirty_worktree_abort_keeps_refs_and_metadata() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("one-fork", |meta| {
            meta.data_mut().default_target = None;
        })?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bf53300 (A) add A
| * b1540e5 (HEAD -> main) M
|/
| * 0e391b2 (origin/B) add B
|/
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
    );
    let mut project_meta = graph.project_meta().clone();
    project_meta.target_commit_id = None;
    let graph = Graph::from_repo(&repo, &meta, project_meta, Overlay::default())?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:2:main <> ✓!
└── ≡👉:2:main[🌳] <> origin/main →:3:⇡1 {1}
    ├── 👉:2:main[🌳] <> origin/main →:3:⇡1
    │   └── ·b1540e5 (🏘️)
    └── :3:origin/main →:2:
        └── ❄e31e6ca (🏘️)

"#]]
    );
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    let ws = out.workspace;
    let out = but_workspace::branch::apply(
        r("refs/remotes/origin/B"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓! on e31e6ca
├── ≡📙:8:main <> origin/main →:10:⇡1 {1a5}
│   └── 📙:8:main <> origin/main →:10:⇡1
│       ├── ·b1540e5 (🏘️)
│       └── ❄e31e6ca (🏘️) ►origin/main
├── ≡📙:9:A {41}
│   └── 📙:9:A
│       ├── ·bf53300 (🏘️)
│       └── ·e31e6ca (🏘️) ►origin/main
└── ≡📙:5:B <> origin/B →:6: {42}
    └── 📙:5:B <> origin/B →:6:
        ├── ❄0e391b2 (🏘️) ►origin/B
        └── ❄e31e6ca (🏘️) ►origin/main

"#]]
    );

    let ws_before = graph_workspace(&ws).to_string();
    let refs_before = visualize_commit_graph_all(&repo)?;
    let metadata_before = sanitize_uuids_and_timestamps(format!(
        "{:#?}",
        ws.metadata
            .as_ref()
            .expect("managed workspace has metadata")
    ));

    std::fs::write(repo.workdir_path("B").expect("non-bare"), "local edit\n")?;
    let worktree_before =
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string();
    let err =
        but_workspace::branch::unapply(r("refs/heads/B"), &ws, &repo, &mut meta, unapply_options())
            .unwrap_err();
    snapbox::assert_data_eq!(
        err.to_debug(),
        snapbox::str![[r#"
Context {
    code: PreconditionFailed,
    message: Some(
        "Uncommitted files would be overwritten by checkout: \"B\"",
    ),
}

"#]]
        .raw()
    );

    assert_eq!(
        visualize_disk_tree_with_hashes_skip_dot_git(repo.workdir().expect("worktree dir"))?
            .to_string(),
        worktree_before,
        "the locally modified file wasn't changed"
    );
    assert_eq!(
        visualize_commit_graph_all(&repo)?,
        refs_before,
        "refs must not move when dirty worktree checkout aborts"
    );
    let ws_after = but_graph::Graph::from_repo(
        &repo,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Overlay::default(),
    )?
    .into_workspace()?;
    assert_eq!(graph_workspace(&ws_after).to_string(), ws_before);
    assert_eq!(
        sanitize_uuids_and_timestamps(format!(
            "{:#?}",
            ws_after
                .metadata
                .as_ref()
                .expect("managed workspace has metadata")
        )),
        metadata_before
    );
    Ok(())
}

#[test]
fn apply_repairs_stale_outside_metadata_for_reachable_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-ref-ws-commit-one-stack", |meta| {
            add_stack_with_segments(meta, 1, "B", StackState::InWorkspace, &["A"]);
        })?;
    let ws = graph.into_workspace()?;
    assert!(
        ws.stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .any(|segment| segment.ref_name() == Some(r("refs/heads/B"))),
        "fixture must start with B visible in the cached workspace graph"
    );

    let mut ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    for stack in &mut ws_md.stacks {
        stack.workspacecommit_relation = Outside;
    }
    meta.set_workspace(&ws_md)?;

    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "reachable-but-unapplied metadata must be repaired instead of reported as a no-op"
    );
    assert_eq!(out.applied_branches, [r("refs/heads/B").to_owned()]);

    let ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    assert!(
        ws_md
            .find_branch(r("refs/heads/B"), StackKind::Applied)
            .is_some(),
        "apply should put the reachable branch back into the applied metadata set"
    );

    Ok(())
}

#[test]
fn apply_multiple_segments_of_stack_in_order_merge_if_needed() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "single-stack-two-segments",
            |_meta| {},
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f1889e7 (A2) add A2
* 7de99e1 (A1) add A1
| * 53ad0c2 (unrelated) add U1
|/
* 3183e43 (HEAD -> main, origin/main) M1

"#]]
    );

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:1:main <> ✓refs/remotes/origin/main on 3183e43
└── ≡👉:1:main[🌳] <> origin/main →:2: on 3183e43 {1}
    └── 👉:1:main[🌳] <> origin/main →:2:

"#]]
    );

    assert_eq!(
        apply_options().workspace_merge,
        WorkspaceMerge::MergeIfNeeded
    );

    // Add another stack to be sure we correctly handle the removal of existing stacks later (i.e. don't get the index wrong)
    let out = but_workspace::branch::apply(
        r("refs/heads/unrelated"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: true,
    applied_branches: "[refs/heads/unrelated]",
}

"#]]
    );
    // TODO: should this not avoid creating a workspace commit? Yes.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f1889e7 (A2) add A2
* 7de99e1 (A1) add A1
| * 6848743 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 53ad0c2 (unrelated) add U1
|/
* 3183e43 (origin/main, main) M1

"#]]
    );

    let ws = out.workspace;

    let out =
        but_workspace::branch::apply(r("refs/heads/A1"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/A1]",
}

"#]]
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:7:unrelated on 3183e43 {3c4}
│   └── 📙:7:unrelated
│       └── ·53ad0c2 (🏘️)
└── ≡📙:8:A1 on 3183e43 {72}
    └── 📙:8:A1
        └── ·7de99e1 (🏘️)

"#]]
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/A2"),
        ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            // TODO: remove this, use default options when they are the default.
            on_workspace_conflict: OnWorkspaceMergeConflict::MaterializeAndReportConflictingStacks,
            ..apply_options()
        },
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/A2]",
}

"#]]
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:7:unrelated on 3183e43 {3c4}
│   └── 📙:7:unrelated
│       └── ·53ad0c2 (🏘️)
└── ≡📙:9:A2 on 3183e43 {73}
    ├── 📙:9:A2
    │   └── ·f1889e7 (🏘️)
    └── 📙:10:A1
        └── ·7de99e1 (🏘️)

"#]]
    );

    // The metadata is in sync, and A1 is outside the workspace.
    snapbox::assert_data_eq!(
        ws.metadata.to_debug(),
        snapbox::str![[r#"
Some(
    Workspace {
        ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
        stacks: [
            WorkspaceStack {
                id: 00000000-0000-0000-0000-0000000003c4,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/unrelated",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Merged,
            },
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000072,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/A1",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Outside,
            },
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000073,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/A2",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Merged,
            },
        ],
        target_ref: "refs/remotes/origin/main",
        target_commit_id: Sha1(3183e43ff482a2c4c8ff531d595453b64f58d90b),
        push_remote: None,
    },
)

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A2"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // A2 is removed, along with A1
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:5:unrelated on 3183e43 {3c4}
    └── 📙:5:unrelated
        └── ·53ad0c2 (🏘️) ►gitbutler/workspace[🌳]

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A1"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    // this is a no-op, A1 was removed with A2 prior
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // nothing changed, this was a no-op
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:5:unrelated on 3183e43 {3c4}
    └── 📙:5:unrelated
        └── ·53ad0c2 (🏘️) ►gitbutler/workspace[🌳]

"#]]
    );

    let opts = unapply_options();
    assert!(
        matches!(
            opts.workspace_disposition,
            WorkspaceDisposition::KeepWorkspaceReference
        ),
        "this should make the workspace commit disappear, but keep the workspace reference"
    );
    let out =
        but_workspace::branch::unapply(r("refs/heads/unrelated"), &ws, &repo, &mut meta, opts)?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43

"#]]
    );

    //
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f1889e7 (A2) add A2
* 7de99e1 (A1) add A1
| * 53ad0c2 (unrelated) add U1
|/
* 3183e43 (HEAD -> gitbutler/workspace, origin/main, main) M1

"#]]
    );
    Ok(())
}

#[test]
fn unapply_existing_branch_outside_detached_ad_hoc_workspace_is_noop() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "detached-with-multiple-branches",
            |_meta| {},
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 49d4b34 (A) A1
| * f57c528 (B) B1
|/
| * aaa195b (HEAD, C) C1
|/
* 3183e43 (main) M1

"#]]
    );
    let ws = graph
        .into_workspace()
        .expect("detached graph is a workspace");
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:1:DETACHED <> ✓! on 3183e43
└── ≡👉:1:anon on 3183e43 {1}
    ├── 👉:1:anon
    │   └── ·aaa195b (🏘️) ►C
    └── :2:main

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    // it's a noop, the workspace didn't change, the branch was never applied
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    checked_out: None,
}

"#]]
    );
    Ok(())
}

#[test]
fn unapply_branch_from_detached_ad_hoc_workspace_is_an_error() -> anyhow::Result<()> {
    let (_tmp, _, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "single-stack-two-segments",
            |_meta| {},
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f1889e7 (A2) add A2
* 7de99e1 (A1) add A1
| * 53ad0c2 (unrelated) add U1
|/
* 3183e43 (HEAD -> main, origin/main) M1

"#]]
    );

    let a2_id = repo.rev_parse_single("A2")?.detach();
    let ws = Graph::from_repo(
        &repo,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Overlay::default().with_entrypoint(a2_id, None),
    )?
    .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:1:DETACHED <> ✓!
└── ≡👉:1:anon {1}
    ├── 👉:1:anon
    │   └── ·f1889e7 (🏘️) ►A2
    ├── :6:A1
    │   └── ·7de99e1 (🏘️)
    └── :3:main[🌳] <> origin/main →:4:
        └── ❄3183e43 (🏘️) ►origin/main

"#]]
    );

    let unapply_this = r("refs/heads/A1");
    let err =
        but_workspace::branch::unapply(unapply_this, &ws, &repo, &mut meta, unapply_options())
            .expect_err("detached ad-hoc workspaces cannot unapply their contained branch");
    assert_eq!(
        err.to_string(),
        "Cannot unapply a branch from an ad-hoc detached workspace"
    );
    Ok(())
}

#[test]
fn detached_head_journey() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "detached-with-multiple-branches",
            |_meta| {},
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 49d4b34 (A) A1
| * f57c528 (B) B1
|/
| * aaa195b (HEAD, C) C1
|/
* 3183e43 (main) M1

"#]]
    );
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:1:DETACHED <> ✓! on 3183e43
└── ≡👉:1:anon on 3183e43 {1}
    ├── 👉:1:anon
    │   └── ·aaa195b (🏘️) ►C
    └── :2:main

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options())?;

    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: true,
    applied_branches: "[refs/heads/C]",
}

"#]]
    );

    let ws = out.workspace;
    // the actual HEAD is ignored, and we only see C (instead of C + HEAD-ref)
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:4:gitbutler/workspace[🌳] <> ✓! on 3183e43
└── ≡📙:3:C on 3183e43 {43}
    ├── 📙:3:C
    │   └── ·aaa195b (🏘️) ►gitbutler/workspace[🌳]
    └── :2:main

"#]]
    );
    // A new workspace reference was created, and checked out, without enforcing a workspace commit
    // as there is no need.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 49d4b34 (A) A1
| * f57c528 (B) B1
|/
| * aaa195b (HEAD -> gitbutler/workspace, C) C1
|/
* 3183e43 (main) M1

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;

    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 49d4b34 (A) A1
| *   fdec130 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| |\
| | * f57c528 (B) B1
| |/
|/|
| * aaa195b (C) C1
|/
* 3183e43 (main) M1

"#]]
        .raw()
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:5:C on 3183e43 {43}
│   └── 📙:5:C
│       └── ·aaa195b (🏘️)
└── ≡📙:6:B on 3183e43 {42}
    └── 📙:6:B
        └── ·f57c528 (🏘️)

"#]]
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            // Make 'A' appear at the front.
            order: Some(0),
            ..apply_options()
        },
    )?;

    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:6:A on 3183e43 {41}
│   └── 📙:6:A
│       └── ·49d4b34 (🏘️)
├── ≡📙:8:C on 3183e43 {43}
│   └── 📙:8:C
│       └── ·aaa195b (🏘️)
└── ≡📙:9:B on 3183e43 {42}
    └── 📙:9:B
        └── ·f57c528 (🏘️)

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   951ff29 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \
| | * f57c528 (B) B1
| * | aaa195b (C) C1
| |/
* / 49d4b34 (A) A1
|/
* 3183e43 (main) M1

"#]]
        .raw()
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // A was removed
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:5:C on 3183e43 {43}
│   └── 📙:5:C
│       └── ·aaa195b (🏘️)
└── ≡📙:6:B on 3183e43 {42}
    └── 📙:6:B
        └── ·f57c528 (🏘️)

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // B was removed
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:4:gitbutler/workspace[🌳] <> ✓! on 3183e43
└── ≡📙:3:C on 3183e43 {43}
    ├── 📙:3:C
    │   └── ·aaa195b (🏘️) ►gitbutler/workspace[🌳]
    └── :2:main

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/C"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )
    .expect("C can be removed");
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // the workspace reference remains checked out and empty because no non-stack fallback exists
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓! on 3183e43

"#]]
    );
    Ok(())
}

#[test]
fn unapply_workspace_ref_without_target_checks_out_named_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "detached-with-multiple-branches",
            |_meta| {},
        )?;
    // the initial checkout is detached, so the workspace has no target ref to fall back to
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 49d4b34 (A) A1
| * f57c528 (B) B1
|/
| * aaa195b (HEAD, C) C1
|/
* 3183e43 (main) M1

"#]]
    );
    let mut ws = graph.into_workspace()?;

    for branch_to_apply in ["C", "B"] {
        let out = but_workspace::branch::apply(
            Category::LocalBranch
                .to_full_name(branch_to_apply)?
                .as_ref(),
            ws,
            &repo,
            &mut meta,
            apply_options(),
        )?;
        ws = out.workspace;
    }

    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            order: Some(0),
            ..apply_options()
        },
    )?;
    let ws = out.workspace;
    // the workspace has named stacks but no target ref
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:6:A on 3183e43 {41}
│   └── 📙:6:A
│       └── ·49d4b34 (🏘️)
├── ≡📙:8:C on 3183e43 {43}
│   └── 📙:8:C
│       └── ·aaa195b (🏘️)
└── ≡📙:9:B on 3183e43 {42}
    └── 📙:9:B
        └── ·f57c528 (🏘️)

"#]]
    );

    // Simulate project metadata previously ported to repo-local Git config. Unapplying the
    // workspace reference removes the workspace metadata and must clear this copy as well.
    ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(id_by_rev(&repo, "main").detach()),
        push_remote: Some("origin".into()),
    }
    .persist_to_local_config(&repo)?;
    assert!(
        ref_metadata::ProjectMeta::is_ported_repo(&repo)?,
        "the ported marker was just written to repo-local config"
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/gitbutler/workspace"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )
    .expect("workspace ref can be unapplied by falling back to a named stack");
    // without a target ref, unapplying the workspace ref checks out the named stack with the lowest generation
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/A",
    ),
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // the projection shows the checked-out named stack
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:2:A <> ✓!
└── ≡👉:2:A[🌳] {1}
    ├── 👉:2:A[🌳]
    │   └── ·49d4b34 (🏘️)
    └── :3:main
        └── ·3183e43 (🏘️)

"#]]
    );
    // the managed workspace ref is deleted and HEAD points to the fallback stack
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 49d4b34 (HEAD -> A) A1
| * f57c528 (B) B1
|/
| * aaa195b (C) C1
|/
* 3183e43 (main) M1

"#]]
    );

    let config = but_core::git_config::open_repo_local_config_for_reading(&repo)?;
    assert_eq!(
        ref_metadata::ProjectMeta::try_from_config(&config)?,
        ref_metadata::ProjectMeta::default(),
        "unapplying the workspace reference clears the ported project metadata copy"
    );
    assert!(
        !ref_metadata::ProjectMeta::is_ported_repo(&repo)?,
        "the ported marker is removed along with the other gitbutler.project.* keys"
    );
    Ok(())
}

#[test]
fn unapply_workspace_ref_refuses_conflicted_named_stack_checkout() -> anyhow::Result<()> {
    let (_tmp, _, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("with-conflict", |meta| {
            meta.data_mut().default_target = None;
        })?;
    // the fixture starts on a conflicted main commit
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 8450331 (HEAD -> main, tag: conflicted) GitButler WIP Commit
* a047f81 (tag: normal) init

"#]]
    );

    git(&repo)
        .args(["branch", "tip-conflicted", "conflicted"])
        .run();
    git(&repo).args(["reset", "--hard", "normal"]).run();

    let ws = Graph::from_repo(&repo, &meta, project_meta(&meta), Overlay::default())?
        .into_workspace()?;
    let out = but_workspace::branch::apply(
        r("refs/heads/tip-conflicted"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    let ws = out.workspace;
    // the target-less workspace can contain a conflicted named stack
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓! on a047f81
├── ≡📙:3:tip-conflicted {595}
│   └── 📙:3:tip-conflicted
│       ├── ·8450331 (🏘️)
│       └── ·a047f81 (🏘️) ►main
└── ≡📙:4:main {1a5}
    └── 📙:4:main
        └── ·a047f81 (🏘️)

"#]]
    );

    let err = but_workspace::branch::unapply(
        r("refs/heads/gitbutler/workspace"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )
    .expect_err("workspace ref unapply must not check out conflicted stack tips");
    assert_eq!(
        err.to_string(),
        "Cannot unapply workspace reference by checking out conflicted commit at 'tip-conflicted'"
    );
    // the failed unapply leaves HEAD and the workspace ref unchanged
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   ce70607 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
| * 8450331 (tag: conflicted, tip-conflicted) GitButler WIP Commit
|/
* a047f81 (tag: normal, main) init

"#]]
    );
    Ok(())
}

#[test]
fn apply_two_ambiguous_stacks_with_target_with_dependent_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "no-ws-ref-stack-and-dependent-branch",
            |meta| {
                add_stack_with_segments(meta, 1, "C", StackState::Inactive, &["E"]);
                add_stack_with_segments(meta, 2, "B", StackState::Inactive, &["D"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:1:main <> ✓refs/remotes/origin/main on 85efbe4
└── ≡👉:1:main[🌳] <> origin/main →:2: on 85efbe4 {1}
    └── 👉:1:main[🌳] <> origin/main →:2:

"#]]
    );

    // Apply the dependent branch, to bring in only the dependent branch
    let out =
        but_workspace::branch::apply(r("refs/heads/E"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: true,
    applied_branches: "[refs/heads/E]",
}

"#]]
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:3:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:5:E on 85efbe4 {1}
    └── 📙:5:E
        └── ·7076dee (🏘️) ►D

"#]]
    );

    // Apply the former tip of the stack, to create a new stack. Note how it won't double-list the
    // other stack.
    let out =
        but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options())?;
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:9:C on 85efbe4 {43}
    └── 📙:9:C
        ├── ·f084d61 (🏘️) ►A, ►B
        └── ·7076dee (🏘️) ►D, ►E

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 759ef3d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    // Adding `B` as tip of an unapplied stack brings in the whole stack.
    // BUT: Currently it overrides the previous stack C, which points to the same commit, and avoids any merge!
    // Accepting this behaviour for now as it's quite rare to have such ambiguity, even though I'd love if one day
    // for this to just work as people might intuitively want, even if that means the same commit is used multiple times.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:10:B on 85efbe4 {2}
    ├── 📙:10:B
    │   └── ·f084d61 (🏘️) ►A, ►C
    └── 📙:4:D
        └── ·7076dee (🏘️) ►E

"#]]
    );

    // Applying C again… works, but it's creating a dependent stack.
    // This is what happens because we notice that C can't be applied as independent stack due to the graph algorithm,
    // and then it tries it a dependent stack, which should always work.
    let out =
        but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options())
            .unwrap();
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:10:B on 85efbe4 {2}
│   ├── 📙:10:B
│   │   └── ·f084d61 (🏘️) ►A, ►C
│   └── 📙:5:D
│       └── ·7076dee (🏘️) ►E
└── ≡📙:11:C on 7076dee {43}
    └── 📙:11:C
        └── ·f084d61 (🏘️) ►A, ►B

"#]]
    );

    Ok(())
}

#[test]
fn apply_two_ambiguous_stacks_with_target() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "no-ws-ref-stack-and-dependent-branch",
            |_meta| {},
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:1:main <> ✓refs/remotes/origin/main on 85efbe4
└── ≡👉:1:main[🌳] <> origin/main →:2: on 85efbe4 {1}
    └── 👉:1:main[🌳] <> origin/main →:2:

"#]]
    );

    // Apply `A` first.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: true,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:7:A on 85efbe4 {41}
    └── 📙:7:A
        ├── ·f084d61 (🏘️) ►B, ►C
        └── ·7076dee (🏘️) ►D, ►E

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 773e030 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    // Apply `B` - the only sane way is to make it its own stack, but allow it to diverge.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:7:A on 85efbe4 {41}
│   └── 📙:7:A
│       ├── ·f084d61 (🏘️) ►B, ►C
│       └── ·7076dee (🏘️) ►D, ►E
└── ≡📙:8:B on 85efbe4 {42}
    └── 📙:8:B
        ├── ·f084d61 (🏘️) ►A, ►C
        └── ·7076dee (🏘️) ►D, ►E

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 4a73dbe (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    // The remaining same-tip refs each retain explicit workspace ownership.
    let out =
        but_workspace::branch::apply(r("refs/heads/C"), ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    // Applying C succeeds and records a third independently owned stack.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/C]",
}

"#]]
    );

    let ws = out.workspace;
    // A, B, and C remain distinct stacks even though they point to the same commit.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:7:A on 85efbe4 {41}
│   └── 📙:7:A
│       ├── ·f084d61 (🏘️) ►B, ►C
│       └── ·7076dee (🏘️) ►D, ►E
├── ≡📙:8:B on 85efbe4 {42}
│   └── 📙:8:B
│       ├── ·f084d61 (🏘️) ►A, ►C
│       └── ·7076dee (🏘️) ►D, ►E
└── ≡📙:9:C on 85efbe4 {43}
    └── 📙:9:C
        ├── ·f084d61 (🏘️) ►A, ►B
        └── ·7076dee (🏘️) ►D, ►E

"#]]
    );
    // The workspace commit records each explicit stack parent.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*[31m-[m[31m.[m 6a3fcf3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/ /
|/
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/D"), ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    // Applying D succeeds and gives the lower same-tip ref explicit metadata ownership.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/D]",
}

"#]]
    );

    let ws = out.workspace;
    // Until D becomes a workspace root, it is shown below each upper same-tip stack.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:9:A on 85efbe4 {41}
│   ├── 📙:9:A
│   │   └── ·f084d61 (🏘️) ►B, ►C
│   └── 📙:5:D
│       └── ·7076dee (🏘️) ►E
├── ≡📙:10:B on 85efbe4 {42}
│   ├── 📙:10:B
│   │   └── ·f084d61 (🏘️) ►A, ►C
│   └── 📙:5:D
│       └── ·7076dee (🏘️) ►E
└── ≡📙:11:C on 85efbe4 {43}
    ├── 📙:11:C
    │   └── ·f084d61 (🏘️) ►A, ►B
    └── 📙:5:D
        └── ·7076dee (🏘️) ►E

"#]]
    );
    // The workspace commit still records the three upper roots.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*[31m-[m[31m.[m 6a3fcf3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/ /
|/
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/E"), ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    // Applying E makes D's metadata-owned stack a workspace root; E remains a decoration at its tip.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/E]",
}

"#]]
    );

    let ws = out.workspace;
    // Every metadata owner is now projected once: A, B, C, and D have distinct stack IDs.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:9:A on 85efbe4 {41}
│   └── 📙:9:A
│       ├── ·f084d61 (🏘️) ►B, ►C
│       └── ·7076dee (🏘️) ►D, ►E
├── ≡📙:10:B on 85efbe4 {42}
│   └── 📙:10:B
│       ├── ·f084d61 (🏘️) ►A, ►C
│       └── ·7076dee (🏘️) ►D, ►E
├── ≡📙:11:C on 85efbe4 {43}
│   └── 📙:11:C
│       ├── ·f084d61 (🏘️) ►A, ►B
│       └── ·7076dee (🏘️) ►D, ►E
└── ≡📙:5:D on 85efbe4 {44}
    └── 📙:5:D
        └── ·7076dee (🏘️) ►E

"#]]
    );
    // future improvement: it should most definitely not create a merge if both tips have shared in-workspace commits in their ancestry
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*--[31m-[m[31m.[m 69cfcf0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/ / /
|/ /
* / f084d61 (C, B, A) A2
|/
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/E"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    // E is only a decoration, so unapplying it leaves the explicit stacks unchanged.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // Legacy mode keeps the workspace commit and all four metadata-owned stacks.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:9:A on 85efbe4 {41}
│   └── 📙:9:A
│       ├── ·f084d61 (🏘️) ►B, ►C
│       └── ·7076dee (🏘️) ►D, ►E
├── ≡📙:10:B on 85efbe4 {42}
│   └── 📙:10:B
│       ├── ·f084d61 (🏘️) ►A, ►C
│       └── ·7076dee (🏘️) ►D, ►E
├── ≡📙:11:C on 85efbe4 {43}
│   └── 📙:11:C
│       ├── ·f084d61 (🏘️) ►A, ►B
│       └── ·7076dee (🏘️) ►D, ►E
└── ≡📙:5:D on 85efbe4 {44}
    └── 📙:5:D
        └── ·7076dee (🏘️) ►E

"#]]
    );
    let named_refs =
        ws.stacks
            .iter()
            .flat_map(|stack| {
                stack.segments.iter().filter_map(|segment| {
                    segment.ref_name().map(|name| (stack.id, name.to_string()))
                })
            })
            .collect::<Vec<_>>();
    assert_eq!(
        named_refs
            .iter()
            .map(|(_, name)| name)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        named_refs.len(),
        "a named reference belongs to only one projected stack: {named_refs:?}"
    );
    // The workspace commit still contains A, B, C, and D.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*--[31m-[m[31m.[m 69cfcf0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/ / /
|/ /
* / f084d61 (C, B, A) A2
|/
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/D"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    // Unapplying D removes its explicit stack and exposes E as the remaining lower owner.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // A, B, and C stay explicit while E becomes the lower stack.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:8:A on 85efbe4 {41}
│   └── 📙:8:A
│       ├── ·f084d61 (🏘️) ►B, ►C
│       └── ·7076dee (🏘️) ►E
├── ≡📙:9:B on 85efbe4 {42}
│   └── 📙:9:B
│       ├── ·f084d61 (🏘️) ►A, ►C
│       └── ·7076dee (🏘️) ►E
├── ≡📙:10:C on 85efbe4 {43}
│   └── 📙:10:C
│       ├── ·f084d61 (🏘️) ►A, ►B
│       └── ·7076dee (🏘️) ►E
└── ≡📙:5:E on 85efbe4 {45}
    └── 📙:5:E
        └── ·7076dee (🏘️)

"#]]
    );
    // The existing workspace commit already represents this equivalent parent set.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*--[31m-[m[31m.[m 69cfcf0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/ / /
|/ /
* / f084d61 (C, B, A) A2
|/
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/C"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    // Unapplying C removes its independently owned stack.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // A, B, and E remain applied; D is only a decoration on E's tip.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:9:A on 85efbe4 {41}
│   └── 📙:9:A
│       ├── ·f084d61 (🏘️) ►B
│       └── ·7076dee (🏘️) ►D, ►E
├── ≡📙:10:B on 85efbe4 {42}
│   └── 📙:10:B
│       ├── ·f084d61 (🏘️) ►A
│       └── ·7076dee (🏘️) ►D, ►E
└── ≡📙:6:E on 85efbe4 {45}
    └── 📙:6:E
        └── ·7076dee (🏘️) ►D

"#]]
    );
    // The workspace commit is rewritten for the remaining explicit parents.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*[31m-[m[31m.[m 1be4251 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/ /
* | f084d61 (C, B, A) A2
|/
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    // Unapplying B removes its independently owned stack.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // A and E remain as the two applied stacks.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:9:A on 85efbe4 {41}
│   └── 📙:9:A
│       ├── ·f084d61 (🏘️) ►C
│       └── ·7076dee (🏘️) ►D, ►E
└── ≡📙:6:E on 85efbe4 {45}
    └── 📙:6:E
        └── ·7076dee (🏘️) ►D

"#]]
    );
    // Legacy mode keeps a workspace commit for the remaining A and E parents.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   348bb6b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* | f084d61 (C, B, A) A2
|/
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    // Unapplying A removes the last upper stack.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // E is now the only applied stack.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:3:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:5:E on 85efbe4 {45}
    └── 📙:5:E
        └── ·7076dee (🏘️) ►D

"#]]
    );
    // The workspace commit is rewritten around E.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f084d61 (C, B, A) A2
| * 578b8e9 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* 7076dee (E, D) A1
* 85efbe4 (origin/main, main) M

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/gitbutler/workspace"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )
    .expect("workspace ref can be unapplied by checking out a named target");
    // unapplying the workspace ref switches to the target's local branch when the disposition allows deleting the workspace ref
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: Some(
        "refs/heads/main",
    ),
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // the projection is the checked-out target branch
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:1:main <> ✓!
└── ≡👉:1:main[🌳] <> origin/main →:2: {1}
    └── 👉:1:main[🌳] <> origin/main →:2:
        └── ❄85efbe4 (🏘️) ►origin/main

"#]]
    );
    // the managed workspace ref is deleted and HEAD points to main
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f084d61 (C, B, A) A2
* 7076dee (E, D) A1
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );

    Ok(())
}

#[test]
fn apply_with_conflicts_shows_exact_conflict_info() -> anyhow::Result<()> {
    let (_tmp, _graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "various-heads-for-multi-line-merge-conflict",
            |_meta| {},
        )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d3cce74 (clean-A) add A
| * 115e41b (clean-B) add B
|/
| * 34c4591 (clean-C) add C
|/
| * bf09eae (conflict-F1) add F1
|/
| * f2ce66d (conflict-F2) add F2
|/
| * 4bbb93c (HEAD -> conflict-hero) add conflicting-F2
| * 98519e9 add conflicting-F1
|/
* 85efbe4 (main, gitbutler/workspace) M

"#]]
    );

    git(&repo).args(["checkout", "main"]).run();
    git(&repo)
        .args(["branch", "-d", "gitbutler/workspace"])
        .run();
    // The fixture helper created `graph` while `HEAD` still pointed to `conflict-hero`.
    // Replaying that graph would correctly keep using `conflict-hero` as the traversal
    // entrypoint, even though the test just checked out `main`. Build the graph from
    // the current repository state so the workspace under test starts at `main`.
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_commit_id: repo.rev_parse_single("main").ok().map(|id| id.detach()),
        ..Default::default()
    };
    let mut ws = but_graph::Graph::from_repo(&repo, &meta, project_meta, Overlay::default())?
        .into_workspace()?;

    for branch_to_apply in [
        "clean-A",
        "conflict-F1",
        "clean-B",
        "conflict-F2",
        "clean-C",
    ] {
        let out = but_workspace::branch::apply(
            Category::LocalBranch
                .to_full_name(branch_to_apply)?
                .as_ref(),
            ws,
            &repo,
            &mut meta,
            apply_options(),
        )
        .unwrap_or_else(|err| panic!("{branch_to_apply}: {err}"));
        ws = out.workspace;
    }

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:12:gitbutler/workspace[🌳] <> ✓! on 85efbe4
├── ≡📙:9:main on 85efbe4 {1a5}
│   └── 📙:9:main
├── ≡📙:11:clean-A on 85efbe4 {271}
│   └── 📙:11:clean-A
│       └── ·d3cce74 (🏘️)
├── ≡📙:10:conflict-F1 on 85efbe4 {3f6}
│   └── 📙:10:conflict-F1
│       └── ·bf09eae (🏘️)
├── ≡📙:7:clean-B on 85efbe4 {272}
│   └── 📙:7:clean-B
│       └── ·115e41b (🏘️)
├── ≡📙:13:conflict-F2 on 85efbe4 {3f7}
│   └── 📙:13:conflict-F2
│       └── ·f2ce66d (🏘️)
└── ≡📙:8:clean-C on 85efbe4 {273}
    └── 📙:8:clean-C
        └── ·34c4591 (🏘️)

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 4bbb93c (conflict-hero) add conflicting-F2
* 98519e9 add conflicting-F1
| *-----.   e13e11a (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/|\ \ \ \
| | | | | * 34c4591 (clean-C) add C
| |_|_|_|/
|/| | | |
| | | | * f2ce66d (conflict-F2) add F2
| |_|_|/
|/| | |
| | | * 115e41b (clean-B) add B
| |_|/
|/| |
| | * bf09eae (conflict-F1) add F1
| |/
|/|
| * d3cce74 (clean-A) add A
|/
* 85efbe4 (main) M

"#]]
        .raw()
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/conflict-hero"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    snapbox::assert_data_eq!(
        sanitize_uuids_and_timestamps(format!("{out:#?}")),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[]",
    conflicting_stacks: [
        ConflictingStack {
            id: 1,
            ref_name: "refs/heads/conflict-F1",
        },
        ConflictingStack {
            id: 2,
            ref_name: "refs/heads/conflict-F2",
        },
    ],
}
"#]]
    );
    let ws = out.workspace;
    // By default, it fails and just reports the conflicting stacks, so it's the same as it was before.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:15:gitbutler/workspace[🌳] <> ✓! on 85efbe4
├── ≡📙:12:main on 85efbe4 {1a5}
│   └── 📙:12:main
├── ≡📙:14:clean-A on 85efbe4 {271}
│   └── 📙:14:clean-A
│       └── ·d3cce74 (🏘️)
├── ≡📙:13:conflict-F1 on 85efbe4 {3f6}
│   └── 📙:13:conflict-F1
│       └── ·bf09eae (🏘️)
├── ≡📙:9:clean-B on 85efbe4 {272}
│   └── 📙:9:clean-B
│       └── ·115e41b (🏘️)
├── ≡📙:16:conflict-F2 on 85efbe4 {3f7}
│   └── 📙:16:conflict-F2
│       └── ·f2ce66d (🏘️)
└── ≡📙:10:clean-C on 85efbe4 {273}
    └── 📙:10:clean-C
        └── ·34c4591 (🏘️)

"#]]
    );
    let conflicting_stacks = out
        .conflicting_stacks
        .iter()
        .map(|stack| stack.ref_name.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        conflicting_stacks,
        ["refs/heads/conflict-F1", "refs/heads/conflict-F2"]
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/conflict-hero"),
        ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            on_workspace_conflict: OnWorkspaceMergeConflict::MaterializeAndReportConflictingStacks,
            ..apply_options()
        },
    )?;
    // It does still report conflicts.
    snapbox::assert_data_eq!(
        sanitize_uuids_and_timestamps(format!("{out:#?}")),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/conflict-hero]",
    conflicting_stacks: [
        ConflictingStack {
            id: 1,
            ref_name: "refs/heads/conflict-F1",
        },
        ConflictingStack {
            id: 2,
            ref_name: "refs/heads/conflict-F2",
        },
    ],
}
"#]]
    );

    // Now the other stacks are unapplied.
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:11:gitbutler/workspace[🌳] <> ✓! on 85efbe4
├── ≡📙:10:main on 85efbe4 {1a5}
│   └── 📙:10:main
├── ≡📙:12:clean-A on 85efbe4 {271}
│   └── 📙:12:clean-A
│       └── ·d3cce74 (🏘️)
├── ≡📙:7:clean-B on 85efbe4 {272}
│   └── 📙:7:clean-B
│       └── ·115e41b (🏘️)
├── ≡📙:8:clean-C on 85efbe4 {273}
│   └── 📙:8:clean-C
│       └── ·34c4591 (🏘️)
└── ≡📙:9:conflict-hero on 85efbe4 {52d}
    └── 📙:9:conflict-hero
        ├── ·4bbb93c (🏘️)
        └── ·98519e9 (🏘️)

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bf09eae (conflict-F1) add F1
| * f2ce66d (conflict-F2) add F2
|/
| *---.   c51f37c (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/|\ \ \
| | | | * 4bbb93c (conflict-hero) add conflicting-F2
| | | | * 98519e9 add conflicting-F1
| |_|_|/
|/| | |
| | | * 34c4591 (clean-C) add C
| |_|/
|/| |
| | * 115e41b (clean-B) add B
| |/
|/|
| * d3cce74 (clean-A) add A
|/
* 85efbe4 (main) M

"#]]
        .raw()
    );

    let ws_md = sanitize_uuids_and_timestamps(format!(
        "{:#?}",
        ws.metadata
            .as_ref()
            .expect("managed workspace has metadata")
    ));
    snapbox::assert_data_eq!(
        ws_md,
        snapbox::str![[r#"
Workspace {
    ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
    stacks: [
        WorkspaceStack {
            id: 1,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/main",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
        WorkspaceStack {
            id: 2,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/clean-A",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
        WorkspaceStack {
            id: 3,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/conflict-F1",
                    archived: false,
                },
            ],
            workspacecommit_relation: Outside,
        },
        WorkspaceStack {
            id: 4,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/clean-B",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
        WorkspaceStack {
            id: 5,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/conflict-F2",
                    archived: false,
                },
            ],
            workspacecommit_relation: Outside,
        },
        WorkspaceStack {
            id: 6,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/clean-C",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
        WorkspaceStack {
            id: 7,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/conflict-hero",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ],
    target_ref: "refs/remotes/origin/main",
    target_commit_id: Sha1(85efbe4d5a663bff0ed8fb5fbc38a72be0592f55),
    push_remote: None,
}
"#]]
    );

    Ok(())
}

#[test]
fn conflicting_apply_reports_no_applied_branches_and_names_conflicting_stacks() -> anyhow::Result<()>
{
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "one-fork-with-conflicting-sibling",
            |_meta| {},
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bf53300 (A) add A
| * 543911c (add-A-too) add a different A
| * b1540e5 (HEAD -> main) M
|/
| * 0e391b2 (origin/B) add B
|/
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
    );

    let ws = graph.into_workspace()?;
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    let ws = out.workspace;
    // A is applied before trying the conflicting sibling branch
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e31e6ca
└── ≡📙:6:A on e31e6ca {41}
    └── 📙:6:A
        └── ·bf53300 (🏘️)

"#]]
    );
    let refs_before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        &refs_before,
        snapbox::str![[r#"
* 543911c (add-A-too) add a different A
* b1540e5 (main) M
| * 9f5b797 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * bf53300 (A) add A
|/
| * 0e391b2 (origin/B) add B
|/
* e31e6ca (origin/main, origin/HEAD) add init

"#]]
    );

    let out = but_workspace::branch::apply(
        r("refs/heads/add-A-too"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    assert_eq!(
        out.status,
        OutcomeStatus::ConflictAborted,
        "conflicted apply should be classified as an aborted apply"
    );
    // The workspace is changed, notably, as it always passes the most recent projection.
    // a conflict-aborted apply must not report branches as applied
    snapbox::assert_data_eq!(
        sanitize_uuids_and_timestamps(format!("{out:#?}")),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[]",
    conflicting_stacks: [
        ConflictingStack {
            id: 1,
            ref_name: "refs/heads/A",
        },
    ],
}
"#]]
    );
    assert_eq!(
        visualize_commit_graph_all(&repo)?,
        refs_before,
        "an aborting conflict must leave refs unchanged"
    );

    Ok(())
}

#[test]
fn unapply_with_workspace_merge_conflicts_always_works_as_conflicts_do_not_repeat_on_unapply()
-> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "various-heads-for-multi-line-merge-conflict-on-main",
            |_meta| {},
        )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d3cce74 (clean-A) add A
| * 115e41b (clean-B) add B
|/
| * 34c4591 (clean-C) add C
|/
| * bf09eae (conflict-F1) add F1
|/
| * f2ce66d (conflict-F2) add F2
|/
| * 4bbb93c (conflict-hero) add conflicting-F2
| * 98519e9 add conflicting-F1
|/
* 85efbe4 (HEAD -> main) M

"#]]
    );
    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:1:main <> ✓! on 85efbe4
└── ≡👉:1:main[🌳] on 85efbe4 {1}
    └── 👉:1:main[🌳]

"#]]
    );
    let branches = [
        "clean-A",
        "conflict-F1",
        "clean-B",
        "conflict-F2",
        "clean-C",
        "conflict-hero",
    ];
    for branch_to_apply in branches {
        let out = but_workspace::branch::apply(
            Category::LocalBranch
                .to_full_name(branch_to_apply)?
                .as_ref(),
            ws,
            &repo,
            &mut meta,
            but_workspace::branch::apply::Options {
                on_workspace_conflict:
                    OnWorkspaceMergeConflict::MaterializeAndReportConflictingStacks,
                ..apply_options()
            },
        )?;
        ws = out.workspace;
    }
    // all branches are applied
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:11:gitbutler/workspace[🌳] <> ✓! on 85efbe4
├── ≡📙:10:main on 85efbe4 {1a5}
│   └── 📙:10:main
├── ≡📙:12:clean-A on 85efbe4 {271}
│   └── 📙:12:clean-A
│       └── ·d3cce74 (🏘️)
├── ≡📙:7:clean-B on 85efbe4 {272}
│   └── 📙:7:clean-B
│       └── ·115e41b (🏘️)
├── ≡📙:8:clean-C on 85efbe4 {273}
│   └── 📙:8:clean-C
│       └── ·34c4591 (🏘️)
└── ≡📙:9:conflict-hero on 85efbe4 {52d}
    └── 📙:9:conflict-hero
        ├── ·4bbb93c (🏘️)
        └── ·98519e9 (🏘️)

"#]]
    );

    for branch_to_unapply in branches.into_iter().rev() {
        let out = but_workspace::branch::unapply(
            Category::LocalBranch
                .to_full_name(branch_to_unapply)?
                .as_ref(),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::KeepWorkspaceReference),
        )?;
        ws = out.workspace.into_owned();
    }

    // the workspace ref remains because the regenerated workspace still has main available
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓! on 85efbe4
└── ≡📙:2:main on 85efbe4 {1a5}
    └── 📙:2:main

"#]]
    );

    Ok(())
}

#[test]
fn auto_checkout_of_enclosing_workspace_flat() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-no-ws-commit-one-stack-one-branch",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:1:A on e5d0542 {1}
│   └── 📙:1:A
└── ≡📙:2:B on e5d0542 {2}
    └── 📙:2:B

"#]]
    );

    // Apply the workspace ref itself, it's a no-op
    let out = but_workspace::branch::apply(
        r("refs/heads/gitbutler/workspace"),
        ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    // nothing actually changed, so nothing is mentioned
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    workspace_ref_created: false,
    applied_branches: "[]",
}

"#]]
    );

    let (b_id, b_ref) = id_at(&repo, "B");
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        ..Default::default()
    };
    let ws = but_graph::Graph::from_repo(
        &repo,
        &meta,
        project_meta,
        Overlay::default().with_entrypoint(b_id.detach(), Some(b_ref.clone())),
    )?
    .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:1:A on e5d0542 {1}
│   └── 📙:1:A
└── ≡👉📙:2:B on e5d0542 {2}
    └── 👉📙:2:B

"#]]
    );
    // Already applied (the HEAD points to it).
    let out = but_workspace::branch::apply(
        b_ref.as_ref(),
        ws.clone(),
        &repo,
        &mut meta,
        apply_options(),
    )?;
    // no-ops aren't listing the already applied branches
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    workspace_ref_created: false,
    applied_branches: "[]",
}

"#]]
    );

    // Simulate stale metadata: the cached graph still contains A, but metadata says it is outside.
    let mut ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    let (stack_idx, _) = ws_md
        .find_owner_indexes_by_name(r("refs/heads/A"), StackKind::AppliedAndUnapplied)
        .expect("A is in metadata");
    ws_md.stacks[stack_idx].workspacecommit_relation = Outside;
    meta.set_workspace(&ws_md)?;

    // To apply A, we checkout the surrounding workspace and repair the stale metadata.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "enclosed branches should report a successful apply"
    );
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );
    let ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    assert!(
        ws_md
            .find_branch(r("refs/heads/A"), StackKind::Applied)
            .is_some(),
        "apply should repair stale enclosing-branch metadata"
    );

    // Now the workspace ref itself is checked out.
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:1:A on e5d0542 {1}
│   └── 📙:1:A
└── ≡📙:2:B on e5d0542 {2}
    └── 📙:2:B

"#]]
    );
    // Even though the real repo seemingly didn't change, after all, our entrypoint was just 'virtual'.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    // make "A" an applied dependent branch that is included in B so apply will do nothing.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &["A"]);

    let (b_id, b_ref) = id_at(&repo, "B");

    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        ..Default::default()
    };
    let ws = but_graph::Graph::from_repo(
        &repo,
        &meta,
        project_meta,
        Overlay::default().with_entrypoint(b_id.detach(), Some(b_ref.clone())),
    )?
    .into_workspace()?;
    // V-branch B is checked out
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡👉📙:2:B on e5d0542 {2}
    ├── 👉📙:2:B
    └── 📙:1:A

"#]]
    );

    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    // Nothing changed, the desired branch was already applied
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    workspace_ref_created: false,
    applied_branches: "[]",
}

"#]]
    );

    // There is no known branch, and adding it will just add metadata.
    meta.data_mut().branches.clear();
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        ..Default::default()
    };
    let ws = but_graph::Graph::from_repo(&repo, &meta, project_meta, Overlay::default())?
        .into_workspace()?;
    // There is nothing yet.
    // metadata defines no branches
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );

    // Apply the first branch, it must be independent.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "already-visible branches should report a successful apply"
    );
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );
    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:1:A on e5d0542 {41}
    └── 📙:1:A

"#]]
    );

    // Apply the first branch, it must be independent.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), ws, &repo, &mut meta, apply_options())?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/B]",
}

"#]]
    );
    // B is added as independent stack
    snapbox::assert_data_eq!(
        graph_workspace(&out.workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:1:A on e5d0542 {41}
│   └── 📙:1:A
└── ≡📙:2:B on e5d0542 {42}
    └── 📙:2:B

"#]]
    );

    let (b_id, b_ref) = id_at(&repo, "B");
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        ..Default::default()
    };
    let ws = but_graph::Graph::from_repo(
        &repo,
        &meta,
        project_meta,
        Overlay::default().with_entrypoint(b_id.detach(), Some(b_ref.clone())),
    )?
    .into_workspace()?;
    // the same result when checked out directly
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on e5d0542
├── ≡📙:1:A on e5d0542 {41}
│   └── 📙:1:A
└── ≡👉📙:2:B on e5d0542 {42}
    └── 👉📙:2:B

"#]]
    );

    let ws = out.workspace;
    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );

    let ws = out.workspace.into_owned();
    // the workspace is empty again
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓! on e5d0542
└── ≡📙:1:B on e5d0542 {42}
    └── 📙:1:B

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    checked_out: None,
}

"#]]
    );
    let ws = out.workspace.into_owned();
    // the workspace is empty again
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓! on e5d0542

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );
    Ok(())
}

#[test]
fn auto_checkout_of_enclosing_workspace_with_commits() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-two-stacks",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\
| * 09d8e52 (A) A
* | c813d8d (B) B
|/
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:4:A on 85efbe4 {1}
│   └── 📙:4:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:8:B on 85efbe4 {2}
    └── 📙:8:B
        └── ·c813d8d (🏘️)

"#]]
    );

    // Apply the workspace ref itself, it's a no-op
    let ws_ref = r("refs/heads/gitbutler/workspace");
    let out = but_workspace::branch::apply(ws_ref, ws, &repo, &mut meta, apply_options())?;
    // the workspace ref itself counts as no-op as well
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    workspace_ref_created: false,
    applied_branches: "[]",
}

"#]]
    );

    let (b_id, b_ref) = id_at(&repo, "B");
    let ws = but_graph::Graph::from_repo(
        &repo,
        &meta,
        project_meta(&meta),
        Overlay::default().with_entrypoint(b_id.detach(), Some(b_ref.clone())),
    )?
    .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:4:A on 85efbe4 {1}
│   └── 📙:4:A
│       └── ·09d8e52 (🏘️)
└── ≡👉📙:8:B on 85efbe4 {2}
    └── 👉📙:8:B
        └── ·c813d8d (🏘️)

"#]]
    );

    // Already applied (the HEAD points to it, it literally IS the workspace).
    let out = but_workspace::branch::apply(
        b_ref.as_ref(),
        ws.clone(),
        &repo,
        &mut meta,
        apply_options(),
    )?;
    // already applied branches are a no-op, even when a stack segment is checked out
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    workspace_ref_created: false,
    applied_branches: "[]",
}

"#]]
    );

    let err = but_workspace::branch::apply(ws_ref, ws.clone(), &repo, &mut meta, apply_options())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to apply a reference that already is a workspace: 'gitbutler/workspace'",
        "it's never good to merge one managed workspace into another, and we just disallow it.\
         Note that we could also check it out."
    );

    // To apply, we just checkout the surrounding workspace.
    let b_tip_before_apply = id_by_rev(&repo, "B");
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), ws, &repo, &mut meta, apply_options())?;
    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "enclosed branches with commits should report a successful apply"
    );
    assert_eq!(
        id_by_rev(&repo, "B"),
        b_tip_before_apply,
        "applying an enclosed branch must not move the previously checked-out branch"
    );
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: true,
    workspace_ref_created: false,
    applied_branches: "[refs/heads/A]",
}

"#]]
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:4:A on 85efbe4 {1}
│   └── 📙:4:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:8:B on 85efbe4 {2}
    └── 📙:8:B
        └── ·c813d8d (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn apply_nonexisting_branch_failure() -> anyhow::Result<()> {
    let (repo, mut meta) =
        named_read_only_in_memory_scenario("ws-ref-no-ws-commit-one-stack-one-branch", "")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    meta.data_mut()
        .default_target
        .as_mut()
        .expect("workspace configured")
        .sha = gix::hash::Kind::Sha1.null();
    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Overlay::default(),
    )?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
└── ≡:0:anon
    └── :0:anon
        └── ·e5d0542 (🏘️) ►A, ►B, ►gitbutler/workspace[🌳], ►main

"#]]
    );

    let err = but_workspace::branch::apply(
        r("refs/heads/does-not-exist"),
        ws,
        &repo,
        &mut *meta,
        apply_options(),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Cannot apply non-existing branch 'does-not-exist'"
    );

    // Nothing should be changed
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );
    Ok(())
}

#[test]
fn unapply_nonexisting_branch() -> anyhow::Result<()> {
    let (repo, mut meta) =
        named_read_only_in_memory_scenario("ws-ref-no-ws-commit-one-stack-one-branch", "")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );

    meta.data_mut()
        .default_target
        .as_mut()
        .expect("workspace configured")
        .sha = gix::hash::Kind::Sha1.null();
    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Overlay::default(),
    )?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
└── ≡:0:anon
    └── :0:anon
        └── ·e5d0542 (🏘️) ►A, ►B, ►gitbutler/workspace[🌳], ►main

"#]]
    );

    let err = but_workspace::branch::unapply(
        r("refs/heads/does-not-exist"),
        &ws,
        &repo,
        &mut *meta,
        unapply_options(),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Cannot unapply non-existing branch 'does-not-exist'"
    );

    // Nothing should be changed
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A

"#]]
    );
    Ok(())
}

#[test]
fn unborn_apply_needs_base() -> anyhow::Result<()> {
    let (repo, mut meta) =
        named_read_only_in_memory_scenario("unborn-empty-detached-remote", "unborn")?;
    // Depending on the Git version it produces`* 3183e43 (orphan/main, orphan/HEAD) M1` on CI,
    // so a comment is used as reference.
    // snapbox::assert_data_eq!(visualize_commit_graph_all(&repo)?, snapbox::str!["* 3183e43 (orphan/main) M1"]);

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Overlay::default(),
    )?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:-:main <> ✓!

"#]]
    );

    // Idempotency in ad-hoc workspace
    let out = but_workspace::branch::apply(
        r("refs/heads/main"),
        ws.clone(),
        &repo,
        &mut *meta,
        apply_options(),
    )?;
    // the HEAD is already at 'main', so nothing changes
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    workspace_ref_created: false,
    applied_branches: "[]",
}

"#]]
    );

    // Cannot apply branch without a base,
    // but since remote is transformed into a local tracking branch, it's a noop.
    let out = but_workspace::branch::apply(
        r("refs/remotes/orphan/main"),
        ws,
        &repo,
        &mut *meta,
        apply_options(),
    )?;
    // this won't happen (often) in the real world, but it's a no-op
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
Outcome {
    workspace_changed: false,
    workspace_ref_created: false,
    applied_branches: "[]",
}

"#]]
    );

    let ws = out.workspace;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:-:main <> ✓!

"#]]
    );

    Ok(())
}

fn apply_options() -> but_workspace::branch::apply::Options {
    but_workspace::branch::apply::Options {
        // TODO: make this the Default once it's MergeIfNeeded.
        workspace_merge: WorkspaceMerge::MergeIfNeeded,
        on_workspace_conflict: OnWorkspaceMergeConflict::AbortAndReportConflictingStacks,
        workspace_reference_naming: WorkspaceReferenceNaming::Default,
        order: None,
        new_stack_id: Some(stack_id_for_name),
    }
}

fn unapply_options() -> but_workspace::branch::unapply::Options {
    but_workspace::branch::unapply::Options {
        // TODO: make this the Default once it's KeepWorkspaceReference.
        workspace_disposition: WorkspaceDisposition::KeepWorkspaceReference,
    }
}

/// Legacy because we plan to only have the commits we really need.
fn legacy_unapply_options() -> but_workspace::branch::unapply::Options {
    unapply_options_with(WorkspaceDisposition::KeepWorkspaceCommit)
}

fn unapply_options_with(
    disposition: WorkspaceDisposition,
) -> but_workspace::branch::unapply::Options {
    but_workspace::branch::unapply::Options {
        workspace_disposition: disposition,
    }
}

fn stack_id_for_name(rn: &gix::refs::FullNameRef) -> StackId {
    StackId::from_number_for_testing(rn.shorten().chars().map(|c| c as u128).sum())
}
