use but_core::RefMetadata;
use but_graph::Workspace;
use but_meta::virtual_branches_legacy_types::Target;
use but_testsupport::{graph_dag, visualize_commit_graph_all};

use super::project_meta;
use crate::walk::utils::{
    add_workspace, add_workspace_without_target, named_read_only_in_memory_scenario,
    read_only_in_memory_scenario, standard_options,
};

#[test]
fn with_target_ref_extracts_remote_name() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;

    add_workspace(&mut meta);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    assert!(ws.target_ref.is_some());
    assert_eq!(
        ws.remote_name(),
        Some("origin".into()),
        "target_ref is 'refs/remotes/origin/main', should extract 'origin'"
    );

    Ok(())
}

#[test]
fn slash_named_remote_extracts_the_full_remote_name() -> anyhow::Result<()> {
    let (repo, mut meta) = named_read_only_in_memory_scenario("slash-remote", "slash-remote")?;

    add_workspace(&mut meta);
    // The target remote's name contains a slash — extraction must longest-match
    // against the configured remote names, never split at the first slash.
    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("special/origin", "main"),
        remote_url: "does not matter".to_string(),
        sha: gix::hash::Kind::Sha1.null(),
        push_remote_name: None,
    });

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    assert_eq!(
        ws.target_ref_name().map(|rn| rn.as_bstr()),
        Some("refs/remotes/special/origin/main".into()),
        "the target resolves through the slash-named remote"
    );
    assert_eq!(
        ws.remote_name(),
        Some("special/origin".into()),
        "the full remote name is extracted, not the first path component"
    );

    Ok(())
}

#[test]
fn returns_none_when_no_target_and_no_push_remote() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;

    add_workspace_without_target(&mut meta);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

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

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    // the target remote and its local tracking branch get sibling links even when another branch owns the shared commit
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str!["*  👉✂·bce0c5e (⌂|🏘|✓) ►A, ►B, ►main, ►origin/main <> origin/main"]
    );

    assert_eq!(
        ws.target_ref_name().map(|rn| rn.as_bstr()),
        Some("refs/remotes/origin/main".into()),
        "fixture should resolve the workspace target as origin/main"
    );
    Ok(())
}
