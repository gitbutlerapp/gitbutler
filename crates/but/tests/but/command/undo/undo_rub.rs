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
fn undo_squash_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub")
            .arg("9a")
            .arg("fe")
            .assert()
            .success()
            .stdout_eq("Squashed 9ac4652 → f66c907\n")
            .stderr_eq("");
    });
}

// RubOperation::UnassignUncommitted
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between uncommitted, branch, and stack buckets. https://linear.app/gitbutler/issue/GB-1435/cannot-undo-rub-operations-that-deal-with-uncommitted-changes"]
fn undo_unassign_uncommitted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("unassign-uncommitted.txt", "content\n");
    env.but("rub unassign-uncommitted.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A@{stack}:unassign-uncommitted.txt zz")
            .assert()
            .success()
            .stdout_eq("Unstaged the only hunk in unassign-uncommitted.txt in a stack\n")
            .stderr_eq("");
    });
}

// RubOperation::UncommittedToCommit
#[test]
fn undo_uncommitted_hunk_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("uncommitted-to-commit.txt", "content\n");
    let target_commit = branch_commit_ids(&status_json(&env).unwrap(), "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub uncommitted-to-commit.txt {target_commit}"))
            .assert()
            .success()
            .stdout_eq("Amended [..] → [..]\n")
            .stderr_eq("");
    });
}

// RubOperation::UncommittedToBranch
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between uncommitted, branch, and stack buckets. https://linear.app/gitbutler/issue/GB-1435/cannot-undo-rub-operations-that-deal-with-uncommitted-changes"]
fn undo_uncommitted_hunk_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("uncommitted-to-branch.txt", "content\n");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub uncommitted-to-branch.txt A")
            .assert()
            .success()
            .stdout_eq(
                "Staged the only hunk in uncommitted-to-branch.txt in the uncommitted area → [A].\n",
            )
            .stderr_eq("");
    });
}

// RubOperation::UncommittedToStack
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between uncommitted, branch, and stack buckets. https://linear.app/gitbutler/issue/GB-1435/cannot-undo-rub-operations-that-deal-with-uncommitted-changes"]
fn undo_uncommitted_hunk_to_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("uncommitted-to-stack.txt", "content\n");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub uncommitted-to-stack.txt A@{stack}")
            .assert()
            .success()
            .stdout_eq("Staged the only hunk in uncommitted-to-stack.txt in the uncommitted area → stack [..].\n")
            .stderr_eq("");
    });
}

// RubOperation::StackToUncommittedArea
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between uncommitted, branch, and stack buckets. https://linear.app/gitbutler/issue/GB-1435/cannot-undo-rub-operations-that-deal-with-uncommitted-changes"]
fn undo_stack_to_uncommitted_area() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("stack-to-uncommitted.txt", "content\n");
    env.but("rub stack-to-uncommitted.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A@{stack} zz")
            .assert()
            .success()
            .stdout_eq("Unstaged all [A] changes.\n")
            .stderr_eq("");
    });
}

// RubOperation::StackToStack
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between uncommitted, branch, and stack buckets. https://linear.app/gitbutler/issue/GB-1435/cannot-undo-rub-operations-that-deal-with-uncommitted-changes"]
fn undo_stack_to_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("stack-to-stack.txt", "content\n");
    env.but("rub stack-to-stack.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A@{stack} B@{stack}")
            .assert()
            .success()
            .stdout_eq("Staged all [A] changes to [B].\n")
            .stderr_eq("");
    });
}

// RubOperation::StackToBranch
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between uncommitted, branch, and stack buckets. https://linear.app/gitbutler/issue/GB-1435/cannot-undo-rub-operations-that-deal-with-uncommitted-changes"]
fn undo_stack_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("stack-to-branch.txt", "content\n");
    env.but("rub stack-to-branch.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A@{stack} B")
            .assert()
            .success()
            .stdout_eq("Staged all [A] changes to [B].\n")
            .stderr_eq("");
    });
}

// RubOperation::StackToCommit
#[test]
fn undo_stack_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("stack-to-commit.txt", "content\n");
    env.but("rub stack-to-commit.txt A").assert().success();
    let target_commit = branch_commit_ids(&status_json(&env).unwrap(), "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub A@{{stack}} {target_commit}"))
            .assert()
            .success()
            .stdout_eq("Amended files assigned to [A] → [..]\n")
            .stderr_eq("");
    });
}

// RubOperation::UncommittedAreaToCommit
#[test]
fn undo_uncommitted_area_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("uncommitted-to-commit.txt", "content\n");
    let target_commit = branch_commit_ids(&status_json(&env).unwrap(), "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub zz {target_commit}"))
            .assert()
            .success()
            .stdout_eq("Amended uncommitted files → [..]\n")
            .stderr_eq("");
    });
}

// RubOperation::UncommittedAreaToBranch
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between uncommitted, branch, and stack buckets. https://linear.app/gitbutler/issue/GB-1435/cannot-undo-rub-operations-that-deal-with-uncommitted-changes"]
fn undo_uncommitted_area_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("uncommitted-to-branch.txt", "content\n");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub zz A")
            .assert()
            .success()
            .stdout_eq("Staged all unstaged changes to [A].\n")
            .stderr_eq("");
    });
}

// RubOperation::UncommittedAreaToStack
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between uncommitted, branch, and stack buckets. https://linear.app/gitbutler/issue/GB-1435/cannot-undo-rub-operations-that-deal-with-uncommitted-changes"]
fn undo_uncommitted_area_to_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("uncommitted-to-stack.txt", "content\n");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub zz A@{stack}")
            .assert()
            .success()
            .stdout_eq("Staged all unstaged changes to [A].\n")
            .stderr_eq("");
    });
}

// RubOperation::CommitToUncommittedArea
#[test]
fn undo_commit_to_uncommitted_area() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(
        &env,
        "A",
        "commit-to-zz-a.txt",
        "commit-to-zz-b.txt",
        "first",
    );
    let source_commit = branch_commit_ids(&status_json(&env).unwrap(), "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit} zz"))
            .assert()
            .success()
            .stdout_eq("Uncommitted [..]\n")
            .stderr_eq("");
    });
}

// RubOperation::CommitToStack
#[test]
fn undo_commit_to_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(
        &env,
        "A",
        "commit-to-stack-a.txt",
        "commit-to-stack-b.txt",
        "first",
    );
    let source_commit = branch_commit_ids(&status_json(&env).unwrap(), "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit} B@{{stack}}"))
            .assert()
            .success()
            .stdout_eq("Uncommitted [..] to [B]\n")
            .stderr_eq("");
    });
}

// RubOperation::MoveCommitToBranch
#[test]
fn undo_move_commit_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(
        &env,
        "A",
        "commit-to-branch-a.txt",
        "commit-to-branch-b.txt",
        "first",
    );
    let source_commit = branch_commit_ids(&status_json(&env).unwrap(), "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit} B"))
            .assert()
            .success()
            .stdout_eq("Moved [..] → [B]\n")
            .stderr_eq("");
    });
}

// RubOperation::BranchToUncommittedArea
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between uncommitted, branch, and stack buckets. https://linear.app/gitbutler/issue/GB-1435/cannot-undo-rub-operations-that-deal-with-uncommitted-changes"]
fn undo_branch_to_uncommitted_area() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("branch-to-uncommitted.txt", "content\n");
    env.but("rub branch-to-uncommitted.txt A")
        .assert()
        .success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A zz")
            .assert()
            .success()
            .stdout_eq("Unstaged all [A] changes.\n")
            .stderr_eq("");
    });
}

// RubOperation::BranchToStack
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between uncommitted, branch, and stack buckets. https://linear.app/gitbutler/issue/GB-1435/cannot-undo-rub-operations-that-deal-with-uncommitted-changes"]
fn undo_branch_to_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("branch-to-stack.txt", "content\n");
    env.but("rub branch-to-stack.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A B@{stack}")
            .assert()
            .success()
            .stdout_eq("Staged all [A] changes to [B].\n")
            .stderr_eq("");
    });
}

// RubOperation::BranchToCommit
#[test]
fn undo_branch_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("branch-to-commit.txt", "content\n");
    env.but("rub branch-to-commit.txt A").assert().success();
    let target_commit = branch_commit_ids(&status_json(&env).unwrap(), "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub A {target_commit}"))
            .assert()
            .success()
            .stdout_eq("Amended assigned files [A] → [..]\n")
            .stderr_eq("");
    });
}

// RubOperation::BranchToBranch
#[test]
#[ignore = "undo currently does not restore hunk assignment metadata for rub operations that only move changes between uncommitted, branch, and stack buckets. https://linear.app/gitbutler/issue/GB-1435/cannot-undo-rub-operations-that-deal-with-uncommitted-changes"]
fn undo_branch_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("branch-to-branch.txt", "content\n");
    env.but("rub branch-to-branch.txt A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("rub A B")
            .assert()
            .success()
            .stdout_eq("Staged all [A] changes to [B].\n")
            .stderr_eq("");
    });
}

// RubOperation::CommittedFileToBranch
#[test]
fn undo_committed_file_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(
        &env,
        "A",
        "file-to-branch-a.txt",
        "file-to-branch-b.txt",
        "first",
    );
    let source_commit = branch_commit_ids(&status_json(&env).unwrap(), "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit}:file-to-branch-a.txt B"))
            .assert()
            .success()
            .stdout_eq("Uncommitted changes\n")
            .stderr_eq("");
    });
}

// RubOperation::CommittedFileToCommit
#[test]
fn undo_committed_file_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    commit_two_files_as_two_hunks_each(&env, "A", "source-a.txt", "source-b.txt", "source");
    commit_two_files_as_two_hunks_each(&env, "A", "target-a.txt", "target-b.txt", "target");
    let status = status_json_with_files(&env).unwrap();
    let source_commit = branch_commit_id_for_file(&status, "A", "source-a.txt").unwrap();
    let target_commit = branch_commit_id_for_file(&status, "A", "target-a.txt").unwrap();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit}:source-a.txt {target_commit}"))
            .assert()
            .success()
            .stdout_eq("Moved files between commits!\n")
            .stderr_eq("");
    });
}

// RubOperation::CommittedFileToUncommittedArea
#[test]
fn undo_committed_file_to_uncommitted_area() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "file-to-zz-a.txt", "file-to-zz-b.txt", "first");
    let source_commit = branch_commit_ids(&status_json(&env).unwrap(), "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("rub {source_commit}:file-to-zz-a.txt zz"))
            .assert()
            .success()
            .stdout_eq("Uncommitted changes\n")
            .stderr_eq("");
    });
}
