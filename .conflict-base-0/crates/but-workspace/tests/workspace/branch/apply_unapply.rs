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
use but_graph::{
    Graph,
    init::{Options, Overlay, Tip},
};
use but_testsupport::{
    CommandExt, InMemoryRefMetadata, git, graph_workspace, graph_workspace_determinisitcally,
    id_at, id_by_rev, sanitize_uuids_and_timestamps, visualize_commit_graph_all,
    visualize_disk_tree_with_hashes_skip_dot_git,
};
use but_workspace::branch::{
    OnWorkspaceMergeConflict,
    apply::{OutcomeStatus, WorkspaceMerge, WorkspaceReferenceNaming},
    create_reference::{Anchor, Position::Above},
    unapply::WorkspaceDisposition,
};
use gix::refs::{Category, transaction::PreviousValue};

fn project_meta(meta: &impl RefMetadata) -> ref_metadata::ProjectMeta {
    meta.workspace(
        but_core::WORKSPACE_REF_NAME
            .try_into()
            .expect("valid workspace ref"),
    )
    .map(|workspace| workspace.project_meta())
    .unwrap_or_default()
}

#[test]
fn operation_denied_on_improper_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-one-stack-ws-advanced",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 0d01196 (HEAD -> gitbutler/workspace) O1
    * 4979833 GitButler Workspace Commit
    * 3183e43 (main, B, A) M1
    ");
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    └── ≡:2:anon: on 3183e43
        └── :2:anon:
            ├── ·0d01196 (🏘️)
            └── ·4979833 (🏘️)
    ");

    let branch_b = r("refs/heads/B");
    let err =
        but_workspace::branch::apply(branch_b, &ws, &repo, &mut meta, apply_options()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to work on workspace whose workspace commit isn't at the top",
        "cannot apply on a workspace that isn't proper"
    );

    let err = but_workspace::branch::apply(r("HEAD"), &ws, &repo, &mut meta, apply_options())
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "fixture starts with a single local main branch", @"
    * 281da94 (HEAD -> main) 3
    * 12995d7 2
    * 3d57fc1 1
    ");

    let ws = but_graph::Graph::from_head(
        &repo,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Options::default(),
    )?
    .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), "a normal checked-out branch is still an ad-hoc workspace without workspace metadata", @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
            ├── ·281da94
            ├── ·12995d7
            └── ·3d57fc1
    ");

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
fn unapply_branch_from_named_ad_hoc_workspace_affects_metadata() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = named_writable_scenario("single-stack-two-segments")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "fixture starts with a single local main branch", @"
    * f1889e7 (A2) add A2
    * 7de99e1 (A1) add A1
    | * 53ad0c2 (unrelated) add U1
    |/  
    * 3183e43 (HEAD -> main, origin/main) M1
    ");

    let a2_ref = r("refs/heads/A2");
    let a2_tip = repo.rev_parse_single("A2")?;
    let ws = but_graph::Graph::from_commit_traversal(
        a2_tip,
        a2_ref.to_owned(),
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Options::default(),
    )?
    .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), "a normal checked-out branch is still an ad-hoc workspace without workspace metadata", @"
    ⌂:0:A2 <> ✓!
    └── ≡:0:A2 {1}
        ├── :0:A2
        │   └── ·f1889e7
        ├── :1:A1
        │   └── ·7de99e1
        └── :2:main[🌳]
            └── ·3183e43
    ");

    let branch = Category::LocalBranch.to_full_name("on-A1")?;
    let first_id = id_by_rev(&repo, "A1");
    let ws = but_workspace::branch::create_reference(
        branch.as_ref(),
        Anchor::AtCommit {
            commit_id: first_id.detach(),
            position: Above,
        },
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?
    .into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "creating a branch in the ad-hoc workspace gives unapply a visible segment to reject, it has its own ref-metadata", @"
    ⌂:0:A2 <> ✓!
    └── ≡:0:A2 {1}
        ├── :0:A2
        │   └── ·f1889e7
        ├── 📙:1:on-A1
        │   └── ·7de99e1 ►A1
        └── :2:main[🌳]
            └── ·3183e43
    ");

    let out = but_workspace::branch::unapply(
        branch.as_ref(),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )
    .expect("this works because `on-A1` is only present thanks to ref-metadata which we removed");

    insta::assert_snapshot!(graph_workspace(&out.workspace), "on-A1 no longer has a metadata-disambiguated segment", @"
    ⌂:0:A2 <> ✓!
    └── ≡:0:A2 {1}
        ├── :0:A2
        │   ├── ·f1889e7
        │   └── ·7de99e1 ►A1, ►on-A1
        └── :1:main[🌳]
            └── ·3183e43
    ");

    let err = but_workspace::branch::unapply(
        r("refs/heads/main"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )
    .expect_err("without project target metadata in the graph, main is a visible non-tip segment");
    assert_eq!(
        err.to_string(),
        "Cannot unapply branch 'main' from an ad-hoc workspace because non-tip branches can only disappear if their now removed metadata disambiguated them"
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    // We know a stack, but nothing is actually in the workspace.
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542");

    // Put "B" into the workspace, even though it's the dependent branch of A.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/B]",
    }
    "#);
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    └── ≡📙:2:B on e5d0542 {1}
        └── 📙:2:B
    ");

    // Applying A is always a new stack then.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_snapshot!(graph_workspace(&out.workspace), "the workspace ref still points to the base e5d0542", @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    ├── ≡📙:2:B on e5d0542 {1}
    │   └── 📙:2:B
    └── ≡📙:3:A on e5d0542 {41}
        └── 📙:3:A
    ");

    // It's all virtual.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    let ws = out.workspace.into_owned();
    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    insta::assert_debug_snapshot!(out, @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws),"A was removed with metadata only", @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    └── ≡📙:2:B on e5d0542 {1}
        └── 📙:2:B
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    insta::assert_debug_snapshot!(out, "virtual stacks don't cause checkouts.", @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");
    insta::assert_snapshot!(graph_workspace(&out.workspace), "no stacks are left", @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "no workspace commit is present", @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
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
        insta::assert_snapshot!(graph_workspace(&out.workspace), "A was removed", @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
        ├── ≡📙:4:virtual-base on 85efbe4 {1}
        │   └── 📙:4:virtual-base
        └── ≡📙:3:B on 85efbe4 {3}
            └── 📙:3:B
                └── ·c813d8d (🏘️)
        ");
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the workspace commit is still there with two parents (even though there is no need)", @"
        * 09d8e52 (A) A
        | * 3b77768 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        |/| 
        | * c813d8d (B) B
        |/  
        * 85efbe4 (origin/main, virtual-base, main) M
        ");

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
        insta::assert_snapshot!(graph_workspace(&out.workspace), "There still is a workspace merge commit, we have two stacks, virtual or not", @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
        ├── ≡📙:4:virtual-base on 85efbe4 {1}
        │   └── 📙:4:virtual-base
        └── ≡📙:3:B on 85efbe4 {3}
            └── 📙:3:B
                └── ·c813d8d (🏘️)
        ");
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the virtual stack counts as real stack and it merges the base to account for it", @"
        * 09d8e52 (A) A
        | * 3b77768 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        |/| 
        | * c813d8d (B) B
        |/  
        * 85efbe4 (origin/main, virtual-base, main) M
        ");

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
        insta::assert_debug_snapshot!(&out, "with one real stack left, the workspace ref is deleted and the remaining stack is checked out", @r#"
        Outcome {
            workspace_changed: true,
            checked_out: Some(
                "refs/heads/B",
            ),
        }
        "#);
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the workspace ref is deleted and HEAD points to the last real stack", @"
        * 09d8e52 (A) A
        | * c813d8d (HEAD -> B) B
        |/  
        * 85efbe4 (origin/main, main) M
        ");
        insta::assert_snapshot!(graph_workspace(&out.workspace), "the projection is the checked-out remaining stack", @"
        ⌂:0:B[🌳] <> ✓refs/remotes/origin/main on 85efbe4
        └── ≡📙:0:B[🌳] on 85efbe4 {1}
            └── 📙:0:B[🌳]
                └── ·c813d8d
        ");

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
        insta::assert_debug_snapshot!(&out, @"
        Outcome {
            workspace_changed: true,
            checked_out: None,
        }
        ");
        assert!(
            out.workspace.kind.has_managed_commit(),
            "compatibility disposition should preserve the managed workspace commit when the workspace ref remains"
        );
        assert!(
            out.workspace_merge.is_some(),
            "compatibility disposition should rebuild the workspace merge instead of collapsing the ref to the last real stack"
        );
        insta::assert_snapshot!(graph_workspace(&out.workspace), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
        ├── ≡📙:3:A on 85efbe4 {2}
        │   └── 📙:3:A
        │       └── ·09d8e52 (🏘️)
        └── ≡📙:4:B on 85efbe4 {3}
            └── 📙:4:B
                └── ·c813d8d (🏘️)
        ");
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        *   483c8bd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        |\  
        | * c813d8d (B) B
        * | 09d8e52 (A) A
        |/  
        * 85efbe4 (origin/main, virtual-base, main) M
        ");

        Ok(())
    }

    #[test]
    fn allow_workspace_reference_deletion() -> anyhow::Result<()> {
        let (_tmp, _, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "no-ws-ref-no-ws-commit-two-branches",
                |_meta| {},
            )?;

        let ws = Graph::from_head(
            &repo,
            &meta,
            project_meta(&meta),
            standard_traversal_options(),
        )?
        .into_workspace()?;
        let out = but_workspace::branch::apply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            apply_options(),
        )?;
        let ws = out.workspace.into_owned();
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
        └── ≡📙:3:A on e5d0542 {41}
            └── 📙:3:A
        ");

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
        )?;
        insta::assert_debug_snapshot!(&out, "deleting the workspace reference should switch to the local tracking branch of the target", @r#"
        Outcome {
            workspace_changed: true,
            checked_out: Some(
                "refs/heads/main",
            ),
        }
        "#);

        insta::assert_snapshot!(graph_workspace(&out.workspace), "everything is dissolved, there is no gitbutler/workspace ref either", @"
        ⌂:0:main[🌳] <> ✓refs/remotes/origin/main on e5d0542
        └── ≡:0:main[🌳] <> origin/main →:1: {1}
            └── :0:main[🌳] <> origin/main →:1:
        ");

        Ok(())
    }

    #[test]
    fn compatibility_mode_deletes_workspace_reference_when_possible() -> anyhow::Result<()> {
        let (_tmp, _, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "no-ws-ref-no-ws-commit-two-branches",
                |_meta| {},
            )?;

        let ws = Graph::from_head(
            &repo,
            &meta,
            project_meta(&meta),
            standard_traversal_options(),
        )?
        .into_workspace()?;
        let out = but_workspace::branch::apply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            apply_options(),
        )?;
        let ws = out.workspace.into_owned();

        let out = but_workspace::branch::unapply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            unapply_options_with(
                WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
            ),
        )?;
        insta::assert_debug_snapshot!(&out, @r#"
        Outcome {
            workspace_changed: true,
            checked_out: Some(
                "refs/heads/main",
            ),
        }
        "#);

        insta::assert_snapshot!(graph_workspace(&out.workspace), "the compatibility mode still removes the whole workspace when it can", @"
        ⌂:0:main[🌳] <> ✓refs/remotes/origin/main on e5d0542
        └── ≡:0:main[🌳] <> origin/main →:1: {1}
            └── :0:main[🌳] <> origin/main →:1:
        ");

        Ok(())
    }

    #[test]
    fn unapply_workspace_ref_requires_disposition_that_allows_switching() -> anyhow::Result<()> {
        let (_tmp, _, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "no-ws-ref-no-ws-commit-two-branches",
                |_meta| {},
            )?;

        let ws = Graph::from_head(
            &repo,
            &meta,
            project_meta(&meta),
            standard_traversal_options(),
        )?
        .into_workspace()?;
        let out = but_workspace::branch::apply(
            r("refs/heads/A"),
            &ws,
            &repo,
            &mut meta,
            apply_options(),
        )?;
        let ws = out.workspace.into_owned();
        insta::assert_snapshot!(graph_workspace(&ws), "the workspace ref is checked out", @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
        └── ≡📙:3:A on e5d0542 {41}
            └── 📙:3:A
        ");

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

        insta::assert_snapshot!(graph_workspace(&ws), "the workspace is unchanged", @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
        └── ≡📙:3:A on e5d0542 {41}
            └── 📙:3:A
        ");
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
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
        * 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        * d69fe94 (B) B
        * 09d8e52 (A) A
        * 85efbe4 (origin/main, main) M
        ");

        let ws = graph.into_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
        └── ≡📙:3:B on 85efbe4 {1}
            ├── 📙:3:B
            │   └── ·d69fe94 (🏘️)
            └── 📙:4:A
                └── ·09d8e52 (🏘️)
        ");

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
        insta::assert_snapshot!(graph_workspace(&out.workspace), "the workspace is empty", @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4");
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the workspace commit is kept as no-op, it's legacy behaviour that will be removed", @"
        * d69fe94 (B) B
        * 09d8e52 (A) A
        | * bde8ed6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        |/  
        * 85efbe4 (origin/main, main) M
        ");

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
        let ws = Graph::from_head(
            &repo,
            &meta,
            project_meta(&meta),
            standard_traversal_options(),
        )?
        .into_workspace()?;
        assert!(
            ws.kind.has_managed_commit(),
            "fixture starts with a managed workspace commit"
        );

        let repo_log = visualize_commit_graph_all(&repo)?;
        insta::allow_duplicates! {
            insta::assert_snapshot!(repo_log, @r"
            *   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
            |\  
            | * 09d8e52 (A) A
            * | c813d8d (B) B
            |/  
            * 85efbe4 (origin/main, virtual-base, main) M
            ");
            insta::assert_snapshot!(graph_workspace(&ws), @"
            📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
            ├── ≡📙:5:virtual-base on 85efbe4 {1}
            │   └── 📙:5:virtual-base
            ├── ≡📙:3:A on 85efbe4 {2}
            │   └── 📙:3:A
            │       └── ·09d8e52 (🏘️)
            └── ≡📙:4:B on 85efbe4 {3}
                └── 📙:4:B
                    └── ·c813d8d (🏘️)
            ");
        }
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

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 6b40b15 (origin/feature) without-local-tracking
    | * 552e7dc (origin/main) only-on-remote
    |/  
    * 3183e43 (HEAD -> main) M1
    ");

    let mut meta = InMemoryRefMetadata::default();
    meta.workspaces.push((
        "refs/heads/gitbutler/workspace".try_into()?,
        ref_metadata::Workspace::default(),
    ));
    let graph = Graph::from_head(
        &repo,
        &vb_version_cannot_have_remotes,
        ref_metadata::ProjectMeta::default(),
        Options::limited(),
    )?;
    let ws = graph.into_workspace()?;
    // note how the remote isn't interesting as we have no target configured, nor an extra target.
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
            └── ·3183e43
    ");

    // We cannot apply remote tracking branches directly, but it resolves automatically to local tracking branches.
    let out = but_workspace::branch::apply(
        r("refs/remotes/origin/main"),
        &ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    insta::assert_debug_snapshot!(out, "nothing was actually applied as the `main` branch is already in the workspace", @r#"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
        applied_branches: "[]",
    }
    "#);

    // Set up an automatic tracking of origin/feature, as remote tracking branches can't be in the workspace.
    let out = but_workspace::branch::apply(
        r("refs/remotes/origin/feature"),
        &ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
        applied_branches: "[refs/heads/main, refs/heads/feature]",
    }
    "#);
    let ws = out.workspace.into_owned();
    // both branches, main and feature, are available in the newly created workspace ref.
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡📙:1:feature {2ec}
        ├── 📙:1:feature
        │   └── ·6b40b15 (🏘️)
        └── 📙:2:main
            └── ·3183e43 (🏘️)
    ");

    // the new local tracking ref actually exists, and is in the right spot.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 3d23cfb (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 6b40b15 (origin/feature, feature) without-local-tracking
    | * 552e7dc (origin/main) only-on-remote
    |/  
    * 3183e43 (main) M1
    ");

    repo.reload()?;
    let config = repo.config_snapshot();
    let section = config.section("branch", Some("feature".into()))?;
    insta::assert_snapshot!(section.to_bstring(), @r#"
    [branch "feature"]
    	remote = origin
    	merge = refs/heads/feature
    "#);

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
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, apply_options())?;
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:3:B <> origin/B →:5: on 85efbe4 {42}
        └── 📙:3:B <> origin/B →:5:
            ├── ❄️f084d61 (🏘️) ►A, ►C
            └── ❄️7076dee (🏘️) ►D, ►E
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(
            WorkspaceDisposition::PreventUnnecessaryWorkspaceReferencesKeepWorkspaceCommit,
        ),
    )?;
    insta::assert_debug_snapshot!(out, "deleting the workspace ref uses the target fallback instead of the unapplied stack", @r#"
    Outcome {
        workspace_changed: true,
        checked_out: Some(
            "refs/heads/main",
        ),
    }
    "#);
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the managed workspace ref is deleted and the target's local tracking branch is checked out", @"
    * f084d61 (origin/C, origin/B, origin/A, C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (HEAD -> main, origin/main) M
    ");

    insta::assert_snapshot!(graph_workspace(&out.workspace), "an ad-hoc workspace remains on the target's local tracking branch", @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡:0:main[🌳] <> origin/main →:1: {1}
        └── :0:main[🌳] <> origin/main →:1:
    ");
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
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
    ");

    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡:4:anon: on 3183e43
    │   └── :4:anon:
    │       └── ·d6bdeab (🏘️)
    └── ≡📙:5:outside →:3: on 3183e43 {1}
        └── 📙:5:outside →:3:
            ├── ·5121eb9*
            └── ·67c6397 (🏘️)
    ");

    let out = but_workspace::branch::apply(
        r("refs/heads/feature"),
        &ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/feature]",
    }
    "#);

    insta::assert_snapshot!(graph_workspace(&out.workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡:5:anon: on 3183e43
    │   └── :5:anon:
    │       └── ·d6bdeab (🏘️)
    ├── ≡📙:3:outside on 3183e43 {1}
    │   └── 📙:3:outside
    │       ├── ·5121eb9 (🏘️)
    │       └── ·67c6397 (🏘️)
    └── ≡📙:4:feature on 3183e43 {2ec}
        └── 📙:4:feature
            └── ·d03b217 (🏘️)
    ");
    Ok(())
}

#[test]
fn ws_ref_no_ws_commit_two_stacks_on_same_commit() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-no-ws-commit-one-stack-one-branch",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542");

    // Put "A" into the workspace, yielding a single branch.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/A]",
    }
    "#);
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    └── ≡📙:2:A on e5d0542 {41}
        └── 📙:2:A
    ");
    // No commit was created, as it's not enabled by default.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    let branch_b = r("refs/heads/B");
    let out = but_workspace::branch::apply(branch_b, &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/B]",
    }
    "#);
    // Note how it will create a new stack (to keep it simple),
    // in theory we could also add B as dependent branch.
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    ├── ≡📙:2:A on e5d0542 {41}
    │   └── 📙:2:A
    └── ≡📙:3:B on e5d0542 {42}
        └── 📙:3:B
    ");

    // Nothing changed visibly, still, it's all in the metadata.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    let out = but_workspace::branch::unapply(branch_b, &ws, &repo, &mut meta, unapply_options())?;
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    └── ≡📙:2:A on e5d0542 {41}
        └── 📙:2:A
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    insta::assert_snapshot!(graph_workspace(&out.workspace), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

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
    insta::assert_snapshot!(graph_workspace(&ws), "A is naturally visible, and B has metadata that matches it with an extra segment in metadata that's not there", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:B on 85efbe4 {1}
    │   └── 📙:3:B
    │       └── ·c813d8d (🏘️)
    └── ≡:4:A on 85efbe4
        └── :4:A
            └── ·09d8e52 (🏘️)
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;

    insta::assert_snapshot!(graph_workspace(&out.workspace), "A is removed and B is left", @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:3:B on 85efbe4 {1}
        └── 📙:3:B
            └── ·c813d8d (🏘️)
    ");

    let ws_md = out.workspace.metadata.as_ref().expect("present");
    insta::assert_debug_snapshot!(&ws_md,"the extra segment is still present in metadata to help with stack identities", @r#"
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
    "#);
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
    insta::assert_snapshot!(graph_workspace(&ws), "a single and multi-branch stack are visible without any workspace metadata", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 893d602
    ├── ≡:3:C on 893d602
    │   ├── :3:C
    │   │   └── ·356de85 (🏘️)
    │   └── :5:B
    │       └── ·f25f65c (🏘️)
    └── ≡:4:A on 893d602
        └── :4:A
            └── ·26e45af (🏘️)
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/C"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;

    insta::assert_snapshot!(graph_workspace_determinisitcally(&out.workspace), "C was unapplied, and the workspace commit removed", @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 893d602
    └── ≡📙:3:A on 893d602 {1}
        └── 📙:3:A
            └── ·26e45af (🏘️)
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?,"the workspace commit isn't retained due to the workspace disposition", @"
    * 26e45af (HEAD -> gitbutler/workspace, A) A
    | * 356de85 (C) C
    | * f25f65c (B) B
    |/  
    * 893d602 (origin/main, main) M
    ");

    let ws_md = out.workspace.metadata.as_ref().expect("present");
    insta::assert_snapshot!(sanitize_uuids_and_timestamps(format!("{ws_md:#?}")), "the metadata matches the real workspace now", @r#"
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
    "#);
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
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta),
        standard_traversal_options(),
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> main, origin/main, B, A) A");
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
            └── ·e5d0542 ►A, ►B
    ");

    // Put "A" into the workspace, creating the workspace ref, but never put a branch related to the target in as well,
    // which is currently checked out with `main`.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
        applied_branches: "[refs/heads/main, refs/heads/A]",
    }
    "#);

    insta::assert_snapshot!(graph_workspace(&out.workspace), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    ├── ≡📙:2:main on e5d0542 {1a5}
    │   └── 📙:2:main
    └── ≡📙:3:A on e5d0542 {41}
        └── 📙:3:A
    ");

    // No commit was created, as it's not enabled by default, but a ws-ref was created, and it's checked out.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A");

    let out = but_workspace::branch::apply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            // Make it appear in place of A, in the center.
            order: Some(1),
            ..apply_options()
        },
    )?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/main, refs/heads/B]",
    }
    "#);
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    ├── ≡📙:2:main on e5d0542 {1a5}
    │   └── 📙:2:main
    ├── ≡📙:3:B on e5d0542 {42}
    │   └── 📙:3:B
    └── ≡📙:4:A on e5d0542 {41}
        └── 📙:4:A
    ");

    // Nothing changed visibly, still, it's all in the metadata.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A");

    // Reset the workspace to 'unapply', but keep the per-branch metadata.
    let mut ws_md = meta.workspace(ws.ref_name().expect("proper gb workspace"))?;
    for stack in &mut ws_md.stacks {
        stack.workspacecommit_relation = Outside;
    }
    meta.set_workspace(&ws_md)?;

    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &meta, Overlay::default())?
        .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:anon: {41}
        └── :1:anon:
            └── ·e5d0542 (🏘️) ►A, ►B, ►main
    ");

    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            workspace_merge: WorkspaceMerge::AlwaysMerge,
            ..apply_options()
        },
    )?;
    // A workspace commit was created, even though it does nothing.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 6277161 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * e5d0542 (origin/main, main, B, A) A
    ");

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡📙:2:A {41}
        └── 📙:2:A
            └── ·e5d0542 (🏘️) ►B, ►main
    ");

    let out = but_workspace::branch::apply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            workspace_merge: WorkspaceMerge::AlwaysMerge,
            ..apply_options()
        },
    )?;

    // It's idempotent, but has to update the workspace commit nonetheless for the comment, which depends on the stacks.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 5ecf7a4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\
    * e5d0542 (origin/main, main, B, A) A
    ");

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    ├── ≡📙:2:B on e5d0542 {42}
    │   └── 📙:2:B
    └── ≡📙:3:A on e5d0542 {41}
        └── 📙:3:A
    ");

    Ok(())
}

#[test]
fn no_ws_ref_no_ws_commit_two_stacks_on_same_commit_ad_hoc_workspace_with_target()
-> anyhow::Result<()> {
    let (_tmp, _, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "no-ws-ref-no-ws-commit-two-branches",
            |_meta| {},
        )?;

    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta),
        standard_traversal_options(),
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> main, origin/main, B, A) A");
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main on e5d0542
    └── ≡:0:main[🌳] <> origin/main →:1: {1}
        └── :0:main[🌳] <> origin/main →:1:
    ");

    // Put "A" into the workspace, creating the workspace ref, but never put a branch related to the target in as well,
    // which is currently checked out with `main`.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
        applied_branches: "[refs/heads/A]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
    └── ≡📙:3:A on e5d0542 {41}
        └── 📙:3:A
    ");

    // No commit was created, as it's not enabled by default, but a ws-ref was created, and it's checked out.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A");

    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/B]",
    }
    "#);
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
    ├── ≡📙:3:A on e5d0542 {41}
    │   └── 📙:3:A
    └── ≡📙:4:B on e5d0542 {42}
        └── 📙:4:B
    ");

    // Nothing changed visibly, still, it's all in the metadata.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A");

    // Cannot put local tracking branch of target into workspace that has it configured.
    for branch in ["refs/heads/main", "refs/remotes/origin/main"] {
        let err = but_workspace::branch::apply(r(branch), &ws, &repo, &mut meta, apply_options())
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
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e5d0542
    └── ≡📙:3:A on e5d0542 {41}
        └── 📙:3:A
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A");

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )?;
    insta::assert_snapshot!(graph_workspace(&out.workspace), "the target's local tracking branch is checked out", @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main on e5d0542
    └── ≡:0:main[🌳] <> origin/main →:1: {1}
        └── :0:main[🌳] <> origin/main →:1:
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "no workspace ref is present anymore", @"* e5d0542 (HEAD -> main, origin/main, B, A) A");

    Ok(())
}

#[test]
fn new_workspace_exists_elsewhere_and_to_be_applied_branch_exists_there() -> anyhow::Result<()> {
    let (_tmp, ws_graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-no-ws-commit-one-stack-one-branch",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
    // The default workspace, it's empty as target is set to `main`.
    insta::assert_snapshot!(graph_workspace(&ws_graph.into_workspace()?), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542");

    // Pretend "B" is checked out (it's at the right state independently of that)
    let (b_id, b_ref) = id_at(&repo, "B");
    let graph = but_graph::Graph::from_commit_traversal(
        b_id,
        b_ref,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Options::default(),
    )?;
    let ws = graph.into_workspace()?;
    // Note how the existing `gitbutler/workspace` disappears, which is expected here.
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:1:B <> ✓!
    └── ≡:1:B {1}
        └── :1:B
            └── ·e5d0542 ►A, ►main
    ");

    // Put "A" into the workspace, hence we want "A" and "B" in it.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/B, refs/heads/A]",
    }
    "#);

    let ws = out.workspace.into_owned();
    // This apply brings both branches into the existing workspace, and it's where HEAD points to.
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    ├── ≡📙:2:B on e5d0542 {42}
    │   └── 📙:2:B
    └── ≡📙:3:A on e5d0542 {41}
        └── 📙:3:A
    ");

    // HEAD must now point to the workspace (that already existed)
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    Ok(())
}

mod unapply_checked_out {
    use but_testsupport::gix_testtools::tempfile::TempDir;

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
        insta::allow_duplicates! {
            insta::assert_snapshot!(initial_graph, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
        }

        git(&repo).args(["checkout", "B"]).run();
        let ws = Graph::from_head(
            &repo,
            &meta,
            but_core::ref_metadata::ProjectMeta::default(),
            standard_traversal_options(),
        )?
        .into_workspace()?;
        insta::allow_duplicates! {
            insta::assert_snapshot!(graph_workspace(&ws), "B is checked out directly, and its stack is virtual", @"
            📕🏘️⚠️:1:gitbutler/workspace <> ✓! on e5d0542
            ├── ≡📙:2:A on e5d0542 {1}
            │   └── 📙:2:A
            └── ≡👉📙:3:B[🌳] on e5d0542 {2}
                └── 👉📙:3:B[🌳]
            ");
        }

        Ok((tmp, repo, meta, ws))
    }

    fn real_stack_tip_checked_out() -> anyhow::Result<Scenario> {
        let (tmp, graph, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph(
                "detached-with-multiple-branches",
                |_meta| {},
            )?;
        let initial_graph = visualize_commit_graph_all(&repo)?;
        insta::allow_duplicates! {
            insta::assert_snapshot!(initial_graph, "the initial checkout is detached, so the workspace has no target ref to fall back to", @"
            * 49d4b34 (A) A1
            | * f57c528 (B) B1
            |/  
            | * aaa195b (HEAD, C) C1
            |/  
            * 3183e43 (main) M1
            ");
        }
        let mut ws = graph.into_workspace()?;
        for branch_to_apply in ["C", "B"] {
            let out = but_workspace::branch::apply(
                Category::LocalBranch
                    .to_full_name(branch_to_apply)?
                    .as_ref(),
                &ws,
                &repo,
                &mut meta,
                apply_options(),
            )?;
            ws = out.workspace.into_owned();
        }
        insta::allow_duplicates! {
            insta::assert_snapshot!(graph_workspace(&ws), "C and B are real stacks in the managed workspace", @"
            📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
            ├── ≡📙:2:C on 3183e43 {43}
            │   └── 📙:2:C
            │       └── ·aaa195b (🏘️)
            └── ≡📙:3:B on 3183e43 {42}
                └── 📙:3:B
                    └── ·f57c528 (🏘️)
            ");
        }

        git(&repo).args(["checkout", "B"]).run();
        let ws = Graph::from_head(
            &repo,
            &meta,
            but_core::ref_metadata::ProjectMeta::default(),
            standard_traversal_options(),
        )?
        .into_workspace()?;
        insta::allow_duplicates! {
            insta::assert_snapshot!(graph_workspace(&ws), "B is checked out directly, and its stack has a real commit", @"
            📕🏘️:1:gitbutler/workspace <> ✓! on 3183e43
            ├── ≡📙:2:C on 3183e43 {43}
            │   └── 📙:2:C
            │       └── ·aaa195b (🏘️)
            └── ≡👉📙:0:B[🌳] on 3183e43 {42}
                └── 👉📙:0:B[🌳]
                    └── ·f57c528 (🏘️)
            ");
        }

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
        insta::assert_debug_snapshot!(out, "the workspace is checked out as the current brnach was unapplied", @r#"
        Outcome {
            workspace_changed: true,
            checked_out: Some(
                "refs/heads/gitbutler/workspace",
            ),
        }
        "#);

        insta::assert_snapshot!(graph_workspace(&out.workspace), "the returned workspace is projected from the managed workspace ref", @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
        └── ≡📙:1:A {1}
            └── 📙:1:A
                └── ·e5d0542 (🏘️) ►main
        ");
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "HEAD switches back to the managed workspace when the checked-out branch is unapplied", @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
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
        insta::assert_debug_snapshot!(out, "the checked-out stack is unapplied, then the remaining stack is checked out directly", @r#"
        Outcome {
            workspace_changed: true,
            checked_out: Some(
                "refs/heads/A",
            ),
        }
        "#);
        insta::assert_snapshot!(graph_workspace(&out.workspace), "the returned workspace is projected from the remaining stack", @"
        ⌂:0:A[🌳] <> ✓!
        └── ≡📙:0:A[🌳] {1}
            └── 📙:0:A[🌳]
                └── ·e5d0542 ►main
        ");
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "HEAD briefly returned to the workspace ref internally, then dissolved it and checked out A (previously virtual)", @"* e5d0542 (HEAD -> A, main, B) A");
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
        let ws = Graph::from_head(
            &repo,
            &meta,
            project_meta(&meta),
            standard_traversal_options(),
        )?
        .into_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), "A is checked out as a lower segment of the B stack", @"
        📕🏘️⚠️:1:gitbutler/workspace <> ✓! on e5d0542
        └── ≡📙:2:B on e5d0542 {2}
            ├── 📙:2:B
            └── 👉📙:3:A[🌳]
        ");

        let out = but_workspace::branch::unapply(
            r("refs/heads/B"),
            &ws,
            &repo,
            &mut meta,
            unapply_options(),
        )?;
        insta::assert_debug_snapshot!(out, "the workspace is checked out as the current branch's stack was unapplied", @r#"
        Outcome {
            workspace_changed: true,
            checked_out: Some(
                "refs/heads/gitbutler/workspace",
            ),
        }
        "#);
        insta::assert_snapshot!(graph_workspace(&out.workspace), "the returned workspace is projected from the managed workspace ref", @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542");
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "HEAD switches back to the managed workspace when the checked-out stack is unapplied", @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
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
        insta::assert_debug_snapshot!(out, "no change in what's checked out", @"
        Outcome {
            workspace_changed: true,
            checked_out: None,
        }
        ");

        insta::assert_snapshot!(graph_workspace(&out.workspace), "the returned workspace preserves the unrelated checked-out entrypoint", @"
        📕🏘️⚠️:1:gitbutler/workspace <> ✓!
        └── ≡👉📙:0:B[🌳] {2}
            └── 👉📙:0:B[🌳]
                └── ·e5d0542 (🏘️) ►main
        ");
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "HEAD remains on B while the managed workspace also contains B", @"* e5d0542 (HEAD -> B, main, gitbutler/workspace, A) A");
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
        insta::assert_debug_snapshot!(out, "the workspace ref is checked out", @r#"
        Outcome {
            workspace_changed: true,
            checked_out: Some(
                "refs/heads/gitbutler/workspace",
            ),
        }
        "#);

        insta::assert_snapshot!(graph_workspace(&out.workspace), "the returned workspace is projected from the managed workspace ref", @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
        └── ≡📙:1:C {43}
            ├── 📙:1:C
            │   └── ·aaa195b (🏘️)
            └── :2:main
                └── ·3183e43 (🏘️)
        ");
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "HEAD switches back to the managed workspace when the checked-out branch is unapplied, without forcing a workspace commit (this is fine, as this is outside of what legacy can do anyway)", @r"
        * 49d4b34 (A) A1
        | * f57c528 (B) B1
        |/  
        | * aaa195b (HEAD -> gitbutler/workspace, C) C1
        |/  
        * 3183e43 (main) M1
        ");
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
        insta::assert_debug_snapshot!(out, "the checked out branch isn't changed", @"
        Outcome {
            workspace_changed: true,
            checked_out: None,
        }
        ");

        insta::assert_snapshot!(graph_workspace(&out.workspace), "the returned workspace preserves the unrelated checked-out entrypoint", @"
        📕🏘️⚠️:1:gitbutler/workspace <> ✓!
        └── ≡👉📙:0:B[🌳] {42}
            ├── 👉📙:0:B[🌳]
            │   └── ·f57c528 (🏘️)
            └── :2:main
                └── ·3183e43 (🏘️)
        ");
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "HEAD remains on B while the managed workspace also contains B", @r"
        * 49d4b34 (A) A1
        | * f57c528 (HEAD -> B, gitbutler/workspace) B1
        |/  
        | * aaa195b (C) C1
        |/  
        * 3183e43 (main) M1
        ");
        Ok(())
    }
}

#[test]
fn apply_multiple_without_target_or_metadata_or_base() -> anyhow::Result<()> {
    let (_tmp, mut graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("one-fork", |meta| {
            meta.data_mut().default_target = None;
        })?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * bf53300 (A) add A
    | * b1540e5 (HEAD -> main) M
    |/  
    | * 0e391b2 (origin/B) add B
    |/  
    * e31e6ca (origin/main, origin/HEAD) add init
    ");

    graph.options.extra_target_commit_id = None;
    let graph = graph.redo_traversal_with_overlay(&repo, &meta, Overlay::default())?;
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
            ├── ·b1540e5
            └── ·e31e6ca
    ");

    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
        applied_branches: "[refs/heads/main, refs/heads/A]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on e31e6ca
    ├── ≡📙:1:main on e31e6ca {1a5}
    │   └── 📙:1:main
    │       └── ·b1540e5 (🏘️)
    └── ≡📙:2:A on e31e6ca {41}
        └── 📙:2:A
            └── ·bf53300 (🏘️)
    ");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   d87b903 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * bf53300 (A) add A
    * | b1540e5 (main) M
    |/  
    | * 0e391b2 (origin/B) add B
    |/  
    * e31e6ca (origin/main, origin/HEAD) add init
    ");

    let branch_b_rt = r("refs/remotes/origin/B");
    let out = but_workspace::branch::apply(branch_b_rt, &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/B]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on e31e6ca
    ├── ≡📙:1:main on e31e6ca {1a5}
    │   └── 📙:1:main
    │       └── ·b1540e5 (🏘️)
    ├── ≡📙:2:A on e31e6ca {41}
    │   └── 📙:2:A
    │       └── ·bf53300 (🏘️)
    └── ≡📙:3:B on e31e6ca {42}
        └── 📙:3:B
            └── ·0e391b2 (🏘️)
    ");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   7bcf528 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \  
    | | * 0e391b2 (origin/B, B) add B
    | * | bf53300 (A) add A
    | |/  
    * / b1540e5 (main) M
    |/  
    * e31e6ca (origin/main, origin/HEAD) add init
    ");

    let out =
        but_workspace::branch::unapply(branch_b_rt, &ws, &repo, &mut meta, unapply_options())?;
    insta::assert_debug_snapshot!(out, "remote tracking branches can't be unapplied, as they aren't applied in the first place", @"
    Outcome {
        workspace_changed: false,
        checked_out: None,
    }
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    insta::assert_debug_snapshot!(out, "the local tracking branch, howeer, will be unapplied", @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on e31e6ca
    ├── ≡📙:1:main on e31e6ca {1a5}
    │   └── 📙:1:main
    │       └── ·b1540e5 (🏘️)
    └── ≡📙:2:A on e31e6ca {41}
        └── 📙:2:A
            └── ·bf53300 (🏘️)
    ");
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
    insta::assert_debug_snapshot!(out, "A is removed and main is checked out", @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "By default, we keep the workspace, and `main` is in there as a target is missing", @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡📙:1:main {1a5}
        └── 📙:1:main
            ├── ·b1540e5 (🏘️)
            └── ·e31e6ca (🏘️)
    ");
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
    let (_tmp, mut graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("one-fork", |meta| {
            meta.data_mut().default_target = None;
        })?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * bf53300 (A) add A
    | * b1540e5 (HEAD -> main) M
    |/  
    | * 0e391b2 (origin/B) add B
    |/  
    * e31e6ca (origin/main, origin/HEAD) add init
    ");
    graph.options.extra_target_commit_id = None;
    let graph = graph.redo_traversal_with_overlay(&repo, &meta, Overlay::default())?;
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
            ├── ·b1540e5
            └── ·e31e6ca
    ");
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    let ws = out.workspace.into_owned();
    let out = but_workspace::branch::apply(
        r("refs/remotes/origin/B"),
        &ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on e31e6ca
    ├── ≡📙:1:main on e31e6ca {1a5}
    │   └── 📙:1:main
    │       └── ·b1540e5 (🏘️)
    ├── ≡📙:2:A on e31e6ca {41}
    │   └── 📙:2:A
    │       └── ·bf53300 (🏘️)
    └── ≡📙:3:B on e31e6ca {42}
        └── 📙:3:B
            └── ·0e391b2 (🏘️)
    ");

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
    insta::assert_debug_snapshot!(err, @r#"
    Context {
        code: PreconditionFailed,
        message: Some(
            "Uncommitted files would be overwritten by checkout: \"B\"",
        ),
    }
    "#);

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
    let ws_after = but_graph::Graph::from_head(
        &repo,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_traversal_options(),
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
        ws.is_reachable_from_entrypoint(r("refs/heads/B")),
        "fixture must start with B visible in the cached workspace graph"
    );

    let mut ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    for stack in &mut ws_md.stacks {
        stack.workspacecommit_relation = Outside;
    }
    meta.set_workspace(&ws_md)?;

    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, apply_options())?;
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * f1889e7 (A2) add A2
    * 7de99e1 (A1) add A1
    | * 53ad0c2 (unrelated) add U1
    |/  
    * 3183e43 (HEAD -> main, origin/main) M1
    ");

    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main on 3183e43
    └── ≡:0:main[🌳] <> origin/main →:1: {1}
        └── :0:main[🌳] <> origin/main →:1:
    ");

    assert_eq!(
        apply_options().workspace_merge,
        WorkspaceMerge::MergeIfNeeded
    );

    // Add another stack to be sure we correctly handle the removal of existing stacks later (i.e. don't get the index wrong)
    let out = but_workspace::branch::apply(
        r("refs/heads/unrelated"),
        &ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
        applied_branches: "[refs/heads/unrelated]",
    }
    "#);
    // TODO: should this not avoid creating a workspace commit? Yes.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * f1889e7 (A2) add A2
    * 7de99e1 (A1) add A1
    | * 6848743 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 53ad0c2 (unrelated) add U1
    |/  
    * 3183e43 (origin/main, main) M1
    ");

    let ws = out.workspace.into_owned();

    let out =
        but_workspace::branch::apply(r("refs/heads/A1"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/A1]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:3:unrelated on 3183e43 {3c4}
    │   └── 📙:3:unrelated
    │       └── ·53ad0c2 (🏘️)
    └── ≡📙:4:A1 on 3183e43 {72}
        └── 📙:4:A1
            └── ·7de99e1 (🏘️)
    ");

    let out = but_workspace::branch::apply(
        r("refs/heads/A2"),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            // TODO: remove this, use default options when they are the default.
            on_workspace_conflict: OnWorkspaceMergeConflict::MaterializeAndReportConflictingStacks,
            ..apply_options()
        },
    )?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/A2]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:3:unrelated on 3183e43 {3c4}
    │   └── 📙:3:unrelated
    │       └── ·53ad0c2 (🏘️)
    └── ≡📙:4:A2 on 3183e43 {73}
        ├── 📙:4:A2
        │   └── ·f1889e7 (🏘️)
        └── 📙:5:A1
            └── ·7de99e1 (🏘️)
    ");

    // The metadata is in sync, and A1 is outside the workspace.
    insta::assert_debug_snapshot!(ws.metadata, @r#"
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
    "#);

    let out = but_workspace::branch::unapply(
        r("refs/heads/A2"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    insta::assert_debug_snapshot!(out, @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "A2 is removed, along with A1", @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    └── ≡📙:3:unrelated on 3183e43 {3c4}
        └── 📙:3:unrelated
            └── ·53ad0c2 (🏘️)
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/A1"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    insta::assert_debug_snapshot!(out, "this is a no-op, A1 was removed with A2 prior", @"
    Outcome {
        workspace_changed: false,
        checked_out: None,
    }
    ");
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "nothing changed, this was a no-op" , @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    └── ≡📙:3:unrelated on 3183e43 {3c4}
        └── 📙:3:unrelated
            └── ·53ad0c2 (🏘️)
    ");

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
    insta::assert_debug_snapshot!(out, @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43");

    insta::assert_snapshot!(
        visualize_commit_graph_all(&repo)?, "",
        @"
    * f1889e7 (A2) add A2
    * 7de99e1 (A1) add A1
    | * 53ad0c2 (unrelated) add U1
    |/  
    * 3183e43 (HEAD -> gitbutler/workspace, origin/main, main) M1
    "
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 49d4b34 (A) A1
    | * f57c528 (B) B1
    |/  
    | * aaa195b (HEAD, C) C1
    |/  
    * 3183e43 (main) M1
    ");
    let ws = graph
        .into_workspace()
        .expect("detached graph is a workspace");
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:DETACHED <> ✓! on 3183e43
    └── ≡:0:anon: on 3183e43 {1}
        └── :0:anon:
            └── ·aaa195b ►C
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    insta::assert_debug_snapshot!(out, "it's a noop, the workspace didn't change, the branch was never applied", @"
    Outcome {
        workspace_changed: false,
        checked_out: None,
    }
    ");
    Ok(())
}

#[test]
fn unapply_branch_from_detached_ad_hoc_workspace_is_an_error() -> anyhow::Result<()> {
    let (_tmp, _, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "single-stack-two-segments",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * f1889e7 (A2) add A2
    * 7de99e1 (A1) add A1
    | * 53ad0c2 (unrelated) add U1
    |/  
    * 3183e43 (HEAD -> main, origin/main) M1
    ");

    let a2_id = repo.rev_parse_single("A2")?.detach();
    let ws = Graph::from_commit_traversal_tips(
        &repo,
        [Tip::detached_entrypoint(a2_id)],
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_traversal_options(),
    )?
    .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:DETACHED <> ✓!
    └── ≡:0:anon: {1}
        ├── :0:anon:
        │   └── ·f1889e7 ►A2
        ├── :1:A1
        │   └── ·7de99e1
        └── :2:main[🌳]
            └── ·3183e43
    ");

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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 49d4b34 (A) A1
    | * f57c528 (B) B1
    |/  
    | * aaa195b (HEAD, C) C1
    |/  
    * 3183e43 (main) M1
    ");
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:DETACHED <> ✓! on 3183e43
    └── ≡:0:anon: on 3183e43 {1}
        └── :0:anon:
            └── ·aaa195b ►C
    ");

    let out =
        but_workspace::branch::apply(r("refs/heads/C"), &ws, &repo, &mut meta, apply_options())?;

    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
        applied_branches: "[refs/heads/C]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "the actual HEAD is ignored, and we only see C (instead of C + HEAD-ref)", @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    └── ≡📙:2:C on 3183e43 {43}
        └── 📙:2:C
            └── ·aaa195b (🏘️)
    ");
    // A new workspace reference was created, and checked out, without enforcing a workspace commit
    // as there is no need.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 49d4b34 (A) A1
    | * f57c528 (B) B1
    |/  
    | * aaa195b (HEAD -> gitbutler/workspace, C) C1
    |/  
    * 3183e43 (main) M1
    ");

    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, apply_options())?;

    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/B]",
    }
    "#);
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 49d4b34 (A) A1
    | *   fdec130 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | |\  
    | | * f57c528 (B) B1
    | |/  
    |/|   
    | * aaa195b (C) C1
    |/  
    * 3183e43 (main) M1
    ");

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:2:C on 3183e43 {43}
    │   └── 📙:2:C
    │       └── ·aaa195b (🏘️)
    └── ≡📙:3:B on 3183e43 {42}
        └── 📙:3:B
            └── ·f57c528 (🏘️)
    ");

    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            // Make 'A' appear at the front.
            order: Some(0),
            ..apply_options()
        },
    )?;

    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/A]",
    }
    "#);
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:2:A on 3183e43 {41}
    │   └── 📙:2:A
    │       └── ·49d4b34 (🏘️)
    ├── ≡📙:3:C on 3183e43 {43}
    │   └── 📙:3:C
    │       └── ·aaa195b (🏘️)
    └── ≡📙:4:B on 3183e43 {42}
        └── 📙:4:B
            └── ·f57c528 (🏘️)
    ");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   951ff29 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \  
    | | * f57c528 (B) B1
    | * | aaa195b (C) C1
    | |/  
    * / 49d4b34 (A) A1
    |/  
    * 3183e43 (main) M1
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    insta::assert_debug_snapshot!(out, @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "A was removed", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:2:C on 3183e43 {43}
    │   └── 📙:2:C
    │       └── ·aaa195b (🏘️)
    └── ≡📙:3:B on 3183e43 {42}
        └── 📙:3:B
            └── ·f57c528 (🏘️)
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    insta::assert_debug_snapshot!(out, @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "B was removed", @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    └── ≡📙:2:C on 3183e43 {43}
        └── 📙:2:C
            └── ·aaa195b (🏘️)
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/C"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )
    .expect("C can be removed");
    insta::assert_debug_snapshot!(out, @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "the workspace reference remains checked out and empty because no non-stack fallback exists", @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43");
    Ok(())
}

#[test]
fn unapply_workspace_ref_without_target_checks_out_named_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "detached-with-multiple-branches",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the initial checkout is detached, so the workspace has no target ref to fall back to", @"
    * 49d4b34 (A) A1
    | * f57c528 (B) B1
    |/  
    | * aaa195b (HEAD, C) C1
    |/  
    * 3183e43 (main) M1
    ");
    let mut ws = graph.into_workspace()?;

    for branch_to_apply in ["C", "B"] {
        let out = but_workspace::branch::apply(
            Category::LocalBranch
                .to_full_name(branch_to_apply)?
                .as_ref(),
            &ws,
            &repo,
            &mut meta,
            apply_options(),
        )?;
        ws = out.workspace.into_owned();
    }

    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            order: Some(0),
            ..apply_options()
        },
    )?;
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "the workspace has named stacks but no target ref", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:2:A on 3183e43 {41}
    │   └── 📙:2:A
    │       └── ·49d4b34 (🏘️)
    ├── ≡📙:3:C on 3183e43 {43}
    │   └── 📙:3:C
    │       └── ·aaa195b (🏘️)
    └── ≡📙:4:B on 3183e43 {42}
        └── 📙:4:B
            └── ·f57c528 (🏘️)
    ");

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
    insta::assert_debug_snapshot!(out, "without a target ref, unapplying the workspace ref checks out the named stack with the lowest generation", @r#"
    Outcome {
        workspace_changed: true,
        checked_out: Some(
            "refs/heads/A",
        ),
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "the projection shows the checked-out named stack", @"
    ⌂:0:A[🌳] <> ✓! on 3183e43
    └── ≡:0:A[🌳] on 3183e43 {1}
        └── :0:A[🌳]
            └── ·49d4b34
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the managed workspace ref is deleted and HEAD points to the fallback stack", @"
    * 49d4b34 (HEAD -> A) A1
    | * f57c528 (B) B1
    |/  
    | * aaa195b (C) C1
    |/  
    * 3183e43 (main) M1
    ");

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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the fixture starts on a conflicted main commit", @"
    * 8450331 (HEAD -> main, tag: conflicted) GitButler WIP Commit
    * a047f81 (tag: normal) init
    ");

    git(&repo)
        .args(["branch", "tip-conflicted", "conflicted"])
        .run();
    git(&repo).args(["reset", "--hard", "normal"]).run();

    let ws = Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta),
        standard_traversal_options(),
    )?
    .into_workspace()?;
    let out = but_workspace::branch::apply(
        r("refs/heads/tip-conflicted"),
        &ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "the target-less workspace can contain a conflicted named stack", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡📙:1:tip-conflicted {595}
        ├── 📙:1:tip-conflicted
        │   └── ·8450331 (🏘️) ►tags/conflicted
        └── 📙:2:main
            └── ·a047f81 (🏘️) ►tags/normal
    ");

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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the failed unapply leaves HEAD and the workspace ref unchanged", @"
    * 8a0bbba (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8450331 (tag: conflicted, tip-conflicted) GitButler WIP Commit
    * a047f81 (tag: normal, main) init
    ");
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (HEAD -> main, origin/main) M
    ");

    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡:0:main[🌳] <> origin/main →:1: {1}
        └── :0:main[🌳] <> origin/main →:1:
    ");

    // Apply the dependent branch, to bring in only the dependent branch
    let out =
        but_workspace::branch::apply(r("refs/heads/E"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
        applied_branches: "[refs/heads/E]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:E on 85efbe4 {1}
        └── 📙:4:E
            └── ·7076dee (🏘️) ►D
    ");

    // Apply the former tip of the stack, to create a new stack. Note how it won't double-list the
    // other stack.
    let out =
        but_workspace::branch::apply(r("refs/heads/C"), &ws, &repo, &mut meta, apply_options())?;
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:E on 85efbe4 {1}
    │   └── 📙:5:E
    │       └── ·7076dee (🏘️) ►D
    └── ≡📙:6:C on 7076dee {43}
        └── 📙:6:C
            └── ·f084d61 (🏘️) ►A, ►B
    ");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   78f3659 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * f084d61 (C, B, A) A2
    |/  
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    // Adding `B` as tip of an unapplied stack brings in the whole stack.
    // BUT: Currently it overrides the previous stack C, which points to the same commit, and avoids any merge!
    // Accepting this behaviour for now as it's quite rare to have such ambiguity, even though I'd love if one day
    // for this to just work as people might intuitively want, even if that means the same commit is used multiple times.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, apply_options())?;
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:E on 85efbe4 {1}
    │   └── 📙:5:E
    │       └── ·7076dee (🏘️) ►D
    └── ≡📙:6:B on 7076dee {2}
        └── 📙:6:B
            └── ·f084d61 (🏘️) ►A, ►C
    ");

    // Applying C again… works, but it's creating a dependent stack.
    // This is what happens because we notice that C can't be applied as independent stack due to the graph algorithm,
    // and then it tries it a dependent stack, which should always work.
    let out =
        but_workspace::branch::apply(r("refs/heads/C"), &ws, &repo, &mut meta, apply_options())
            .unwrap();
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:E on 85efbe4 {1}
    │   └── 📙:5:E
    │       └── ·7076dee (🏘️) ►D
    └── ≡📙:6:C on 7076dee {2}
        ├── 📙:6:C
        └── 📙:7:B
            └── ·f084d61 (🏘️) ►A
    ");

    Ok(())
}

#[test]
fn apply_two_ambiguous_stacks_with_target() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "no-ws-ref-stack-and-dependent-branch",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (HEAD -> main, origin/main) M
    ");

    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡:0:main[🌳] <> origin/main →:1: {1}
        └── :0:main[🌳] <> origin/main →:1:
    ");

    // Apply `A` first.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
        applied_branches: "[refs/heads/A]",
    }
    "#);
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:3:A on 85efbe4 {41}
        └── 📙:3:A
            ├── ·f084d61 (🏘️) ►B, ►C
            └── ·7076dee (🏘️) ►D, ►E
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 773e030 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    // Apply `B` - the only sane way is to make it its own stack, but allow it to diverge.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/B]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:B on 85efbe4 {41}
        ├── 📙:4:B
        └── 📙:5:A
            ├── ·f084d61 (🏘️) ►C
            └── ·7076dee (🏘️) ►D, ►E
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    // What follows is a bit wonky, but for now is here to document what happens in a complex scenario.
    let out =
        but_workspace::branch::apply(r("refs/heads/C"), &ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    insta::assert_debug_snapshot!(out, "applying C succeeds and updates the workspace metadata", @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/C]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "C is recorded as another segment at the same tip in the B/A stack", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:B on 85efbe4 {41}
        ├── 📙:4:B
        ├── 📙:5:C
        └── 📙:6:A
            ├── ·f084d61 (🏘️)
            └── ·7076dee (🏘️) ►D, ►E
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "adding C only changes workspace metadata, not Git refs or objects", @"
    * b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    let out =
        but_workspace::branch::apply(r("refs/heads/D"), &ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    insta::assert_debug_snapshot!(out, "applying D succeeds and updates the workspace metadata", @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/D]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "D becomes a lower segment in the same projected stack, leaving E as the remaining alternate ref at that commit", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:5:B on 85efbe4 {41}
        ├── 📙:5:B
        ├── 📙:6:C
        ├── 📙:7:A
        │   └── ·f084d61 (🏘️)
        └── 📙:4:D
            └── ·7076dee (🏘️) ►E
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "adding D also remains metadata-only even though the stack presentation changes", @"
    * b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    let out =
        but_workspace::branch::apply(r("refs/heads/E"), &ws, &repo, &mut meta, apply_options())
            .expect("apply actually works");
    insta::assert_debug_snapshot!(out, "applying E forces the lower same-commit branch pair into its own stack", @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/E]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "applying all ambiguous dependent branches ends with B/C/A and E/D split into two stacks", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:B {41}
    │   ├── 📙:5:B
    │   ├── 📙:6:C
    │   └── 📙:7:A
    │       └── ·f084d61 (🏘️)
    └── ≡📙:8:E on 85efbe4 {44}
        ├── 📙:8:E
        └── 📙:9:D
            └── ·7076dee (🏘️)
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "future improvement: it should most definitely not create a merge if both tips have shared in-workspace commits in their ancestry", @r"
    *   2c125a1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | f084d61 (C, B, A) A2
    |/  
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/E"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    insta::assert_debug_snapshot!(out, "unapplying E removes the E/D stack and keeps the B/C/A stack applied", @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");
    assert!(
        out.workspace_merge.is_some(),
        "legacy mode should rebuild the workspace commit after removing E/D"
    );

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "after unapplying E, legacy mode keeps a workspace commit", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:5:B on 85efbe4 {41}
        ├── 📙:5:B
        ├── 📙:6:C
        ├── 📙:7:A
        │   └── ·f084d61 (🏘️)
        └── 📙:4:D
            └── ·7076dee (🏘️)
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "after unapplying E, the workspace commit contains only the remaining B/C/A stack", @"
    * b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/D"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    insta::assert_debug_snapshot!(out, "unapplying D after E is already gone is a no-op", @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "the B/C/A stack stays applied after the D no-op", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:5:B on 85efbe4 {41}
        ├── 📙:5:B
        ├── 📙:6:C
        ├── 📙:7:A
        │   └── ·f084d61 (🏘️)
        └── 📙:4:E
            └── ·7076dee (🏘️)
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the Git graph is unchanged after the D no-op", @"
    * b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/C"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    insta::assert_debug_snapshot!(out, "unapplying C removes that middle metadata segment from B/C/A", @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "after unapplying C, B/A/E remains applied with D only as an ambiguous ref on E's tip", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:5:B on 85efbe4 {41}
        ├── 📙:5:B
        ├── 📙:6:A
        │   └── ·f084d61 (🏘️)
        └── 📙:4:E
            └── ·7076dee (🏘️) ►D
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the Git graph is unchanged after removing C from metadata", @"
    * b390237 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    insta::assert_debug_snapshot!(out, "unapplying B removes the remaining B/A applied stack", @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "after unapplying B, no applied stacks remain", @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "after unapplying B, there are no mergeable stacks left so legacy mode keeps an empty workspace commit", @"
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    | * bde8ed6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        legacy_unapply_options(),
    )
    .expect("unapply actually works");
    insta::assert_debug_snapshot!(out, "unapplying A after B removed the remaining stack is a no-op", @"
    Outcome {
        workspace_changed: false,
        checked_out: None,
    }
    ");

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "the workspace stays empty after the A no-op", @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the Git graph is unchanged after the A no-op", @"
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    | * bde8ed6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    let out = but_workspace::branch::unapply(
        r("refs/heads/gitbutler/workspace"),
        &ws,
        &repo,
        &mut meta,
        unapply_options_with(WorkspaceDisposition::PreventUnnecessaryWorkspaceReferences),
    )
    .expect("workspace ref can be unapplied by checking out a named target");
    insta::assert_debug_snapshot!(out, "unapplying the workspace ref switches to the target's local branch when the disposition allows deleting the workspace ref", @r#"
    Outcome {
        workspace_changed: true,
        checked_out: Some(
            "refs/heads/main",
        ),
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "the projection is the checked-out target branch", @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡:0:main[🌳] <> origin/main →:1: {1}
        └── :0:main[🌳] <> origin/main →:1:
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, "the managed workspace ref is deleted and HEAD points to main", @"
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (HEAD -> main, origin/main) M
    ");

    Ok(())
}

#[test]
fn apply_with_conflicts_shows_exact_conflict_info() -> anyhow::Result<()> {
    let (_tmp, _graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "various-heads-for-multi-line-merge-conflict",
            |_meta| {},
        )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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
    ");

    git(&repo).args(["checkout", "main"]).run();
    git(&repo)
        .args(["branch", "-d", "gitbutler/workspace"])
        .run();
    // The fixture helper created `graph` while `HEAD` still pointed to `conflict-hero`.
    // Replaying that graph would correctly keep using `conflict-hero` as the traversal
    // entrypoint, even though the test just checked out `main`. Build the graph from
    // the current repository state so the workspace under test starts at `main`.
    let mut ws = but_graph::Graph::from_head(
        &repo,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Options {
            extra_target_commit_id: repo.rev_parse_single("main").ok().map(|id| id.detach()),
            ..Options::limited()
        },
    )?
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
            &ws,
            &repo,
            &mut meta,
            apply_options(),
        )
        .unwrap_or_else(|err| panic!("{branch_to_apply}: {err}"));
        ws = out.workspace.into_owned();
    }

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on 85efbe4
    ├── ≡📙:7:main on 85efbe4 {1a5}
    │   └── 📙:7:main
    ├── ≡📙:2:clean-A on 85efbe4 {271}
    │   └── 📙:2:clean-A
    │       └── ·d3cce74 (🏘️)
    ├── ≡📙:3:conflict-F1 on 85efbe4 {3f6}
    │   └── 📙:3:conflict-F1
    │       └── ·bf09eae (🏘️)
    ├── ≡📙:4:clean-B on 85efbe4 {272}
    │   └── 📙:4:clean-B
    │       └── ·115e41b (🏘️)
    ├── ≡📙:5:conflict-F2 on 85efbe4 {3f7}
    │   └── 📙:5:conflict-F2
    │       └── ·f2ce66d (🏘️)
    └── ≡📙:6:clean-C on 85efbe4 {273}
        └── 📙:6:clean-C
            └── ·34c4591 (🏘️)
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
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
    ");

    let out = but_workspace::branch::apply(
        r("refs/heads/conflict-hero"),
        &ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    insta::assert_snapshot!(sanitize_uuids_and_timestamps(format!("{out:#?}")), @r#"
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
    "#);
    let ws = out.workspace.into_owned();
    // By default, it fails and just reports the conflicting stacks, so it's the same as it was before.
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on 85efbe4
    ├── ≡📙:8:main on 85efbe4 {1a5}
    │   └── 📙:8:main
    ├── ≡📙:2:clean-A on 85efbe4 {271}
    │   └── 📙:2:clean-A
    │       └── ·d3cce74 (🏘️)
    ├── ≡📙:3:conflict-F1 on 85efbe4 {3f6}
    │   └── 📙:3:conflict-F1
    │       └── ·bf09eae (🏘️)
    ├── ≡📙:4:clean-B on 85efbe4 {272}
    │   └── 📙:4:clean-B
    │       └── ·115e41b (🏘️)
    ├── ≡📙:5:conflict-F2 on 85efbe4 {3f7}
    │   └── 📙:5:conflict-F2
    │       └── ·f2ce66d (🏘️)
    └── ≡📙:6:clean-C on 85efbe4 {273}
        └── 📙:6:clean-C
            └── ·34c4591 (🏘️)
    ");
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
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            on_workspace_conflict: OnWorkspaceMergeConflict::MaterializeAndReportConflictingStacks,
            ..apply_options()
        },
    )?;
    // It does still report conflicts.
    insta::assert_snapshot!(sanitize_uuids_and_timestamps(format!("{out:#?}")), @r#"
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
    "#);

    // Now the other stacks are unapplied.
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on 85efbe4
    ├── ≡📙:6:main on 85efbe4 {1a5}
    │   └── 📙:6:main
    ├── ≡📙:2:clean-A on 85efbe4 {271}
    │   └── 📙:2:clean-A
    │       └── ·d3cce74 (🏘️)
    ├── ≡📙:3:clean-B on 85efbe4 {272}
    │   └── 📙:3:clean-B
    │       └── ·115e41b (🏘️)
    ├── ≡📙:4:clean-C on 85efbe4 {273}
    │   └── 📙:4:clean-C
    │       └── ·34c4591 (🏘️)
    └── ≡📙:5:conflict-hero on 85efbe4 {52d}
        └── 📙:5:conflict-hero
            ├── ·4bbb93c (🏘️)
            └── ·98519e9 (🏘️)
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
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
    ");

    let ws_md = sanitize_uuids_and_timestamps(format!(
        "{:#?}",
        ws.metadata
            .as_ref()
            .expect("managed workspace has metadata")
    ));
    insta::assert_snapshot!(ws_md, @r#"
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
    "#);

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
    let graph_snapshot = visualize_commit_graph_all(&repo)?.replace("|/  ", "|/");
    insta::assert_snapshot!(graph_snapshot, @r"
    * bf53300 (A) add A
    | * 543911c (add-A-too) add a different A
    | * b1540e5 (HEAD -> main) M
    |/
    | * 0e391b2 (origin/B) add B
    |/
    * e31e6ca (origin/main, origin/HEAD) add init
    ");

    let ws = graph.into_workspace()?;
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "A is applied before trying the conflicting sibling branch", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e31e6ca
    └── ≡📙:3:A on e31e6ca {41}
        └── 📙:3:A
            └── ·bf53300 (🏘️)
    ");
    let refs_before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(refs_before, @"
    * 543911c (add-A-too) add a different A
    * b1540e5 (main) M
    | * 9f5b797 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * bf53300 (A) add A
    |/  
    | * 0e391b2 (origin/B) add B
    |/  
    * e31e6ca (origin/main, origin/HEAD) add init
    ");

    let out = but_workspace::branch::apply(
        r("refs/heads/add-A-too"),
        &ws,
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
    insta::assert_snapshot!(sanitize_uuids_and_timestamps(format!("{out:#?}")), "a conflict-aborted apply must not report branches as applied", @r#"
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
    "#);
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

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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
    ");
    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓! on 85efbe4
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
    ");
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
            &ws,
            &repo,
            &mut meta,
            but_workspace::branch::apply::Options {
                on_workspace_conflict:
                    OnWorkspaceMergeConflict::MaterializeAndReportConflictingStacks,
                ..apply_options()
            },
        )?;
        ws = out.workspace.into_owned();
    }
    insta::assert_snapshot!(graph_workspace(&ws), "all branches are applied", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on 85efbe4
    ├── ≡📙:6:main on 85efbe4 {1a5}
    │   └── 📙:6:main
    ├── ≡📙:2:clean-A on 85efbe4 {271}
    │   └── 📙:2:clean-A
    │       └── ·d3cce74 (🏘️)
    ├── ≡📙:3:clean-B on 85efbe4 {272}
    │   └── 📙:3:clean-B
    │       └── ·115e41b (🏘️)
    ├── ≡📙:4:clean-C on 85efbe4 {273}
    │   └── 📙:4:clean-C
    │       └── ·34c4591 (🏘️)
    └── ≡📙:5:conflict-hero on 85efbe4 {52d}
        └── 📙:5:conflict-hero
            ├── ·4bbb93c (🏘️)
            └── ·98519e9 (🏘️)
    ");

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

    insta::assert_snapshot!(graph_workspace(&ws), "the workspace ref remains because the regenerated workspace still has main available", @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 85efbe4
    └── ≡📙:2:main on 85efbe4 {1a5}
        └── 📙:2:main
    ");

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

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    ├── ≡📙:2:A on e5d0542 {1}
    │   └── 📙:2:A
    └── ≡📙:3:B on e5d0542 {2}
        └── 📙:3:B
    ");

    // Apply the workspace ref itself, it's a no-op
    let out = but_workspace::branch::apply(
        r("refs/heads/gitbutler/workspace"),
        &ws,
        &repo,
        &mut meta,
        apply_options(),
    )?;
    insta::assert_debug_snapshot!(out, "nothing actually changed, so nothing is mentioned", @r#"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
        applied_branches: "[]",
    }
    "#);

    let (b_id, b_ref) = id_at(&repo, "B");
    let ws = but_graph::Graph::from_commit_traversal(
        b_id,
        b_ref.clone(),
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_traversal_options_with_extra_target(&repo),
    )?
    .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓! on e5d0542
    ├── ≡📙:2:A on e5d0542 {1}
    │   └── 📙:2:A
    └── ≡👉📙:3:B on e5d0542 {2}
        └── 👉📙:3:B
    ");
    // Already applied (the HEAD points to it).
    let out = but_workspace::branch::apply(b_ref.as_ref(), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, "no-ops aren't listing the already applied branches", @r#"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
        applied_branches: "[]",
    }
    "#);

    // Simulate stale metadata: the cached graph still contains A, but metadata says it is outside.
    let mut ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    let (stack_idx, _) = ws_md
        .find_owner_indexes_by_name(r("refs/heads/A"), StackKind::AppliedAndUnapplied)
        .expect("A is in metadata");
    ws_md.stacks[stack_idx].workspacecommit_relation = Outside;
    meta.set_workspace(&ws_md)?;

    // To apply A, we checkout the surrounding workspace and repair the stale metadata.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "enclosed branches should report a successful apply"
    );
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/A]",
    }
    "#);
    let ws_md = meta.workspace(r(WORKSPACE_REF_NAME))?;
    assert!(
        ws_md
            .find_branch(r("refs/heads/A"), StackKind::Applied)
            .is_some(),
        "apply should repair stale enclosing-branch metadata"
    );

    // Now the workspace ref itself is checked out.
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    ├── ≡📙:2:A on e5d0542 {1}
    │   └── 📙:2:A
    └── ≡📙:3:B on e5d0542 {2}
        └── 📙:3:B
    ");
    // Even though the real repo seemingly didn't change, after all, our entrypoint was just 'virtual'.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    // make "A" an applied dependent branch that is included in B so apply will do nothing.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &["A"]);

    let (b_id, b_ref) = id_at(&repo, "B");

    let ws = but_graph::Graph::from_commit_traversal(
        b_id,
        b_ref.clone(),
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_traversal_options_with_extra_target(&repo),
    )?
    .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), "V-branch B is checked out", @"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓! on e5d0542
    └── ≡👉📙:2:B on e5d0542 {2}
        ├── 👉📙:2:B
        └── 📙:3:A
    ");

    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, "Nothing changed, the desired branch was already applied", @r#"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
        applied_branches: "[]",
    }
    "#);

    // There is no known branch, and adding it will just add metadata.
    meta.data_mut().branches.clear();
    let ws = but_graph::Graph::from_head(
        &repo,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_traversal_options_with_extra_target(&repo),
    )?
    .into_workspace()?;
    // There is nothing yet.
    insta::assert_snapshot!(graph_workspace(&ws), "metadata defines no branches", @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542");

    // Apply the first branch, it must be independent.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "already-visible branches should report a successful apply"
    );
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/A]",
    }
    "#);
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    └── ≡📙:2:A on e5d0542 {41}
        └── 📙:2:A
    ");

    // Apply the first branch, it must be independent.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/B]",
    }
    "#);
    insta::assert_snapshot!(graph_workspace(&out.workspace), "B is added as independent stack", @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    ├── ≡📙:2:A on e5d0542 {41}
    │   └── 📙:2:A
    └── ≡📙:3:B on e5d0542 {42}
        └── 📙:3:B
    ");

    let (b_id, b_ref) = id_at(&repo, "B");
    let ws = but_graph::Graph::from_commit_traversal(
        b_id,
        b_ref.clone(),
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_traversal_options_with_extra_target(&repo),
    )?
    .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), "the same result when checked out directly", @"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓! on e5d0542
    ├── ≡📙:2:A on e5d0542 {41}
    │   └── 📙:2:A
    └── ≡👉📙:3:B on e5d0542 {42}
        └── 👉📙:3:B
    ");

    let ws = out.workspace.into_owned();
    let out = but_workspace::branch::unapply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    insta::assert_debug_snapshot!(out, @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "the workspace is empty again", @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542
    └── ≡📙:2:B on e5d0542 {42}
        └── 📙:2:B
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    let out = but_workspace::branch::unapply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        unapply_options(),
    )?;
    insta::assert_debug_snapshot!(out, @"
    Outcome {
        workspace_changed: true,
        checked_out: None,
    }
    ");
    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), "the workspace is empty again", @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on e5d0542");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
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

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    * | c813d8d (B) B
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:A on 85efbe4 {1}
    │   └── 📙:3:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:4:B on 85efbe4 {2}
        └── 📙:4:B
            └── ·c813d8d (🏘️)
    ");

    // Apply the workspace ref itself, it's a no-op
    let ws_ref = r("refs/heads/gitbutler/workspace");
    let out = but_workspace::branch::apply(ws_ref, &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, "the workspace ref itself counts as no-op as well", @r#"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
        applied_branches: "[]",
    }
    "#);

    let (b_id, b_ref) = id_at(&repo, "B");
    let ws = but_graph::Graph::from_commit_traversal(
        b_id,
        b_ref.clone(),
        &meta,
        project_meta(&meta),
        but_graph::init::Options::default(),
    )?
    .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:A on 85efbe4 {1}
    │   └── 📙:4:A
    │       └── ·09d8e52 (🏘️)
    └── ≡👉📙:0:B on 85efbe4 {2}
        └── 👉📙:0:B
            └── ·c813d8d (🏘️)
    ");

    // Already applied (the HEAD points to it, it literally IS the workspace).
    let out = but_workspace::branch::apply(b_ref.as_ref(), &ws, &repo, &mut meta, apply_options())?;
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
        applied_branches: "[]",
    }
    "#);

    let err =
        but_workspace::branch::apply(ws_ref, &ws, &repo, &mut meta, apply_options()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to apply a reference that already is a workspace: 'gitbutler/workspace'",
        "it's never good to merge one managed workspace into another, and we just disallow it.\
         Note that we could also check it out."
    );

    // To apply, we just checkout the surrounding workspace.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, apply_options())?;
    assert_eq!(
        out.status,
        OutcomeStatus::Applied,
        "enclosed branches with commits should report a successful apply"
    );
    insta::assert_debug_snapshot!(out, @r#"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
        applied_branches: "[refs/heads/A]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:A on 85efbe4 {1}
    │   └── 📙:3:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:4:B on 85efbe4 {2}
        └── 📙:4:B
            └── ·c813d8d (🏘️)
    ");
    Ok(())
}

#[test]
fn apply_nonexisting_branch_failure() -> anyhow::Result<()> {
    let (repo, mut meta) =
        named_read_only_in_memory_scenario("ws-ref-no-ws-commit-one-stack-one-branch", "")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    meta.data_mut()
        .default_target
        .as_mut()
        .expect("workspace configured")
        .sha = gix::hash::Kind::Sha1.null();
    let graph = but_graph::Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Options::limited(),
    )?;
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:anon:
        └── :1:anon:
            └── ·e5d0542 (🏘️) ►A, ►B, ►main
    ");

    let err = but_workspace::branch::apply(
        r("refs/heads/does-not-exist"),
        &ws,
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
    Ok(())
}

#[test]
fn unapply_nonexisting_branch() -> anyhow::Result<()> {
    let (repo, mut meta) =
        named_read_only_in_memory_scenario("ws-ref-no-ws-commit-one-stack-one-branch", "")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    meta.data_mut()
        .default_target
        .as_mut()
        .expect("workspace configured")
        .sha = gix::hash::Kind::Sha1.null();
    let graph = but_graph::Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Options::limited(),
    )?;
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:anon:
        └── :1:anon:
            └── ·e5d0542 (🏘️) ►A, ►B, ►main
    ");

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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
    Ok(())
}

#[test]
fn unborn_apply_needs_base() -> anyhow::Result<()> {
    let (repo, mut meta) =
        named_read_only_in_memory_scenario("unborn-empty-detached-remote", "unborn")?;
    // Depending on the Git version it produces`* 3183e43 (orphan/main, orphan/HEAD) M1` on CI,
    // so a comment is used as reference.
    // insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (orphan/main) M1");

    let graph = but_graph::Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Options::limited(),
    )?;
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
    ");

    // Idempotency in ad-hoc workspace
    let out = but_workspace::branch::apply(
        r("refs/heads/main"),
        &ws,
        &repo,
        &mut *meta,
        apply_options(),
    )?;
    insta::assert_debug_snapshot!(out, "the HEAD is already at 'main', so nothing changes", @r#"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
        applied_branches: "[]",
    }
    "#);

    // Cannot apply branch without a base,
    // but since remote is transformed into a local tracking branch, it's a noop.
    let out = but_workspace::branch::apply(
        r("refs/remotes/orphan/main"),
        &ws,
        &repo,
        &mut *meta,
        apply_options(),
    )?;
    insta::assert_debug_snapshot!(out, "this won't happen (often) in the real world, but it's a no-op", @r#"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
        applied_branches: "[]",
    }
    "#);

    let ws = out.workspace.into_owned();
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
    ");

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

mod utils {
    pub fn standard_traversal_options() -> but_graph::init::Options {
        but_graph::init::Options {
            collect_tags: true,
            commits_limit_hint: None,
            commits_limit_recharge_location: vec![],
            hard_limit: None,
            extra_target_commit_id: None,
            dangerously_skip_postprocessing_for_debugging: false,
        }
    }

    pub fn standard_traversal_options_with_extra_target(
        repo: &gix::Repository,
    ) -> but_graph::init::Options {
        but_graph::init::Options {
            extra_target_commit_id: Some(repo.rev_parse_single("main").expect("present").detach()),
            ..standard_traversal_options()
        }
    }
}
use utils::{standard_traversal_options, standard_traversal_options_with_extra_target};
