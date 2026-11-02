use anyhow::Result;
use but_evo::MarkerStore as _;
use but_graph::Graph;
use but_rebase::graph_rebase::Editor;
use snapbox::prelude::*;

use crate::utils::{fixture_writable, standard_options};

#[test]
fn materializing_writes_markers_for_the_rewrites() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let mut reworded = repo.find_commit(a)?.decode()?.to_owned()?;
    reworded.message = "a, reworded".into();
    let reworded = repo.write_object(reworded)?.detach();
    editor.amend_pick(a, reworded)?;
    let rebase = editor.rebase()?;
    drop(rebase);

    assert_eq!(
        db.find_by_prec(a)?,
        [],
        "a rebase that isn't materialized has only been a dry run"
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    editor.amend_pick(a, reworded)?;
    editor.rebase()?.materialize(Default::default())?;

    // The description changed, and with it nothing else about the commit.
    snapbox::assert_data_eq!(
        db.find_by_prec(a)?.to_debug(),
        snapbox::str![[r#"
[
    Marker {
        prec: Sha1(d591dfed1777b8f00f5b7b6f427537eeb5878178),
        succs: [
            Sha1(2dba176f6bd27b5078587cef1731828016f7bc1f),
        ],
        meta: {
            "ef1": "1",
            "user": "Committer (Memory Override) <committer@example.com>",
        },
        date: Time {
            seconds: 946771200,
            offset: 0,
        },
        parents: Some(
            [
                Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
            ],
        ),
    },
]

"#]]
    );

    Ok(())
}

#[test]
fn rewording_replaces_the_commit_and_those_above_it() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let mut reworded = repo.find_commit(a)?.decode()?.to_owned()?;
    reworded.message = "a, reworded".into();
    let reworded = repo.write_object(reworded)?.detach();
    editor.amend_pick(a, reworded)?;

    let outcome = editor.rebase()?;

    // Nothing below the reworded commit changed, so `base` has no rewrite.
    snapbox::assert_data_eq!(
        outcome.rewrites()?.to_debug(),
        snapbox::str![[r#"
[
    Replace {
        prec: Sha1(120e3a90b753a492cef9a552ae3b9ba1f1391362),
        succ: Sha1(1810728e1fea0976b59daf4a9d021ae3b146eae6),
    },
    Replace {
        prec: Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
        succ: Sha1(3e7a6a5d5ec46953396df0da25bfb4aba9776a0f),
    },
    Replace {
        prec: Sha1(d591dfed1777b8f00f5b7b6f427537eeb5878178),
        succ: Sha1(2dba176f6bd27b5078587cef1731828016f7bc1f),
    },
]

"#]]
    );

    Ok(())
}
