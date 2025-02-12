use crate::utils::{assure_stable_env, commit_graph, fixture_writable, four_commits_writable};
use anyhow::Result;
use but_rebase::{RebaseBuilder, RebaseStep};

mod error_handling;

#[test]
fn single_stack_journey() -> Result<()> {
    assure_stable_env();
    let (repo, commits, _tmp) = four_commits_writable()?;
    let mut builder = RebaseBuilder::new(&repo, commits.base)?;
    let out = builder
        .step(RebaseStep::Pick {
            commit_id: commits.a,
            new_message: Some("first step: pick a".into()),
        })?
        .step(RebaseStep::SquashIntoPreceding {
            commit_id: commits.b,
            new_message: Some("second step: squash b into a".into()),
        })?
        .step(RebaseStep::Reference(but_core::Reference::Virtual(
            "anchor".into(),
        )))?
        .step(RebaseStep::Merge {
            commit_id: commits.c,
            new_message: "third step: merge C into b".into(),
        })?
        .rebase()?;
    insta::assert_snapshot!(commit_graph(&repo, "@")?, @r"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");
    // The base remains unchanged, and two commits remain: a squash commit and a merge with
    // the original `c` commit.
    insta::assert_snapshot!(commit_graph(&repo, out.top_commit)?, @r"
    *   2e89cda third step: merge C into b
    |\  
    | * 120e3a9 (HEAD -> main) c
    | * a96434e b
    | * d591dfe a
    * | caf2eb2 second step: squash b into a
    |/  
    * 35b8235 base
    ");

    // The reference points to the commit and correctly refers to the one that was fixed up.
    insta::assert_debug_snapshot!(out, @r#"
    RebaseOutput {
        top_commit: Sha1(2e89cda20aa24cf27d947ade0858df7aab48cdf6),
        references: [
            ReferenceSpec {
                reference: Virtual(
                    "anchor",
                ),
                commit_id: Sha1(caf2eb225788ceb3f3ad8fd9866af40719a88dac),
                previous_commit_id: Sha1(a96434e2505c2ea0896cf4f58fec0778e074d3da),
            },
        ],
    }
    "#);
    Ok(())
}

#[test]
fn amended_commit_integration() -> Result<()> {
    assure_stable_env();
    let (repo, _tmp) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(commit_graph(&repo, "@")?, @r"
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
    * 8f0d338 base
    ");
    let mut builder = RebaseBuilder::new(&repo, repo.rev_parse_single("C~1")?.detach())?;
    let out = builder
        // Pretend we have rewritten the commit at the tip of C.
        .step(RebaseStep::Pick {
            commit_id: repo.rev_parse_single("C")?.into(),
            new_message: Some("C: add another 10 lines to new file - amended".into()),
        })?
        // Picking a merge commit means to repeat the merge with the latest rewritten commit
        // from the previous step.
        .step(RebaseStep::Pick {
            commit_id: repo.rev_parse_single("main")?.into(),
            new_message: Some("Merge branches 'A', 'B' and 'C' - rewritten".into()),
        })?
        .rebase()?;
    insta::assert_snapshot!(commit_graph(&repo, out.top_commit)?, @r"
    *-.   6b6b859 Merge branches 'A', 'B' and 'C' - rewritten
    |\ \  
    | | * 66e72cd C: add another 10 lines to new file - amended
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    | * | a748762 (B) B: another 10 lines at the bottom
    | * | 62e05ba B: 10 lines at the bottom
    | |/  
    * / add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 base
    ");
    // This time without anchor.
    insta::assert_debug_snapshot!(out, @r"
    RebaseOutput {
        top_commit: Sha1(6b6b859a0465faa77b4bf45b26f1f28b428bb1f4),
        references: [],
    }
    ");
    Ok(())
}

#[test]
fn pick_the_first_commit_with_no_parents_for_squashing() -> Result<()> {
    assure_stable_env();
    let (repo, commits, _tmp) = four_commits_writable()?;
    let mut builder = RebaseBuilder::new(&repo, None)?;
    let out = builder
        .step(RebaseStep::Pick {
            commit_id: commits.base,
            new_message: Some("reword base".into()),
        })?
        .step(RebaseStep::SquashIntoPreceding {
            commit_id: commits.a,
            new_message: Some("reworded base after squash".into()),
        })?
        .rebase()?;
    insta::assert_snapshot!(commit_graph(&repo, out.top_commit)?, @"* dc4aa9e reworded base after squash");
    insta::assert_debug_snapshot!(out, @r"
    RebaseOutput {
        top_commit: Sha1(dc4aa9e43cb8316c8a00f096951ef593cc2f244b),
        references: [],
    }
    ");
    Ok(())
}

pub mod utils {
    use anyhow::Result;
    use bstr::ByteSlice;
    use gix::ObjectId;

    /// Produce a graph of all commits reachable from `refspec`.
    pub fn commit_graph(repo: &gix::Repository, refspec: impl ToString) -> Result<String> {
        let log = std::process::Command::new(gix::path::env::exe_invocation())
            .current_dir(repo.path())
            .args(["log", "--oneline", "--graph", "--decorate"])
            .arg(refspec.to_string())
            .output()?;
        assert!(log.status.success());
        Ok(log.stdout.to_str().expect("no illformed UTF-8").to_string())
    }

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
        let tmp = gix_testtools::scripted_fixture_writable("rebase.sh")
            .map_err(anyhow::Error::from_boxed)?;
        let worktree_root = tmp.path().join(fixture_name);
        let repo = gix::open_opts(worktree_root, gix::open::Options::isolated())?;
        Ok((repo, tmp))
    }

    #[derive(Debug)]
    pub struct Commits {
        pub base: ObjectId,
        pub a: ObjectId,
        pub b: ObjectId,
        pub c: ObjectId,
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

    /// TODO: remove the need for this, impl everything in `gitoxide`.
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
}
