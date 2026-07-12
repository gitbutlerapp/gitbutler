//! Change id tests

use anyhow::Result;
use but_graph::Workspace;
use but_rebase::graph_rebase::{Editor, LookupStep, Step, ToSelector};
use gix::prelude::ObjectIdExt;
use snapbox::prelude::*;

use crate::utils::{fixture_writable, standard_options};

#[test]
fn temporary_change_id_persisted() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

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

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    // An operation to cause the parent we care about to be rebased
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;
    let target_selector = target.to_selector(&editor)?;
    editor.replace(target_parent, Step::None)?;

    let outcome = editor.rebase()?;

    let new_target = outcome.lookup_pick(target_selector)?;
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
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

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
