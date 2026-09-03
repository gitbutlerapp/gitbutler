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
┊●   lsw original
┊│     lsw:k A one
┊│     lsw:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("split lsw:k")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from lsw to new commit qkw above commit lsw

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   qkw (no commit message)
┊│     qkw:k A one
┊●   lsw original
┊│     lsw:t A two
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

    env.but("diff szk")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────╮
 s:q:3 file │
────────────╯

@@ -1,3 +1,4 @@
───────────────
  ┊ 1 │ +beginning
1 ┊ 2 │  one
2 ┊ 3 │  two
3 ┊ 4 │  three

────────────╮
 s:q:8 file │
────────────╯

@@ -5,3 +6,4 @@
───────────────
5 ┊  6 │  five
6 ┊  7 │  six
7 ┊  8 │  seven
  ┊  9 │ +end

"#]]);

    env.but("split szk:q:3")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from szk to new commit qkw above commit szk

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   qkw (no commit message)
┊│     qkw:q M file
┊●   szk Update file
┊│     szk:q M file
┊●   knw Add file
┊│     knw:q A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // New commit contains only selected hunk.
    env.but("diff qkw")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────╮
 q:q:3 file │
────────────╯

@@ -1,3 +1,4 @@
───────────────
  ┊ 1 │ +beginning
1 ┊ 2 │  one
2 ┊ 3 │  two
3 ┊ 4 │  three

"#]]);

    // Source commit retains only hunk that was not split out.
    env.but("diff szk")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────╮
 s:q:8 file │
────────────╯

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
    env.but("split szk:q").assert().success();
    let trees_entire_file = commit_trees(&env);

    // Hunk order.
    env.but("undo").assert().success();
    env.but("split szk:q:3 szk:q:8").assert().success();
    assert_eq!(
        commit_trees(&env),
        trees_entire_file,
        "outcome should be the same regardless of hunk order"
    );

    // Reverse hunk order.
    env.but("undo").assert().success();
    env.but("split szk:q:8 szk:q:3").assert().success();
    assert_eq!(
        commit_trees(&env),
        trees_entire_file,
        "outcome should be the same regardless of hunk order"
    );

    // Repeated hunks.
    env.but("undo").assert().success();
    env.but("split szk:q:8 szk:q:3 szk:q:8").assert().success();
    assert_eq!(
        commit_trees(&env),
        trees_entire_file,
        "repeated hunks are deduplicated"
    );

    // Hunk then file.
    env.but("undo").assert().success();
    env.but("split szk:q:8 szk:q").assert().success();
    assert_eq!(
        commit_trees(&env),
        trees_entire_file,
        "file overlapping with hunks is deduplicated",
    );

    // File then hunk.
    env.but("undo").assert().success();
    env.but("split szk:q szk:q:8").assert().success();
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
┊●   qxq original
┊│     qxq:t A two
┊●   zts original
┊│     zts:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("split qxq:t zts:k")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Can only split changes from one commit. Got qxq and zts

"#]]);
}
