//! These tests exercise the replace operation.
use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, Step};
use but_testsupport::{git_status, graph_tree, visualize_commit_graph_all, visualize_tree};
use snapbox::prelude::*;

use crate::utils::{fixture_writable, standard_options};

#[test]
fn reword_a_commit() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("merge-in-the-middle")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
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
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let head_tree = repo.head_tree()?.id;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // get the original a
    let a = repo.rev_parse_single("A")?.detach();

    // reword commit a
    let a_obj = repo.find_commit(a)?;
    let mut a_obj = a_obj.decode()?;
    a_obj.message = "A: a second coming".into();
    let a_new = repo.write_object(a_obj)?.detach();

    // select the original a out of the graph
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    // replace it with the new one
    editor.replace(a_selector, Step::new_pick(a_new))?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉►:0[0]:with-inner-merge[🌳]
    └── ·78aaae2 (⌂|1)
        └── ►:1[1]:anon:
            └── ·53af95a (⌂|1)
                ├── ►:2[2]:A
                │   └── ·6de6b92 (⌂|1)
                │       └── ►:4[3]:main
                │           └── 🏁·8f0d338 (⌂|1) ►tags/base
                └── ►:3[2]:B
                    └── ·984fd1c (⌂|1)
                        └── →:4: (main)

"#]]
    );
    let outcome = outcome.materialize(Default::default())?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    assert_eq!(head_tree, repo.head_tree()?.id);

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 78aaae2 (HEAD -> with-inner-merge) on top of inner merge
*   53af95a Merge branch 'B' into with-inner-merge
|\  
| * 984fd1c (B) C: new file with 10 lines
* | 6de6b92 (A) A: a second coming
|/  
* 8f0d338 (tag: base, main) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);
    snapbox::assert_data_eq!(
        outcome.history.commit_mappings().to_debug(),
        snapbox::str![[r#"
{
    Sha1(2fc288c36c8bb710c78203f78ea9883724ce142b): Sha1(53af95adeaf78258ee71c74fe4daa6628d750ff1),
    Sha1(add59d26b2ffd7468fcb44c2db48111dd8f481e5): Sha1(6de6b92e431243cc4676179bd5ef17d95642d250),
    Sha1(e8ee978dac10e6a85006543ef08be07c5824b4f7): Sha1(78aaae2b4d822ed0cc7e0e83767b5dec2c88791b),
}

"#]]
    );

    Ok(())
}

#[test]
fn amend_a_commit() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("merge-in-the-middle")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
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
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let head_tree = repo.head_tree()?.id();
    snapbox::assert_data_eq!(visualize_tree(head_tree).to_string(), snapbox::str![[r#"
f766d1f
├── added-after-with-inner-merge:100644:861be1b "seq 10\n"
├── file:100644:d78dd4f "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
└── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"

"#]].raw());

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // get the original a
    let a = repo.rev_parse_single("A")?;
    snapbox::assert_data_eq!(visualize_tree(a).to_string(), snapbox::str![[r#"
0cc630c
└── file:100644:d78dd4f "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"

"#]].raw());

    // reword commit a
    let mut a_obj = but_core::Commit::from_id(a)?;

    let mut builder = repo.edit_tree(a_obj.tree)?;
    let new_blob = repo.write_blob("I'm a new file :D\n")?;
    builder.upsert("new-file.txt", gix::objs::tree::EntryKind::Blob, new_blob)?;
    let tree = builder.write()?;

    a_obj.tree = tree.detach();
    a_obj.message = "A: a second coming".into();
    let a_new = repo.write_object(a_obj.inner)?.detach();

    // select the original a out of the graph
    let a_selector = editor
        .select_commit(a.detach())
        .context("Failed to find commit a in editor graph")?;
    // replace it with the new one
    editor.replace(a_selector, Step::new_pick(a_new))?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉►:0[0]:with-inner-merge[🌳]
    └── ·e7221b5 (⌂|1)
        └── ►:1[1]:anon:
            └── ·8101192 (⌂|1)
                ├── ►:2[2]:A
                │   └── ·f1905a8 (⌂|1)
                │       └── ►:4[3]:main
                │           └── 🏁·8f0d338 (⌂|1) ►tags/base
                └── ►:3[2]:B
                    └── ·984fd1c (⌂|1)
                        └── →:4: (main)

"#]]
    );
    let outcome = outcome.materialize(Default::default())?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e7221b5 (HEAD -> with-inner-merge) on top of inner merge
*   8101192 Merge branch 'B' into with-inner-merge
|\  
| * 984fd1c (B) C: new file with 10 lines
* | f1905a8 (A) A: a second coming
|/  
* 8f0d338 (tag: base, main) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);
    snapbox::assert_data_eq!(
        outcome.history.commit_mappings().to_debug(),
        snapbox::str![[r#"
{
    Sha1(2fc288c36c8bb710c78203f78ea9883724ce142b): Sha1(810119232dd43ad1edc6b3d1a9cc2cd507d92a4e),
    Sha1(add59d26b2ffd7468fcb44c2db48111dd8f481e5): Sha1(f1905a822d4cad49595b47f24d40702dc41a0b57),
    Sha1(e8ee978dac10e6a85006543ef08be07c5824b4f7): Sha1(e7221b5ace99ba38e222e19e5da9c6966955e37b),
}

"#]]
    );

    // A should include our extra blob
    let a = repo.rev_parse_single("A")?;
    snapbox::assert_data_eq!(visualize_tree(a).to_string(), snapbox::str![[r#"
0c482d4
├── file:100644:d78dd4f "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
└── new-file.txt:100644:715faaf "I\'m a new file :D\n"

"#]].raw());

    // New head tree should also include our extra blob
    let new_head_tree = repo.head_tree()?.id();
    snapbox::assert_data_eq!(visualize_tree(new_head_tree).to_string(), snapbox::str![[r#"
89042ca
├── added-after-with-inner-merge:100644:861be1b "seq 10\n"
├── file:100644:d78dd4f "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
├── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
└── new-file.txt:100644:715faaf "I\'m a new file :D\n"

"#]].raw());

    Ok(())
}

#[test]
#[ignore]
fn replaces_violating_fp_protection_should_cause_rebase_failure() -> Result<()> {
    panic!("Branch protection hasn't been implemented yet");
}
