use but_core::RefMetadata;

use crate::{
    command::{
        undo::run_mutate_undo_roundtrip_test,
        util::{add_dirty_detached_worktree, add_worktree_with_commit},
    },
    utils::Sandbox,
};

#[test]
fn can_undo_single_branch_commit_on_new_branch() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.file("new-file.txt", "content\n");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -b middle -m 'on middle'")
            .assert()
            .success();
    });

    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "main",
        "undo must restore the original checkout"
    );
}

#[test]
fn can_undo_single_branch_commit_on_checked_out_branch() {
    let env = Sandbox::open_with_default_settings("one-fork");
    env.file("new-file.txt", "content\n");
    let head_before = env.invoke_git("rev-parse HEAD");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'on main'").assert().success();
    });

    assert_eq!(
        env.invoke_git("rev-parse HEAD"),
        head_before,
        "undo must restore the original branch tip"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "main",
        "undo must keep the original branch checked out"
    );
}

#[test]
fn can_undo_single_branch_commit_at_each_existing_stack_branch() {
    for branch in ["middle", "top"] {
        let env = Sandbox::open_with_default_settings("one-fork");
        env.but("commit -b middle -m 'on middle'")
            .assert()
            .success();
        env.but("commit -b top --above middle -m 'on top'")
            .assert()
            .success();
        env.file("new-file.txt", "content\n");
        let log_before = env.git_log();

        run_mutate_undo_roundtrip_test(&env, |env| {
            env.but(format!("commit -b {branch} -m 'add file'"))
                .assert()
                .success();
        });

        // Undo restores every rewritten branch tip, not just the selected branch.
        snapbox::assert_data_eq!(env.git_log(), log_before);
    }
}

#[test]
fn can_undo_single_branch_commit_on_new_branch_above() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("commit -b middle -m 'on middle'")
        .assert()
        .success();

    env.file("selected.txt", "selected content\n");
    env.file("remaining.txt", "leave this uncommitted\n");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -b top --above middle -m 'on top' selected.txt")
            .assert()
            .success();
        assert_eq!(
            env.invoke_git("show top:selected.txt"),
            "selected content",
            "the insertion commits the selected file"
        );
        assert_eq!(
            env.invoke_git("status --porcelain"),
            "?? remaining.txt",
            "the other file remains uncommitted"
        );
    });

    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "middle",
        "undo must switch back from the newly created top branch"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join("selected.txt")).unwrap(),
        b"selected content\n",
        "undo must restore the selected file's exact bytes"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join("remaining.txt")).unwrap(),
        b"leave this uncommitted\n",
        "undo must preserve the unselected file's exact bytes"
    );
}

#[test]
fn can_undo_single_branch_commit_on_new_branch_below() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("commit -b middle -m 'on middle'")
        .assert()
        .success();
    env.but("commit -b top --above middle -m 'on top'")
        .assert()
        .success();

    env.file("selected.txt", "selected content\n");
    env.file("remaining.txt", "leave this uncommitted\n");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -b bottom --below middle -m 'on bottom' selected.txt")
            .assert()
            .success();
        assert_eq!(
            env.invoke_git("show bottom:selected.txt"),
            "selected content",
            "the insertion must commit the selected file"
        );
        assert_eq!(
            env.invoke_git("status --porcelain"),
            "?? remaining.txt",
            "the other file must remain uncommitted"
        );
    });

    assert_eq!(
        std::fs::read(env.projects_root().join("selected.txt")).unwrap(),
        b"selected content\n",
        "undo must restore the selected file's exact bytes"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join("remaining.txt")).unwrap(),
        b"leave this uncommitted\n",
        "undo must preserve the unselected file's exact bytes"
    );
}

#[test]
fn can_undo_single_branch_commit_above_lower_branch() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("commit -b bottom -m 'on bottom'")
        .assert()
        .success();
    env.but("commit -b top --above bottom -m 'on top'")
        .assert()
        .success();

    // Modify a tracked file so the roundtrip checks an exact patch as well as bytes.
    env.file("remaining.txt", "original\n");
    env.but("commit -b top -m 'add tracked file'")
        .assert()
        .success();
    env.file("remaining.txt", "changed\n");
    env.file("selected.txt", "selected content\n");
    let diff_before = env.invoke_git("diff HEAD -- remaining.txt");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -b middle --above bottom -m 'on middle' selected.txt")
            .assert()
            .success();
        assert_eq!(
            env.invoke_git("show middle:selected.txt"),
            "selected content",
            "the insertion commits the selected file"
        );
        assert_eq!(
            env.invoke_git("diff HEAD -- remaining.txt"),
            diff_before,
            "the partial commit leaves the unselected patch intact"
        );
    });

    assert_eq!(
        env.invoke_git("diff HEAD -- remaining.txt"),
        diff_before,
        "undo must preserve the exact unselected patch"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join("remaining.txt")).unwrap(),
        b"changed\n",
        "undo must preserve the unselected file's exact bytes"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join("selected.txt")).unwrap(),
        b"selected content\n",
        "undo must restore the committed file's exact bytes"
    );
}

#[test]
fn can_undo_and_redo_single_branch_commit_creating_workspace() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("commit -b one -m 'on one'").assert().success();
    env.file("new-file.txt", "content\n");

    let tips_before = env.invoke_git("rev-parse one");
    let mut refs_after = String::new();
    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -b two -m 'on two'").assert().success();
        refs_after = env.invoke_git("show-ref --heads");
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "gitbutler/workspace",
            "committing to an independent branch must enter workspace mode"
        );
    });

    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "one",
        "undo must leave workspace mode and restore the original checkout"
    );
    assert_eq!(
        env.invoke_git("rev-parse one"),
        tips_before,
        "undo restores the original tip"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join("new-file.txt")).unwrap(),
        b"content\n",
        "undo restores the file bytes"
    );

    env.but("redo").assert().success();
    // Compare exact refs rather than the unstable ASCII merge-graph layout.
    snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "gitbutler/workspace",
        "redo restores the workspace checkout"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join("new-file.txt")).unwrap(),
        b"content\n",
        "redo preserves the file bytes"
    );
    assert_eq!(
        env.invoke_git("show two:new-file.txt"),
        "content",
        "redo restores the committed content"
    );
    assert_eq!(
        env.invoke_git("status --porcelain"),
        "",
        "redo leaves the committed file clean"
    );
}

#[test]
fn can_undo_and_redo_single_branch_commit_switch_from_branch() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("commit -b one -m 'on one'").assert().success();
    env.file("new-file.txt", "content\n");

    let tips_before = env.invoke_git("rev-parse one");
    let mut log_after = String::new();
    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit --switch -b two -m 'on two'")
            .assert()
            .success();
        log_after = env.git_log();
    });

    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "one",
        "undo must restore the branch checked out before --switch"
    );
    assert_eq!(
        env.invoke_git("rev-parse one"),
        tips_before,
        "undo restores the original tip"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join("new-file.txt")).unwrap(),
        b"content\n",
        "undo restores the file bytes"
    );

    env.but("redo").assert().success();
    // Redo restores both branch tips and the switched checkout.
    snapbox::assert_data_eq!(env.git_log(), log_after);
    assert_eq!(
        std::fs::read(env.projects_root().join("new-file.txt")).unwrap(),
        b"content\n",
        "redo preserves the file bytes"
    );
    assert_eq!(
        env.invoke_git("show two:new-file.txt"),
        "content",
        "redo restores the committed content"
    );
    assert_eq!(
        env.invoke_git("status --porcelain"),
        "",
        "redo leaves the committed file clean"
    );
}

#[test]
fn can_undo_and_redo_single_branch_commit_switch_from_workspace() {
    for branch in ["three", "one"] {
        let env = Sandbox::open_with_default_settings("single-branch-mode");
        env.but("commit -b one -m 'on one'").assert().success();
        env.but("commit -b two -m 'on two'").assert().success();
        env.file("new-file.txt", "content\n");

        let tips_before = env.invoke_git("rev-parse one two gitbutler/workspace");
        let mut refs_after = String::new();
        // Cover switching to both a newly created and an existing applied branch.
        run_mutate_undo_roundtrip_test(&env, |env| {
            env.but(format!("commit --switch -b {branch} -m 'add file'"))
                .assert()
                .success();
            assert_eq!(
                env.invoke_git("symbolic-ref --short HEAD"),
                branch,
                "--switch must check out the commit's branch"
            );
            refs_after = env.invoke_git("show-ref --heads");
        });

        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "gitbutler/workspace",
            "undo must restore the workspace checkout"
        );
        assert_eq!(
            env.invoke_git("rev-parse one two gitbutler/workspace"),
            tips_before,
            "undo restores all original tips"
        );
        assert_eq!(
            std::fs::read(env.projects_root().join("new-file.txt")).unwrap(),
            b"content\n",
            "undo restores the file bytes"
        );

        env.but("redo").assert().success();
        // Redo restores every branch tip, including the pre-existing workspace.
        snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            branch,
            "redo restores the switched checkout"
        );
        assert_eq!(
            std::fs::read(env.projects_root().join("new-file.txt")).unwrap(),
            b"content\n",
            "redo preserves the file bytes"
        );
        assert_eq!(
            env.invoke_git("show HEAD:new-file.txt"),
            "content",
            "redo restores the committed content"
        );
        assert_eq!(
            env.invoke_git("status --porcelain"),
            "",
            "redo leaves the committed file clean"
        );
    }
}

#[test]
fn can_undo_single_branch_commit_switch_to_existing_lower_branch() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("commit -b bottom -m 'on bottom'")
        .assert()
        .success();
    env.but("commit -b middle --above bottom -m 'on middle'")
        .assert()
        .success();
    env.but("commit -b top --above middle -m 'on top'")
        .assert()
        .success();
    env.file("new-file.txt", "content\n");
    let tips_before = env.invoke_git("rev-parse bottom middle top");
    let middle_before = env.invoke_git("rev-parse middle");
    let top_before = env.invoke_git("rev-parse top");
    let mut log_after = String::new();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit --switch -b bottom -m 'update bottom'")
            .assert()
            .success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "bottom",
            "commit must switch to the lower branch"
        );
        assert_ne!(
            env.invoke_git("rev-parse middle"),
            middle_before,
            "committing below middle must rewrite it"
        );
        assert_ne!(
            env.invoke_git("rev-parse top"),
            top_before,
            "committing below top must rewrite it"
        );
        log_after = env.git_log();
    });

    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "top",
        "undo must restore the original checkout"
    );
    assert_eq!(
        env.invoke_git("rev-parse bottom middle top"),
        tips_before,
        "undo must restore all three original tips"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join("new-file.txt")).unwrap(),
        b"content\n",
        "undo restores the exact file bytes"
    );

    env.but("redo").assert().success();
    // Redo must also recover the descendant rewrites and the lower checkout.
    snapbox::assert_data_eq!(env.git_log(), log_after);
    assert_eq!(
        std::fs::read(env.projects_root().join("new-file.txt")).unwrap(),
        b"content\n",
        "redo preserves the exact file bytes"
    );
    assert_eq!(
        env.invoke_git("status --porcelain"),
        "",
        "redo restores the committed file"
    );
}

#[test]
fn can_undo_single_branch_commit_reentering_existing_workspace() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("commit -b one -m 'on one'").assert().success();
    env.but("commit -b two -m 'on two'").assert().success();
    env.but("switch one").assert().success();
    env.file("new-file.txt", "content\n");
    let tips_before = env.invoke_git("rev-parse one two gitbutler/workspace");
    let workspace_ref = but_core::WORKSPACE_REF_NAME.try_into().unwrap();
    let workspace_before = (*env.meta().workspace(workspace_ref).unwrap()).clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -b three -m 'on three'").assert().success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "gitbutler/workspace",
            "an independent branch must re-enter the existing workspace"
        );
    });

    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "one",
        "undo restores the original checkout"
    );
    assert_eq!(
        env.invoke_git("rev-parse one two gitbutler/workspace"),
        tips_before,
        "undo restores the pre-existing workspace and branch tips"
    );
    assert_eq!(
        *env.meta().workspace(workspace_ref).unwrap(),
        workspace_before,
        "undo restores the pre-existing workspace metadata"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join("new-file.txt")).unwrap(),
        b"content\n",
        "undo restores the exact file bytes"
    );
}

pub(super) fn commit_empty_with_message(env: &Sandbox, message: &str) -> String {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CommitJson {
        commit_id: String,
        #[expect(dead_code)]
        change_id: String,
    }

    let output = env
        .but("commit --empty -b A --json")
        .args(["-m", message])
        .assert()
        .success();
    let output = output.get_output();
    serde_json::from_slice::<CommitJson>(&output.stdout)
        .unwrap()
        .commit_id
}

#[test]
fn can_undo_but_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file'").assert().success();
    });
}

#[test]
fn can_undo_but_commit_on_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' -b A").assert().success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_create() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' -b").assert().success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_create_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' -b my-new-branch")
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_create_existing_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' -b A").assert().success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    env.file("new-file.txt", "content");
    env.file("other-new-file.txt", "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' new-file.txt")
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_empty() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit --empty --no-message").assert().success();
    });
}

#[test]
fn can_undo_but_commit_empty_with_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit --empty -m 'Plan empty slot'")
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_empty_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    env.file("new-file.txt", "content");

    env.but("branch new my-new-branch").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit --empty --no-message -b my-new-branch")
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_empty_dash_dash_before() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    env.file("new-file.txt", "content");

    commit_empty_with_message(&env, "one");
    commit_empty_with_message(&env, "two");
    let target = commit_empty_with_message(&env, "two");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("commit --empty --no-message --below {target}"))
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_empty_dash_dash_after() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    env.file("new-file.txt", "content");

    commit_empty_with_message(&env, "one");
    let target = commit_empty_with_message(&env, "two");
    commit_empty_with_message(&env, "two");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("commit --empty --no-message --above {target}"))
            .assert()
            .success();
    });
}

/// The oplog is shared, so a commit made in a linked worktree is what `but undo` targets from
/// every checkout, and it restores only the main checkout.
#[test]
fn undo_in_the_worktree_that_made_a_commit_names_the_commit_to_uncommit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("status").assert().success();
    undo_in_worktree_hints_uncommit(&env, "main");
}

#[test]
fn undo_in_the_single_branch_worktree_that_made_a_commit_names_the_commit_to_uncommit() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.invoke_git("checkout -q -b feature");
    env.but("status").assert().success();
    undo_in_worktree_hints_uncommit(&env, "feature");
}

#[track_caller]
fn undo_in_worktree_hints_uncommit(env: &Sandbox, start_point: &str) {
    let wt_dir = add_worktree_with_commit(env, "wt-feature", start_point);
    let tip_before = env.invoke_git("rev-parse wt-feature");
    std::fs::write(wt_dir.join("new.txt"), "content\n").unwrap();
    env.but("commit -m 'from wt' new.txt")
        .current_dir(&wt_dir)
        .assert()
        .success();
    let commit = env.invoke_git("rev-parse --short wt-feature");

    env.but("undo")
        .current_dir(&wt_dir)
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(format!(
            "Error: Cannot restore branch 'wt-feature' because it is checked out in this linked \
            worktree, and `but undo` only restores the main checkout\n\n\
            Hint: Run `but uncommit {commit}` to take the commit off 'wt-feature' and keep its \
            changes uncommitted\n"
        ));

    // From another checkout it may be another agent's commit, so nothing asks to reverse it.
    env.but("undo")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Cannot restore branch 'wt-feature' because it is checked out in worktrees: [..]

Hint: Reverse the operation with its inverse instead, such as `but move <commit> --branch <branch>`, `but reword <commit>` or `but uncommit <commit>`

"#]]);

    env.but(format!("uncommit {commit}"))
        .current_dir(&wt_dir)
        .assert()
        .success();
    assert_eq!(
        env.invoke_git("rev-parse wt-feature"),
        tip_before,
        "the hinted command takes only that commit off the branch"
    );
    assert!(
        wt_dir.join("new.txt").exists(),
        "the hinted command keeps the change in the worktree, uncommitted"
    );
}

/// A commit that makes the worktree's branch has no earlier tip to compare with.
#[test]
fn undo_in_a_detached_worktree_names_the_commit_that_made_its_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("status").assert().success();
    let wt_dir = add_dirty_detached_worktree(&env, "wt-detached", "B");
    env.but("commit -b fresh -m 'from wt'")
        .current_dir(&wt_dir)
        .assert()
        .success();
    let commit = env.invoke_git("rev-parse --short fresh");

    env.but("undo")
        .current_dir(&wt_dir)
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(format!(
            "Error: Cannot restore branch 'fresh' because it is checked out in this linked \
            worktree, and `but undo` only restores the main checkout\n\n\
            Hint: Run `but uncommit {commit}` to take the commit off 'fresh' and keep its \
            changes uncommitted\n"
        ));

    env.but(format!("uncommit {commit}"))
        .current_dir(&wt_dir)
        .assert()
        .success();
    assert_eq!(
        env.invoke_git("rev-parse fresh"),
        env.invoke_git("rev-parse B"),
        "the hinted command takes the commit off the branch"
    );
}

/// `but uncommit` only reverses the last operation if that made the commit at the branch's tip.
#[test]
fn undo_in_a_worktree_names_no_commit_unless_the_last_operation_made_the_tip() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("status").assert().success();
    let wt_dir = add_worktree_with_commit(&env, "wt-feature", "main");

    // A move adds one commit to the branch as well, but it's reversed with another move.
    let commit = env.invoke_git("rev-parse A");
    env.but(format!("move {commit} --branch wt-feature"))
        .current_dir(&wt_dir)
        .assert()
        .success();
    undo_is_refused_without_naming_a_commit(&env, &wt_dir);

    // The new commit went below the tip, which the operation only rewrote.
    let tip = env.invoke_git("rev-parse wt-feature");
    env.but(format!("commit --empty --no-message --below {tip}"))
        .current_dir(&wt_dir)
        .assert()
        .success();
    undo_is_refused_without_naming_a_commit(&env, &wt_dir);
}

/// Single-branch snapshots record every branch, so a commit made elsewhere holds the worktree's
/// tip, and the one commit the worktree's branch gained since isn't the one that operation made.
#[test]
fn undo_in_a_single_branch_worktree_names_no_commit_the_last_operation_made_elsewhere() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.invoke_git("checkout -q -b feature");
    env.but("status").assert().success();
    let wt_dir = add_worktree_with_commit(&env, "wt-feature", "feature");
    env.file("one.txt", "content\n");
    env.but("commit -m one one.txt").assert().success();
    but_testsupport::invoke_bash_at_dir(
        "echo later >later.txt && git add later.txt && git commit -q -m 'later'",
        &wt_dir,
    );

    undo_is_refused_without_naming_a_commit(&env, &wt_dir);
}

#[track_caller]
fn undo_is_refused_without_naming_a_commit(env: &Sandbox, wt_dir: &std::path::Path) {
    env.but("undo")
        .current_dir(wt_dir)
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Cannot restore branch 'wt-feature' because it is checked out in this linked worktree, and `but undo` only restores the main checkout

Hint: Reverse the operation with its inverse instead, such as `but move <commit> --branch <branch>`, `but reword <commit>` or `but uncommit <commit>`

"#]]);
}

#[test]
fn redo_in_a_worktree_names_no_commit() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.invoke_git("checkout -q -b feature");
    env.but("status").assert().success();
    let wt_dir = add_worktree_with_commit(&env, "wt-feature", "feature");
    // Single-branch snapshots record every branch, so these two also hold the worktree's tip.
    for file in ["one.txt", "two.txt"] {
        env.file(file, "content\n");
        env.but(format!("commit -m {file} {file}"))
            .assert()
            .success();
    }
    env.but("undo").assert().success();
    but_testsupport::invoke_bash_at_dir(
        "echo later >later.txt && git add later.txt && git commit -q -m 'later'",
        &wt_dir,
    );

    env.but("redo")
        .current_dir(&wt_dir)
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Cannot restore branch 'wt-feature' because it is checked out in this linked worktree, and `but redo` only restores the main checkout

Hint: Reverse the operation with its inverse instead, such as `but move <commit> --branch <branch>`, `but reword <commit>` or `but uncommit <commit>`

"#]]);
}
