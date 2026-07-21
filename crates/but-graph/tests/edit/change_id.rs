//! Change id tests

use anyhow::Result;
use but_graph::edit::ToSelector;
use gix::prelude::ObjectIdExt;
use snapbox::prelude::*;

use crate::utils::fixture_writable;

#[test]
fn temporary_change_id_persisted() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits")?;

    let target = repo.rev_parse_single("HEAD~")?;
    let target_parent = repo.rev_parse_single("HEAD~~")?;
    let target_commit = but_core::Commit::from_id(target)?;
    snapbox::assert_data_eq!(
        target_commit.change_id().to_string(),
        snapbox::str!["uonoxlzsyllzwskypkxkwtqyzusvwpzp"]
    );
    snapbox::assert_data_eq!(
        target_commit.extra_headers.to_debug(),
        snapbox::str![[r#"
[]

"#]]
    );

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    // An operation to cause the parent we care about to be rebased
    let mut editor = graph.into_mut(&repo)?;
    let target_selector = target.to_selector(&editor)?;
    editor.remove(target_parent)?;

    let outcome = editor.rebase()?;

    let new_target = crate::pick_id(outcome.pick_at(target_selector));
    let new_target_commit = but_core::Commit::from_id(new_target.attach(outcome.repo()))?;
    snapbox::assert_data_eq!(
        new_target_commit.extra_headers.to_debug(),
        snapbox::str![[r#"
[
    (
        "gitbutler-headers-version",
        "2",
    ),
    (
        "change-id",
        "uonoxlzsyllzwskypkxkwtqyzusvwpzp",
    ),
]

"#]]
    );

    assert_eq!(
        new_target_commit.change_id(),
        target_commit.change_id(),
        "The change ID should remain and end up persisted in the output commit's headers"
    );

    Ok(())
}

#[test]
fn empty_commit_uses_default_change_id() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits")?;

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;

    let editor = graph.into_mut(&repo)?;

    let ec = editor.empty_commit()?;

    snapbox::assert_data_eq!(ec.change_id().to_string(), snapbox::str!["1"]);
    snapbox::assert_data_eq!(
        ec.extra_headers.to_debug(),
        snapbox::str![[r#"
[
    (
        "gitbutler-headers-version",
        "2",
    ),
    (
        "change-id",
        "1",
    ),
]

"#]]
    );

    Ok(())
}
