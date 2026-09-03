use crate::{command::util::status_json_with_files as status_json, utils::Sandbox};

#[test]
fn split_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "contents of one");
    env.file("two", "contents of two");

    env.but("commit -m original").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 original
┊│     1:k A one
┊│     1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("split 1:k")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from 1 to new commit 1 above commit 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 (no commit message)
┊│     1#0:k A one
┊●   1#1 original
┊│     1#1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn split_committed_hunk_creates_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    let content = "one
two
three
four
five
six
seven
";

    env.file("file", content);
    env.but("commit -m 'Add file'").assert().success();

    env.file("file", format!("beginning\n{content}end"));
    env.but("commit -m 'Update file'").assert().success();

    env.but("diff 1#0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────╮
 1#0:q:3 file │
──────────────╯

@@ -1,3 +1,4 @@
───────────────
  ┊ 1 │ +beginning
1 ┊ 2 │  one
2 ┊ 3 │  two
3 ┊ 4 │  three

──────────────╮
 1#0:q:8 file │
──────────────╯

@@ -5,3 +6,4 @@
───────────────
5 ┊  6 │  five
6 ┊  7 │  six
7 ┊  8 │  seven
  ┊  9 │ +end

"#]]);

    env.but("split 1#0:q:3")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from 1 to new commit 1 above commit 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 (no commit message)
┊│     1#0:q M file
┊●   1#1 Update file
┊│     1#1:q M file
┊●   1#2 Add file
┊│     1#2:q A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // New commit contains only selected hunk.
    env.but("diff 1#0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────╮
 1#0:q:3 file │
──────────────╯

@@ -1,3 +1,4 @@
───────────────
  ┊ 1 │ +beginning
1 ┊ 2 │  one
2 ┊ 3 │  two
3 ┊ 4 │  three

"#]]);

    // Source commit retains only hunk that was not split out.
    env.but("diff 1#1")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────╮
 1#1:q:8 file │
──────────────╯

@@ -5,3 +5,4 @@
───────────────
5 ┊ 5 │  five
6 ┊ 6 │  six
7 ┊ 7 │  seven
  ┊ 8 │ +end

"#]]);
}

#[test]
fn split_committed_hunks_in_different_ways_yields_same_result() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    let content = "one
two
three
four
five
six
seven
";

    env.file("file", content);
    env.but("commit -m 'Add file'").assert().success();

    env.file("file", format!("beginning\n{content}end"));
    env.but("commit -m 'Update file'").assert().success();

    let commit_trees = |env: &Sandbox| {
        status_json(env)["stacks"][0]["branches"][0]["commits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|commit| {
                let commit_id = commit["commitId"].as_str().unwrap();
                env.invoke_git(&format!("rev-parse {commit_id}^{{tree}}"))
            })
            .collect::<Vec<_>>()
    };

    // Entire file is baseline.
    env.but("split 1#0:q").assert().success();
    let trees_entire_file = commit_trees(&env);

    // Hunk order.
    env.but("undo").assert().success();
    env.but("split 1#0:q:3 1#0:q:8").assert().success();
    assert_eq!(
        commit_trees(&env),
        trees_entire_file,
        "outcome should be the same regardless of hunk order"
    );

    // Reverse hunk order.
    env.but("undo").assert().success();
    env.but("split 1#0:q:8 1#0:q:3").assert().success();
    assert_eq!(
        commit_trees(&env),
        trees_entire_file,
        "outcome should be the same regardless of hunk order"
    );

    // Repeated hunks.
    env.but("undo").assert().success();
    env.but("split 1#0:q:8 1#0:q:3 1#0:q:8").assert().success();
    assert_eq!(
        commit_trees(&env),
        trees_entire_file,
        "repeated hunks are deduplicated"
    );

    // Hunk then file.
    env.but("undo").assert().success();
    env.but("split 1#0:q:8 1#0:q").assert().success();
    assert_eq!(
        commit_trees(&env),
        trees_entire_file,
        "file overlapping with hunks is deduplicated",
    );

    // File then hunk.
    env.but("undo").assert().success();
    env.but("split 1#0:q 1#0:q:8").assert().success();
    assert_eq!(
        commit_trees(&env),
        trees_entire_file,
        "hunks overlapping with a file are deduplicated",
    );
}

#[test]
fn cannot_split_sources_from_multiple_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "contents of one");
    env.file("two", "contents of two");

    env.but("commit -m original one").assert().success();
    env.but("commit -m original two").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 original
┊│     1#0:t A two
┊●   1#1 original
┊│     1#1:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("split 1#0:t 1#1:k")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Can only split changes from one commit. Got 1 and 1

"#]]);
}
