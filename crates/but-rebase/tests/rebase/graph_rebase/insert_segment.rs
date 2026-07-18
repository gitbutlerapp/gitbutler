//! These tests exercise the insert segment operation.
use anyhow::{Context, Result};
use bstr::ByteSlice;
use but_rebase::graph_rebase::{Editor, mutate};
use but_testsupport::{git_status, graph_tree, visualize_commit_graph, visualize_commit_graph_all};
use snapbox::IntoData;

use crate::utils::fixture_writable;

fn parent_subjects(repo: &gix::Repository, rev: &str) -> Result<Vec<String>> {
    let commit = repo.rev_parse_single(rev)?.object()?.peel_to_commit()?;
    commit
        .parent_ids()
        .map(|parent_id| {
            let parent = parent_id.object()?.peel_to_commit()?;
            let subject = parent
                .message_raw()?
                .as_bstr()
                .lines()
                .next()
                .unwrap_or_default()
                .to_str_lossy()
                .into_owned();
            Ok(subject)
        })
        .collect()
}
#[test]
fn insert_single_node_segment_above() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
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
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = "refs/heads/B".try_into()?;
    let b_selector = editor
        .select_reference(b)
        .context("Failed to find reference b in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_segment(b_selector, delimiter, mutate::InsertSide::Above)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  main[🌳]
●    👉·ee7f107 (→)
├─╮
◎ │  A
● │    ·69221b4 (→)
├───╮
◎ │ │  B
● │ │  ·a748762 (→)
● │ │  ·62e05ba (→)
├───╯
│ ◎  C
│ ●  ·930563a (→)
│ ●  ·68a2fc3 (→)
│ ●  ·984fd1c (→)
├─╯
●  🏁·8f0d338 (→)

"#]]
    );
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   ee7f107 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\
| * 930563a (C) C: add another 10 lines to new file
| * 68a2fc3 C: add 10 lines to new file
| * 984fd1c C: new file with 10 lines
* | 69221b4 (A) A: 10 lines on top
|\|
* | a748762 (B) B: another 10 lines at the bottom
* | 62e05ba B: 10 lines at the bottom
|/
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}
#[test]
fn insert_single_node_segment_below() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
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
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_segment(b_selector, delimiter, mutate::InsertSide::Below)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  A
│ ◎  main[🌳]
│ ●    👉·b005f3c (→)
╭─┼─╮
│ ◎ │  B
│ ● │  ·a3301fe (→)
├─╯ │
●   │  ·7f0cc55 (→)
├─╮ │
● │ │  ·62e05ba (→)
├─╯ │
│   ◎  C
│   ●  ·930563a (→)
│   ●  ·68a2fc3 (→)
│   ●  ·984fd1c (→)
├───╯
●  🏁·8f0d338 (→)

"#]]
    );
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   b005f3c (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
| * | a3301fe (B) B: another 10 lines at the bottom
|/ /
* | 7f0cc55 (A) A: 10 lines on top
|\|
* | 62e05ba B: 10 lines at the bottom
|/
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}
#[test]
fn insert_multi_node_segment_above() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
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
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let b_parent = repo.rev_parse_single("B~")?.detach();
    let b_parent_selector = editor
        .select_commit(b_parent)
        .context("Failed to find parent of commit b in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: b_selector,
        parent: b_parent_selector,
    };

    editor.insert_segment(a_selector, delimiter, mutate::InsertSide::Above)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  A
│ ◎  B
├─╯
│ ◎  main[🌳]
│ ●  👉·61b2679 (→)
╭─┤
● │  ·758c8a3 (→)
● │    ·db40ffc (→)
├───╮
● │ │  ·add59d2 (→)
├───╯
│ ◎  C
│ ●  ·930563a (→)
│ ●  ·68a2fc3 (→)
│ ●  ·984fd1c (→)
├─╯
●  🏁·8f0d338 (→)

"#]]
    );
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   61b2679 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\
| * 930563a (C) C: add another 10 lines to new file
| * 68a2fc3 C: add 10 lines to new file
| * 984fd1c C: new file with 10 lines
* | 758c8a3 (B, A) B: another 10 lines at the bottom
* | db40ffc B: 10 lines at the bottom
|\|
* | add59d2 A: 10 lines on top
|/
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}

#[test]
fn insert_multi_node_segment_below() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
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
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let b_parent = repo.rev_parse_single("B~")?.detach();
    let b_parent_selector = editor
        .select_commit(b_parent)
        .context("Failed to find parent of commit b in editor graph")?;
    let base = repo.rev_parse_single("base")?.detach();
    let base_selector = editor
        .select_commit(base)
        .context("Failed to find base commit in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: b_selector,
        parent: b_parent_selector,
    };

    editor.insert_segment(a_selector, delimiter, mutate::InsertSide::Below)?;
    let mut inserted_parents = editor.direct_parents(b_parent_selector)?;
    inserted_parents.sort_by_key(|(_, order)| *order);
    assert_eq!(
        inserted_parents,
        [(base_selector, 0)],
        "the shared base parent should occur once in first-parent position"
    );

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  B
│ ◎  main[🌳]
│ ●    👉·4db28a9 (→)
╭─┼─╮
│ ◎ │  A
│ ● │  ·71dfc8f (→)
├─╯ │
●   │  ·a748762 (→)
●   │  ·62e05ba (→)
│   ◎  C
│   ●  ·930563a (→)
│   ●  ·68a2fc3 (→)
│   ●  ·984fd1c (→)
├───╯
●  🏁·8f0d338 (→)

"#]]
    );
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   4db28a9 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
* | | 71dfc8f (A) A: 10 lines on top
|/ /
* | a748762 (B) B: another 10 lines at the bottom
* | 62e05ba B: 10 lines at the bottom
|/
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}

#[test]
fn insert_single_node_segment_above_with_explicit_children() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
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
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_selector = editor
        .select_commit(c)
        .context("Failed to find commit c in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_segment_into(
        b_selector,
        delimiter,
        mutate::InsertSide::Above,
        Some(mutate::SomeSelectors::new(vec![c_selector])?),
        mutate::ParentReparentingOrder::Prepend,
    )?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  A
│ ◎  B
│ │ ◎  main[🌳]
│ │ ●  👉·cca953f (→)
╭─┬─┤
│ │ ◎  C
│ │ ●  ·76e2160 (→)
╭───┤
│ │ ●  ·68a2fc3 (→)
│ │ ●  ·984fd1c (→)
● │ │  ·69221b4 (→)
╰─┬─╮
  ● │  ·a748762 (→)
  ● │  ·62e05ba (→)
  ├─╯
  ●  🏁·8f0d338 (→)

"#]]
    );
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   cca953f (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \
| | *   76e2160 (C) C: add another 10 lines to new file
| | |\
| |_|/
|/| |
* | |   69221b4 (A) A: 10 lines on top
|\ \ \
| |/ /
|/| |
* | | a748762 (B) B: another 10 lines at the bottom
* | | 62e05ba B: 10 lines at the bottom
|/ /
| * 68a2fc3 C: add 10 lines to new file
| * 984fd1c C: new file with 10 lines
|/
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}

#[test]
fn insert_single_node_segment_below_with_explicit_parents() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
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
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_selector = editor
        .select_commit(c)
        .context("Failed to find commit c in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: b_selector,
        parent: b_selector,
    };

    editor.insert_segment_into(
        a_selector,
        delimiter,
        mutate::InsertSide::Below,
        Some(mutate::SomeSelectors::new(vec![c_selector])?),
        mutate::ParentReparentingOrder::Prepend,
    )?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_workspace()?.graph).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"
◎  B
│ ◎  C
│ │ ◎  main[🌳]
│ │ ●  👉·54f9cab (→)
╭─┬─┤
│ │ ◎  A
│ │ ●  ·9501727 (→)
╭───┤
● │ │  ·347772f (→)
├─╮ │
│ ● │  ·930563a (→)
│ ● │  ·68a2fc3 (→)
│ ● │  ·984fd1c (→)
│ ├─╯
● │  ·62e05ba (→)
├─╯
●  🏁·8f0d338 (→)

"#]]
    );
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());
    assert_eq!(
        parent_subjects(&repo, "B")?,
        [
            "C: add another 10 lines to new file",
            "B: 10 lines at the bottom"
        ]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   54f9cab (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \
* | | 9501727 (A) A: 10 lines on top
|\| |
| * |   347772f (B) B: another 10 lines at the bottom
| |\ \
| | |/
| |/|
| | * 62e05ba B: 10 lines at the bottom
| |/
|/|
| * 930563a (C) C: add another 10 lines to new file
| * 68a2fc3 C: add 10 lines to new file
| * 984fd1c C: new file with 10 lines
|/
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}

#[test]
fn insert_single_node_segment_below_can_append_reparented_parent() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    let graph = but_graph::Graph::from_repo(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Overlay::default(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_selector = editor
        .select_commit(c)
        .context("Failed to find commit c in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: b_selector,
        parent: b_selector,
    };

    editor.insert_segment_into(
        a_selector,
        delimiter,
        mutate::InsertSide::Below,
        Some(mutate::SomeSelectors::new(vec![c_selector])?),
        mutate::ParentReparentingOrder::Append,
    )?;

    editor.rebase()?.materialize()?;
    assert_eq!(
        parent_subjects(&repo, "B")?,
        [
            "B: 10 lines at the bottom",
            "C: add another 10 lines to new file"
        ]
    );

    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}
