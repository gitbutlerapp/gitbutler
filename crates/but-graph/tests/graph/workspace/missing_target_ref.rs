use but_graph::Workspace;
use but_testsupport::visualize_commit_graph_all;
use snapbox::prelude::*;

use super::target_meta;
use crate::walk::utils::{
    add_workspace, add_workspace_without_target, read_only_in_memory_scenario, standard_options,
};

/// A target recorded in metadata whose ref the repository does not have. The view carries on
/// unbounded — a recording is a floor only while its ref resolves — so without this fact nothing
/// distinguishes "no target configured" from "the configured target is gone".
#[test]
fn a_configured_target_whose_ref_is_gone_is_reported() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* fafd9d0 (HEAD -> main, origin/main, gitbutler/workspace, F, E, D, C, B, A) init

"#]]
        .raw()
    );

    // `add_workspace` records `refs/remotes/origin/main`, which this repo HAS.
    add_workspace(&mut meta);
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        target_meta(),
        &mut but_testsupport::in_memory_db(),
        standard_options(),
    )?
    .validated()?;
    assert_eq!(
        ws.target_ref_name().map(ToOwned::to_owned),
        Some("refs/remotes/origin/main".try_into()?),
        "sanity: the recorded target resolves here"
    );
    assert_eq!(
        ws.missing_target_ref_name(),
        None,
        "a target that resolves is not missing"
    );

    // Now point the recording at a remote branch the repository does not have — the
    // remote-removed / never-fetched case.
    let gone_target = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/gone".try_into()?),
        ..target_meta()
    };
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        gone_target,
        &mut but_testsupport::in_memory_db(),
        standard_options(),
    )?
    .validated()?;

    assert_eq!(
        ws.target_ref_name(),
        None,
        "the view has no target: an unresolvable recording is not a target"
    );
    assert_eq!(
        ws.missing_target_ref_name().map(ToOwned::to_owned),
        Some("refs/remotes/origin/gone".try_into()?),
        "...but the configured name is reported, so a caller can say the upstream is gone"
    );

    Ok(())
}

/// The other half: no target recorded at all is NOT a missing upstream — there is nothing to
/// report, and conflating the two would nag every workspace that never had a target.
#[test]
fn no_configured_target_is_not_a_missing_one() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;
    add_workspace_without_target(&mut meta);

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        super::no_target_meta(),
        &mut but_testsupport::in_memory_db(),
        standard_options(),
    )?
    .validated()?;
    assert_eq!(ws.target_ref_name(), None);
    assert_eq!(
        ws.missing_target_ref_name(),
        None,
        "nothing was configured, so nothing is missing"
    );

    Ok(())
}
