use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Context;
use gitbutler_core::virtual_branches::branch;
use once_cell::sync::Lazy;

use crate::shared::{Case, Suite};

use self::branch::BranchId;

use super::*;

static TEST_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

fn new_test_branch() -> Branch {
    TEST_INDEX.fetch_add(1, Ordering::Relaxed);

    Branch {
        id: BranchId::generate(),
        name: format!("branch_name_{}", TEST_INDEX.load(Ordering::Relaxed)),
        notes: String::new(),
        applied: true,
        upstream: Some(
            format!(
                "refs/remotes/origin/upstream_{}",
                TEST_INDEX.load(Ordering::Relaxed)
            )
            .parse()
            .unwrap(),
        ),
        upstream_head: None,
        created_timestamp_ms: TEST_INDEX.load(Ordering::Relaxed) as u128,
        updated_timestamp_ms: (TEST_INDEX.load(Ordering::Relaxed) + 100) as u128,
        head: format!(
            "0123456789abcdef0123456789abcdef0123456{}",
            TEST_INDEX.load(Ordering::Relaxed)
        )
        .parse()
        .unwrap(),
        tree: format!(
            "0123456789abcdef0123456789abcdef012345{}",
            TEST_INDEX.load(Ordering::Relaxed) + 10
        )
        .parse()
        .unwrap(),
        ownership: gitbutler_core::virtual_branches::branch::BranchOwnershipClaims {
            claims: vec![gitbutler_core::virtual_branches::branch::OwnershipClaim {
                file_path: format!("file/{}:1-2", TEST_INDEX.load(Ordering::Relaxed)).into(),
                hunks: vec![],
            }],
        },
        order: TEST_INDEX.load(Ordering::Relaxed),
        selected_for_changes: Some(1),
    }
}

#[test]
fn write_branch() -> anyhow::Result<()> {
    let suite = Suite::default();
    let Case {
        gb_repository,
        project,
        ..
    } = &suite.new_case();

    let mut branch = new_test_branch();

    let writer = branch::Writer::new(gb_repository, project.gb_dir())?;
    writer.write(&mut branch)?;

    let root = gb_repository
        .root()
        .join("branches")
        .join(branch.id.to_string());

    assert_eq!(
        fs::read_to_string(root.join("meta").join("name").to_str().unwrap())
            .context("Failed to read branch name")?,
        branch.name
    );
    assert_eq!(
        fs::read_to_string(root.join("meta").join("applied").to_str().unwrap())?
            .parse::<bool>()
            .context("Failed to read branch applied")?,
        branch.applied
    );
    assert_eq!(
        fs::read_to_string(root.join("meta").join("upstream").to_str().unwrap())
            .context("Failed to read branch upstream")?,
        branch.upstream.clone().unwrap().to_string()
    );
    assert_eq!(
        fs::read_to_string(
            root.join("meta")
                .join("created_timestamp_ms")
                .to_str()
                .unwrap()
        )
        .context("Failed to read branch created timestamp")?
        .parse::<u128>()
        .context("Failed to parse branch created timestamp")?,
        branch.created_timestamp_ms
    );
    assert_eq!(
        fs::read_to_string(
            root.join("meta")
                .join("updated_timestamp_ms")
                .to_str()
                .unwrap()
        )
        .context("Failed to read branch updated timestamp")?
        .parse::<u128>()
        .context("Failed to parse branch updated timestamp")?,
        branch.updated_timestamp_ms
    );

    writer.delete(&branch)?;
    fs::read_dir(root).unwrap_err();

    Ok(())
}

#[test]
fn should_create_session() -> anyhow::Result<()> {
    let suite = Suite::default();
    let Case {
        gb_repository,
        project,
        ..
    } = &suite.new_case();

    let mut branch = new_test_branch();

    let writer = branch::Writer::new(gb_repository, project.gb_dir())?;
    writer.write(&mut branch)?;

    assert!(gb_repository.get_current_session()?.is_some());

    Ok(())
}

#[test]
fn should_update() -> anyhow::Result<()> {
    let suite = Suite::default();
    let Case {
        gb_repository,
        project,
        ..
    } = &suite.new_case();

    let mut branch = new_test_branch();

    let writer = branch::Writer::new(gb_repository, project.gb_dir())?;
    writer.write(&mut branch)?;

    let mut updated_branch = Branch {
        name: "updated_name".to_string(),
        applied: false,
        upstream: Some("refs/remotes/origin/upstream_updated".parse().unwrap()),
        created_timestamp_ms: 2,
        updated_timestamp_ms: 3,
        ownership: gitbutler_core::virtual_branches::branch::BranchOwnershipClaims {
            claims: vec![],
        },
        ..branch.clone()
    };

    writer.write(&mut updated_branch)?;

    let root = gb_repository
        .root()
        .join("branches")
        .join(branch.id.to_string());

    assert_eq!(
        fs::read_to_string(root.join("meta").join("name").to_str().unwrap())
            .context("Failed to read branch name")?,
        updated_branch.name
    );
    assert_eq!(
        fs::read_to_string(root.join("meta").join("applied").to_str().unwrap())?
            .parse::<bool>()
            .context("Failed to read branch applied")?,
        updated_branch.applied
    );
    assert_eq!(
        fs::read_to_string(root.join("meta").join("upstream").to_str().unwrap())
            .context("Failed to read branch upstream")?,
        updated_branch.upstream.unwrap().to_string()
    );
    assert_eq!(
        fs::read_to_string(
            root.join("meta")
                .join("created_timestamp_ms")
                .to_str()
                .unwrap()
        )
        .context("Failed to read branch created timestamp")?
        .parse::<u128>()
        .context("Failed to parse branch created timestamp")?,
        updated_branch.created_timestamp_ms
    );
    assert_eq!(
        fs::read_to_string(
            root.join("meta")
                .join("updated_timestamp_ms")
                .to_str()
                .unwrap()
        )
        .context("Failed to read branch updated timestamp")?
        .parse::<u128>()
        .context("Failed to parse branch updated timestamp")?,
        updated_branch.updated_timestamp_ms
    );

    Ok(())
}
