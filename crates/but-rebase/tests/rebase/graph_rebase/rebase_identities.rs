//! These tests demonstrate that if none of the steps are changed, the same
//! graphs are returned.

use anyhow::Result;
use but_graph::Workspace;
use but_rebase::graph_rebase::Editor;
use but_testsupport::{graph_dag, graph_workspace, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::utils::{fixture_writable, standard_options};

#[test]
fn four_commits() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        before.as_str(),
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );

    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;
    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.redo(outcome.repo(), outcome.meta(), outcome.rebase_overlay()?)?);
    snapbox::assert_data_eq!(
        overlayed.as_str(),
        snapbox::str![[r#"
*  👉·120e3a9 (⌂) ►main[🌳]
*  ·a96434e (⌂)
*  ·d591dfe (⌂)
*  🏁·35b8235 (⌂)
"#]]
    );
    let outcome = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome.arena().clone(), &repo, outcome.meta)?;
    assert_eq!(overlayed, graph_dag(&ws));

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    snapbox::assert_data_eq!(
        outcome.history.commit_mappings().to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );

    Ok(())
}

#[test]
fn four_commits_with_short_traversal() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        before.as_str(),
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );

    let options = standard_options().with_hard_limit(4);
    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        options,
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]
        ├── ·120e3a9
        ├── ·a96434e
        ├── ·d591dfe
        └── ·35b8235

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;
    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.redo(outcome.repo(), outcome.meta(), outcome.rebase_overlay()?)?);
    snapbox::assert_data_eq!(
        overlayed.as_str(),
        snapbox::str![[r#"
*  👉·120e3a9 (⌂) ►main[🌳]
*  ·a96434e (⌂)
*  ·d591dfe (⌂)
*  🏁·35b8235 (⌂)
"#]]
    );
    let outcome = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome.arena().clone(), &repo, outcome.meta)?;
    assert_eq!(overlayed, graph_dag(&ws));

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    snapbox::assert_data_eq!(
        outcome.history.commit_mappings().to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );

    Ok(())
}

#[test]
fn merge_in_the_middle() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("merge-in-the-middle")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        before.as_str(),
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

    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;
    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.redo(outcome.repo(), outcome.meta(), outcome.rebase_overlay()?)?);
    snapbox::assert_data_eq!(
        overlayed.as_str(),
        snapbox::str![[r#"
*  👉·e8ee978 (⌂) ►with-inner-merge[🌳]
*    ·2fc288c (⌂)
├─╮
* │  ·add59d2 (⌂) ►A
│ *  ·984fd1c (⌂) ►B
├─╯
*  🏁·8f0d338 (⌂) ►main, ►tags/base
"#]]
    );
    let outcome = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome.arena().clone(), &repo, outcome.meta)?;
    assert_eq!(overlayed, graph_dag(&ws));

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    snapbox::assert_data_eq!(
        outcome.history.commit_mappings().to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );

    Ok(())
}

#[test]
fn three_branches_merged() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("three-branches-merged")?;

    let before = visualize_commit_graph_all(&repo)?;
    snapbox::assert_data_eq!(
        before.as_str(),
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

    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;
    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.redo(outcome.repo(), outcome.meta(), outcome.rebase_overlay()?)?);
    snapbox::assert_data_eq!(
        overlayed.as_str(),
        snapbox::str![[r#"
*      👉·1348870 (⌂) ►main[🌳]
├─┬─╮
* │ │  ·add59d2 (⌂) ►A
│ * │  ·a748762 (⌂) ►B
│ * │  ·62e05ba (⌂)
├─╯ │
│   *  ·930563a (⌂) ►C
│   *  ·68a2fc3 (⌂)
│   *  ·984fd1c (⌂)
├───╯
*  🏁·8f0d338 (⌂) ►tags/base
"#]]
    );
    let outcome = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome.arena().clone(), &repo, outcome.meta)?;
    assert_eq!(overlayed, graph_dag(&ws));

    assert_eq!(visualize_commit_graph_all(&repo)?, before);
    snapbox::assert_data_eq!(
        outcome.history.commit_mappings().to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );

    Ok(())
}
