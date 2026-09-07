use crate::{
    command::{
        undo::run_mutate_undo_roundtrip_test,
        util::{branch_commit_cli_ids, commit_two_files_as_two_hunks_each, status_json},
    },
    utils::Sandbox,
};

#[test]
fn can_undo_and_redo_single_branch_move_commit_to_new_branch() {
    let env = Sandbox::open_with_default_settings("single-branch-three-dependent-branches");
    let tips_before = env.invoke_git("rev-parse A B C");
    let mut refs_after = String::new();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move myy -b moved").assert().success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "gitbutler/workspace",
            "moving to an independent branch must check out the workspace"
        );
        refs_after = env.invoke_git("show-ref --heads");
    });

    assert_eq!(
        env.invoke_git("rev-parse A B C"),
        tips_before,
        "undo must restore all original branch tips"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "C",
        "undo must restore the original checkout"
    );

    env.but("redo").assert().success();
    // Exact refs cover rewritten descendants and the newly created workspace.
    snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "gitbutler/workspace",
        "redo must restore the workspace checkout"
    );
}

#[test]
fn can_undo_and_redo_single_branch_move_commit_to_new_branch_and_switch() {
    let env = Sandbox::open_with_default_settings("single-branch-three-dependent-branches");
    let tips_before = env.invoke_git("rev-parse A B C");
    let mut refs_after = String::new();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move myy -b moved --switch").assert().success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "moved",
            "--switch must check out the new destination branch"
        );
        refs_after = env.invoke_git("show-ref --heads");
    });

    assert_eq!(
        env.invoke_git("rev-parse A B C"),
        tips_before,
        "undo must restore all original branch tips"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "C",
        "undo must restore the original checkout"
    );

    env.but("redo").assert().success();
    // Exact refs cover rewritten descendants and the new destination branch.
    snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "moved",
        "redo must restore the destination checkout"
    );
}

#[test]
fn can_undo_and_redo_single_branch_move_commit_above_checkout() {
    let env = Sandbox::open_with_default_settings("single-branch-three-dependent-branches");
    let tips_before = env.invoke_git("rev-parse A B C");
    let mut refs_after = String::new();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move myy --above C -b moved").assert().success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "moved",
            "moving above the checkout must implicitly check out the new branch"
        );
        refs_after = env.invoke_git("show-ref --heads");
    });

    assert_eq!(
        env.invoke_git("rev-parse A B C"),
        tips_before,
        "undo must restore all original branch tips"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "C",
        "undo must restore the original checkout"
    );

    env.but("redo").assert().success();
    // Exact refs cover rewritten descendants and the new destination branch.
    snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "moved",
        "redo must restore the implicit checkout of the new branch"
    );
}

#[test]
fn can_undo_and_redo_single_branch_move_commit_above_ancestor() {
    let env = Sandbox::open_with_default_settings("single-branch-three-dependent-branches");
    let tips_before = env.invoke_git("rev-parse A B C");
    let mut refs_after = String::new();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move vuw --above A -b moved").assert().success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "C",
            "moving above an ancestor must preserve the checkout"
        );
        refs_after = env.invoke_git("show-ref --heads");
    });

    assert_eq!(
        env.invoke_git("rev-parse A B C"),
        tips_before,
        "undo must restore all original branch tips"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "C",
        "undo must preserve the original checkout"
    );

    env.but("redo").assert().success();
    // Exact refs cover rewritten descendants and the new destination branch.
    snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "C",
        "redo must preserve the original checkout"
    );
}

#[test]
fn can_undo_and_redo_single_branch_move_commit_below_ancestor() {
    let env = Sandbox::open_with_default_settings("single-branch-three-dependent-branches");
    let tips_before = env.invoke_git("rev-parse A B C");
    let mut refs_after = String::new();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move vuw --below B -b moved").assert().success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "C",
            "moving below an ancestor must preserve the checkout"
        );
        refs_after = env.invoke_git("show-ref --heads");
    });

    assert_eq!(
        env.invoke_git("rev-parse A B C"),
        tips_before,
        "undo must restore all original branch tips"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "C",
        "undo must preserve the original checkout"
    );

    env.but("redo").assert().success();
    // Exact refs cover rewritten descendants and the new destination branch.
    snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "C",
        "redo must preserve the original checkout"
    );
}

#[test]
fn can_undo_and_redo_single_branch_move_commit_to_existing_branch() {
    let env = Sandbox::open_with_default_settings("single-branch-three-dependent-branches");
    let tips_before = env.invoke_git("rev-parse A B C");
    let mut refs_after = String::new();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move myy -b A").assert().success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "C",
            "moving to an existing ancestor branch must preserve the checkout"
        );
        refs_after = env.invoke_git("show-ref --heads");
    });

    assert_eq!(
        env.invoke_git("rev-parse A B C"),
        tips_before,
        "undo must restore all original branch tips"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "C",
        "undo must preserve the original checkout"
    );

    env.but("redo").assert().success();
    // Exact refs cover the destination and rewritten descendants.
    snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "C",
        "redo must preserve the original checkout"
    );
}

#[test]
fn can_undo_and_redo_single_branch_move_commit_to_existing_branch_and_switch() {
    let env = Sandbox::open_with_default_settings("single-branch-three-dependent-branches");
    let tips_before = env.invoke_git("rev-parse A B C");
    let mut refs_after = String::new();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move myy -b A --switch").assert().success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "A",
            "--switch must check out the existing destination branch"
        );
        refs_after = env.invoke_git("show-ref --heads");
    });

    assert_eq!(
        env.invoke_git("rev-parse A B C"),
        tips_before,
        "undo must restore all original branch tips"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "C",
        "undo must restore the original checkout"
    );

    env.but("redo").assert().success();
    // Exact refs cover the destination and rewritten descendants.
    snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "A",
        "redo must restore the destination checkout"
    );
}

#[test]
fn can_undo_and_redo_single_branch_move_committed_file() {
    for (args, checkout) in [
        ("--unstack -b moved", "gitbutler/workspace"),
        ("-b moved --switch", "moved"),
        ("--above C -b moved", "moved"),
        ("--above A -b moved", "C"),
        ("--below B -b moved", "C"),
    ] {
        let env = Sandbox::open_with_default_settings("single-branch-three-dependent-branches");
        env.file("remaining.txt", "leave this uncommitted\n");
        let tips_before = env.invoke_git("rev-parse A B C");
        let content_before = std::fs::read(env.projects_root().join("B")).unwrap();
        let mut refs_after = String::new();

        run_mutate_undo_roundtrip_test(&env, |env| {
            env.but(format!("move myy:B {args} -m extracted"))
                .assert()
                .success();
            assert_eq!(
                env.invoke_git("symbolic-ref --short HEAD"),
                checkout,
                "extracting a file must select the expected checkout for {args}"
            );
            assert_eq!(
                env.invoke_git("show moved:B"),
                String::from_utf8_lossy(&content_before).trim(),
                "the selected file must be committed on the destination"
            );
            refs_after = env.invoke_git("show-ref --heads");
        });

        assert_eq!(
            env.invoke_git("rev-parse A B C"),
            tips_before,
            "undo must restore both the source commit and its descendants"
        );
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "C",
            "undo must restore the original checkout"
        );
        assert_eq!(
            std::fs::read(env.projects_root().join("B")).unwrap(),
            content_before,
            "undo must restore the selected file's exact bytes"
        );
        assert_eq!(
            std::fs::read(env.projects_root().join("remaining.txt")).unwrap(),
            b"leave this uncommitted\n",
            "undo must preserve unrelated uncommitted content"
        );

        env.but("redo").assert().success();
        // Redo restores the extracted commit as well as all rewritten refs.
        snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            checkout,
            "redo must restore the destination checkout"
        );
        assert_eq!(
            std::fs::read(env.projects_root().join("remaining.txt")).unwrap(),
            b"leave this uncommitted\n",
            "redo must preserve unrelated uncommitted content"
        );
    }
}

#[test]
fn can_undo_and_redo_single_branch_move_branch() {
    for (args, checkout) in [
        ("B --unstack", "gitbutler/workspace"),
        ("B --unstack --switch", "B"),
        ("C --unstack --switch", "C"),
        ("C -b A --switch", "C"),
        ("C --above A", "B"),
    ] {
        let env = Sandbox::open_with_default_settings("single-branch-three-dependent-branches");
        let tips_before = env.invoke_git("rev-parse A B C");
        let mut refs_after = String::new();

        run_mutate_undo_roundtrip_test(&env, |env| {
            env.but(format!("move {args}")).assert().success();
            assert_eq!(
                env.invoke_git("symbolic-ref --short HEAD"),
                checkout,
                "moving a branch must select the expected checkout for {args}"
            );
            refs_after = env.invoke_git("show-ref --heads");
        });

        assert_eq!(
            env.invoke_git("rev-parse A B C"),
            tips_before,
            "undo must restore all original branch tips for {args}"
        );
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "C",
            "undo must restore the original checkout for {args}"
        );

        env.but("redo").assert().success();
        // Compare refs instead of the unstable workspace merge-graph layout.
        snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            checkout,
            "redo must restore the moved branch's checkout for {args}"
        );
    }
}

#[test]
fn can_undo_and_redo_single_branch_move_above_checkout_preserving_hidden_branch() {
    let env = Sandbox::open_with_default_settings("single-branch-three-dependent-branches");
    env.invoke_git("checkout B");
    let tips_before = env.invoke_git("rev-parse A B C");
    let hidden_tip = env.invoke_git("rev-parse C");
    let mut tips_after = String::new();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move A --above B").assert().success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "A",
            "moving A above the checkout must implicitly check out A"
        );
        assert_eq!(
            env.invoke_git("rev-parse C"),
            hidden_tip,
            "moving A must not rewrite the hidden branch C"
        );
        assert_eq!(
            env.invoke_git("log --format=%s origin/main..HEAD"),
            "add A\nadd B",
            "only the visible branches must be reordered"
        );
        tips_after = env.invoke_git("rev-parse A B C");
    });

    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "B",
        "undo must restore the original checkout without revealing C"
    );
    assert_eq!(
        env.invoke_git("rev-parse A B C"),
        tips_before,
        "undo must restore A and B while preserving hidden C"
    );

    env.but("redo").assert().success();
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "A",
        "redo must restore the implicit checkout of A"
    );
    assert_eq!(
        env.invoke_git("rev-parse A B C"),
        tips_after,
        "redo must restore the reordered tips while preserving hidden C"
    );
}

#[test]
fn can_undo_single_branch_move_hunk_above_checkout() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    let content = "one\ntwo\nthree\nfour\nfive\nsix\nseven\n";
    env.file("file", content);
    env.but("commit -b source -m 'Add file'").assert().success();
    env.file("file", format!("beginning\n{content}end"));
    env.but("commit -m 'Update file'").assert().success();
    let source_before = env.invoke_git("rev-parse source");
    env.file("file", format!("beginning\n{content}end\nuncommitted\n"));
    let diff_before = env.invoke_git("diff HEAD -- file");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move wrz:file:3 --above source -b moved -m extracted")
            .assert()
            .success();
        assert_eq!(
            env.invoke_git("show source:file"),
            format!("{content}end"),
            "the source must retain the unselected hunk"
        );
        assert_eq!(
            env.invoke_git("show moved:file"),
            format!("beginning\n{content}end"),
            "the destination must contain the extracted hunk"
        );
    });

    assert_eq!(
        env.invoke_git("rev-parse source"),
        source_before,
        "undo must restore the original commit containing both hunks"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "source",
        "undo must return from the newly created top branch"
    );
    assert_eq!(
        env.invoke_git("diff HEAD -- file"),
        diff_before,
        "undo must preserve the exact uncommitted patch"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join("file")).unwrap(),
        format!("beginning\n{content}end\nuncommitted\n").as_bytes(),
        "undo must preserve the file's exact bytes"
    );
}

#[test]
fn can_undo_and_redo_single_branch_move_switch_from_workspace() {
    for branch in ["moved", "A"] {
        let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
        env.setup_metadata(&["A", "B"]);
        let tips_before = env.invoke_git("rev-parse A B gitbutler/workspace");
        let workspace_ref = but_core::WORKSPACE_REF_NAME.try_into().unwrap();
        let workspace_before = env
            .db()
            .meta()
            .unwrap()
            .workspace(workspace_ref)
            .cloned()
            .unwrap();
        let mut refs_after = String::new();

        run_mutate_undo_roundtrip_test(&env, |env| {
            env.but(format!("move d3e2ba3 -b {branch} --switch"))
                .assert()
                .success();
            assert_eq!(
                env.invoke_git("symbolic-ref --short HEAD"),
                branch,
                "--switch must check out the destination branch"
            );
            refs_after = env.invoke_git("show-ref --heads");
        });

        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "gitbutler/workspace",
            "undo must restore the workspace checkout"
        );
        assert_eq!(
            env.invoke_git("rev-parse A B gitbutler/workspace"),
            tips_before,
            "undo must restore all original branch and workspace tips"
        );
        assert_eq!(
            *env.db().meta().unwrap().workspace(workspace_ref).unwrap(),
            workspace_before,
            "undo must restore the original workspace metadata"
        );

        env.but("redo").assert().success();
        // Redo restores the existing workspace ref as well as the destination.
        snapbox::assert_data_eq!(env.invoke_git("show-ref --heads"), refs_after);
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            branch,
            "redo must restore the switched checkout"
        );
    }
}

#[test]
fn can_undo_single_branch_move_reentering_existing_workspace() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.file("first", "first\n");
    env.but("commit -b one -m first").assert().success();
    env.but("commit -b two -m second").assert().success();
    env.but("switch one").assert().success();
    let commit = env.invoke_git("rev-parse one");
    let tips_before = env.invoke_git("rev-parse one two gitbutler/workspace");
    let workspace_ref = but_core::WORKSPACE_REF_NAME.try_into().unwrap();
    let workspace_before = env
        .db()
        .meta()
        .unwrap()
        .workspace(workspace_ref)
        .cloned()
        .unwrap();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("move {commit} -b moved"))
            .assert()
            .success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "gitbutler/workspace",
            "moving to an independent branch must re-enter the workspace"
        );
    });

    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "one",
        "undo must restore the original checkout"
    );
    assert_eq!(
        env.invoke_git("rev-parse one two gitbutler/workspace"),
        tips_before,
        "undo must restore the pre-existing workspace and branch tips"
    );
    assert_eq!(
        *env.db().meta().unwrap().workspace(workspace_ref).unwrap(),
        workspace_before,
        "undo must restore the pre-existing workspace metadata"
    );
}

#[test]
fn can_undo_single_branch_move_all_commits() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("branch new source").assert().success();
    env.file("first", "first\n");
    env.but("commit -m first").assert().success();
    env.file("second", "second\n");
    env.but("commit -m second").assert().success();
    let source_before = env.invoke_git("rev-parse source");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move ukz zpr --above source -b moved")
            .assert()
            .success();
        assert_eq!(
            env.invoke_git("rev-parse source"),
            env.invoke_git("rev-parse origin/main"),
            "moving all commits must leave the source branch empty"
        );
    });

    assert_eq!(
        env.invoke_git("rev-parse source"),
        source_before,
        "undo must restore both commits to the emptied source branch"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "source",
        "undo must check out the source branch again"
    );
}

#[test]
fn can_undo_single_branch_move_from_target() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.file("first", "first\n");
    env.invoke_git("add first");
    env.invoke_git("commit -m first");
    let main_before = env.invoke_git("rev-parse main");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move nyl -b moved").assert().success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "moved",
            "moving off the target must check out the new branch"
        );
    });

    assert_eq!(
        env.invoke_git("rev-parse main"),
        main_before,
        "undo must restore the commit on the target branch"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "main",
        "undo must restore the original target checkout"
    );
}

#[test]
fn can_undo_single_branch_unstack_empty_branch_and_switch() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("branch new bottom").assert().success();
    env.file("first", "first\n");
    env.but("commit -m first").assert().success();
    env.but("branch new empty --above bottom")
        .assert()
        .success();
    env.but("branch new top --above empty").assert().success();
    env.file("second", "second\n");
    env.but("commit -m second").assert().success();
    let tips_before = env.invoke_git("rev-parse bottom empty top");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("move empty --unstack --switch").assert().success();
        assert_eq!(
            env.invoke_git("symbolic-ref --short HEAD"),
            "empty",
            "unstacking must switch to the empty branch"
        );
    });

    assert_eq!(
        env.invoke_git("rev-parse bottom empty top"),
        tips_before,
        "undo must restore the empty branch's position between its neighbors"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "top",
        "undo must restore the original checkout"
    );
}

// move: commit to branch
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
    let source_cli_id = branch_commit_cli_ids(&status_json(&env), "A")[0].clone();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("move {source_cli_id} --branch B"))
            .assert()
            .success();
    });
}
