use std::fs;

use crate::{
    command::util::{add_dirty_worktree, enable_worktree_manipulation},
    utils::{CommandExt, Sandbox},
};

#[test]
fn uncommitted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file(
        "file",
        r#"
items = ["ink ribbon", "old key", "green herb", "crank", "lighter"]

puts "You check the desk drawer..."
sleep 0.8

found = items.sample

if found == "green herb"
  puts "You found a #{found}."
  puts "You feel just a little better."
else
  puts "You found an #{found}." rescue puts "You found a #{found}."
  puts "Probably useful somewhere."
end

puts "\nA distant door unlocks."
"#,
    );

    env.but("diff")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/diff/uncommitted.stdout.term.svg"]);

    env.but("diff")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/diff/uncommitted.stdout"].raw());
}

#[test]
fn path_prefix() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("a/b/c.txt", "content of c");
    env.file("a/b/d.txt", "content of d");
    env.file("a/b.txt", "content of b");

    env.but("diff a/b/")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────────╮
 up:2 a/b/c.txt │
────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +content of c

────────────────╮
 oz:8 a/b/d.txt │
────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +content of d

"#]]);
}

#[test]
fn worktree() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    enable_worktree_manipulation(&env);
    env.but("status").assert().success();
    add_dirty_worktree(&env, "wt-feature", "A");
    env.file("main.txt", "dirty in main\n");

    env.but("diff wt-feature")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
───────────────╮
[..] note.txt │
───────────────╯
[..]
@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +dirty

"#]]);

    env.but("diff --json wt-feature")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "id": "[..]",
      "path": "note.txt",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+dirty/n"
          }
        ]
      }
    }
  ]
}

"#]]);
}

#[test]
fn json_uncommitted_targets() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("a/b/c.txt", "content of c\n");
    env.file("a/b/d.txt", "content of d\n");
    env.file("a/b.txt", "content of b\n");

    env.but("diff --json")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file!["snapshots/diff/json-uncommitted.stdout"].raw());
    env.but("diff --json a/b/c.txt")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file!["snapshots/diff/json-uncommitted-file.stdout"].raw());
    env.but("diff --json a/b/")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file!["snapshots/diff/json-path-prefix.stdout"].raw());
}

#[test]
fn json_committed_targets() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    for target in ["A", "tpm", "tpm:t"] {
        env.but(format!("diff --json {target}"))
            .allow_json()
            .assert()
            .success()
            .stderr_eq(snapbox::str![])
            .stdout_eq(snapbox::file!["snapshots/diff/json-committed-a.stdout"].raw());
    }
}

#[test]
fn json_tree_change_statuses() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("modified.txt", "before\n");
    env.file("deleted.txt", "delete me\n");
    env.file("renamed-before.txt", "rename me\n");
    env.but("commit -b A -m status-base").assert().success();

    env.file("added.txt", "added\n");
    env.file("modified.txt", "after\n");
    fs::remove_file(env.projects_root().join("deleted.txt")).unwrap();
    fs::rename(
        env.projects_root().join("renamed-before.txt"),
        env.projects_root().join("renamed-after.txt"),
    )
    .unwrap();
    env.but("commit -b A -m status-target").assert().success();

    env.but("diff --json 1#0")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file!["snapshots/diff/json-tree-change-statuses.stdout"].raw());
}
