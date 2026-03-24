//! Some tests that expliclity test the overlay functionality

use but_graph::{Graph, init::Overlay};
use but_testsupport::{graph_tree, visualize_commit_graph_all};

use crate::init::{read_only_in_memory_scenario, standard_options};

#[test]
fn drop_and_add_regular_refs() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
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
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:merged[🌳]
        └── ·8a6c109 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·592abec (⌂|1)
            │       │       └── ►:7[3]:main
            │       │           └── ·965998b (⌂|1)
            │       └── ►:4[2]:B
            │           └── ·f16dddf (⌂|1)
            │               └── →:7: (main)
            └── ►:2[1]:C
                └── ·7ed512a (⌂|1)
                    ├── ►:5[2]:anon:
                    │   └── ·35ee481 (⌂|1)
                    │       └── →:7: (main)
                    └── ►:6[2]:D
                        └── ·ecb1877 (⌂|1)
                            └── →:7: (main)
    ");

    let to_reference = repo.rev_parse_single("35ee481")?;

    let overlay = Overlay::default()
        .with_references([gix::refs::Reference {
            name: "refs/heads/new-reference".try_into()?,
            target: gix::refs::Target::Object(to_reference.detach()),
            peeled: Some(to_reference.detach()),
        }])
        .with_dropped_references(["refs/heads/C".try_into()?]);

    let graph = graph.redo_traversal_with_overlay(&repo, &*meta, overlay)?;

    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:merged[🌳]
        └── ·8a6c109 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·592abec (⌂|1)
            │       │       └── ►:7[3]:main
            │       │           └── ·965998b (⌂|1)
            │       └── ►:4[2]:B
            │           └── ·f16dddf (⌂|1)
            │               └── →:7: (main)
            └── ►:2[1]:anon:
                └── ·7ed512a (⌂|1)
                    ├── ►:5[2]:new-reference
                    │   └── ·35ee481 (⌂|1)
                    │       └── →:7: (main)
                    └── ►:6[2]:D
                        └── ·ecb1877 (⌂|1)
                            └── →:7: (main)
    ");

    Ok(())
}

#[test]
fn drop_head_ref() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
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
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:merged[🌳]
        └── ·8a6c109 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·592abec (⌂|1)
            │       │       └── ►:7[3]:main
            │       │           └── ·965998b (⌂|1)
            │       └── ►:4[2]:B
            │           └── ·f16dddf (⌂|1)
            │               └── →:7: (main)
            └── ►:2[1]:C
                └── ·7ed512a (⌂|1)
                    ├── ►:5[2]:anon:
                    │   └── ·35ee481 (⌂|1)
                    │       └── →:7: (main)
                    └── ►:6[2]:D
                        └── ·ecb1877 (⌂|1)
                            └── →:7: (main)
    ");

    let overlay = Overlay::default().with_dropped_references(["refs/heads/merged".try_into()?]);

    let graph = graph.redo_traversal_with_overlay(&repo, &*meta, overlay)?;

    insta::assert_snapshot!(graph_tree(&graph), @"

    └── ►:0[0]:anon:
        └── 👉·8a6c109 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·592abec (⌂|1)
            │       │       └── ►:7[3]:main
            │       │           └── ·965998b (⌂|1)
            │       └── ►:4[2]:B
            │           └── ·f16dddf (⌂|1)
            │               └── →:7: (main)
            └── ►:2[1]:C
                └── ·7ed512a (⌂|1)
                    ├── ►:5[2]:anon:
                    │   └── ·35ee481 (⌂|1)
                    │       └── →:7: (main)
                    └── ►:6[2]:D
                        └── ·ecb1877 (⌂|1)
                            └── →:7: (main)
    ");

    Ok(())
}
