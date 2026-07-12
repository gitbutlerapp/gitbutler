//! Some tests that explicitly test the overlay functionality

use but_graph::{Workspace, walk::Overlay};
use but_testsupport::{graph_dag, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::walk::{read_only_in_memory_scenario, standard_options};

#[test]
fn drop_and_add_regular_refs() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   8a6c109 (HEAD -> merged) Merge branch 'C' into merged
|\  
| *   7ed512a (C) Merge branch 'D' into C
| |\  
| | * ecb1877 (D) D
| * | 35ee481 C
| |/  
* |   62b409a (A) Merge branch 'B' into A
|\ \  
| * | f16dddf (B) B
| |/  
* / 592abec A
|/  
* 965998b (main) base

"#]]
        .raw()
    );

    let graph = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂) ►merged[🌳]
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂)
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂)
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂) ►main
"#]]
    );

    let to_reference = repo.rev_parse_single("35ee481")?;

    let overlay = Overlay::default()
        .with_references([gix::refs::Reference {
            name: "refs/heads/new-reference".try_into()?,
            target: gix::refs::Target::Object(to_reference.detach()),
            peeled: Some(to_reference.detach()),
        }])
        .with_dropped_references(["refs/heads/C".try_into()?]);

    let graph = graph.redo(&repo, &*meta, overlay)?;

    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂) ►merged[🌳]
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂)
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂)
│ ├─╮
│ * │  ·35ee481 (⌂) ►new-reference
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂) ►main
"#]]
    );

    Ok(())
}

#[test]
fn drop_head_ref() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   8a6c109 (HEAD -> merged) Merge branch 'C' into merged
|\  
| *   7ed512a (C) Merge branch 'D' into C
| |\  
| | * ecb1877 (D) D
| * | 35ee481 C
| |/  
* |   62b409a (A) Merge branch 'B' into A
|\ \  
| * | f16dddf (B) B
| |/  
* / 592abec A
|/  
* 965998b (main) base

"#]]
        .raw()
    );

    let graph = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂) ►merged[🌳]
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂)
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂)
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂) ►main
"#]]
    );

    let overlay = Overlay::default().with_dropped_references(["refs/heads/merged".try_into()?]);

    let graph = graph.redo(&repo, &*meta, overlay)?;

    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂)
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂)
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂)
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂) ►main
"#]]
    );

    Ok(())
}

#[test]
fn overriding_references() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   8a6c109 (HEAD -> merged) Merge branch 'C' into merged
|\  
| *   7ed512a (C) Merge branch 'D' into C
| |\  
| | * ecb1877 (D) D
| * | 35ee481 C
| |/  
* |   62b409a (A) Merge branch 'B' into A
|\ \  
| * | f16dddf (B) B
| |/  
* / 592abec A
|/  
* 965998b (main) base

"#]]
        .raw()
    );

    let graph = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂) ►merged[🌳]
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂)
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂)
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂) ►main
"#]]
    );

    let merged_a = repo.rev_parse_single("35ee481")?;
    let merged_b = repo.rev_parse_single("592abec")?;
    let merged: gix::refs::FullName = "refs/heads/merged".try_into()?;

    // The dropped takes precedence over git or overriding references.
    let overlay = Overlay::default()
        .with_dropped_references([merged.clone()])
        .with_references([
            gix::refs::Reference {
                name: merged.clone(),
                target: gix::refs::Target::Object(merged_a.detach()),
                peeled: Some(merged_a.detach()),
            },
            gix::refs::Reference {
                name: merged.clone(),
                target: gix::refs::Target::Object(merged_b.detach()),
                peeled: Some(merged_b.detach()),
            },
        ]);

    let graph = graph.redo(&repo, &*meta, overlay)?;

    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂)
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂)
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂)
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂) ►main
"#]]
    );

    // The first overriding reference precedence over git or other overriding references.
    let overlay = Overlay::default().with_references([
        gix::refs::Reference {
            name: merged.clone(),
            target: gix::refs::Target::Object(merged_a.detach()),
            peeled: Some(merged_a.detach()),
        },
        gix::refs::Reference {
            name: merged.clone(),
            target: gix::refs::Target::Object(merged_b.detach()),
            peeled: Some(merged_b.detach()),
        },
    ]);

    let graph = graph.redo(&repo, &*meta, overlay)?;

    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂)
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂)
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂) ►merged[🌳]
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂) ►main
"#]]
    );

    // overriding references take precedence over git.
    let overlay = Overlay::default().with_references([gix::refs::Reference {
        name: merged.clone(),
        target: gix::refs::Target::Object(merged_b.detach()),
        peeled: Some(merged_b.detach()),
    }]);

    let graph = graph.redo(&repo, &*meta, overlay)?;

    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂)
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂) ►merged[🌳]
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂)
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂) ►main
"#]]
    );

    Ok(())
}
