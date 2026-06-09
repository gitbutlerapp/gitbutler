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

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack, add_stack_with_segments, named_writable_scenario_with_description,
};

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
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
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
    | * ffb801b (B) B
    | * 448b195 (A) A
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
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
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
    * | | ffb801b (B) B
    |/ /  
    * / 448b195 (A) A
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
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
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
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
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
    * | | 3d3bfa7 (B) B
    |/ /  
    * / f5b02d3 (A) A
    |/  
    * 85aa44b (o1) o1
    ");

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
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
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

    let but_workspace::IntegrateUpstreamOutcome { rebase, ws_meta } = integrate_upstream(
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

    let materialized = rebase.materialize()?;
    if let Some(ref_name) = materialized.workspace.ref_name() {
        let mut md = materialized.meta.workspace(ref_name)?;
        *md = ws_meta;
        materialized.meta.set_workspace(&md)?;
    }
    drop(materialized);

    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
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

    let but_workspace::IntegrateUpstreamOutcome { rebase, ws_meta } = integrate_upstream(
        &mut workspace,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?;

    let materialized = rebase.materialize()?;
    if let Some(ref_name) = materialized.workspace.ref_name() {
        let mut md = materialized.meta.workspace(ref_name)?;
        *md = ws_meta;
        materialized.meta.set_workspace(&md)?;
    }
    drop(materialized);

    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
    let workspace = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&workspace), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 905d6e5");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * be76b24 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 905d6e5 (origin/main) add A1
    * 3183e43 (main) M1
    ");

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
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;

    std::fs::write(tmp.path().join("shared.txt"), "dirty\n")?;
    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
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
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut workspace = graph.into_workspace()?;

    std::fs::write(tmp.path().join("shared.txt"), "dirty\n")?;
    git(&repo).args(["add", "shared.txt"]).run();
    std::fs::write(tmp.path().join("shared.txt"), "base\n")?;

    let but_workspace::IntegrateUpstreamOutcome { rebase, .. } = integrate_upstream(
        &mut workspace,
        &mut meta,
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

    let but_workspace::IntegrateUpstreamOutcome { rebase, ws_meta } = integrate_upstream(
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

    let materialized = rebase.materialize()?;
    if let Some(ref_name) = materialized.workspace.ref_name() {
        let mut md = materialized.meta.workspace(ref_name)?;
        *md = ws_meta;
        materialized.meta.set_workspace(&md)?;
    }
    drop(materialized);

    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
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

    let but_workspace::IntegrateUpstreamOutcome { rebase, ws_meta } = integrate_upstream(
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

    let materialized = rebase.materialize()?;
    if let Some(ref_name) = materialized.workspace.ref_name() {
        let mut md = materialized.meta.workspace(ref_name)?;
        *md = ws_meta;
        materialized.meta.set_workspace(&md)?;
    }
    drop(materialized);

    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
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
