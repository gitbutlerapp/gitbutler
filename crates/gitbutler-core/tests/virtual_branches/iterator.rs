use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use gitbutler_core::virtual_branches;
use once_cell::sync::Lazy;

use gitbutler_testsupport::{Case, Suite};

static TEST_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

fn new_test_branch() -> virtual_branches::branch::Branch {
    TEST_INDEX.fetch_add(1, Ordering::Relaxed);

    virtual_branches::branch::Branch {
        id: virtual_branches::BranchId::generate(),
        name: format!("branch_name_{}", TEST_INDEX.load(Ordering::Relaxed)),
        notes: String::new(),
        old_applied: true,
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
        ownership: virtual_branches::branch::BranchOwnershipClaims::default(),
        order: TEST_INDEX.load(Ordering::Relaxed),
        selected_for_changes: Some(1),
    }
}

static TEST_TARGET_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

fn new_test_target() -> virtual_branches::target::Target {
    virtual_branches::target::Target {
        branch: format!(
            "refs/remotes/branch name{}/remote name {}",
            TEST_TARGET_INDEX.load(Ordering::Relaxed),
            TEST_TARGET_INDEX.load(Ordering::Relaxed)
        )
        .parse()
        .unwrap(),
        remote_url: format!("remote url {}", TEST_TARGET_INDEX.load(Ordering::Relaxed)),
        sha: format!(
            "0123456789abcdef0123456789abcdef0123456{}",
            TEST_TARGET_INDEX.load(Ordering::Relaxed)
        )
        .parse()
        .unwrap(),
        push_remote_name: None,
    }
}

#[test]
fn empty_iterator() -> Result<()> {
    let suite = Suite::default();
    let Case { project, .. } = &suite.new_case();

    let vb_state = project.virtual_branches();
    let iter = vb_state.list_branches()?;

    assert_eq!(iter.len(), 0);

    Ok(())
}

#[test]
fn iterate_all() -> Result<()> {
    let suite = Suite::default();
    let Case { project, .. } = &suite.new_case();

    let vb_state = project.virtual_branches();
    vb_state.set_default_target(new_test_target())?;
    let branch_1 = new_test_branch();
    vb_state.set_branch(branch_1.clone())?;
    let branch_2 = new_test_branch();
    vb_state.set_branch(branch_2.clone())?;
    let branch_3 = new_test_branch();
    vb_state.set_branch(branch_3.clone())?;

    let iter = vb_state.list_branches()?;
    assert_eq!(iter.len(), 3);
    assert!(iter.contains(&branch_1));
    assert!(iter.contains(&branch_2));
    assert!(iter.contains(&branch_3));

    Ok(())
}
