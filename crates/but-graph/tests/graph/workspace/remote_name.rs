use but_core::RefMetadata;
use but_graph::Graph;
use but_testsupport::{graph_tree, visualize_commit_graph_all};

use super::target_meta;
use crate::init::utils::{add_workspace, read_only_in_memory_scenario, standard_options};

#[test]
fn with_target_ref_extracts_remote_name() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;

    add_workspace(&mut meta);

    let ws = Graph::from_head(&repo, &*meta, target_meta(), standard_options())?
        .validated()?
        .into_workspace()?;

    assert!(ws.target_ref.is_some());
    assert_eq!(
        ws.remote_name(),
        Some("origin".into()),
        "target_ref is 'refs/remotes/origin/main', should extract 'origin'"
    );

    Ok(())
}

#[test]
fn returns_none_when_no_target_and_no_push_remote() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;

    add_workspace(&mut meta);

    let ws = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    assert!(ws.target_ref.is_none(), "should not have a target_ref");
    assert!(
        ws.remote_name().is_none(),
        "should return None without target or metadata"
    );

    Ok(())
}

#[test]
fn target_local_tracking_ref_exists_when_other_branch_metadata_names_the_same_tip()
-> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-ws-ref-no-ws-commit-two-branches")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bce0c5e (HEAD -> gitbutler/workspace, origin/main, main, B, A) M2
* 3183e43 M1

"#]]
    );

    add_workspace(&mut meta);
    // This is the state left by unapplying the last workspace stack: the branch
    // is no longer applied, but its branch metadata still disambiguates the
    // same commit that `main` and `origin/main` also point to.
    let branch_name = "refs/heads/A";
    let mut branch = meta.branch(branch_name.try_into()?)?;
    branch.update_times(false);
    meta.set_branch(&branch)?;

    let ws = Graph::from_head(&repo, &*meta, target_meta(), standard_options())?
        .validated()?
        .into_workspace()?;
    // the target remote and its local tracking branch get sibling links even when another branch owns the shared commit
    snapbox::assert_data_eq!(
        graph_tree(&ws.graph).to_string(),
        snapbox::str![[r#"

├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
│   └── 📙►:2[2]:A
│       └── ✂·bce0c5e (⌂|🏘|✓|1) ►B
└── ►:1[0]:origin/main →:3:
    └── ►:3[1]:main <> origin/main →:1:
        └── →:2: (A)

"#]]
    );

    assert_eq!(
        ws.target_ref_name().map(|rn| rn.as_bstr()),
        Some("refs/remotes/origin/main".into()),
        "fixture should resolve the workspace target as origin/main"
    );
    assert_eq!(
        ws.target_local_tracking_ref_info()
            .map(|ri| ri.ref_name.to_string()),
        Some("refs/heads/main".to_string()),
        "target/local tracking relationship should be available from the graph projection"
    );

    Ok(())
}
