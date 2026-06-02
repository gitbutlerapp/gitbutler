use anyhow::Result;
use but_core::Commit;
use but_graph::init::Options;
use but_meta::virtual_branches_legacy_types::Target;
use but_rebase::graph_rebase::mutate::RelativeTo;
use but_testsupport::{graph_workspace, visualize_commit_graph_all};
use but_workspace::{BottomUpdate, BottomUpdateKind, integrate_upstream};
use gix::prelude::ObjectIdExt;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack, named_writable_scenario_with_description,
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
