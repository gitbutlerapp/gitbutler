//! These tests demonstrate that if none of the steps are changed, the same
//! graphs are returned.

use anyhow::Result;
use but_testsupport::{graph_tree, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::utils::fixture_writable;

#[test]
fn four_commits() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        &before,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let editor = graph.into_mut(&repo)?;
    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  main[🌳]
●  👉·120e3a9 (→)
●  ·a96434e (→)
●  ·d591dfe (→)
●  🏁·35b8235 (→)

"#]]
    );
    let outcome =
        outcome.materialize_changes(&*meta, but_graph::edit::MaterializeOptions::default())?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    snapbox::assert_data_eq!(
        outcome.commit_mappings.to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );

    Ok(())
}

#[test]
fn merge_in_the_middle() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("merge-in-the-middle")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        &before,
        snapbox::str![[r#"
* e8ee978 (HEAD -> with-inner-merge) on top of inner merge
*   2fc288c Merge branch 'B' into with-inner-merge
|\
| * 984fd1c (B) C: new file with 10 lines
* | add59d2 (A) A: 10 lines on top
|/
* 8f0d338 (tag: base, main) base

"#]]
        .raw()
    );

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let editor = graph.into_mut(&repo)?;
    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  main
│ ◎  with-inner-merge[🌳]
│ ●  👉·e8ee978 (→)
│ ●    ·2fc288c (→)
│ ├─╮
│ ◎ │  A
│ ● │  ·add59d2 (→)
├─╯ │
│   ◎  B
│   ●  ·984fd1c (→)
├───╯
●  🏁·8f0d338 (→)

"#]]
    );
    let outcome =
        outcome.materialize_changes(&*meta, but_graph::edit::MaterializeOptions::default())?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    snapbox::assert_data_eq!(
        outcome.commit_mappings.to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );

    Ok(())
}

#[test]
fn three_branches_merged() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("three-branches-merged")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        &before,
        snapbox::str![[r#"
*-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
| * | a748762 (B) B: another 10 lines at the bottom
| * | 62e05ba B: 10 lines at the bottom
| |/
* / add59d2 (A) A: 10 lines on top
|/
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let editor = graph.into_mut(&repo)?;
    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  main[🌳]
●      👉·1348870 (→)
├─┬─╮
◎ │ │  A
● │ │  ·add59d2 (→)
│ ◎ │  B
│ ● │  ·a748762 (→)
│ ● │  ·62e05ba (→)
├─╯ │
│   ◎  C
│   ●  ·930563a (→)
│   ●  ·68a2fc3 (→)
│   ●  ·984fd1c (→)
├───╯
●  🏁·8f0d338 (→)

"#]]
    );
    let outcome =
        outcome.materialize_changes(&*meta, but_graph::edit::MaterializeOptions::default())?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    snapbox::assert_data_eq!(
        outcome.commit_mappings.to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );

    Ok(())
}
