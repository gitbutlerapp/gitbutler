use anyhow::Result;
use bstr::ByteSlice;
use but_rebase::{Rebase, RebaseStep};
use but_testsupport::{assure_stable_env, visualize_commit_graph};
use gix::prelude::ObjectIdExt;

use crate::utils::{
    assure_nonconflicting, conflicted, fixture_writable, four_commits_writable, visualize_tree,
};

mod editor_creation;
mod error_handling;

mod commit {
    mod store_author_globally_if_unset {
        use but_rebase::commit;

        use crate::utils::{fixture, fixture_writable};

        #[test]
        fn fail_if_nothing_can_be_written() -> anyhow::Result<()> {
            let (mut repo, _) = fixture("four-commits")?;
            {
                let mut config = repo.config_snapshot_mut();
                config.set_raw_value(&"user.name", "name")?;
                config.set_raw_value(&"user.email", "email")?;
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
            let (repo, _tmp) = fixture_writable("four-commits")?;
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
            insta::assert_snapshot!(std::fs::read_to_string(local_config_path)?, @r"
            # a comment
            [special] 
            value=foo #value comment
            [user]
            	name = user
            	email = email
            ");
            Ok(())
        }
    }
}

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
    * b036cfe second step: squash b into a
    * 35b8235 base
    ");

    // The reference points to the commit and correctly refers to the one that was fixed up.
    insta::assert_debug_snapshot!(out, @r#"
    RebaseOutput {
        top_commit: Sha1(b036cfe2b7250396114e433883a48271a90ebe4d),
        references: [
            ReferenceSpec {
                reference: Virtual(
                    "anchor",
                ),
                commit_id: Sha1(b036cfe2b7250396114e433883a48271a90ebe4d),
                previous_commit_id: Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
            },
        ],
        commit_mapping: [
            (
                Some(
                    Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                ),
                Sha1(d591dfed1777b8f00f5b7b6f427537eeb5878178),
                Sha1(1975d0213524434c3c7470404e3165a3d13bce06),
            ),
            (
                Some(
                    Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                ),
                Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
                Sha1(b036cfe2b7250396114e433883a48271a90ebe4d),
            ),
            (
                Some(
                    Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                ),
                Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
                Sha1(b036cfe2b7250396114e433883a48271a90ebe4d),
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
    *-.   cc1d6d5 Merge branches 'A', 'B' and 'C' - rewritten
    |\ \  
    | | * b24c308 C: add another 10 lines to new file - amended
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
        top_commit: Sha1(cc1d6d57b45f1967b8ee333941a6d6d6d512da13),
        references: [],
        commit_mapping: [
            (
                Some(
                    Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
                ),
                Sha1(930563a048351f05b14cc7b9c0a48640e5a306b0),
                Sha1(b24c30842ff50512edff72f5d89cafdea5be99d8),
            ),
            (
                Some(
                    Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
                ),
                Sha1(134887021e06909021776c023a608f8ef179e859),
                Sha1(cc1d6d57b45f1967b8ee333941a6d6d6d512da13),
            ),
        ],
    }
    ");
    assure_nonconflicting(&repo, &out)?;
    Ok(())
}

#[test]
fn reorder_merge_in_reverse() -> Result<()> {
    assure_stable_env();
    let (repo, _tmp) = fixture_writable("merge-in-the-middle")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "with-inner-merge")?, @r"
    * e8ee978 (HEAD -> with-inner-merge) on top of inner merge
    *   2fc288c Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");

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
    insta::assert_snapshot!(visualize_commit_graph(&repo, out.top_commit)?, @r"
    * 5949b5b was dd59d2 below merge
    * 24494dc was e8ee978 on top
    *   5b2cbb3 was merge 2fc288c one below top
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_debug_snapshot!(out, @r"
    RebaseOutput {
        top_commit: Sha1(5949b5b6f3de27a6f8db16c3ae2bdda155220e6a),
        references: [],
        commit_mapping: [
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(2fc288c36c8bb710c78203f78ea9883724ce142b),
                Sha1(5b2cbb31707d3362c461eeb117393c6d5420372f),
            ),
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(e8ee978dac10e6a85006543ef08be07c5824b4f7),
                Sha1(24494dcbc1aeea776c5ac427f7ca720bd6cc640a),
            ),
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(add59d26b2ffd7468fcb44c2db48111dd8f481e5),
                Sha1(5949b5b6f3de27a6f8db16c3ae2bdda155220e6a),
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
        top_commit: Sha1(555c076b0434c148991fc6be55cafbcdde37f8eb),
        references: [],
        commit_mapping: [
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(984fd1c6d3975901147b1f02aae6ef0a16e5904e),
                Sha1(f1add68a71f4dbc2d142e2a7b12afccef9159f9d),
            ),
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(930563a048351f05b14cc7b9c0a48640e5a306b0),
                Sha1(93e675006a90be2c95f722ef9dd40f426331f4b9),
            ),
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
                Sha1(189f8c4dd7a5b2029c902257ae1cdbacc3cdd688),
            ),
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(134887021e06909021776c023a608f8ef179e859),
                Sha1(555c076b0434c148991fc6be55cafbcdde37f8eb),
            ),
        ],
    }
    ");
    insta::assert_snapshot!(visualize_commit_graph(&repo, out.top_commit)?, @r"
    *-.   555c076 Re-merge branches 'A', 'B' and 'C'
    |\ \  
    | | * 189f8c4 C~1
    | | * 93e6750 C
    | | * f1add68 C~2
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
    parent 93e675006a90be2c95f722ef9dd40f426331f4b9
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946684800 +0000
    gitbutler-headers-version 2
    gitbutler-change-id 00000000-0000-0000-0000-000000000001
    gitbutler-conflicted 1

    C~1
    ");

    // And they are added to merge commits.
    insta::assert_snapshot!(out.top_commit.attach(&repo).object()?.data.as_bstr(), @r"
    tree 6abc3da6f1642bfd5543ef97f98b924f4f232a96
    parent add59d26b2ffd7468fcb44c2db48111dd8f481e5
    parent a7487625f079bedf4d20e48f052312c010117b38
    parent 189f8c4dd7a5b2029c902257ae1cdbacc3cdd688
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946684800 +0000
    gitbutler-headers-version 2
    gitbutler-change-id 00000000-0000-0000-0000-000000000001

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
    gitbutler-change-id 00000000-0000-0000-0000-000000000001

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
    *-.   555c076 Re-merge branches 'A', 'B' and 'C'
    |\ \  
    | | * 189f8c4 C~1
    | | * 93e6750 C
    | | * f1add68 C~2
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
    insta::assert_snapshot!(visualize_commit_graph(&repo, out.top_commit)?, @"* 9c68471 reworded base after squash");
    insta::assert_debug_snapshot!(out, @r"
    RebaseOutput {
        top_commit: Sha1(9c68471968e68ffe5df832a4cb850e8c3e7b7cd0),
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
                Sha1(9c68471968e68ffe5df832a4cb850e8c3e7b7cd0),
            ),
        ],
    }
    ");
    assure_nonconflicting(&repo, &out)?;
    Ok(())
}

pub mod utils {
    use anyhow::Result;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_rebase::RebaseOutput;
    use but_testsupport::gix_testtools;
    use gix::{ObjectId, prelude::ObjectIdExt};

    /// Returns a fixture that may not be written to, objects will never touch disk either.
    pub fn fixture(
        fixture_name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let root = gix_testtools::scripted_fixture_read_only("rebase.sh")
            .map_err(anyhow::Error::from_boxed)?;
        let worktree_root = root.join(fixture_name);
        let repo =
            gix::open_opts(&worktree_root, gix::open::Options::isolated())?.with_object_memory();

        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        Ok((repo, std::mem::ManuallyDrop::new(meta)))
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
        let (repo, _) = fixture("four-commits")?;
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

    pub fn standard_options() -> but_graph::init::Options {
        but_graph::init::Options {
            collect_tags: true,
            commits_limit_hint: None,
            commits_limit_recharge_location: vec![],
            hard_limit: None,
            extra_target_commit_id: None,
            dangerously_skip_postprocessing_for_debugging: false,
        }
    }
}
