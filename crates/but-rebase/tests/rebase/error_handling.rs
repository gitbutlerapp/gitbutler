use crate::utils::{fixture, four_commits};
use but_rebase::{Rebase, RebaseStep};
use gix::ObjectId;
use std::str::FromStr;

fn non_existing_commit() -> gix::ObjectId {
    ObjectId::from_str("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap()
}

#[test]
fn base_non_existing() -> anyhow::Result<()> {
    let repo = fixture("four-commits")?;
    let result = Rebase::new(&repo, non_existing_commit(), None);
    assert_eq!(
        result.unwrap_err().to_string(),
        "Base commit must exist if provided: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
    );
    Ok(())
}

#[test]
fn non_existing_commit_in_pick_step() -> anyhow::Result<()> {
    let (repo, commits) = four_commits()?;
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder.step(RebaseStep::Pick {
        commit_id: non_existing_commit(),
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder.step(RebaseStep::Merge {
        commit_id: non_existing_commit(),
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder.step(RebaseStep::SquashIntoPreceding {
        commit_id: non_existing_commit(),
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder.step(RebaseStep::SquashIntoPreceding {
        commit_id: commits.base,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder.step(RebaseStep::Merge {
        commit_id: commits.base,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder.step(RebaseStep::SquashIntoPreceding {
        commit_id: commits.base,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder
        .step(RebaseStep::Pick {
            commit_id: commits.a,
            new_message: None,
        })?
        .step(RebaseStep::Pick {
            commit_id: commits.a,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder
        .step(RebaseStep::Merge {
            commit_id: commits.a,
            new_message: "merge commit".into(),
        })?
        .step(RebaseStep::Pick {
            commit_id: commits.a,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder
        .step(RebaseStep::Pick {
            commit_id: commits.a,
            new_message: None,
        })?
        .step(RebaseStep::SquashIntoPreceding {
            commit_id: commits.b,
            new_message: None,
        })?
        .step(RebaseStep::Pick {
            commit_id: commits.b,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder
        .step(RebaseStep::Pick {
            commit_id: commits.a,
            new_message: None,
        })?
        .step(RebaseStep::Merge {
            commit_id: commits.a,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder
        .step(RebaseStep::Merge {
            commit_id: commits.a,
            new_message: "merge commit".into(),
        })?
        .step(RebaseStep::Merge {
            commit_id: commits.a,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder
        .step(RebaseStep::Pick {
            commit_id: commits.a,
            new_message: None,
        })?
        .step(RebaseStep::SquashIntoPreceding {
            commit_id: commits.b,
            new_message: None,
        })?
        .step(RebaseStep::Merge {
            commit_id: commits.b,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder
        .step(RebaseStep::Pick {
            commit_id: commits.a,
            new_message: None,
        })?
        .step(RebaseStep::SquashIntoPreceding {
            commit_id: commits.a,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder
        .step(RebaseStep::Merge {
            commit_id: commits.a,
            new_message: "merge commit".into(),
        })?
        .step(RebaseStep::SquashIntoPreceding {
            commit_id: commits.a,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder
        .step(RebaseStep::Pick {
            commit_id: commits.a,
            new_message: None,
        })?
        .step(RebaseStep::SquashIntoPreceding {
            commit_id: commits.b,
            new_message: None,
        })?
        .step(RebaseStep::SquashIntoPreceding {
            commit_id: commits.b,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder.step(RebaseStep::SquashIntoPreceding {
        commit_id: commits.a,
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
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder
        .step(RebaseStep::Reference(but_core::Reference::Virtual(
            "foo/bar".into(),
        )))?
        .step(RebaseStep::SquashIntoPreceding {
            commit_id: commits.a,
            new_message: None,
        });
    assert_eq!(
        result.unwrap_err().to_string(),
        "Fixup commit must not come after a reference step"
    );
    Ok(())
}

#[test]
fn empty_reference_step() -> anyhow::Result<()> {
    let (repo, commits) = four_commits()?;
    let mut builder = Rebase::new(&repo, commits.base, None)?;
    let result = builder.step(RebaseStep::Reference(but_core::Reference::Virtual(
        "".into(),
    )));
    assert_eq!(
        result.unwrap_err().to_string(),
        "Reference step must have a non-empty virtual branch name"
    );
    Ok(())
}
