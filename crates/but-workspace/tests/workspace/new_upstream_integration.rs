use anyhow::Result;
use but_graph::init::Options;
use but_meta::virtual_branches_legacy_types::Target;
use but_rebase::graph_rebase::mutate::RelativeTo;
use but_testsupport::{graph_workspace, visualize_commit_graph_all};
use but_workspace::{BottomUpdate, BottomUpdateKind, integrate_upstream};

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
    * 03f9366 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 22aa077 (E) E
    *   8719a89 (C) C
    |\  
    | * 59b39ea (D) D
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
    * 96c95ba (HEAD -> gitbutler/workspace) GitButler Workspace Commit
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
    * 5ba22ce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 04b1a3e (E) E
    *   1144e41 (C) C
    |\  
    | * dd00172 (D) D
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
    * 6c75185 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
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
