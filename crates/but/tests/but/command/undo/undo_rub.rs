use crate::{
    command::{
        undo::run_mutate_undo_roundtrip_test,
        util::{
            branch_commit_id_for_file, branch_commit_ids, commit_two_files_as_two_hunks_each,
            status_json, status_json_with_files,
        },
    },
    utils::Sandbox,
};

// RubOperation::SquashCommits
#[test]
fn undo_squash_commits() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub")
            .arg("9a")
            .arg("fe")
            .assert()
            .success()
            .stdout_eq("Squashed 9ac4652 → 37c7b0c\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::UnassignUncommitted
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between unassigned, branch, and stack buckets"]
fn undo_unassign_uncommitted() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("unassign-uncommitted.txt", "content\n");
    env.but("rub unassign-uncommitted.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A@{stack}:unassign-uncommitted.txt zz")
            .assert()
            .success()
            .stdout_eq("Unstaged the only hunk in unassign-uncommitted.txt in a stack\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::UncommittedToCommit
#[test]
fn undo_uncommitted_to_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("uncommitted-to-commit.txt", "content\n");
    let target_commit = branch_commit_ids(&status_json(&env)?, "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub uncommitted-to-commit.txt {target_commit}"))
            .assert()
            .success()
            .stdout_eq("Amended [..] → [..]\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::UncommittedToBranch
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between unassigned, branch, and stack buckets"]
fn undo_uncommitted_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("uncommitted-to-branch.txt", "content\n");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub uncommitted-to-branch.txt A")
            .assert()
            .success()
            .stdout_eq(
                "Staged the only hunk in uncommitted-to-branch.txt in the unassigned area → [A].\n",
            )
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::UncommittedToStack
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between unassigned, branch, and stack buckets"]
fn undo_uncommitted_to_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("uncommitted-to-stack.txt", "content\n");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub uncommitted-to-stack.txt A@{stack}")
            .assert()
            .success()
            .stdout_eq("Staged the only hunk in uncommitted-to-stack.txt in the unassigned area → stack [..].\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::StackToUnassigned
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between unassigned, branch, and stack buckets"]
fn undo_stack_to_unassigned() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("stack-to-unassigned.txt", "content\n");
    env.but("rub stack-to-unassigned.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A@{stack} zz")
            .assert()
            .success()
            .stdout_eq("Unstaged all [A] changes.\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::StackToStack
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between unassigned, branch, and stack buckets"]
fn undo_stack_to_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("stack-to-stack.txt", "content\n");
    env.but("rub stack-to-stack.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A@{stack} B@{stack}")
            .assert()
            .success()
            .stdout_eq("Staged all [A] changes to [B].\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::StackToBranch
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between unassigned, branch, and stack buckets"]
fn undo_stack_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("stack-to-branch.txt", "content\n");
    env.but("rub stack-to-branch.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A@{stack} B")
            .assert()
            .success()
            .stdout_eq("Staged all [A] changes to [B].\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::StackToCommit
#[test]
fn undo_stack_to_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("stack-to-commit.txt", "content\n");
    env.but("rub stack-to-commit.txt A").assert().success();
    let target_commit = branch_commit_ids(&status_json(&env)?, "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub A@{{stack}} {target_commit}"))
            .assert()
            .success()
            .stdout_eq("Amended files assigned to [A] → [..]\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::UnassignedToCommit
#[test]
fn undo_unassigned_to_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("unassigned-to-commit.txt", "content\n");
    let target_commit = branch_commit_ids(&status_json(&env)?, "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub zz {target_commit}"))
            .assert()
            .success()
            .stdout_eq("Amended unassigned files → [..]\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::UnassignedToBranch
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between unassigned, branch, and stack buckets"]
fn undo_unassigned_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("unassigned-to-branch.txt", "content\n");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub zz A")
            .assert()
            .success()
            .stdout_eq("Staged all unstaged changes to [A].\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::UnassignedToStack
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between unassigned, branch, and stack buckets"]
fn undo_unassigned_to_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("unassigned-to-stack.txt", "content\n");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub zz A@{stack}")
            .assert()
            .success()
            .stdout_eq("Staged all unstaged changes to [A].\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::CommitToUnassigned
#[test]
fn undo_commit_to_unassigned() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    commit_two_files_as_two_hunks_each(
        &env,
        "A",
        "commit-to-zz-a.txt",
        "commit-to-zz-b.txt",
        "first",
    );
    let source_commit = branch_commit_ids(&status_json(&env)?, "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit} zz"))
            .assert()
            .success()
            .stdout_eq("Uncommitted [..]\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::CommitToStack
#[test]
fn undo_commit_to_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    commit_two_files_as_two_hunks_each(
        &env,
        "A",
        "commit-to-stack-a.txt",
        "commit-to-stack-b.txt",
        "first",
    );
    let source_commit = branch_commit_ids(&status_json(&env)?, "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit} B@{{stack}}"))
            .assert()
            .success()
            .stdout_eq("Uncommitted [..] to [B]\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::MoveCommitToBranch
#[test]
fn undo_move_commit_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    commit_two_files_as_two_hunks_each(
        &env,
        "A",
        "commit-to-branch-a.txt",
        "commit-to-branch-b.txt",
        "first",
    );
    let source_commit = branch_commit_ids(&status_json(&env)?, "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit} B"))
            .assert()
            .success()
            .stdout_eq("Moved [..] → [B]\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::BranchToUnassigned
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between unassigned, branch, and stack buckets"]
fn undo_branch_to_unassigned() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("branch-to-unassigned.txt", "content\n");
    env.but("rub branch-to-unassigned.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A zz")
            .assert()
            .success()
            .stdout_eq("Unstaged all [A] changes.\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::BranchToStack
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between unassigned, branch, and stack buckets"]
fn undo_branch_to_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("branch-to-stack.txt", "content\n");
    env.but("rub branch-to-stack.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A B@{stack}")
            .assert()
            .success()
            .stdout_eq("Staged all [A] changes to [B].\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::BranchToCommit
#[test]
fn undo_branch_to_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("branch-to-commit.txt", "content\n");
    env.but("rub branch-to-commit.txt A").assert().success();
    let target_commit = branch_commit_ids(&status_json(&env)?, "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub A {target_commit}"))
            .assert()
            .success()
            .stdout_eq("Amended assigned files [A] → [..]\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::BranchToBranch
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between unassigned, branch, and stack buckets"]
fn undo_branch_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("branch-to-branch.txt", "content\n");
    env.but("rub branch-to-branch.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A B")
            .assert()
            .success()
            .stdout_eq("Staged all [A] changes to [B].\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::CommittedFileToBranch
#[test]
fn undo_committed_file_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    commit_two_files_as_two_hunks_each(
        &env,
        "A",
        "file-to-branch-a.txt",
        "file-to-branch-b.txt",
        "first",
    );
    let source_commit = branch_commit_ids(&status_json(&env)?, "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit}:file-to-branch-a.txt B"))
            .assert()
            .success()
            .stdout_eq("Uncommitted changes\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::CommittedFileToCommit
#[test]
fn undo_committed_file_to_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    commit_two_files_as_two_hunks_each(&env, "A", "source-a.txt", "source-b.txt", "source");
    commit_two_files_as_two_hunks_each(&env, "A", "target-a.txt", "target-b.txt", "target");
    let status = status_json_with_files(&env)?;
    let source_commit = branch_commit_id_for_file(&status, "A", "source-a.txt").unwrap();
    let target_commit = branch_commit_id_for_file(&status, "A", "target-a.txt").unwrap();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit}:source-a.txt {target_commit}"))
            .assert()
            .success()
            .stdout_eq("Moved files between commits!\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

// RubOperation::CommittedFileToUnassigned
#[test]
fn undo_committed_file_to_unassigned() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    commit_two_files_as_two_hunks_each(&env, "A", "file-to-zz-a.txt", "file-to-zz-b.txt", "first");
    let source_commit = branch_commit_ids(&status_json(&env)?, "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit}:file-to-zz-a.txt zz"))
            .assert()
            .success()
            .stdout_eq("Uncommitted changes\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}
