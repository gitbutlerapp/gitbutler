use crate::utils::{
    assure_nonconflicting, assure_stable_env, fixture_writable, four_commits_writable,
    visualize_tree,
};
use anyhow::Result;
use but_rebase::{Rebase, RebaseStep};
use but_testsupport::visualize_commit_graph;
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
            RebaseStep::Merge {
                commit_id: commits.c,
                new_message: "third step: merge C into b".into(),
            },
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
    *   2d43db8 third step: merge C into b
    |\  
    | * 120e3a9 (HEAD -> main) c
    | * a96434e b
    | * d591dfe a
    * | cff6d50 second step: squash b into a
    |/  
    * 35b8235 base
    ");

    // The reference points to the commit and correctly refers to the one that was fixed up.
    insta::assert_debug_snapshot!(out, @r#"
    RebaseOutput {
        top_commit: Sha1(2d43db878f75eb143dc6de5a4ba7e6854fc3c289),
        references: [
            ReferenceSpec {
                reference: Virtual(
                    "anchor",
                ),
                commit_id: Sha1(cff6d50539eb1e7decb8e3ab533435af6a8390bd),
                previous_commit_id: Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
            },
        ],
        commit_mapping: [
            (
                Some(
                    Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                ),
                Sha1(d591dfed1777b8f00f5b7b6f427537eeb5878178),
                Sha1(cb580b312dbacb76e7ca7298e0f3f478437f8e41),
            ),
            (
                Some(
                    Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                ),
                Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
                Sha1(cff6d50539eb1e7decb8e3ab533435af6a8390bd),
            ),
            (
                Some(
                    Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                ),
                Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
                Sha1(cff6d50539eb1e7decb8e3ab533435af6a8390bd),
            ),
            (
                Some(
                    Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                ),
                Sha1(120e3a90b753a492cef9a552ae3b9ba1f1391362),
                Sha1(2d43db878f75eb143dc6de5a4ba7e6854fc3c289),
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
    *-.   bf2b2c8 Merge branches 'A', 'B' and 'C' - rewritten
    |\ \  
    | | * a349d28 C: add another 10 lines to new file - amended
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
        top_commit: Sha1(bf2b2c8e643b194cb2b52b19735fc5c968677c7a),
        references: [],
        commit_mapping: [
            (
                Some(
                    Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
                ),
                Sha1(930563a048351f05b14cc7b9c0a48640e5a306b0),
                Sha1(a349d28d8f54b7fff64752260c33903ca1daf52d),
            ),
            (
                Some(
                    Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
                ),
                Sha1(134887021e06909021776c023a608f8ef179e859),
                Sha1(bf2b2c8e643b194cb2b52b19735fc5c968677c7a),
            ),
        ],
    }
    ");
    assure_nonconflicting(&repo, &out)?;
    Ok(())
}

#[test]
fn reorder_with_conflict_and_remerge() -> Result<()> {
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
        top_commit: Sha1(bd766f8c4da714cc2de511e50d1e011adbf66b82),
        references: [],
        commit_mapping: [
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(984fd1c6d3975901147b1f02aae6ef0a16e5904e),
                Sha1(54eda7593379f1922c84b40cb26eb6fdd86cdf0e),
            ),
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(930563a048351f05b14cc7b9c0a48640e5a306b0),
                Sha1(6844bf4c9640e04aaded1eca6389c67896fb07e6),
            ),
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(68a2fc349e13a186e6d65871a31bad244d25e6f4),
                Sha1(4e3b7bf536b065291e64a4f20ef7292d4318f4dd),
            ),
            (
                Some(
                    Sha1(8f0d33828e5c859c95fb9e9fc063374fdd482536),
                ),
                Sha1(134887021e06909021776c023a608f8ef179e859),
                Sha1(bd766f8c4da714cc2de511e50d1e011adbf66b82),
            ),
        ],
    }
    ");
    insta::assert_snapshot!(visualize_commit_graph(&repo, out.top_commit)?, @r"
    *-.   bd766f8 Re-merge branches 'A', 'B' and 'C'
    |\ \  
    | | * 4e3b7bf C~1
    | | * 6844bf4 C
    | | * 54eda75 C~2
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
    37f8adc
    ├── file:100644:06581b4 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n"
    └── new-file:100644:213ec44 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
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
    insta::assert_snapshot!(visualize_commit_graph(&repo, out.top_commit)?, @"* a67ed12 reworded base after squash");
    insta::assert_debug_snapshot!(out, @r"
    RebaseOutput {
        top_commit: Sha1(a67ed124df50035c1cfa05f66777bdcbaae92b72),
        references: [],
        commit_mapping: [
            (
                None,
                Sha1(35b8235197020a417e9405ab5d4db6f204e8d84b),
                Sha1(99f9822f74bda4b33ecdf2323e92a3fe40dce851),
            ),
            (
                None,
                Sha1(d591dfed1777b8f00f5b7b6f427537eeb5878178),
                Sha1(a67ed124df50035c1cfa05f66777bdcbaae92b72),
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

    /// Sets and environment that assures commits are reproducible.
    /// This needs the `testing` feature enabled in `but-core` as well to work.
    /// This changes the process environment, be aware.
    pub fn assure_stable_env() {
        let env = gix_testtools::Env::new()
            // TODO(gix): once everything is ported, the only variable needed here
            //            is CHANGE_ID, and even that could be a global. Call `but_testsupport::open_repo()`
            //            for basic settings.
            .set("GIT_AUTHOR_DATE", "2000-01-01 00:00:00 +0000")
            .set("GIT_AUTHOR_EMAIL", "author@example.com")
            .set("GIT_AUTHOR_NAME", "author")
            .set("GIT_COMMITTER_DATE", "2000-01-02 00:00:00 +0000")
            .set("GIT_COMMITTER_EMAIL", "committer@example.com")
            .set("GIT_COMMITTER_NAME", "committer")
            .set("CHANGE_ID", "committer");
        // assure it doesn't get racy.
        std::mem::forget(env);
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
}
