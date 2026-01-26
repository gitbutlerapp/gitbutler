use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{GraphExt, testing::Testing as _};
use but_testsupport::{graph_tree, visualize_commit_graph_all};

use crate::utils::{fixture, standard_options};

#[test]
fn four_commits() -> Result<()> {
    let (repo, meta) = fixture("four-commits")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_ascii(), @r"
    ● refs/heads/main
    ● 120e3a9 c
    ● a96434e b
    ● d591dfe a
    ● 35b8235 base
    ╵
    ");

    Ok(())
}

#[test]
fn merge_in_the_middle() -> Result<()> {
    let (repo, meta) = fixture("merge-in-the-middle")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * e8ee978 (HEAD -> with-inner-merge) on top of inner merge
    *   2fc288c Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_ascii(), @r"
    ● refs/heads/with-inner-merge
    ● e8ee978 on top of inner merge
    ● 2fc288c Merge branch 'B' into with-inner-merge
    ├─╮
    ● │ refs/heads/A
    ● │ add59d2 A: 10 lines on top
    │ ● refs/heads/B
    │ ● 984fd1c C: new file with 10 lines
    ├─╯
    ● refs/heads/main
    ● refs/tags/base
    ● 8f0d338 base
    ╵
    ");

    Ok(())
}

#[test]
fn three_branches_merged() -> Result<()> {
    let (repo, meta) = fixture("three-branches-merged")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
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
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_ascii(), @r"
    ● refs/heads/main
    ● 1348870 Merge branches 'A', 'B' and 'C'
    ├─┬─╮
    ● │ │ refs/heads/A
    ● │ │ add59d2 A: 10 lines on top
    │ ● │ refs/heads/B
    │ ● │ a748762 B: another 10 lines at the bottom
    │ ● │ 62e05ba B: 10 lines at the bottom
    │ │ ● refs/heads/C
    │ │ ● 930563a C: add another 10 lines to new file
    │ │ ● 68a2fc3 C: add 10 lines to new file
    │ │ ● 984fd1c C: new file with 10 lines
    ├─┴─╯
    ● refs/tags/base
    ● 8f0d338 base
    ╵
    ");

    Ok(())
}

#[test]
fn many_references() -> Result<()> {
    let (repo, meta) = fixture("many-references")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe (Z, Y, X) a
    * 35b8235 base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:main[🌳]
        ├── ·120e3a9 (⌂|1)
        ├── ·a96434e (⌂|1)
        ├── ·d591dfe (⌂|1) ►X, ►Y, ►Z
        └── ·35b8235 (⌂|1)
    ");

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_ascii(), @r"
    ● refs/heads/main
    ● 120e3a9 c
    ● a96434e b
    ● refs/heads/X
    ● refs/heads/Y
    ● refs/heads/Z
    ● d591dfe a
    ● 35b8235 base
    ╵
    ");

    Ok(())
}

#[test]
fn first_parent_leg_long() -> Result<()> {
    let (repo, meta) = fixture("first-parent-leg-long")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 6ac5745 (HEAD -> with-inner-merge) on top of inner merge
    *   d20f547 Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | 198d2e4 (A) A: 10 more more lines on top
    * | 7325853 A: 10 more lines on top
    * | add59d2 A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:with-inner-merge[🌳]
        └── ·6ac5745 (⌂|1)
            └── ►:1[1]:anon:
                └── ·d20f547 (⌂|1)
                    ├── ►:2[2]:A
                    │   ├── ·198d2e4 (⌂|1)
                    │   ├── ·7325853 (⌂|1)
                    │   └── ·add59d2 (⌂|1)
                    │       └── ►:4[3]:main
                    │           └── ·8f0d338 (⌂|1) ►tags/base
                    └── ►:3[2]:B
                        └── ·984fd1c (⌂|1)
                            └── →:4: (main)
    ");

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_ascii(), @r"
    ● refs/heads/with-inner-merge
    ● 6ac5745 on top of inner merge
    ● d20f547 Merge branch 'B' into with-inner-merge
    ├─╮
    ● │ refs/heads/A
    ● │ 198d2e4 A: 10 more more lines on top
    ● │ 7325853 A: 10 more lines on top
    ● │ add59d2 A: 10 lines on top
    │ ● refs/heads/B
    │ ● 984fd1c C: new file with 10 lines
    ├─╯
    ● refs/heads/main
    ● refs/tags/base
    ● 8f0d338 base
    ╵
    ");

    Ok(())
}

#[test]
fn second_parent_leg_long() -> Result<()> {
    let (repo, meta) = fixture("second-parent-leg-long")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a6775ea (HEAD -> with-inner-merge) on top of inner merge
    *   b85214b Merge branch 'B' into with-inner-merge
    |\  
    | * f87f875 (B) C: 10 more more lines on top
    | * cb181a0 C: 10 more lines on top
    | * 984fd1c C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:with-inner-merge[🌳]
        └── ·a6775ea (⌂|1)
            └── ►:1[1]:anon:
                └── ·b85214b (⌂|1)
                    ├── ►:2[2]:A
                    │   └── ·add59d2 (⌂|1)
                    │       └── ►:4[3]:main
                    │           └── ·8f0d338 (⌂|1) ►tags/base
                    └── ►:3[2]:B
                        ├── ·f87f875 (⌂|1)
                        ├── ·cb181a0 (⌂|1)
                        └── ·984fd1c (⌂|1)
                            └── →:4: (main)
    ");

    let editor = graph.to_editor(&repo)?;

    insta::assert_snapshot!(editor.steps_ascii(), @r"
    ● refs/heads/with-inner-merge
    ● a6775ea on top of inner merge
    ● b85214b Merge branch 'B' into with-inner-merge
    ├─╮
    ● │ refs/heads/A
    ● │ add59d2 A: 10 lines on top
    │ ● refs/heads/B
    │ ● f87f875 C: 10 more more lines on top
    │ ● cb181a0 C: 10 more lines on top
    │ ● 984fd1c C: new file with 10 lines
    ├─╯
    ● refs/heads/main
    ● refs/tags/base
    ● 8f0d338 base
    ╵
    ");

    Ok(())
}
