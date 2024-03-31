use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use gitbutler_core::virtual_branches::{
    branch::{self, BranchOwnershipClaims},
    Branch, BranchId, VirtualBranchesHandle,
};
use once_cell::sync::Lazy;

use crate::shared::{Case, Suite};

static TEST_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

fn test_branch() -> Branch {
    TEST_INDEX.fetch_add(1, Ordering::Relaxed);

    Branch {
        id: BranchId::generate(),
        name: format!("branch_name_{}", TEST_INDEX.load(Ordering::Relaxed)),
        notes: String::new(),
        applied: true,
        order: TEST_INDEX.load(Ordering::Relaxed),
        upstream: Some(
            format!(
                "refs/remotes/origin/upstream_{}",
                TEST_INDEX.load(Ordering::Relaxed)
            )
            .parse()
            .unwrap(),
        ),
        upstream_head: Some(
            format!(
                "0123456789abcdef0123456789abcdef0123456{}",
                TEST_INDEX.load(Ordering::Relaxed)
            )
            .parse()
            .unwrap(),
        ),
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
        ownership: BranchOwnershipClaims {
            claims: vec![format!("file/{}:1-2", TEST_INDEX.load(Ordering::Relaxed))
                .parse()
                .unwrap()],
        },
        selected_for_changes: Some(1),
    }
}

#[test]
fn read_not_found() -> Result<()> {
    let suite = Suite::default();
    let Case {
        gb_repository,
        project,
        ..
    } = &suite.new_case();

    let session = gb_repository.get_or_create_current_session()?;
    let session_reader = gitbutler_core::sessions::Reader::open(gb_repository, &session)?;

    let reader = branch::Reader::new(
        &session_reader,
        VirtualBranchesHandle::new(&project.gb_dir()),
        project.use_toml_vbranches_state(),
    );
    let result = reader.read(&BranchId::generate());
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "file not found");

    Ok(())
}

#[test]
fn read_override() -> Result<()> {
    let suite = Suite::default();
    let Case {
        gb_repository,
        project,
        ..
    } = &suite.new_case();

    let mut branch = test_branch();

    let writer = branch::Writer::new(gb_repository, VirtualBranchesHandle::new(&project.gb_dir()))?;
    writer.write(&mut branch)?;

    let session = gb_repository.get_current_session()?.unwrap();
    let session_reader = gitbutler_core::sessions::Reader::open(gb_repository, &session)?;

    let reader = branch::Reader::new(
        &session_reader,
        VirtualBranchesHandle::new(&project.gb_dir()),
        project.use_toml_vbranches_state(),
    );

    assert_eq!(branch, reader.read(&branch.id).unwrap());

    Ok(())
}
