use anyhow::{Context, Result};
use but_core::{Commit, RefMetadata};
use but_graph::init::Options;
use but_meta::virtual_branches_legacy_types::Target;
use but_rebase::graph_rebase::mutate::RelativeTo;
use but_testsupport::{CommandExt, git, graph_workspace, visualize_commit_graph_all};
use but_workspace::{
    BottomUpdate, BottomUpdateKind, integrate_upstream, worktree_conflicts_for_rebase,
};
use gix::prelude::ObjectIdExt;
use gix::refs::transaction::PreviousValue;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack, add_stack_with_segments, named_writable_scenario_with_description,
};

fn project_meta(meta: &impl RefMetadata) -> Result<but_core::ref_metadata::ProjectMeta> {
    Ok(meta
        .workspace(but_core::WORKSPACE_REF_NAME.try_into()?)?
        .project_meta())
}

#[test]
fn diamond_partially_historically_integrated_rebase() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("diamond-partially-historically-integrated")?;
    let o1_id = repo.rev_parse_single("o1")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "master"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: o1_id,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "E", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(o1_id),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 61ee5f5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 972cf74 (E) E
    *   9e74c75 (C) C
    |\  
    | * d6a7004 (D) D
    | | * 7de2393 (origin/master, master) o4
    | | *   7d62953 (o3) o3
    | | |\  
    | |_|/  
    |/| |   
    * | | ffb801b (B) B
    |/ /  
    * | 448b195 (A) A
    | * d1b2089 o2
    |/  
    * 85aa44b (o1) o1
    ");

    let mut workspace = graph.into_workspace()?;
    let remote_tip_before = repo.rev_parse_single("origin/master")?.detach();
    let project_meta = workspace.graph.project_meta.clone();
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    rebase.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 996b85e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 2eb4a8c (E) E
    *   aecdc68 (C) C
    |\  
    | * 020d090 (D) D
    |/  
    * 7de2393 (origin/master, master) o4
    *   7d62953 (o3) o3
    |\  
    | * ffb801b B
    | * 448b195 A
    * | d1b2089 o2
    |/  
    * 85aa44b (o1) o1
    ");

    assert_eq!(
        repo.rev_parse_single("origin/master")?.detach(),
        remote_tip_before
    );

    Ok(())
}

#[test]
fn diamond_partially_historically_integrated_merge() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("diamond-partially-historically-integrated")?;
    let o1_id = repo.rev_parse_single("o1")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "master"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: o1_id,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "E", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(o1_id),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 61ee5f5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 972cf74 (E) E
    *   9e74c75 (C) C
    |\  
    | * d6a7004 (D) D
    | | * 7de2393 (origin/master, master) o4
    | | *   7d62953 (o3) o3
    | | |\  
    | |_|/  
    |/| |   
    * | | ffb801b (B) B
    |/ /  
    * | 448b195 (A) A
    | * d1b2089 o2
    |/  
    * 85aa44b (o1) o1
    ");

    let mut workspace = graph.into_workspace()?;
    let project_meta = workspace.graph.project_meta.clone();
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Merge,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    rebase.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 292b0b3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    *   ed5f276 (E) Merge refs/remotes/origin/master into merge
    |\  
    | * 7de2393 (origin/master, master) o4
    | *   7d62953 (o3) o3
    | |\  
    | * | d1b2089 o2
    * | | 972cf74 E
    * | |   9e74c75 (C) C
    |\ \ \  
    | |_|/  
    |/| |   
    | * | d6a7004 (D) D
    * | | ffb801b B
    |/ /  
    * / 448b195 A
    |/  
    * 85aa44b (o1) o1
    ");

    Ok(())
}

#[test]
fn diamond_partially_content_integrated_rebase() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("diamond-partially-content-integrated")?;
    let o1_id = repo.rev_parse_single("o1")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "master"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: o1_id,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "E", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(o1_id),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3e02fbd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * a6588cf (E) E
    *   4827d2f (C) C
    |\  
    | * d8d0970 (D) D
    * | 3d3bfa7 (B) B
    |/  
    * f5b02d3 (A) A
    | * 162b064 (origin/master, master) o4
    | * dd87d69 (o3) B
    | * 5c0b375 A
    | * d1b2089 o2
    |/  
    * 85aa44b (o1) o1
    ");

    let mut workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/master⇣4 on 85aa44b
    └── ≡📙:4:E on 85aa44b {1}
        ├── 📙:4:E
        │   └── ·a6588cf (🏘️)
        ├── :6:C
        │   └── ·4827d2f (🏘️)
        ├── :7:B
        │   └── ·3d3bfa7 (🏘️)
        └── :9:A
            └── ·f5b02d3 (🏘️)
    ");
    let project_meta = workspace.graph.project_meta.clone();
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    rebase.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8b48706 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * cb866ec (E) E
    *   c7b32b8 (C) C
    |\  
    | * e05e7c1 (D) D
    |/  
    * 162b064 (origin/master, master) o4
    * dd87d69 (o3) B
    * 5c0b375 A
    * d1b2089 o2
    * 85aa44b (o1) o1
    ");

    Ok(())
}

#[test]
fn diamond_partially_content_integrated_merge() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("diamond-partially-content-integrated")?;
    let o1_id = repo.rev_parse_single("o1")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "master"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: o1_id,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "E", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(o1_id),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3e02fbd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * a6588cf (E) E
    *   4827d2f (C) C
    |\  
    | * d8d0970 (D) D
    * | 3d3bfa7 (B) B
    |/  
    * f5b02d3 (A) A
    | * 162b064 (origin/master, master) o4
    | * dd87d69 (o3) B
    | * 5c0b375 A
    | * d1b2089 o2
    |/  
    * 85aa44b (o1) o1
    ");

    let mut workspace = graph.into_workspace()?;
    let project_meta = workspace.graph.project_meta.clone();
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Merge,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    rebase.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * ebd6fa2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    *   0a395ba (E) Merge refs/remotes/origin/master into merge
    |\  
    | * 162b064 (origin/master, master) o4
    | * dd87d69 (o3) B
    | * 5c0b375 A
    | * d1b2089 o2
    * | a6588cf E
    * |   4827d2f (C) C
    |\ \  
    | * | d8d0970 (D) D
    * | | 3d3bfa7 B
    |/ /  
    * / f5b02d3 A
    |/  
    * 85aa44b (o1) o1
    ");

    Ok(())
}

#[test]
fn integrated_bottom_branch_no_workspace_rebase() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("integrated-bottom-branch-no-workspace")?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    // No workspace branch, commit, or stack metadata: HEAD is checked out directly on `A`,
    // the top of a two-branch stack whose bottom branch `B` is integrated into the target.
    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * e792f40 (HEAD -> A) add A1
    | * 8c8a843 (origin/main) add X1
    |/  
    * b38b04b (B) add B1
    * 3183e43 (main) M1
    ");

    let mut workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    ⌂:0:A[🌳] <> ✓refs/remotes/origin/main⇣2 on 3183e43
    └── ≡:0:A[🌳] on 3183e43 {1}
        ├── :0:A[🌳]
        │   └── ·e792f40
        └── :3:B
            └── ·b38b04b
    ");
    let project_meta = workspace.graph.project_meta.clone();
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("B")?.detach()),
        }],
    )?;

    rebase.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 10e781b (HEAD -> A) add A1
    * 8c8a843 (origin/main) add X1
    * b38b04b add B1
    * 3183e43 (main) M1
    ");

    assert!(
        repo.try_find_reference("B")?.is_none(),
        "the integrated bottom branch should be removed from the refs after rebase integration"
    );
    assert_eq!(
        repo.rev_parse_single("A^")?.detach(),
        repo.rev_parse_single("origin/main")?.detach(),
        "the top branch should be reparented directly onto the integrated target tip"
    );

    Ok(())
}

#[test]
fn integrated_bottom_branch_no_workspace_merge() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("integrated-bottom-branch-no-workspace")?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    // No workspace branch, commit, or stack metadata: HEAD is checked out directly on `A`,
    // the top of a two-branch stack whose bottom branch `B` is integrated into the target.
    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * e792f40 (HEAD -> A) add A1
    | * 8c8a843 (origin/main) add X1
    |/  
    * b38b04b (B) add B1
    * 3183e43 (main) M1
    ");

    let mut workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    ⌂:0:A[🌳] <> ✓refs/remotes/origin/main⇣2 on 3183e43
    └── ≡:0:A[🌳] on 3183e43 {1}
        ├── :0:A[🌳]
        │   └── ·e792f40
        └── :3:B
            └── ·b38b04b
    ");
    let project_meta = workspace.graph.project_meta.clone();
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Merge,
            selector: RelativeTo::Commit(repo.rev_parse_single("B")?.detach()),
        }],
    )?;

    rebase.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   7ce831c (HEAD -> A) Merge refs/remotes/origin/main into merge
    |\  
    | * 8c8a843 (origin/main) add X1
    * | e792f40 add A1
    |/  
    * b38b04b add B1
    * 3183e43 (main) M1
    ");

    assert!(
        repo.try_find_reference("B")?.is_none(),
        "the integrated bottom branch should be removed from the refs after merge integration"
    );

    let branch_tip = repo.find_commit(repo.rev_parse_single("A")?.detach())?;
    let parents = branch_tip.parent_ids().collect::<Vec<_>>();
    assert_eq!(
        parents.len(),
        2,
        "merge integration should create a merge commit at the top of the stack"
    );
    assert_eq!(
        parents[1].detach(),
        repo.rev_parse_single("origin/main")?.detach(),
        "merge integration should keep the integrated target tip as the second parent"
    );

    Ok(())
}

#[test]
fn merge_upstream_with_conflicting_target_materializes_conflicted_merge_commit() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("remote-diverged-with-workspace-conflicting")?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "A"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 8fd8fb6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 61c4a24 (A) local change in A 1
    | * f03fc2c (origin/A, new-origin) remote change in A 1
    |/  
    * 2b73dee (origin/main, main) init-integration
    ");

    let mut workspace = graph.into_workspace()?;
    let project_meta = workspace.graph.project_meta.clone();
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Merge,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    rebase.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 379fa91 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    *   9b4efdf (A) [conflict] Merge refs/remotes/origin/A into merge
    |\  
    | * f03fc2c (origin/A, new-origin) remote change in A 1
    * | 61c4a24 local change in A 1
    |/  
    * 2b73dee (origin/main, main) init-integration
    ");

    let branch_tip = repo.find_commit(repo.rev_parse_single("A")?.detach())?;
    assert!(
        Commit::from_id(branch_tip.id.attach(&repo))?.is_conflicted(),
        "upstream integration merge should materialize a conflicted commit when target and stack changes conflict",
    );

    let parents = branch_tip.parent_ids().collect::<Vec<_>>();
    assert_eq!(
        parents.len(),
        2,
        "upstream integration merge should preserve merge ancestry in conflicted cases",
    );
    assert_eq!(
        parents[1].detach(),
        repo.rev_parse_single("origin/A")?.detach(),
        "upstream integration merge should keep the target branch tip as second parent",
    );

    insta::assert_snapshot!(branch_tip.message_raw()?, @r#"
    [conflict] Merge refs/remotes/origin/A into merge

    GitButler-Conflict: This is a GitButler-managed conflicted commit. Files are auto-resolved
       using the "ours" side. The commit tree contains additional directories:
         .conflict-side-0  — our tree
         .conflict-side-1  — their tree
         .conflict-base-0  — the merge base tree
         .auto-resolution  — the auto-resolved tree
         .conflict-files   — metadata about conflicted files
       To manually resolve, check out this commit, remove the directories
       listed above, resolve the conflicts, and amend the commit.
    "#);

    Ok(())
}

#[test]
fn fully_historically_integrated_branch_leaves_workspace_shape() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("fully-integrated-branch")?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   9d7da88 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 905d6e5 (origin/main, A) add A1
    * | b38b04b (B) add B1
    |/  
    * 3183e43 (main) M1
    ");

    let mut workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:4:A on 3183e43 {1}
    │   └── 📙:4:A
    │       └── ·905d6e5 (🏘️|✓)
    └── ≡📙:3:B on 3183e43 {2}
        └── 📙:3:B
            └── ·b38b04b (🏘️)
    ");

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![
            BottomUpdate {
                kind: BottomUpdateKind::Rebase,
                selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
            },
            BottomUpdate {
                kind: BottomUpdateKind::Rebase,
                selector: RelativeTo::Commit(repo.rev_parse_single("B")?.detach()),
            },
        ],
    )?;

    let graph =
        but_graph::Graph::from_head(&repo, &meta, project_meta(&meta)?, Options::limited())?;
    let workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 905d6e5
    └── ≡📙:3:B on 905d6e5 {2}
        └── 📙:3:B
            └── ·c932222 (🏘️)
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * eaf66d4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * c932222 (B) add B1
    * 905d6e5 (origin/main) add A1
    * 3183e43 (main) M1
    ");

    Ok(())
}

#[test]
fn fully_integrated_single_branch_leaves_workspace_shape() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("fully-integrated-single-branch")?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * f88e9ce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 905d6e5 (origin/main, A) add A1
    * 3183e43 (main) M1
    ");

    let mut workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    └── ≡📙:3:A on 3183e43 {1}
        └── 📙:3:A
            └── ·905d6e5 (🏘️|✓)
    ");

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    let graph =
        but_graph::Graph::from_head(&repo, &meta, project_meta(&meta)?, Options::limited())?;
    let workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 905d6e5");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * f88e9ce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 905d6e5 (origin/main) add A1
    * 3183e43 (main) M1
    ");

    Ok(())
}

#[test]
fn fully_integrated_single_branch_reparents_workspace_commit_to_advanced_target() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("fully-integrated-single-branch-target-advanced")?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 9de7db5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 6b20716 (origin/main) add X
    |/  
    * ffde79e (A) add A
    * 86b55e6 add B
    * 8d5739f (main) add C
    ");

    let mut workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 8d5739f
    └── ≡📙:3:A on 8d5739f {1}
        └── 📙:3:A
            ├── ·ffde79e (🏘️|✓)
            └── ·86b55e6 (🏘️|✓)
    ");

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A^")?.detach()),
        }],
    )?;

    let graph =
        but_graph::Graph::from_head(&repo, &meta, project_meta(&meta)?, Options::limited())?;
    let workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 6b20716");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * fa202eb (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 6b20716 (origin/main) add X
    * ffde79e add A
    * 86b55e6 add B
    * 8d5739f (main) add C
    ");

    Ok(())
}

#[test]
fn fully_integrated_single_branch_reparents_workspace_commit_to_advanced_merge_target() -> Result<()>
{
    let (_tmp, repo, mut meta, _description) = named_writable_scenario_with_description(
        "fully-integrated-single-branch-target-advanced-through-merge",
    )?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 9de7db5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * f27db86 (origin/main) add X
    | *   4f5589a D
    | |\  
    | |/  
    |/|   
    * | ffde79e (A) add A
    * | 86b55e6 add B
    |/  
    * 8d5739f (main) add C
    ");

    let mut workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on 8d5739f
    └── ≡📙:3:A on 8d5739f {1}
        └── 📙:3:A
            ├── ·ffde79e (🏘️|✓)
            └── ·86b55e6 (🏘️|✓)
    ");

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A^")?.detach()),
        }],
    )?;

    let graph =
        but_graph::Graph::from_head(&repo, &meta, project_meta(&meta)?, Options::limited())?;
    let workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on f27db86");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * d60856a (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f27db86 (origin/main) add X
    *   4f5589a D
    |\  
    | * ffde79e add A
    | * 86b55e6 add B
    |/  
    * 8d5739f (main) add C
    ");

    Ok(())
}

#[test]
fn fully_integrated_direct_checkout_creates_unique_canned_branch_at_target_tip() -> Result<()> {
    let (_tmp, mut repo, mut meta, _description) =
        named_writable_scenario_with_description("fully-integrated-single-branch-target-advanced")?;
    force_prefixless_canned_branch_name(&mut repo)?;
    git(&repo).args(["checkout", "A"]).run();
    remove_managed_workspace_ref(&repo)?;
    let target_sha = repo.rev_parse_single("main")?.detach();
    let target_tip = repo.rev_parse_single("origin/main")?.detach();
    let branch_1: gix::refs::FullName = "refs/heads/branch-1".try_into()?;
    let branch_2: gix::refs::FullName = "refs/heads/branch-2".try_into()?;

    repo.reference(
        branch_1.as_ref(),
        target_tip,
        PreviousValue::MustNotExist,
        "reserve first canned branch name",
    )?;
    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;
    let project_meta = workspace.graph.project_meta.clone();

    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A^")?.detach()),
        }],
    )?;

    let preview = rebase.overlayed_graph()?.into_workspace()?;
    assert_eq!(
        preview.ref_name(),
        Some(branch_2.as_ref()),
        "dry-run overlay should show the replacement canned branch as the checked-out branch"
    );
    assert!(
        repo.try_find_reference(branch_2.as_ref())?.is_none(),
        "dry-run preview should not create the replacement branch on disk"
    );
    drop(preview);

    rebase.materialize()?;

    assert!(
        repo.try_find_reference("A")?.is_none(),
        "fully integrated checked-out branch should be removed"
    );
    assert_eq!(
        repo.find_reference(branch_1.as_ref())?.id(),
        target_tip,
        "the pre-existing canned branch collision should be left untouched"
    );
    assert_eq!(
        repo.find_reference(branch_2.as_ref())?.id(),
        target_tip,
        "replacement canned branch should point at the latest target tip"
    );
    assert_eq!(
        repo.head_name()?,
        Some(branch_2),
        "HEAD should stay attached to the replacement canned branch"
    );

    Ok(())
}

#[test]
fn fully_integrated_direct_checkout_creates_canned_branch_at_merge_target_tip() -> Result<()> {
    let (_tmp, mut repo, mut meta, _description) = named_writable_scenario_with_description(
        "fully-integrated-single-branch-target-advanced-through-merge",
    )?;
    force_prefixless_canned_branch_name(&mut repo)?;
    git(&repo).args(["checkout", "A"]).run();
    remove_managed_workspace_ref(&repo)?;
    let target_sha = repo.rev_parse_single("main")?.detach();
    let target_tip = repo.rev_parse_single("origin/main")?.detach();
    let target_tip_parent = repo
        .find_commit(target_tip)?
        .parent_ids()
        .next()
        .context("target tip should have a parent")?
        .detach();
    assert_eq!(
        repo.find_commit(target_tip_parent)?.parent_ids().count(),
        2,
        "this fixture must exercise a target tip based on a merge commit"
    );
    let branch_1: gix::refs::FullName = "refs/heads/branch-1".try_into()?;
    let branch_2: gix::refs::FullName = "refs/heads/branch-2".try_into()?;

    repo.reference(
        branch_1.as_ref(),
        target_tip,
        PreviousValue::MustNotExist,
        "reserve first canned branch name",
    )?;
    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;
    let project_meta = workspace.graph.project_meta.clone();

    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A^")?.detach()),
        }],
    )?;

    let preview = rebase.overlayed_graph()?.into_workspace()?;
    assert_eq!(
        preview.ref_name(),
        Some(branch_2.as_ref()),
        "dry-run overlay should show the replacement canned branch as the checked-out branch"
    );
    drop(preview);

    rebase.materialize()?;

    assert!(
        repo.try_find_reference("A")?.is_none(),
        "fully integrated checked-out branch should be removed"
    );
    assert_eq!(
        repo.find_reference(branch_2.as_ref())?.id(),
        target_tip,
        "replacement canned branch should point at the exact merge target tip, not a replayed merge"
    );
    assert_eq!(
        repo.head_name()?,
        Some(branch_2),
        "HEAD should stay attached to the replacement canned branch"
    );

    Ok(())
}

#[test]
fn empty_workspace_reparents_workspace_commit_to_advanced_target() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("empty-workspace-target-advanced")?;
    let target_sha = repo.rev_parse_single("main^")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;
    integrate_and_materialize(&mut workspace, &mut meta, &repo, vec![])?;

    assert_eq!(
        repo.rev_parse_single("gitbutler/workspace^")?.detach(),
        repo.rev_parse_single("origin/main")?.detach(),
        "empty workspace commit should move from stored target commit to current target ref tip"
    );

    Ok(())
}

#[test]
fn empty_workspace_reparents_workspace_commit_to_merge_advanced_target() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("empty-workspace-target-advanced-through-merge")?;
    let target_sha = repo.rev_parse_single("main~2")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;
    integrate_and_materialize(&mut workspace, &mut meta, &repo, vec![])?;

    assert_eq!(
        workspace_first_parent(&repo)?,
        repo.rev_parse_single("origin/main")?.detach(),
        "empty workspace commit should move to the target tip when the target advanced through a merge"
    );

    Ok(())
}

#[test]
fn workspace_target_parent_updates_while_stack_parent_remains_anonymous_segment_remains()
-> Result<()> {
    let (_tmp, repo, mut meta, _description) = named_writable_scenario_with_description(
        "workspace-target-parent-and-stack-target-advanced",
    )?;
    let target_sha = repo.rev_parse_single("target-sha")?.detach();
    let anonymous_commit_c2 = repo.rev_parse_single("gitbutler/workspace^")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   e854d6a (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 90d25da (A) add A
    * | 0d97cc1 add C2
    |/  
    | * 20a5ffc (origin/main, main) add X
    |/  
    * fe9ae6e (target-sha) add C1
    ");

    let mut workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fe9ae6e
    ├── ≡:5:anon: on fe9ae6e
    │   └── :5:anon:
    │       └── ·0d97cc1 (🏘️)
    └── ≡📙:4:A on fe9ae6e {1}
        └── 📙:4:A
            └── ·90d25da (🏘️)
    ");
    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    let graph =
        but_graph::Graph::from_head(&repo, &meta, project_meta(&meta)?, Options::limited())?;
    let workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fe9ae6e
    ├── ≡:5:anon: on fe9ae6e
    │   └── :5:anon:
    │       └── ·0d97cc1 (🏘️)
    └── ≡📙:3:A on 20a5ffc {1}
        └── 📙:3:A
            └── ·c529875 (🏘️)
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   06beb96 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * c529875 (A) add A
    | * 20a5ffc (origin/main, main) add X
    * | 0d97cc1 add C2
    |/  
    * fe9ae6e (target-sha) add C1
    ");
    assert_eq!(
        workspace_parent_ids(&repo)?,
        vec![anonymous_commit_c2, repo.rev_parse_single("A")?.detach(),],
        "workspace commit should keep the remaining stack parent and anonymous commit"
    );

    Ok(())
}

#[test]
fn dry_run_reports_dirty_worktree_conflicts_against_resulting_workspace_head() -> Result<()> {
    let (tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("upstream-integration-worktree-conflict")?;
    let target_sha = repo.rev_parse_single("main^")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;

    std::fs::write(tmp.path().join("shared.txt"), "dirty\n")?;
    let project_meta = project_meta(&meta)?;
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    let conflicts = worktree_conflicts_for_rebase(&rebase)?;
    assert_eq!(
        conflicts,
        vec![but_serde::BStringForFrontend::from("shared.txt")],
        "dirty worktree conflict preview should report paths that would conflict on the resulting workspace head"
    );
    assert_eq!(
        repo.head()?
            .id()
            .context("HEAD should point to gitbutler/workspace")?
            .detach(),
        repo.rev_parse_single("gitbutler/workspace")?.detach(),
        "dry-run conflict preview must not materialize the rebase"
    );

    Ok(())
}

#[test]
fn dry_run_reports_index_only_conflicts_against_resulting_workspace_head() -> Result<()> {
    let (tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("upstream-integration-worktree-conflict")?;
    let target_sha = repo.rev_parse_single("main^")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;

    std::fs::write(tmp.path().join("shared.txt"), "dirty\n")?;
    git(&repo).args(["add", "shared.txt"]).run();
    std::fs::write(tmp.path().join("shared.txt"), "base\n")?;

    let project_meta = project_meta(&meta)?;
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    let conflicts = worktree_conflicts_for_rebase(&rebase)?;
    assert_eq!(
        conflicts,
        vec![but_serde::BStringForFrontend::from("shared.txt")],
        "index-only conflict preview should report staged paths that would conflict on the resulting workspace head"
    );

    Ok(())
}

#[test]
fn partially_integrated_branch_leaves_multi_branch_stack() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("partially-integrated-multi-branch-stack")?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["C"]);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   cf53402 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 44c9428 (A) add A1
    | * f1e7451 (origin/main, C) add C1
    * | b38b04b (B) add B1
    |/  
    * 3183e43 (main) M1
    ");

    let mut workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:3:A on 3183e43 {1}
    │   ├── 📙:3:A
    │   │   └── ·44c9428 (🏘️)
    │   └── 📙:5:C
    │       └── ·f1e7451 (🏘️|✓)
    └── ≡📙:4:B on 3183e43 {2}
        └── 📙:4:B
            └── ·b38b04b (🏘️)
    ");

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![
            BottomUpdate {
                kind: BottomUpdateKind::Rebase,
                selector: RelativeTo::Commit(repo.rev_parse_single("C")?.detach()),
            },
            BottomUpdate {
                kind: BottomUpdateKind::Rebase,
                selector: RelativeTo::Commit(repo.rev_parse_single("B")?.detach()),
            },
        ],
    )?;

    let graph =
        but_graph::Graph::from_head(&repo, &meta, project_meta(&meta)?, Options::limited())?;
    let workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on f1e7451
    ├── ≡📙:3:A on f1e7451 {1}
    │   └── 📙:3:A
    │       └── ·44c9428 (🏘️)
    └── ≡📙:4:B on f1e7451 {2}
        └── 📙:4:B
            └── ·a27415e (🏘️)
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   780946b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 44c9428 (A) add A1
    * | a27415e (B) add B1
    |/  
    * f1e7451 (origin/main) add C1
    * 3183e43 (main) M1
    ");

    Ok(())
}

#[test]
fn fully_integrated_multi_branch_stack_leaves_workspace_shape() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("fully-integrated-multi-branch-stack")?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["C"]);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   cf53402 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 44c9428 (origin/main, A) add A1
    | * f1e7451 (C) add C1
    * | b38b04b (B) add B1
    |/  
    * 3183e43 (main) M1
    ");

    let mut workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:5:A on 3183e43 {1}
    │   ├── 📙:5:A
    │   │   └── ·44c9428 (🏘️|✓)
    │   └── 📙:3:C
    │       └── ·f1e7451 (🏘️|✓)
    └── ≡📙:4:B on 3183e43 {2}
        └── 📙:4:B
            └── ·b38b04b (🏘️)
    ");

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![
            BottomUpdate {
                kind: BottomUpdateKind::Rebase,
                selector: RelativeTo::Commit(repo.rev_parse_single("C")?.detach()),
            },
            BottomUpdate {
                kind: BottomUpdateKind::Rebase,
                selector: RelativeTo::Commit(repo.rev_parse_single("B")?.detach()),
            },
        ],
    )?;

    let graph =
        but_graph::Graph::from_head(&repo, &meta, project_meta(&meta)?, Options::limited())?;
    let workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 44c9428
    └── ≡📙:3:B on 44c9428 {2}
        └── 📙:3:B
            └── ·f59d71f (🏘️)
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 55ce8ae (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f59d71f (B) add B1
    * 44c9428 (origin/main) add A1
    * f1e7451 add C1
    * 3183e43 (main) M1
    ");

    Ok(())
}

#[test]
fn fully_integrated_two_stacks_leave_workspace_shape() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("fully-integrated-two-stacks")?;
    let target_sha = repo.rev_parse_single("main~2")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   9d7da88 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | | *   5f7d45e (origin/main, main) Merging B into base
    | | |\  
    | |_|/  
    |/| |   
    * | | b38b04b (B) add B1
    | | * 1f7670a Merging A into base
    | |/| 
    |/|/  
    | * 905d6e5 (A) add A1
    |/  
    * 3183e43 M1
    ");

    let mut workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on 3183e43
    ├── ≡📙:4:A on 3183e43 {1}
    │   └── 📙:4:A
    │       └── ·905d6e5 (🏘️|✓)
    └── ≡📙:5:B on 3183e43 {2}
        └── 📙:5:B
            └── ·b38b04b (🏘️|✓)
    ");

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![
            BottomUpdate {
                kind: BottomUpdateKind::Rebase,
                selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
            },
            BottomUpdate {
                kind: BottomUpdateKind::Rebase,
                selector: RelativeTo::Commit(repo.rev_parse_single("B")?.detach()),
            },
        ],
    )?;

    let graph =
        but_graph::Graph::from_head(&repo, &meta, project_meta(&meta)?, Options::limited())?;
    let workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 5f7d45e");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * b44fd24 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    *   5f7d45e (origin/main, main) Merging B into base
    |\  
    | * b38b04b add B1
    * |   1f7670a Merging A into base
    |\ \  
    | |/  
    |/|   
    | * 905d6e5 add A1
    |/  
    * 3183e43 M1
    ");

    Ok(())
}

#[test]
fn orphan_reparent_content_integrated_stack_to_target_tip() -> Result<()> {
    let (_tmp, repo, mut meta, _description) = named_writable_scenario_with_description(
        "fully-content-integrated-single-branch-target-advanced",
    )?;
    let target_sha = repo.rev_parse_single("main~3")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A^")?.detach()),
        }],
    )?;

    assert_eq!(
        workspace_first_parent(&repo)?,
        repo.rev_parse_single("origin/main")?.detach(),
        "orphaned workspace commit should be reparented to the advanced target tip after content integration"
    );

    Ok(())
}

#[test]
fn content_integrated_stack_does_not_reparent_while_stack_parent_remains() -> Result<()> {
    let (_tmp, repo, mut meta, _description) = named_writable_scenario_with_description(
        "content-integrated-stack-with-remaining-stack-target-advanced",
    )?;
    let target_sha = repo.rev_parse_single("main~2")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    assert_eq!(
        workspace_parent_ids(&repo)?,
        vec![repo.rev_parse_single("B")?.detach()],
        "workspace commit should stay parented only to the remaining stack"
    );

    Ok(())
}

#[test]
fn orphan_reparent_does_not_run_when_parent_remains() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("fully-integrated-branch")?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    assert_eq!(
        workspace_first_parent(&repo)?,
        repo.rev_parse_single("B")?.detach(),
        "workspace commit should stay parented to the remaining stack"
    );
    assert_ne!(
        workspace_first_parent(&repo)?,
        repo.rev_parse_single("origin/main")?.detach(),
        "target should not be added while another workspace parent remains"
    );

    Ok(())
}

#[test]
fn orphan_reparent_empty_stack_to_target_tip() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("fully-integrated-empty-stack-target-advanced")?;
    let target_sha = repo.rev_parse_single("main^")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "B", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Reference(gix::refs::FullName::try_from("refs/heads/B")?),
        }],
    )?;

    assert_eq!(
        workspace_first_parent(&repo)?,
        repo.rev_parse_single("origin/main")?.detach(),
        "orphaned workspace commit should be reparented to the target tip after integrating an empty stack"
    );

    Ok(())
}

#[test]
fn orphan_reparent_same_target_tip_keeps_single_parent() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("fully-integrated-single-branch")?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    assert_eq!(
        workspace_first_parent(&repo)?,
        repo.rev_parse_single("origin/main")?.detach(),
        "orphaned workspace commit should stay on the target tip when it already equals the removed parent"
    );
    assert_eq!(
        workspace_parent_count(&repo)?,
        1,
        "workspace commit should not gain duplicate parents"
    );

    Ok(())
}

#[test]
fn orphan_reparent_two_stacks_through_merge_target() -> Result<()> {
    let (_tmp, repo, mut meta, _description) = named_writable_scenario_with_description(
        "fully-integrated-two-stacks-merge-target-advanced",
    )?;
    let target_sha = repo.rev_parse_single("main~3")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&meta)?,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;

    integrate_and_materialize(
        &mut workspace,
        &mut meta,
        &repo,
        vec![
            BottomUpdate {
                kind: BottomUpdateKind::Rebase,
                selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
            },
            BottomUpdate {
                kind: BottomUpdateKind::Rebase,
                selector: RelativeTo::Commit(repo.rev_parse_single("B")?.detach()),
            },
        ],
    )?;

    assert_eq!(
        workspace_first_parent(&repo)?,
        repo.rev_parse_single("origin/main")?.detach(),
        "orphaned workspace commit should be reparented to the merge-advanced target tip"
    );

    Ok(())
}

fn integrate_and_materialize<M: RefMetadata>(
    workspace: &mut but_graph::Workspace,
    meta: &mut M,
    repo: &gix::Repository,
    updates: Vec<BottomUpdate>,
) -> Result<()> {
    let but_workspace::IntegrateUpstreamOutcome {
        rebase,
        ws_meta,
        project_meta,
    } = integrate_upstream(workspace, meta, project_meta(&*meta)?, repo, updates)?;
    let materialized = rebase.materialize()?;
    if let Some(ref_name) = materialized.workspace.ref_name()
        && let Some(ws_meta) = ws_meta
    {
        let mut md = materialized.meta.workspace(ref_name)?;
        *md = ws_meta;
        md.set_project_meta(project_meta);
        materialized.meta.set_workspace(&md)?;
    }
    drop(materialized);

    Ok(())
}

fn workspace_first_parent(repo: &gix::Repository) -> Result<gix::ObjectId> {
    Ok(repo.rev_parse_single("gitbutler/workspace^")?.detach())
}

fn workspace_parent_ids(repo: &gix::Repository) -> Result<Vec<gix::ObjectId>> {
    let workspace_commit =
        repo.find_commit(repo.rev_parse_single("gitbutler/workspace")?.detach())?;
    Ok(workspace_commit
        .parent_ids()
        .map(|id| id.detach())
        .collect())
}

fn workspace_parent_count(repo: &gix::Repository) -> Result<usize> {
    let workspace_commit =
        repo.find_commit(repo.rev_parse_single("gitbutler/workspace")?.detach())?;
    Ok(workspace_commit.parent_ids().count())
}

fn force_prefixless_canned_branch_name(repo: &mut gix::Repository) -> Result<()> {
    let mut config = repo.config_snapshot_mut();
    config.raw_values_mut(&"author.name")?.delete_all();
    config.raw_values_mut(&"author.email")?.delete_all();
    Ok(())
}

fn remove_managed_workspace_ref(repo: &gix::Repository) -> Result<()> {
    if let Some(reference) = repo.try_find_reference(but_core::WORKSPACE_REF_NAME)? {
        reference.delete()?;
    }
    Ok(())
}
