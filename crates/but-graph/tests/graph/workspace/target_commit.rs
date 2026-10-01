use but_core::ref_metadata::ProjectMeta;
use but_graph::{Graph, workspace::WorkspaceKind};
use but_testsupport::{graph_workspace_determinisitcally, visualize_commit_graph_all};
use snapbox::IntoData;

use super::target_meta;
use crate::init::utils::{
    add_workspace, add_workspace_with_target, read_only_in_memory_scenario, standard_options,
};

#[test]
fn main_and_origin_main_in_sync() -> anyhow::Result<()> {
    let (repo, meta, mut db) = read_only_in_memory_scenario("single-branch-in-sync")?;
    // The local branch and its upstream point to the same commit, without a workspace.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );

    // Supply the integration target that CLI initialization would resolve.
    let project_meta = ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        ..Default::default()
    };
    let ws = Graph::from_head(&repo, &*meta, project_meta, &mut db, standard_options())?
        .validated()?
        .into_workspace()?;

    // The local integration branch projects to no stacks when in sync with its target.
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/remotes/origin/main on 85efbe4

"#]]
    );

    Ok(())
}

#[test]
fn empty_feature_branch_at_target_remains_visible() -> anyhow::Result<()> {
    let (repo, meta, mut db) = read_only_in_memory_scenario("single-branch-empty-feature")?;
    // The new feature branch shares the tip of main and origin/main, without a workspace.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 85efbe4 (HEAD -> feature, origin/main, main) M

"#]]
    );

    // Supply the integration target as graph input; target discovery belongs to the caller.
    let project_meta = ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        ..Default::default()
    };
    let ws = Graph::from_head(&repo, &*meta, project_meta, &mut db, standard_options())?
        .validated()?
        .into_workspace()?;

    // Unlike main, the empty feature branch remains visible above the shared base.
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&ws).to_string(),
        snapbox::str![[r#"
⌂:feature[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡:feature[🌳] on 85efbe4 {1}
    └── :feature[🌳]

"#]]
    );

    Ok(())
}

#[test]
fn empty_feature_branch_in_sync_with_own_upstream_remains_visible() -> anyhow::Result<()> {
    let (repo, meta, mut db) =
        read_only_in_memory_scenario("single-branch-empty-feature-with-upstream")?;
    // Publishing the empty feature branch does not make it the integration branch.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 85efbe4 (HEAD -> feature, origin/main, origin/feature, main) M

"#]]
    );

    // No project target: exercise the fallback to the checked-out branch's own upstream.
    let ws = Graph::from_head(
        &repo,
        &*meta,
        ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    // The upstream fallback must not cause an empty feature branch to disappear.
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&ws).to_string(),
        snapbox::str![[r#"
⌂:feature[🌳] <> ✓refs/remotes/origin/feature on 85efbe4
└── ≡:feature[🌳] <> origin/feature on 85efbe4 {1}
    └── :feature[🌳] <> origin/feature

"#]]
    );

    Ok(())
}

#[test]
fn empty_feature_branch_with_own_upstream_and_project_target_remains_visible() -> anyhow::Result<()>
{
    let (repo, meta, mut db) =
        read_only_in_memory_scenario("single-branch-empty-feature-with-upstream")?;
    // The feature branch and its upstream share the integration target's tip.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 85efbe4 (HEAD -> feature, origin/main, origin/feature, main) M

"#]]
    );

    let project_meta = ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        ..Default::default()
    };
    let ws = Graph::from_head(&repo, &*meta, project_meta, &mut db, standard_options())?
        .validated()?
        .into_workspace()?;

    // Tracking origin/feature does not make feature the local integration branch for origin/main.
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&ws).to_string(),
        snapbox::str![[r#"
⌂:feature[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡:feature[🌳] <> origin/feature on 85efbe4 {1}
    └── :feature[🌳] <> origin/feature

"#]]
    );

    Ok(())
}

#[test]
fn ad_hoc_workspace_uses_project_target_ref() -> anyhow::Result<()> {
    let (repo, meta, mut db) = read_only_in_memory_scenario("ad-hoc-branch-integrated-upstream")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?.replace("  \n", "\n"),
        snapbox::str![[r#"
* 7ad98e8 (old-target) OLD
| * d623019 (origin/trunk) RM1
| * d03b217 (HEAD -> feature) F1
|/
* 3183e43 (main) M1

"#]]
    );
    let expected_target = repo.rev_parse_single("refs/remotes/origin/trunk")?.detach();
    let project_meta = ProjectMeta {
        target_ref: Some("refs/remotes/origin/trunk".try_into()?),
        ..Default::default()
    };

    let ws = Graph::from_head(&repo, &*meta, project_meta, &mut db, standard_options())?
        .validated()?
        .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&ws).to_string(),
        snapbox::str![[r#"
⌂:feature[🌳] <> ✓refs/remotes/origin/trunk⇣1 on d03b217
└── ≡:feature[🌳] on d03b217 {1}
    └── :feature[🌳]

"#]]
    );

    assert!(matches!(ws.kind, WorkspaceKind::AdHoc));
    assert_eq!(
        ws.target_ref_name().map(ToString::to_string),
        Some("refs/remotes/origin/trunk".into())
    );
    assert_eq!(
        ws.target_commit.as_ref().map(|target| target.commit_id),
        Some(expected_target),
        "without a stored target commit the projection bases itself on the target ref tip"
    );

    Ok(())
}

#[test]
fn ad_hoc_workspace_uses_stored_project_target_commit() -> anyhow::Result<()> {
    let (repo, meta, mut db) = read_only_in_memory_scenario("ad-hoc-branch-integrated-upstream")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?.replace("  \n", "\n"),
        snapbox::str![[r#"
* 7ad98e8 (old-target) OLD
| * d623019 (origin/trunk) RM1
| * d03b217 (HEAD -> feature) F1
|/
* 3183e43 (main) M1

"#]]
    );
    let expected_target = repo.rev_parse_single(":/OLD")?.detach();
    let project_meta = ProjectMeta {
        target_commit_id: Some(expected_target),
        ..Default::default()
    };

    let ws = Graph::from_head(&repo, &*meta, project_meta, &mut db, standard_options())?
        .validated()?
        .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace_determinisitcally(&ws).to_string(),
        snapbox::str![[r#"
⌂:feature[🌳] <> ✓! on 3183e43
└── ≡:feature[🌳] on 3183e43 {1}
    └── :feature[🌳]
        └── ·d03b217

"#]]
    );

    assert!(matches!(ws.kind, WorkspaceKind::AdHoc));
    assert_eq!(
        ws.target_commit.as_ref().map(|target| target.commit_id),
        Some(expected_target)
    );

    Ok(())
}

#[test]
fn returns_target_tip_when_stacks_have_different_bases() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/two-branches-one-below-base")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   e82dfab (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 6fdab32 (A) A1
* | 78b1b59 (B) B1
| | * 938e6f2 (origin/main, main) M4
| |/  
|/|   
* | f52fcec M3
|/  
* bce0c5e M2
* 3183e43 M1

"#]]
        .raw()
    );

    // A branches from M2, B branches from M3.
    // The stored target commit is M4 (the tip of origin/main).
    add_workspace(&mut meta);

    let ws = Graph::from_head(
        &repo,
        &*meta,
        target_meta(&repo),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    let tip = ws.target_commit.as_ref().map(|target| target.commit_id);
    let expected_m4 = repo.rev_parse_single(":/M4")?.detach();
    assert_eq!(
        tip,
        Some(expected_m4),
        "should return M4, the tip of origin/main"
    );

    Ok(())
}

#[test]
fn returns_target_tip_when_one_stack_is_above_target() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/two-branches-one-above-base")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   c5587c9 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * de6d39c (A) A1
| * a821094 (origin/main, main) M3
* | ce25240 (B) B1
|/  
* bce0c5e M2
* 3183e43 M1

"#]]
        .raw()
    );

    // A branches from M3 (which is also origin/main), B branches from M2.
    // The stored target commit is M3 (the tip of origin/main).
    add_workspace(&mut meta);

    let ws = Graph::from_head(
        &repo,
        &*meta,
        target_meta(&repo),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    let tip = ws.target_commit.as_ref().map(|target| target.commit_id);
    let expected_m3 = repo.rev_parse_single(":/M3")?.detach();
    assert_eq!(
        tip,
        Some(expected_m3),
        "should return M3, the tip of origin/main"
    );

    Ok(())
}

#[test]
fn prefers_target_commit_over_target_ref() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   59a427f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * a62b0de (A) A2
| * 120a217 A1
* | 0a415d8 (main) M3
| | * 1f5c47b (origin/main) RM1
| |/  
|/|   
* | 73ba99d M2
|/  
* fafd9d0 init

"#]]
        .raw()
    );

    // Set target_commit to M2, while target_ref points to origin/main (RM1).
    let m2 = repo.rev_parse_single(":/M2")?.detach();
    let project_meta = add_workspace_with_target(&mut meta, m2);

    let ws = Graph::from_head(&repo, &*meta, project_meta, &mut db, standard_options())?
        .validated()?
        .into_workspace()?;

    assert!(ws.target_ref.is_some(), "target_ref should be set");
    assert!(ws.target_commit.is_some(), "target_commit should be set");

    let result = ws.target_commit.as_ref().map(|target| target.commit_id);
    assert_eq!(
        result,
        Some(m2),
        "should prefer stored target_commit (M2) over target_ref tip (RM1)"
    );

    Ok(())
}

#[test]
fn returns_none_when_no_target() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;

    add_workspace(&mut meta);
    let ws = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    assert!(
        ws.target_commit
            .as_ref()
            .map(|target| target.commit_id)
            .is_none(),
        "should return None when no target is set"
    );

    Ok(())
}
