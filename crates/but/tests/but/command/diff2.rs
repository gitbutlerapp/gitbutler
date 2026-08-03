use std::fs;

use crate::utils::{CommandExt, Sandbox};

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

    env.but("_diff2")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/diff2/uncommitted.stdout.term.svg"
        ]);

    env.but("_diff2")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/diff2/uncommitted.stdout"].raw());
}

#[test]
fn path_prefix() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("a/b/c.txt", "content of c");
    env.file("a/b/d.txt", "content of d");
    env.file("a/b.txt", "content of b");

    env.but("_diff2 a/b/")
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
fn json_uncommitted_targets() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("a/b/c.txt", "content of c\n");
    env.file("a/b/d.txt", "content of d\n");
    env.file("a/b.txt", "content of b\n");

    env.but("_diff2 --json")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file!["snapshots/diff2/json-uncommitted.stdout"].raw());
    env.but("_diff2 --json a/b/c.txt")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file!["snapshots/diff2/json-uncommitted-file.stdout"].raw());
    env.but("_diff2 --json a/b/")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file!["snapshots/diff2/json-path-prefix.stdout"].raw());
}

#[test]
fn json_committed_targets() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    for target in ["A", "tpm", "tpm:t"] {
        env.but(format!("_diff2 --json {target}"))
            .allow_json()
            .assert()
            .success()
            .stderr_eq(snapbox::str![])
            .stdout_eq(snapbox::file!["snapshots/diff2/json-committed-a.stdout"].raw());
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

    env.but("_diff2 --json 1#0")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file!["snapshots/diff2/json-tree-change-statuses.stdout"].raw());
}
