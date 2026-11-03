//! Commands that take a change accept a path the way Git does, as well as from the repository root.

use crate::utils::Sandbox;

use super::util::add_dirty_worktree;

/// `src/sub/a.txt` and `b.txt` are dirty.
fn sandbox() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("src/sub/a.txt", "a\n");
    env.file("b.txt", "b\n");
    env
}

fn dirty(env: &Sandbox) -> String {
    env.invoke_git("status --porcelain -uall")
}

#[test]
fn commit_takes_every_spelling_of_a_path() {
    for (dir, spelling) in [
        (".", "src/sub/a.txt"),
        (".", "src/sub/"),
        (".", "./src/sub/a.txt"),
        (".", "src"),
        (".", "src/sub"),
        (".", "./src/"),
        (".", "{root}/src/sub/a.txt"),
        ("src/sub", "{root}/src/sub/a.txt"),
        ("src/sub", "a.txt"),
        ("src/sub", "."),
        ("src/sub", "../sub/a.txt"),
        ("src/sub", "src/sub/a.txt"),
        ("src", "sub/"),
    ] {
        let env = sandbox();
        let spelling = spelling.replace("{root}", &env.projects_root().display().to_string());
        env.but(format!("commit -m x {spelling}"))
            .current_dir(env.projects_root().join(dir))
            .assert()
            .success();
        assert_eq!(
            dirty(&env),
            "?? b.txt",
            "'{spelling}' from '{dir}' names only src/sub/a.txt"
        );
    }
}

#[test]
fn a_file_in_a_deleted_directory_can_still_be_named() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("gone/x.txt", "x\n");
    env.file("b.txt", "b\n");
    env.but("commit -m x gone/x.txt").assert().success();
    std::fs::remove_dir_all(env.projects_root().join("gone")).unwrap();

    env.but("commit -m y ./gone/x.txt").assert().success();
    assert_eq!(dirty(&env), "?? b.txt");
}

#[test]
fn dash_c_is_the_directory_paths_are_relative_to() {
    let env = sandbox();
    env.but("-C src/sub commit -m x a.txt").assert().success();
    assert_eq!(dirty(&env), "?? b.txt");
}

#[test]
fn dot_at_the_root_is_the_whole_checkout() {
    let env = sandbox();
    env.but("commit -m x .").assert().success();
    assert_eq!(dirty(&env), "");
}

#[test]
fn amend_discard_and_diff_take_the_same_spellings() {
    for (dir, spelling) in [(".", "./src/sub/a.txt"), ("src/sub", "a.txt")] {
        let env = sandbox();
        env.but(format!("amend -t A {spelling}"))
            .current_dir(env.projects_root().join(dir))
            .assert()
            .success();
        assert_eq!(dirty(&env), "?? b.txt", "amended '{spelling}' from '{dir}'");

        let env = sandbox();
        env.but(format!("diff {spelling}"))
            .current_dir(env.projects_root().join(dir))
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
...
[..]+a

"#]]);

        env.but(format!("discard {spelling}"))
            .current_dir(env.projects_root().join(dir))
            .assert()
            .success();
        assert_eq!(
            dirty(&env),
            "?? b.txt",
            "discarded '{spelling}' from '{dir}'"
        );
    }
}

#[test]
#[cfg(unix)]
fn symlinks_spell_the_root_without_changing_what_a_path_names() {
    let env = sandbox();
    let link = env.app_data_dir().join("link-to-repo");
    std::os::unix::fs::symlink(env.projects_root(), &link).unwrap();
    env.but(format!("commit -m x {}/src/sub/a.txt", link.display()))
        .assert()
        .success();
    assert_eq!(dirty(&env), "?? b.txt");

    let env = sandbox();
    std::os::unix::fs::symlink(".", env.projects_root().join("self")).unwrap();
    env.but("commit -m x ./self").assert().success();
    assert_eq!(
        dirty(&env),
        "?? b.txt\n?? src/sub/a.txt",
        "a symlink to the root names the symlink, not all of the checkout"
    );
}

#[test]
fn a_word_that_resolves_today_keeps_its_meaning() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    // `tpm` is the ID of the branch's only commit.
    env.file("tpm/new.txt", "new\n");

    // Without a trailing slash, the commit ID still wins over a directory of the same name.
    env.but("diff tpm")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
...
[..]+A

"#]]);
    // Its other spellings can only be the directory.
    for spelling in ["tpm/", "./tpm"] {
        env.but(format!("diff {spelling}"))
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
...
[..]+new

"#]]);
    }
}

#[test]
fn a_displayed_branch_id_does_not_become_a_directory() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    // `g0` is the ID `but status` shows for branch `A`.
    env.file("g0/new.txt", "new\n");

    env.but("commit -m x g0")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find uncommitted change: 'g0'

Hint: 'g0' is a branch. To commit onto it, run `but commit -b g0 -m "message" [<change>...]`

"#]]);
    assert_eq!(dirty(&env), "?? g0/new.txt");
}

#[test]
fn directories_do_not_stop_a_new_branch_of_the_same_name() {
    let env = sandbox();
    env.file("docs/x.txt", "x\n");
    env.file("src/docs/y.txt", "y\n");
    env.but("commit -b docs -m x ../b.txt")
        .current_dir(env.projects_root().join("src"))
        .assert()
        .success();
}

#[test]
fn a_word_naming_different_changes_from_the_root_and_here_is_refused() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("a.txt", "root\n");
    env.file("src/a.txt", "here\n");
    env.file("sub/x.txt", "root\n");
    env.file("src/sub/y.txt", "here\n");
    let src = env.projects_root().join("src");

    env.but("commit -m x a.txt")
        .current_dir(&src)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Ambiguous uncommitted change 'a.txt', matches multiple items

Hint: Use a longer ID to disambiguate. A path starting with `./` or `../` is read only from the current directory.

"#]]);
    env.but("commit -m x sub/")
        .current_dir(&src)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Ambiguous uncommitted change 'sub/', matches multiple items

Hint: Use a longer ID to disambiguate. A path starting with `./` or `../` is read only from the current directory.

"#]]);
    env.but("discard a.txt")
        .current_dir(&src)
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Ambiguous source 'a.txt', matches multiple items

Hint: Use a longer ID to disambiguate. A path starting with `./` or `../` is read only from the current directory.

"#]]);
    assert_eq!(
        dirty(&env),
        "?? a.txt\n?? src/a.txt\n?? src/sub/y.txt\n?? sub/x.txt",
        "a refused word changes nothing"
    );

    env.but("discard ./a.txt")
        .current_dir(&src)
        .assert()
        .success();
    assert_eq!(
        dirty(&env),
        "?? a.txt\n?? src/sub/y.txt\n?? sub/x.txt",
        "./ only reads from the current directory"
    );
}

#[test]
fn paths_in_a_linked_worktree_are_relative_to_it() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let wt_dir = add_dirty_worktree(&env, "wt-feature", "A");
    std::fs::create_dir_all(wt_dir.join("sub")).unwrap();
    std::fs::write(wt_dir.join("sub/a.txt"), "dirty in the worktree\n").unwrap();
    env.file("sub/a.txt", "dirty in main\n");

    env.but("discard a.txt")
        .current_dir(wt_dir.join("sub"))
        .assert()
        .success();
    assert!(
        !wt_dir.join("sub/a.txt").exists(),
        "the worktree's own file is discarded"
    );
    assert!(
        wt_dir.join("note.txt").exists(),
        "its other changes stay put"
    );
    assert!(
        env.projects_root().join("sub/a.txt").exists(),
        "the main checkout keeps the same path"
    );
}

#[test]
fn a_path_naming_no_change_is_not_found() {
    let env = sandbox();
    env.file("../outside.txt", "not in the repository\n");

    for spelling in ["./clean.txt", "../outside.txt", "./src/sub/missing"] {
        env.but(format!("commit -m x {spelling}"))
            .assert()
            .failure()
            .stderr_eq(format!(
                "Error: Could not find uncommitted change: '{spelling}'\n\nHint: Run `but diff` for the current change IDs; a hunk ID is `<file>:<hunk>`.\n"
            ));
    }
}

#[test]
fn an_empty_word_names_nothing() {
    let env = sandbox();
    env.but("discard ''")
        .current_dir(env.projects_root().join("src"))
        .assert()
        .failure();
    assert_eq!(dirty(&env), "?? b.txt\n?? src/sub/a.txt");
}

#[test]
fn a_conflicted_file_points_at_resolve() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    // Leave behind what a conflicting workspace update produces for an uncommitted file.
    env.file(
        "src/c.txt",
        "<<<<<<< ours\nours\n=======\ntheirs\n>>>>>>> theirs\n",
    );
    env.invoke_bash(
        r#"base=$(echo base | git hash-object -w --stdin) &&
ours=$(echo ours | git hash-object -w --stdin) &&
theirs=$(echo theirs | git hash-object -w --stdin) &&
printf '100644 %s 1\tsrc/c.txt\n100644 %s 2\tsrc/c.txt\n100644 %s 3\tsrc/c.txt\n' "$base" "$ours" "$theirs" | git update-index --index-info"#,
    );
    let hint = "Hint: 'src/c.txt' has unresolved conflicts: edit it to the wanted contents (or delete it), then run `but resolve src/c.txt` to mark it resolved.\n";

    for (dir, spelling) in [(".", "src/c.txt"), (".", "./src/c.txt"), ("src", "c.txt")] {
        let root = env.projects_root().join(dir);
        env.but(format!("commit -m x {spelling}"))
            .current_dir(&root)
            .assert()
            .failure()
            .stderr_eq(format!(
                "Error: Could not find uncommitted change: '{spelling}'\n\n{hint}"
            ));
        env.but(format!("amend -t A {spelling}"))
            .current_dir(&root)
            .assert()
            .failure()
            .stderr_eq(format!(
                "Error: Could not find uncommitted change: '{spelling}'\n\n{hint}"
            ));
        env.but(format!("discard {spelling}"))
            .current_dir(&root)
            .assert()
            .failure()
            .stderr_eq(format!(
                "Error: Could not find source: '{spelling}'\n\n{hint}"
            ));
        env.but(format!("diff {spelling}"))
            .current_dir(&root)
            .assert()
            .failure()
            .stderr_eq(format!(
                "Error: Could not find target: '{spelling}'\n\n{hint}"
            ));
    }
}
