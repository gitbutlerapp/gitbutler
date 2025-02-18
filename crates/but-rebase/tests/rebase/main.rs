use crate::utils::{
    assure_nonconflicting, conflicted, fixture_writable, four_commits_writable, visualize_tree,
};
use anyhow::Result;
use bstr::ByteSlice;
use but_rebase::{Rebase, RebaseStep};
use but_testsupport::{assure_stable_env, visualize_commit_graph};
use gix::prelude::ObjectIdExt;

mod error_handling;

#[test]
fn single_stack_journey() -> Result<()> {
    assure_stable_env();
    let (repo, commits, _tmp) = four_commits_writable()?;
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let out = builder
        .steps([
            RebaseStep::Pick {
                commit_id: commits.a,
                new_message: Some("first step: pick a".into()),
            },
            RebaseStep::SquashIntoPreceding {
                commit_id: commits.b,
                new_message: Some("second step: squash b into a".into()),
            },
            RebaseStep::Reference(but_core::Reference::Virtual("anchor".into())),
        ])?
        .rebase()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");
    // The base remains unchanged, and two commits remain: a squash commit and a merge with
    // the original `c` commit.
    insta::assert_snapshot!(visualize_commit_graph(&repo, out.top_commit)?, @r"
    * a466bf8 second step: squash b into a
    * 35b8235 base
    ");

    // The reference points to the commit and correctly refers to the one that was fixed up.
    insta::assert_debug_snapshot!(out, @r#"
    RebaseOutput {
        top_commit: Sha1(a466bf82eed2e6aa725eb61a85cc73281fc02960),
        references: [
            ReferenceSpec {
                reference: Virtual(
                    "anchor",
                ),
                commit_id: Sha1(a466bf82eed2e6aa725eb61a85cc73281fc02960),
                previous_commit_id: Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
            },
        ],
        commit_mapping: [
            (
                Some(
                    Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                ),
                Sha1(d591dfed1777b8f00f5b7b6f427537eeb5878178),
                Sha1(5c028a33efc5184dc46db016d9567ab79bc3e348),
            ),
            (
                Some(
                    Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                ),
                Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
                Sha1(a466bf82eed2e6aa725eb61a85cc73281fc02960),
            ),
            (
                Some(
                    Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                ),
                Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
                Sha1(a466bf82eed2e6aa725eb61a85cc73281fc02960),
            ),
        ],
    }
    "#);
    assure_nonconflicting(&repo, &out)?;

    assert_eq!(
        builder.rebase().unwrap_err().to_string(),
        "No rebase steps provided",
        "The builder (and its base) can be reused, but it needs new steps"
    );
    Ok(())
}

#[test]
fn amended_commit() -> Result<()> {
    assure_stable_env();
    let (repo, _tmp) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    let mut builder = Rebase::new(&repo, repo.rev_parse_single("C~1")?.detach(), None)?;
    let out = builder
        .steps([
            // Pretend we have rewritten the commit at the tip of C.
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("C")?.into(),
                new_message: Some("C: add another 10 lines to new file - amended".into()),
            },
            // Picking a merge commit means to repeat the merge with the latest rewritten commit
            // from the previous step.
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("main")?.into(),
                new_message: Some("Merge branches 'A', 'B' and 'C' - rewritten".into()),
            },
        ])?
        .rebase()?;
    // Note how the `C` isn't visible anymore as we don't rewrite reference here.
    insta::assert_snapshot!(visualize_commit_graph(&repo, out.top_commit)?, @r"
    *-.   7997ae5 Merge branches 'A', 'B' and 'C' - rewritten
    |\ \  
    | | * 39bb1d3 C: add another 10 lines to new file - amended
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    | * | a748762 (B) B: another 10 lines at the bottom
    | * | 62e05ba B: 10 lines at the bottom
    | |/  
    * / add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base) base
    ");
    // This time without anchor.
    insta::assert_debug_snapshot!(out, @r"
    RebaseOutput {
        top_commit: Sha1(7997ae52819cc4ceb88e2e675453bbfb4dd8cd46),
        references: [],
        commit_mapping: [
            (
                Some(
                    Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
                ),
                Sha1(930563a048351f05b14cc7b9c0a48640e5a306b0),
                Sha1(39bb1d32a72c9aead133a0d867879e88ca724fcb),
            ),
            (
                Some(
                    Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
                ),
                Sha1(134887021e06909021776c023a608f8ef179e859),
                Sha1(7997ae52819cc4ceb88e2e675453bbfb4dd8cd46),
            ),
        ],
    }
    ");
    assure_nonconflicting(&repo, &out)?;
    Ok(())
}

#[test]
fn reorder_with_conflict_and_remerge_and_pick_from_conflicts() -> Result<()> {
    assure_stable_env();
    let (repo, _tmp) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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

    let mut builder = Rebase::new(&repo, repo.rev_parse_single("base")?.detach(), None)?;
    // Re-order commits with conflict, and trigger a re-merge.
    let out = builder
        .steps([
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("C~2")?.into(),
                new_message: Some("C~2".into()),
            },
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("C")?.into(),
                new_message: Some("C".into()),
            },
            RebaseStep::Pick {
                // This will conflict,
                commit_id: repo.rev_parse_single("C~1")?.into(),
                new_message: Some("C~1".into()),
            },
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("main")?.into(),
                new_message: Some("Re-merge branches 'A', 'B' and 'C'".into()),
            },
        ])?
        .rebase()?;
    insta::assert_debug_snapshot!(out, @r"
    RebaseOutput {
        top_commit: Sha1(49915cc7bbd6cf82a009f34b66272766441bc392),
        references: [],
        commit_mapping: [
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(984fd1c6d3975901147b1f02aae6ef0a16e5904e),
                Sha1(da071f9661f4894bd5ea699590940764365fa0f4),
            ),
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(930563a048351f05b14cc7b9c0a48640e5a306b0),
                Sha1(9b5f097bef4a78ed114ca1e0a095e552ef3c2ac2),
            ),
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
                Sha1(a9bf1a73aab4f4a54f748c487a277f84e341aaf2),
            ),
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(134887021e06909021776c023a608f8ef179e859),
                Sha1(49915cc7bbd6cf82a009f34b66272766441bc392),
            ),
        ],
    }
    ");
    insta::assert_snapshot!(visualize_commit_graph(&repo, out.top_commit)?, @r"
    *-.   49915cc Re-merge branches 'A', 'B' and 'C'
    |\ \  
    | | * a9bf1a7 C~1
    | | * 9b5f097 C
    | | * da071f9 C~2
    | * | a748762 (B) B: another 10 lines at the bottom
    | * | 62e05ba B: 10 lines at the bottom
    | |/  
    * / add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base) base
    ");
    assert_ne!(
        out.top_commit.attach(&repo).object()?.peel_to_tree()?.id,
        repo.rev_parse_single("main^{tree}")?.detach(),
        "The newly re-merged tree is different as a conflict was auto-resolved"
    );

    // The auto-resolution towards *ours* causes new-file to look different.
    insta::assert_snapshot!(visualize_tree(&repo, &out ), @r#"
    6abc3da
    ├── file:100644:06581b4 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n"
    └── new-file:100644:0ff3bbb "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"
    "#);

    let conflict_commit_id = repo.rev_parse_single(format!("{}^3", out.top_commit).as_str())?;
    insta::assert_snapshot!(but_testsupport::visualize_tree(conflict_commit_id), @r#"
    db4a5b8
    ├── .auto-resolution:5b3a532 
    │   ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    │   └── new-file:100644:213ec44 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    ├── .conflict-base-0:fa799da 
    │   ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    │   └── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    ├── .conflict-files:100644:5a96881 "ancestorEntries = [\"new-file\"]\nourEntries = [\"new-file\"]\ntheirEntries = [\"new-file\"]\n"
    ├── .conflict-side-0:5b3a532 
    │   ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    │   └── new-file:100644:213ec44 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    ├── .conflict-side-1:71364f9 
    │   ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    │   └── new-file:100644:0ff3bbb "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"
    └── README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
    "#);

    // gitbutler headers were added here to indicate conflict (change-id is frozen for testing)
    insta::assert_snapshot!(conflict_commit_id.object()?.data.as_bstr(), @r"
    tree db4a5b82b209e5165cdf8d04ff4328ec1fc2526d
    parent 9b5f097bef4a78ed114ca1e0a095e552ef3c2ac2
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946684800 +0000
    gitbutler-headers-version 2
    gitbutler-change-id change-id
    gitbutler-conflicted 1

    C~1
    ");

    // And they are added to merge commits.
    insta::assert_snapshot!(out.top_commit.attach(&repo).object()?.data.as_bstr(), @r"
    tree 6abc3da6f1642bfd5543ef97f98b924f4f232a96
    parent add59d26b2ffd7468fcb44c2db48111dd8f481e5
    parent a7487625f079bedf4d20e48f052312c010117b38
    parent a9bf1a73aab4f4a54f748c487a277f84e341aaf2
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946684800 +0000
    gitbutler-headers-version 2
    gitbutler-change-id change-id

    Re-merge branches 'A', 'B' and 'C'
    ");

    // And they are also added to other cherry-picked commits that don't conflict.
    let (_base, original, cherry_picked_no_conflict) = out.commit_mapping.first().unwrap();
    insta::assert_snapshot!(cherry_picked_no_conflict.attach(&repo).object()?.data.as_bstr(), @r"
    tree fa799da5c8300f1e8f8d89f1c5989a8f03ccd852
    parent 8f0d33828e5c859c95fb9e9fc063374fdd482536
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946684800 +0000
    gitbutler-headers-version 2
    gitbutler-change-id change-id

    C~2
    ");

    // The original commit might not have had these extra headers.
    insta::assert_snapshot!(original.attach(&repo).object()?.data.as_bstr(), @r"
    tree fa799da5c8300f1e8f8d89f1c5989a8f03ccd852
    parent 8f0d33828e5c859c95fb9e9fc063374fdd482536
    author author <author@example.com> 946684800 +0000
    committer committer <committer@example.com> 946771200 +0000

    C: new file with 10 lines
    ");

    let mut builder = Rebase::new(&repo, Some(conflict_commit_id.detach()), None)?;
    let out = builder
        .steps([RebaseStep::Pick {
            commit_id: repo.rev_parse_single("C~2")?.into(),
            new_message: Some("picked on top of conflicted base".into()),
        }])?
        .rebase()?;

    // The base doesn't have new file, and we pick that up from the base of `base` of
    // the previous conflict. `our` side then is the original our.
    insta::assert_snapshot!(visualize_tree(&repo, &out ), @r#"
    cb71c19
    ├── .auto-resolution:5b3a532 
    │   ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    │   └── new-file:100644:213ec44 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    ├── .conflict-base-0:e8cfc77 
    │   └── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    ├── .conflict-files:100644:c7fd016 "ancestorEntries = []\nourEntries = [\"new-file\"]\ntheirEntries = [\"new-file\"]\n"
    ├── .conflict-side-0:5b3a532 
    │   ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    │   └── new-file:100644:213ec44 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    ├── .conflict-side-1:fa799da 
    │   ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    │   └── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    └── README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
    "#);

    Ok(())
}

#[test]
fn reversible_conflicts() -> anyhow::Result<()> {
    assure_stable_env();
    // If conflicts are created one way, putting them back the other way auto-resolves them.
    let (repo, _tmp) = fixture_writable("three-branches-merged")?;

    let mut builder = Rebase::new(&repo, repo.rev_parse_single("base")?.detach(), None)?;
    // Re-order commits with conflict, and trigger a re-merge.
    let out = builder
        .steps([
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("C~2")?.into(),
                new_message: Some("C~2".into()),
            },
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("C")?.into(),
                new_message: Some("C".into()),
            },
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("C~1")?.into(),
                new_message: Some("C~1".into()),
            },
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("main")?.into(),
                new_message: Some("Re-merge branches 'A', 'B' and 'C'".into()),
            },
        ])?
        .rebase()?;
    assert_eq!(
        conflicted(&repo, &out),
        [false, false, true, false],
        "putting things into the wrong order has a conflict"
    );

    // Original order would not conflict.
    {
        let out = builder
            .steps([
                RebaseStep::Pick {
                    commit_id: repo.rev_parse_single("C~2")?.into(),
                    new_message: Some("C~2".into()),
                },
                RebaseStep::Pick {
                    commit_id: repo.rev_parse_single("C~1")?.into(),
                    new_message: Some("C~1".into()),
                },
                RebaseStep::Pick {
                    commit_id: repo.rev_parse_single("C")?.into(),
                    new_message: Some("C".into()),
                },
                RebaseStep::Pick {
                    commit_id: repo.rev_parse_single("main")?.into(),
                    new_message: Some("Re-merge branches 'A', 'B' and 'C'".into()),
                },
            ])?
            .rebase()?;

        assert_eq!(
            conflicted(&repo, &out),
            [false, false, false, false],
            "even though keeping the right order would have worked"
        );
    }

    // Rebasing on top of
    {
        let conflict_tip = repo.rev_parse_single(format!("{}^3", out.top_commit).as_str())?;
        assert!(but_core::Commit::from_id(conflict_tip)?.is_conflicted());
        let mut builder = Rebase::new(&repo, conflict_tip.detach(), None)?;
        let out = builder
            .steps([RebaseStep::Pick {
                commit_id: repo.rev_parse_single("C")?.into(),
                new_message: Some("C~1".into()),
            }])?
            .rebase()?;
        assert_eq!(conflicted(&repo, &out), [false]);
        // The conflicting commit is 1-10, 21-30, and now it is putting 21-30 on top again.
        // Important is that it uses the real tree of the base.
        insta::assert_snapshot!(visualize_tree(&repo, &out), @r#"
        18f1011
        ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
        └── new-file:100644:ede4e3c "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
        "#);
    }

    let conflict_tip = repo.rev_parse_single(format!("{}^3", out.top_commit).as_str())?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, out.top_commit)?, @r"
    *-.   49915cc Re-merge branches 'A', 'B' and 'C'
    |\ \  
    | | * a9bf1a7 C~1
    | | * 9b5f097 C
    | | * da071f9 C~2
    | * | a748762 (B) B: another 10 lines at the bottom
    | * | 62e05ba B: 10 lines at the bottom
    | |/  
    * / add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base) base
    ");
    assert!(
        but_core::Commit::from_id(conflict_tip)?.is_conflicted(),
        "The conflict is at the tip"
    );

    let out = builder
        .steps([
            RebaseStep::Pick {
                commit_id: repo
                    .rev_parse_single(format!("{conflict_tip}~2").as_str())?
                    .into(),
                new_message: Some("C~2 is first".into()),
            },
            RebaseStep::Pick {
                commit_id: conflict_tip.detach(),
                new_message: Some("This commit is now unconflicted".into()),
            },
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("C")?.into(),
                new_message: Some("The original C will fit right on top".into()),
            },
            RebaseStep::Pick {
                commit_id: out.top_commit,
                new_message: Some("Re-merge branches 'A', 'B' and 'C'".into()),
            },
        ])?
        .rebase()?;
    assert_eq!(
        conflicted(&repo, &out),
        [false, false, false, false],
        "Nothing is conflicted anymore, but only because we pulled back the correct 'C'"
    );
    // It's the original version, like one would expect from the original order
    insta::assert_snapshot!(visualize_tree(&repo, &out), @r#"
    1111180
    ├── file:100644:06581b4 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n"
    └── new-file:100644:e8823e1 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    "#);
    Ok(())
}

#[test]
fn pick_the_first_commit_with_no_parents_for_squashing() -> Result<()> {
    assure_stable_env();
    let (repo, commits, _tmp) = four_commits_writable()?;
    let mut builder = Rebase::new(&repo, None, None)?;
    let out = builder
        .steps([
            RebaseStep::Pick {
                commit_id: commits.base,
                new_message: Some("reword base".into()),
            },
            RebaseStep::SquashIntoPreceding {
                commit_id: commits.a,
                new_message: Some("reworded base after squash".into()),
            },
        ])?
        .rebase()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, out.top_commit)?, @"* 9078131 reworded base after squash");
    insta::assert_debug_snapshot!(out, @r"
    RebaseOutput {
        top_commit: Sha1(9078131ba71afab019afd55f9dbce97c80858a42),
        references: [],
        commit_mapping: [
            (
                None,
                Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                Sha1(12381f6556a7a8cf9f132a5f930ca9f109f3d0f2),
            ),
            (
                None,
                Sha1(d591dfed1777b8f00f5b7b6f427537eeb5878178),
                Sha1(9078131ba71afab019afd55f9dbce97c80858a42),
            ),
        ],
    }
    ");
    assure_nonconflicting(&repo, &out)?;
    Ok(())
}

pub mod utils {
    use anyhow::Result;
    use but_rebase::RebaseOutput;
    use but_testsupport::gix_testtools;
    use gix::prelude::ObjectIdExt;
    use gix::ObjectId;

    /// Returns a fixture that may not be written to, objects will never touch disk either.
    pub fn fixture(fixture_name: &str) -> Result<gix::Repository> {
        let root = gix_testtools::scripted_fixture_read_only("rebase.sh")
            .map_err(anyhow::Error::from_boxed)?;
        let worktree_root = root.join(fixture_name);
        let repo =
            gix::open_opts(worktree_root, gix::open::Options::isolated())?.with_object_memory();
        Ok(repo)
    }

    /// Returns a fixture that may be written to.
    pub fn fixture_writable(fixture_name: &str) -> Result<(gix::Repository, tempfile::TempDir)> {
        // TODO: remove the need for this, impl everything in `gitoxide`, allowing this to be in-memory entirely.
        let tmp = gix_testtools::scripted_fixture_writable("rebase.sh")
            .map_err(anyhow::Error::from_boxed)?;
        let worktree_root = tmp.path().join(fixture_name);
        let repo = but_testsupport::open_repo(&worktree_root)?;
        Ok((repo, tmp))
    }

    #[derive(Debug)]
    pub struct Commits {
        pub base: ObjectId,
        pub a: ObjectId,
        pub b: ObjectId,
        pub c: ObjectId,
    }

    pub fn visualize_tree(repo: &gix::Repository, out: &RebaseOutput) -> String {
        but_testsupport::visualize_tree(out.top_commit.attach(repo)).to_string()
    }

    /// The commits in the fixture repo, starting from the oldest
    pub fn four_commits() -> Result<(gix::Repository, Commits)> {
        let repo = fixture("four-commits")?;
        let commits: Vec<_> = repo
            .head_id()?
            .ancestors()
            .all()?
            .map(Result::unwrap)
            .map(|info| info.id)
            .collect();
        assert_eq!(commits.len(), 4, "expecting a particular graph");
        Ok((
            repo,
            Commits {
                base: commits[3],
                a: commits[2],
                b: commits[1],
                c: commits[0],
            },
        ))
    }

    pub fn four_commits_writable() -> Result<(gix::Repository, Commits, tempfile::TempDir)> {
        let (repo, tmp) = fixture_writable("four-commits")?;
        let commits: Vec<_> = repo
            .head_id()?
            .ancestors()
            .all()?
            .map(Result::unwrap)
            .map(|info| info.id)
            .collect();
        assert_eq!(commits.len(), 4, "expecting a particular graph");
        Ok((
            repo,
            Commits {
                base: commits[3],
                a: commits[2],
                b: commits[1],
                c: commits[0],
            },
            tmp,
        ))
    }

    pub fn assure_nonconflicting(repo: &gix::Repository, out: &RebaseOutput) -> Result<()> {
        for (_base, old, new) in &out.commit_mapping {
            assert!(
                !but_core::Commit::from_id(new.attach(repo))?.is_conflicted(),
                "Commit mapped from {} to {} was conflicted unexpectedly",
                short_id(old),
                short_id(new)
            );
        }
        Ok(())
    }

    fn short_id(id: &gix::oid) -> String {
        id.to_hex_with_len(7).to_string()
    }

    pub fn conflicted(repo: &gix::Repository, out: &RebaseOutput) -> Vec<bool> {
        out.commit_mapping
            .iter()
            .map(|t| {
                but_core::Commit::from_id(t.2.attach(repo))
                    .unwrap()
                    .is_conflicted()
            })
            .collect()
    }
}
