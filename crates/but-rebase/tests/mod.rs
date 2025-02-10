use std::str::FromStr;

use anyhow::Result;
use but_rebase::{RebaseBuilder, RebaseStep};
use gix::ObjectId;

#[test]
fn base_non_existing() -> Result<()> {
    let result = RebaseBuilder::new(
        fixture("four-commits")?,
        ObjectId::from_str("15b8235197020a417e9405ab5d4db6f204e8d84b")?, // does not exist
    );
    assert_eq!(
        result.err().unwrap().to_string(),
        "An object with id 15b8235197020a417e9405ab5d4db6f204e8d84b could not be found"
    );
    Ok(())
}

#[test]
fn non_existing_commit_in_pick_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    let result = builder.step(RebaseStep::Pick {
        oid: ObjectId::from_str("15b8235197020a417e9405ab5d4db6f204e8d84b")?, // does not exist
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "An object with id 15b8235197020a417e9405ab5d4db6f204e8d84b could not be found"
    );
    Ok(())
}

#[test]
fn non_existing_commit_in_merge_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    let result = builder.step(RebaseStep::Merge {
        oid: ObjectId::from_str("15b8235197020a417e9405ab5d4db6f204e8d84b")?, // does not exist
        new_message: "merge commit".into(),
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "An object with id 15b8235197020a417e9405ab5d4db6f204e8d84b could not be found"
    );
    Ok(())
}

#[test]
fn non_existing_commit_in_fixup_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    let result = builder.step(RebaseStep::Fixup {
        oid: ObjectId::from_str("15b8235197020a417e9405ab5d4db6f204e8d84b")?, // does not exist
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "An object with id 15b8235197020a417e9405ab5d4db6f204e8d84b could not be found"
    );
    Ok(())
}

#[test]
fn using_base_in_pick_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    let result = builder.step(RebaseStep::Fixup {
        oid: commits.base,
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Fixup commit cannot be the base commit"
    );
    Ok(())
}

#[test]
fn using_base_in_merge_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    let result = builder.step(RebaseStep::Merge {
        oid: commits.base,
        new_message: "merge commit".into(),
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Merge commit cannot be the base commit"
    );
    Ok(())
}

#[test]
fn using_base_in_fixup_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    let result = builder.step(RebaseStep::Fixup {
        oid: commits.base,
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Fixup commit cannot be the base commit"
    );
    Ok(())
}

#[test]
fn using_picked_commit_in_a_pick_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder.step(RebaseStep::Pick {
        oid: commits.a,
        new_message: None,
    })?;
    let result = builder.step(RebaseStep::Pick {
        oid: commits.a,
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Picked commit already exists in a previous step"
    );
    Ok(())
}

#[test]
fn using_merged_commit_in_a_pick_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder.step(RebaseStep::Merge {
        oid: commits.a,
        new_message: "merge commit".into(),
    })?;
    let result = builder.step(RebaseStep::Pick {
        oid: commits.a,
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Picked commit already exists in a previous step"
    );
    Ok(())
}

#[test]
fn using_fixup_commit_in_a_pick_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder.step(RebaseStep::Pick {
        oid: commits.a,
        new_message: None,
    })?;
    builder.step(RebaseStep::Fixup {
        oid: commits.b,
        new_message: None,
    })?;
    let result = builder.step(RebaseStep::Pick {
        oid: commits.b,
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Picked commit already exists in a previous step"
    );
    Ok(())
}

#[test]
fn using_picked_commit_in_a_merge_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder.step(RebaseStep::Pick {
        oid: commits.a,
        new_message: None,
    })?;
    let result = builder.step(RebaseStep::Merge {
        oid: commits.a,
        new_message: "merge commit".into(),
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Picked commit already exists in a previous step"
    );
    Ok(())
}

#[test]
fn using_merged_commit_in_a_merge_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder.step(RebaseStep::Merge {
        oid: commits.a,
        new_message: "merge commit".into(),
    })?;
    let result = builder.step(RebaseStep::Merge {
        oid: commits.a,
        new_message: "merge commit".into(),
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Picked commit already exists in a previous step"
    );
    Ok(())
}

#[test]
fn using_fixup_commit_in_a_merge_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder.step(RebaseStep::Pick {
        oid: commits.a,
        new_message: None,
    })?;
    builder.step(RebaseStep::Fixup {
        oid: commits.b,
        new_message: None,
    })?;
    let result = builder.step(RebaseStep::Merge {
        oid: commits.b,
        new_message: "merge commit".into(),
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Picked commit already exists in a previous step"
    );
    Ok(())
}

#[test]
fn using_picked_commit_in_a_fixup_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder.step(RebaseStep::Pick {
        oid: commits.a,
        new_message: None,
    })?;
    let result = builder.step(RebaseStep::Fixup {
        oid: commits.a,
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Picked commit already exists in a previous step"
    );
    Ok(())
}

#[test]
fn using_merged_commit_in_a_fixup_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder.step(RebaseStep::Merge {
        oid: commits.a,
        new_message: "merge commit".into(),
    })?;
    let result = builder.step(RebaseStep::Fixup {
        oid: commits.a,
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Picked commit already exists in a previous step"
    );
    Ok(())
}

#[test]
fn using_fixup_commit_in_a_fixup_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder.step(RebaseStep::Pick {
        oid: commits.a,
        new_message: None,
    })?;
    builder.step(RebaseStep::Fixup {
        oid: commits.b,
        new_message: None,
    })?;
    let result = builder.step(RebaseStep::Fixup {
        oid: commits.b,
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Picked commit already exists in a previous step"
    );
    Ok(())
}

#[test]
fn fixup_is_first_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    let result = builder.step(RebaseStep::Fixup {
        oid: commits.a,
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Fixup must have a commit to work on"
    );
    Ok(())
}

#[test]
fn fixup_is_only_preceeded_by_a_reference_step() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder.step(RebaseStep::Reference {
        refname: "foo/bar".into(),
    })?;
    let result = builder.step(RebaseStep::Fixup {
        oid: commits.a,
        new_message: None,
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Fixup commit must not come after a reference step"
    );
    Ok(())
}

#[test]
fn reference_is_not_a_valid_reference_name() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    let result = builder.step(RebaseStep::Reference {
        refname: "abc".into(),
    });
    assert_eq!(
        result.err().unwrap().to_string(),
        "Standalone references must be all uppercased, like 'HEAD'"
    );
    Ok(())
}

#[test]
fn happy_case_scenario() -> Result<()> {
    let repo = fixture("four-commits")?;
    let commits = commits(&repo)?;
    let mut builder = RebaseBuilder::new(repo, commits.base)?;
    builder.step(RebaseStep::Pick {
        oid: commits.a,
        new_message: Some("updated commit message".into()),
    })?;
    builder.step(RebaseStep::Fixup {
        oid: commits.b,
        new_message: None,
    })?;
    builder.step(RebaseStep::Reference {
        refname: "my/ref".into(),
    })?;
    builder.step(RebaseStep::Merge {
        oid: commits.c,
        new_message: "merge commit".into(),
    })?;
    Ok(())
}

fn fixture(fixture_name: &str) -> Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only("rebase.sh")
        .map_err(anyhow::Error::from_boxed)?;
    let worktree_root = root.join(fixture_name);
    let repo = gix::open(worktree_root)?;

    Ok(repo)
}

#[derive(Debug)]
struct Commits {
    base: ObjectId,
    a: ObjectId,
    b: ObjectId,
    c: ObjectId,
}

/// The commits in the fixture repo, starting from the oldest
fn commits(repo: &gix::Repository) -> Result<Commits> {
    let mut commits = vec![];
    let mut head = repo.head_commit()?;
    commits.push(head.id);
    let mut first_parent = head.parent_ids().next();
    while let Some(parent) = first_parent {
        head = repo.find_commit(parent)?;
        first_parent = head.parent_ids().next();
        commits.push(head.id);
    }
    Ok(Commits {
        base: commits[3],
        a: commits[2],
        b: commits[1],
        c: commits[0],
    })
}
