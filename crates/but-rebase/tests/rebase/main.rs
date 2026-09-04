use anyhow::Result;
use bstr::ByteSlice;
use but_rebase::{Rebase, RebaseStep};
use but_testsupport::visualize_commit_graph;
use gix::prelude::ObjectIdExt;
use snapbox::prelude::*;

use crate::utils::{
    assure_nonconflicting, conflicted, fixture_writable, four_commits_writable, visualize_tree,
};

mod error_handling;
mod graph_rebase;

mod commit {
    mod store_author_globally_if_unset {
        use but_rebase::commit;

        use crate::utils::{fixture, fixture_writable};

        #[test]
        fn fail_if_nothing_can_be_written() -> anyhow::Result<()> {
            let (mut repo, _, _db) = fixture("four-commits")?;
            {
                let mut config = repo.config_snapshot_mut();
                config.set_raw_value("user.name", "name")?;
                config.set_raw_value("user.email", "email")?;
            }
            let err = commit::save_author_if_unset_in_repo(
                &repo,
                gix::config::Source::Local,
                "user",
                "email",
            )
            .unwrap_err();
            assert_eq!(
                err.to_string(),
                "Refusing to overwrite an existing user.name and user.email"
            );
            Ok(())
        }

        #[test]
        fn keep_comments_and_customizations() -> anyhow::Result<()> {
            let (repo, _tmp, _meta, _db) = fixture_writable("four-commits")?;
            let local_config_path = repo.path().join("config");
            std::fs::write(
                &local_config_path,
                b"# a comment\n[special] \nvalue=foo #value comment",
            )?;

            commit::save_author_if_unset_in_repo(
                &repo,
                gix::config::Source::Local,
                "user",
                "email",
            )?;

            // New values are written and everything else is still contained.
            snapbox::assert_data_eq!(
                std::fs::read_to_string(local_config_path)?,
                snapbox::str![[r#"
# a comment
[special] 
value=foo #value comment
[user]
	name = user
	email = email

"#]]
            );
            Ok(())
        }
    }
}

#[test]
fn single_stack_journey() -> Result<()> {
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
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );
    // The base remains unchanged, and two commits remain: a squash commit and a merge with
    // the original `c` commit.
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, out.top_commit)?,
        snapbox::str![[r#"
* f9b8343 second step: squash b into a
* 35b8235 base

"#]]
    );

    // The reference points to the commit and correctly refers to the one that was fixed up.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
RebaseOutput {
    top_commit: Sha1(f9b83431ec517614abb5d0898687a89159b0aa80),
    references: [
        ReferenceSpec {
            reference: Virtual(
                "anchor",
            ),
            commit_id: Sha1(f9b83431ec517614abb5d0898687a89159b0aa80),
            previous_commit_id: Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
        },
    ],
    commit_mapping: [
        (
            Some(
                Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
            ),
            Sha1(d591dfed1777b8f00f5b7b6f427537eeb5878178),
            Sha1(fdb9c68f1ad828598bdb2711246b958b7eef9f19),
        ),
        (
            Some(
                Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
            ),
            Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
            Sha1(f9b83431ec517614abb5d0898687a89159b0aa80),
        ),
        (
            Some(
                Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
            ),
            Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
            Sha1(f9b83431ec517614abb5d0898687a89159b0aa80),
        ),
    ],
}

"#]]
    );
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
    let (repo, _tmp, _meta, _db) = fixture_writable("three-branches-merged")?;
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
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, out.top_commit)?,
        snapbox::str![[r#"
*-.   005b14d Merge branches 'A', 'B' and 'C' - rewritten
|\ \  
| | * c7d70ef C: add another 10 lines to new file - amended
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
    // This time without anchor.
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
RebaseOutput {
    top_commit: Sha1(005b14dbbb183d0f1c147f1d14e2a2b9c7abe633),
    references: [],
    commit_mapping: [
        (
            Some(
                Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
            ),
            Sha1(930563a048351f05b14cc7b9c0a48640e5a306b0),
            Sha1(c7d70ef1f1a7c47fb30db62dca147429dac8b6c2),
        ),
        (
            Some(
                Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
            ),
            Sha1(134887021e06909021776c023a608f8ef179e859),
            Sha1(005b14dbbb183d0f1c147f1d14e2a2b9c7abe633),
        ),
    ],
}

"#]]
    );
    assure_nonconflicting(&repo, &out)?;
    Ok(())
}

#[test]
fn reorder_merge_in_reverse() -> Result<()> {
    let (repo, _tmp, _meta, _db) = fixture_writable("merge-in-the-middle")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "with-inner-merge")?,
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

    let mut builder = Rebase::new(&repo, repo.rev_parse_single("base")?.detach(), None)?;
    let out = builder
        //
        .steps([
            // Pick merge
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("with-inner-merge~1")?.into(),
                new_message: Some("was merge 2fc288c one below top".into()),
            },
            // Pick top
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("with-inner-merge")?.into(),
                new_message: Some("was e8ee978 on top".into()),
            },
            // Pick one above the base (to be the new top)
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("with-inner-merge~2")?.into(),
                new_message: Some("was dd59d2 below merge".into()),
            },
        ])?
        .rebase()
        .expect("the first parent of a merge is replaced unconditionally");
    // Note that we don't rewrite references here.
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, out.top_commit)?,
        snapbox::str![[r#"
* 50e7e4c was dd59d2 below merge
* bae2ba5 was e8ee978 on top
*   65288c7 was merge 2fc288c one below top
|\  
| * 984fd1c (B) C: new file with 10 lines
|/  
* 8f0d338 (tag: base, main) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
RebaseOutput {
    top_commit: Sha1(50e7e4c717718f098c1b578ccf7007d59f20daef),
    references: [],
    commit_mapping: [
        (
            Some(
                Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
            ),
            Sha1(2fc288c36c8bb710c78203f78ea9883724ce142b),
            Sha1(65288c75beb4a6f472029dffc7c644875f25d2be),
        ),
        (
            Some(
                Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
            ),
            Sha1(e8ee978dac10e6a85006543ef08be07c5824b4f7),
            Sha1(bae2ba56ef093d4cf085cf5704d77bb75009373e),
        ),
        (
            Some(
                Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
            ),
            Sha1(add59d26b2ffd7468fcb44c2db48111dd8f481e5),
            Sha1(50e7e4c717718f098c1b578ccf7007d59f20daef),
        ),
    ],
}

"#]]
    );
    assure_nonconflicting(&repo, &out)?;
    Ok(())
}

#[test]
fn reorder_with_conflict_and_remerge_and_pick_from_conflicts() -> Result<()> {
    let (repo, _tmp, _meta, _db) = fixture_writable("three-branches-merged")?;
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

    let mut builder = Rebase::new(&repo, repo.rev_parse_single("base")?.detach(), None)?;
    // Re-order commits with conflict, and trigger a re-merge.
    let out = builder
        .steps([
            RebaseStep::Pick {
                commit_id: repo.rev_parse_single("C~2")?.into(),
                new_message: Some("C~2".into()),
            },
            RebaseStep::Pick {
                // This will conflict.
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
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
RebaseOutput {
    top_commit: Sha1(469558c7c4c36d29a92859c5e0e521cea254dd57),
    references: [],
    commit_mapping: [
        (
            Some(
                Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
            ),
            Sha1(984fd1c6d3975901147b1f02aae6ef0a16e5904e),
            Sha1(e5c725a3fb1b8c92451a04d9dd86d6bd269d76ab),
        ),
        (
            Some(
                Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
            ),
            Sha1(930563a048351f05b14cc7b9c0a48640e5a306b0),
            Sha1(d3c63cc2c9286d397720a0b592f3ed21bca1d52e),
        ),
        (
            Some(
                Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
            ),
            Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
            Sha1(cffa6a3056e67dcfaf8482617be5e60d56834a36),
        ),
        (
            Some(
                Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
            ),
            Sha1(134887021e06909021776c023a608f8ef179e859),
            Sha1(469558c7c4c36d29a92859c5e0e521cea254dd57),
        ),
    ],
}

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, out.top_commit)?,
        snapbox::str![[r#"
*-.   469558c Re-merge branches 'A', 'B' and 'C'
|\ \  
| | * cffa6a3 C~1
| | * d3c63cc [conflict] C
| | * e5c725a C~2
| * | a748762 (B) B: another 10 lines at the bottom
| * | 62e05ba B: 10 lines at the bottom
| |/  
* / add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    assert_ne!(
        out.top_commit.attach(&repo).object()?.peel_to_tree()?.id,
        repo.rev_parse_single("main^{tree}")?,
        "The newly re-merged tree is different as a conflict was auto-resolved"
    );

    // The auto-resolution towards *ours* causes new-file to look different.
    snapbox::assert_data_eq!(visualize_tree(&repo, &out ), snapbox::str![[r#"
6abc3da
├── file:100644:06581b4 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n"
└── new-file:100644:0ff3bbb "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"

"#]].raw());

    let conflict_commit_id = out.commit_mapping[1].2.attach(&repo);
    snapbox::assert_data_eq!(but_testsupport::visualize_tree(conflict_commit_id).to_string(), snapbox::str![[r#"
f92b9c1
├── .auto-resolution:fa799da 
│   ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
│   └── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
├── .conflict-base-0:71364f9 
│   ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
│   └── new-file:100644:0ff3bbb "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"
├── .conflict-files:100644:5a96881 "ancestorEntries = [\"new-file\"]\nourEntries = [\"new-file\"]\ntheirEntries = [\"new-file\"]\n"
├── .conflict-side-0:fa799da 
│   ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
│   └── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
├── .conflict-side-1:fa92d27 
│   ├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
│   └── new-file:100644:e8823e1 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
└── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"

"#]].raw());

    // gitbutler headers were added here to indicate conflict (change-id is frozen for testing)
    snapbox::assert_data_eq!(
        conflict_commit_id.object()?.data.as_bstr().to_string(),
        snapbox::str![[r#"
tree f92b9c1f55ce8576eb80c7fb32eb295ef8f7b288
parent e5c725a3fb1b8c92451a04d9dd86d6bd269d76ab
author author <author@example.com> 946684800 +0000
committer Committer (Memory Override) <committer@example.com> 946771200 +0000
gitbutler-headers-version 2
change-id tvqkmqxpowoyosykwzoktulqknpvypwv

[conflict] C

GitButler-Conflict: This is a GitButler-managed conflicted commit. Files are auto-resolved
   using the "ours" side. The commit tree contains additional directories:
     .conflict-side-0  — our tree
     .conflict-side-1  — their tree
     .conflict-base-0  — the merge base tree
     .auto-resolution  — the auto-resolved tree
     .conflict-files   — metadata about conflicted files
   To manually resolve, check out this commit, remove the directories
   listed above, resolve the conflicts, and amend the commit.

"#]]
    );

    // And they are added to merge commits.
    snapbox::assert_data_eq!(
        out.top_commit
            .attach(&repo)
            .object()?
            .data
            .as_bstr()
            .to_string(),
        snapbox::str![[r#"
tree 6abc3da6f1642bfd5543ef97f98b924f4f232a96
parent add59d26b2ffd7468fcb44c2db48111dd8f481e5
parent a7487625f079bedf4d20e48f052312c010117b38
parent cffa6a3056e67dcfaf8482617be5e60d56834a36
author author <author@example.com> 946684800 +0000
committer Committer (Memory Override) <committer@example.com> 946771200 +0000
gitbutler-headers-version 2
change-id qutxpsntutpmpxvllyxuwumnkqmuxnxs

Re-merge branches 'A', 'B' and 'C'
"#]]
    );

    // And they are also added to other cherry-picked commits that don't conflict.
    let (_base, original, cherry_picked_no_conflict) = out.commit_mapping.first().unwrap();
    snapbox::assert_data_eq!(
        cherry_picked_no_conflict
            .attach(&repo)
            .object()?
            .data
            .as_bstr()
            .to_string(),
        snapbox::str![[r#"
tree fa799da5c8300f1e8f8d89f1c5989a8f03ccd852
parent 8f0d33828e5c859c95fb9e9fc063374fdd482536
author author <author@example.com> 946684800 +0000
committer Committer (Memory Override) <committer@example.com> 946771200 +0000
gitbutler-headers-version 2
change-id qouxtqnurnuxukrypyrmotplppooouvs

C~2
"#]]
    );

    // The original commit might not have had these extra headers.
    snapbox::assert_data_eq!(
        original.attach(&repo).object()?.data.as_bstr().to_string(),
        snapbox::str![[r#"
tree fa799da5c8300f1e8f8d89f1c5989a8f03ccd852
parent 8f0d33828e5c859c95fb9e9fc063374fdd482536
author author <author@example.com> 946684800 +0000
committer committer <committer@example.com> 946771200 +0000

C: new file with 10 lines

"#]]
    );

    let mut builder = Rebase::new(&repo, Some(conflict_commit_id.detach()), None)?;
    let out = builder
        .steps([RebaseStep::Pick {
            commit_id: repo.rev_parse_single("C~2")?.into(),
            new_message: Some("picked on top of conflicted base".into()),
        }])?
        .rebase()?;

    // The base doesn't have new file, and we pick that up from the base of `base` of
    // the previous conflict. `our` side then is the original our.
    snapbox::assert_data_eq!(
        visualize_tree(&repo, &out),
        snapbox::str![[r#"
fa799da
├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
└── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn reversible_conflicts() -> anyhow::Result<()> {
    // If conflicts are created one way, putting them back the other way auto-resolves them.
    let (repo, _tmp, _meta, _db) = fixture_writable("three-branches-merged")?;

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
        [false, true, false, false],
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

    // Rebasing on top of the conflicted pick.
    {
        let conflict_tip = out.commit_mapping[1].2.attach(&repo);
        assert!(but_core::Commit::from_id(conflict_tip)?.is_conflicted());
        let mut builder = Rebase::new(&repo, conflict_tip.detach(), None)?;
        let out = builder
            .steps([RebaseStep::Pick {
                commit_id: repo.rev_parse_single("C~1")?.into(),
                new_message: Some("C~1".into()),
            }])?
            .rebase()?;
        assert_eq!(conflicted(&repo, &out), [false]);
        // The missing middle change applies to the real tree of the conflicted base.
        snapbox::assert_data_eq!(visualize_tree(&repo, &out), snapbox::str![[r#"
71364f9
├── file:100644:5ecf5f4 "50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
└── new-file:100644:0ff3bbb "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"

"#]].raw());
    }

    let conflict_tip = out.commit_mapping[1].2.attach(&repo);
    let rewritten_c_tip = out.commit_mapping[2].2;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, out.top_commit)?,
        snapbox::str![[r#"
*-.   469558c Re-merge branches 'A', 'B' and 'C'
|\ \  
| | * cffa6a3 C~1
| | * d3c63cc [conflict] C
| | * e5c725a C~2
| * | a748762 (B) B: another 10 lines at the bottom
| * | 62e05ba B: 10 lines at the bottom
| |/  
* / add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    assert!(
        but_core::Commit::from_id(conflict_tip)?.is_conflicted(),
        "The reordered C commit conflicts"
    );

    // Replay the materialized branch in the correct order. Keeping both the
    // conflicted C and rewritten merge proves that rebasing recovers the conflict;
    // using the original commits here would merely repeat the clean-order test.
    let out = builder
        .steps([
            RebaseStep::Pick {
                commit_id: repo
                    .rev_parse_single(format!("{conflict_tip}~1").as_str())?
                    .into(),
                new_message: Some("C~2 is first".into()),
            },
            RebaseStep::Pick {
                commit_id: rewritten_c_tip,
                new_message: Some("C~1 is second".into()),
            },
            RebaseStep::Pick {
                commit_id: conflict_tip.detach(),
                new_message: Some("The conflicted C is recovered".into()),
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
        "putting the conflicted C back after C~1 recovers the original order"
    );
    assert!(
        !but_core::Commit::from_id(out.commit_mapping[2].2.attach(&repo))?.is_conflicted(),
        "the materialized conflicted C becomes an ordinary commit again"
    );
    // It's the original version, like one would expect from the original order
    snapbox::assert_data_eq!(visualize_tree(&repo, &out), snapbox::str![[r#"
1111180
├── file:100644:06581b4 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n"
└── new-file:100644:e8823e1 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"

"#]].raw());
    Ok(())
}

#[test]
fn pick_the_first_commit_with_no_parents_for_squashing() -> Result<()> {
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
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, out.top_commit)?,
        snapbox::str![[r#"
* 647bbc7 reworded base after squash

"#]]
    );
    snapbox::assert_data_eq!(
        out.to_debug(),
        snapbox::str![[r#"
RebaseOutput {
    top_commit: Sha1(647bbc7cdbd8e18fd778261250d4a0dbc485e47a),
    references: [],
    commit_mapping: [
        (
            None,
            Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
            Sha1(a7b93ef41a8efade0eb3fe98dbc8e21c34cd16df),
        ),
        (
            None,
            Sha1(d591dfed1777b8f00f5b7b6f427537eeb5878178),
            Sha1(647bbc7cdbd8e18fd778261250d4a0dbc485e47a),
        ),
    ],
}

"#]]
    );
    assure_nonconflicting(&repo, &out)?;
    Ok(())
}

pub mod utils {
    use anyhow::Result;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_rebase::RebaseOutput;
    use gix::{ObjectId, prelude::ObjectIdExt};

    /// Returns a fixture that may not be written to, objects will never touch disk either.
    pub fn fixture(
        fixture_name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
        but_db::DbHandle,
    )> {
        let repo = but_testsupport::read_only_in_memory_scenario(fixture_name)?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        // The fixture is shared and read-only, so its database cannot live on disk.
        let db = but_testsupport::in_memory_db();
        Ok((repo, std::mem::ManuallyDrop::new(meta), db))
    }

    /// Returns a fixture that may be written to.
    pub fn fixture_writable(
        fixture_name: &str,
    ) -> Result<(
        gix::Repository,
        tempfile::TempDir,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
        but_db::DbHandle,
    )> {
        // TODO: remove the need for this, impl everything in `gitoxide`, allowing this to be in-memory entirely.
        let (repo, tmp) = but_testsupport::writable_scenario(fixture_name);
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        let db = but_testsupport::project_db(&repo)?;
        Ok((repo, tmp, std::mem::ManuallyDrop::new(meta), db))
    }

    /// Returns a fixture that may be written to.
    pub fn fixture_writable_with_signing(
        fixture_name: &str,
    ) -> Result<(
        gix::Repository,
        tempfile::TempDir,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
        but_db::DbHandle,
    )> {
        let (repo, tmp) = but_testsupport::writable_scenario_with_ssh_key(fixture_name);
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        let db = but_testsupport::project_db(&repo)?;
        Ok((repo, tmp, std::mem::ManuallyDrop::new(meta), db))
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
        let (repo, _, _db) = fixture("four-commits")?;
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
        let (repo, tmp, _meta, _db) = fixture_writable("four-commits")?;
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

    pub fn standard_options() -> but_graph::init::Options {
        but_graph::init::Options {
            collect_tags: true,
            commits_limit_hint: None,
            commits_limit_recharge_location: vec![],
            hard_limit: None,
            extra_target_commit_id: None,
            dangerously_skip_postprocessing_for_debugging: false,
            worktrees: false,
        }
    }

    pub fn target_meta() -> but_core::ref_metadata::ProjectMeta {
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some(
                "refs/remotes/origin/main"
                    .try_into()
                    .expect("valid target ref"),
            ),
            ..Default::default()
        }
    }
}
