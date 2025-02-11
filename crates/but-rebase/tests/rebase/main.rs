use crate::utils::four_commits;
use anyhow::Result;
use but_rebase::{RebaseBuilder, RebaseStep};

mod error_handling {
    use crate::utils::{fixture, four_commits};
    use but_rebase::{RebaseBuilder, RebaseStep};
    use gix::ObjectId;
    use std::str::FromStr;

    fn non_existing_commit() -> gix::ObjectId {
        ObjectId::from_str("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap()
    }

    #[test]
    fn base_non_existing() -> anyhow::Result<()> {
        let result = RebaseBuilder::new(fixture("four-commits")?, non_existing_commit());
        assert_eq!(
            result.unwrap_err().to_string(),
            "An object with id eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee could not be found"
        );
        Ok(())
    }

    #[test]
    fn non_existing_commit_in_pick_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder.step(RebaseStep::Pick {
            oid: non_existing_commit(),
            new_message: None,
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "An object with id eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee could not be found"
        );
        Ok(())
    }

    #[test]
    fn non_existing_commit_in_merge_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder.step(RebaseStep::Merge {
            oid: non_existing_commit(),
            new_message: "merge commit".into(),
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "An object with id eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee could not be found"
        );
        Ok(())
    }

    #[test]
    fn non_existing_commit_in_fixup_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder.step(RebaseStep::Fixup {
            oid: non_existing_commit(),
            new_message: None,
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "An object with id eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee could not be found"
        );
        Ok(())
    }

    #[test]
    fn using_base_in_pick_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder.step(RebaseStep::Fixup {
            oid: commits.base,
            new_message: None,
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Fixup commit cannot be the base commit"
        );
        Ok(())
    }

    #[test]
    fn using_base_in_merge_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder.step(RebaseStep::Merge {
            oid: commits.base,
            new_message: "merge commit".into(),
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Merge commit cannot be the base commit"
        );
        Ok(())
    }

    #[test]
    fn using_base_in_fixup_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder.step(RebaseStep::Fixup {
            oid: commits.base,
            new_message: None,
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Fixup commit cannot be the base commit"
        );
        Ok(())
    }

    #[test]
    fn using_picked_commit_in_a_pick_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder
            .step(RebaseStep::Pick {
                oid: commits.a,
                new_message: None,
            })?
            .step(RebaseStep::Pick {
                oid: commits.a,
                new_message: None,
            });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Picked commit already exists in a previous step"
        );
        Ok(())
    }

    #[test]
    fn using_merged_commit_in_a_pick_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder
            .step(RebaseStep::Merge {
                oid: commits.a,
                new_message: "merge commit".into(),
            })?
            .step(RebaseStep::Pick {
                oid: commits.a,
                new_message: None,
            });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Picked commit already exists in a previous step"
        );
        Ok(())
    }

    #[test]
    fn using_fixup_commit_in_a_pick_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder
            .step(RebaseStep::Pick {
                oid: commits.a,
                new_message: None,
            })?
            .step(RebaseStep::Fixup {
                oid: commits.b,
                new_message: None,
            })?
            .step(RebaseStep::Pick {
                oid: commits.b,
                new_message: None,
            });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Picked commit already exists in a previous step"
        );
        Ok(())
    }

    #[test]
    fn using_picked_commit_in_a_merge_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder
            .step(RebaseStep::Pick {
                oid: commits.a,
                new_message: None,
            })?
            .step(RebaseStep::Merge {
                oid: commits.a,
                new_message: "merge commit".into(),
            });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Picked commit already exists in a previous step"
        );
        Ok(())
    }

    #[test]
    fn using_merged_commit_in_a_merge_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder
            .step(RebaseStep::Merge {
                oid: commits.a,
                new_message: "merge commit".into(),
            })?
            .step(RebaseStep::Merge {
                oid: commits.a,
                new_message: "merge commit".into(),
            });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Picked commit already exists in a previous step"
        );
        Ok(())
    }

    #[test]
    fn using_fixup_commit_in_a_merge_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder
            .step(RebaseStep::Pick {
                oid: commits.a,
                new_message: None,
            })?
            .step(RebaseStep::Fixup {
                oid: commits.b,
                new_message: None,
            })?
            .step(RebaseStep::Merge {
                oid: commits.b,
                new_message: "merge commit".into(),
            });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Picked commit already exists in a previous step"
        );
        Ok(())
    }

    #[test]
    fn using_picked_commit_in_a_fixup_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder
            .step(RebaseStep::Pick {
                oid: commits.a,
                new_message: None,
            })?
            .step(RebaseStep::Fixup {
                oid: commits.a,
                new_message: None,
            });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Picked commit already exists in a previous step"
        );
        Ok(())
    }

    #[test]
    fn using_merged_commit_in_a_fixup_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder
            .step(RebaseStep::Merge {
                oid: commits.a,
                new_message: "merge commit".into(),
            })?
            .step(RebaseStep::Fixup {
                oid: commits.a,
                new_message: None,
            });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Picked commit already exists in a previous step"
        );
        Ok(())
    }

    #[test]
    fn using_fixup_commit_in_a_fixup_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder
            .step(RebaseStep::Pick {
                oid: commits.a,
                new_message: None,
            })?
            .step(RebaseStep::Fixup {
                oid: commits.b,
                new_message: None,
            })?
            .step(RebaseStep::Fixup {
                oid: commits.b,
                new_message: None,
            });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Picked commit already exists in a previous step"
        );
        Ok(())
    }

    #[test]
    fn fixup_is_first_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder.step(RebaseStep::Fixup {
            oid: commits.a,
            new_message: None,
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Fixup must have a commit to work on"
        );
        Ok(())
    }

    #[test]
    fn fixup_is_only_preceeded_by_a_reference_step() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder
            .step(RebaseStep::Reference {
                refname: "foo/bar".into(),
            })?
            .step(RebaseStep::Fixup {
                oid: commits.a,
                new_message: None,
            });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Fixup commit must not come after a reference step"
        );
        Ok(())
    }

    #[test]
    fn reference_is_not_a_valid_reference_name() -> anyhow::Result<()> {
        let (repo, commits) = four_commits()?;
        let mut builder = RebaseBuilder::new(repo, commits.base)?;
        let result = builder.step(RebaseStep::Reference {
            refname: "abc".into(),
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Standalone references must be all uppercased, like 'HEAD'"
        );
        Ok(())
    }
}

#[test]
fn happy_case_scenario() -> Result<()> {
    let (repo, commits) = four_commits()?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder
        .step(RebaseStep::Pick {
            oid: commits.a,
            new_message: Some("updated commit message".into()),
        })?
        .step(RebaseStep::Fixup {
            oid: commits.b,
            new_message: None,
        })?
        .step(RebaseStep::Reference {
            refname: "my/ref".into(),
        })?
        .step(RebaseStep::Merge {
            oid: commits.c,
            new_message: "merge commit".into(),
        })?;
    // TODO: make assertions
    Ok(())
}

pub mod utils {
    use anyhow::Result;
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
}
