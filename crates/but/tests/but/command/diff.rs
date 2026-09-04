use std::fs;

use crate::{
    command::util::{add_dirty_worktree, enable_worktree_manipulation},
    utils::{CommandExt, Sandbox},
};

#[test]
fn rejects_unnamed_segment_as_target() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-anonymous-segment");
    env.setup_metadata(&["A"]);

    env.but("diff g0")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Cannot operate on anonymous branch 'g0'

Hint: Name it with `but reword g0` first! Note that the short ID is likely to change when the branch is named.

"#]]);
}

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

/// Diff committed and uncommitted using AI_AGENT output that forces more characters in short IDs.
#[test]
fn diff_different_changes_with_agent_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("to-modify.txt", "initial\n");
    env.file("to-rename.txt", "renamed\n");
    env.file("to-delete.txt", "deleted\n");
    env.but("commit -m 'Add files to modify'")
        .assert()
        .success();

    env.but("status -f")
        .env("AI_AGENT", "test-agent")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   txl Add files to modify
┊│     txl:usv A to-delete.txt
┊│     txl:lou A to-modify.txt
┊│     txl:ztt A to-rename.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: commits are listed newest first. The first token on each line is the ID to use in commands.
Hint: run `but help` for all commands

"#]]);

    env.file("to-modify.txt", "modified\n");
    env.rename_file("to-rename.txt", "renamed.txt");
    env.remove_file("to-delete.txt");
    env.file("added.txt", "added\n");

    // uncommitted output
    env.but("diff @")
        .env("AI_AGENT", "test-agent")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────────────╮
 nxx:213 added.txt │
───────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +added

─────────────────────╮
 szk:emp renamed.txt │
─────────────────────╯

No diff available - file is either empty, binary, or too large

───────────────────────╮
 usv:0a5 to-delete.txt │
───────────────────────╯

@@ -1,1 +1,0 @@
───────────────
1 ┊   │ -deleted

───────────────────────╮
 lou:b28 to-modify.txt │
───────────────────────╯

@@ -1,1 +1,1 @@
───────────────
1 ┊   │ -initial
  ┊ 1 │ +modified

"#]]);

    env.but("commit -m 'Add, delete and rename files'")
        .env("AI_AGENT", "test-agent")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit vxw on branch 'a-branch-1'

"#]]);

    // committed output
    env.but("diff vxw")
        .env("AI_AGENT", "test-agent")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────────────────╮
 vxw:nxx:213 added.txt │
───────────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +added

─────────────────────────╮
 vxw:szk:emp renamed.txt │
─────────────────────────╯

No diff available - file is either empty, binary, or too large

───────────────────────────╮
 vxw:usv:0a5 to-delete.txt │
───────────────────────────╯

@@ -1,1 +1,0 @@
───────────────
1 ┊   │ -deleted

───────────────────────────╮
 vxw:lou:b28 to-modify.txt │
───────────────────────────╯

@@ -1,1 +1,1 @@
───────────────
1 ┊   │ -initial
  ┊ 1 │ +modified

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

    env.but("diff wt-feature:@")
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

    env.but("diff --json wt-feature:@")
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
fn remote_only_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("remote-local-divergence");
    env.setup_metadata(&["main", "A"]);

    let remote_commit = env.invoke_git("rev-parse refs/remotes/origin/A");
    env.but(format!("diff {remote_commit}"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────────────╮
 added only-on-remote │
──────────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +only-on-remote

"#]]);
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
    env.but("commit -b A -m status-target")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit oul on branch 'A'

"#]]);

    env.but("diff --json oul")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file!["snapshots/diff/json-tree-change-statuses.stdout"].raw());
}

/// A valid PNG file, useful if you want to test binary files.
const PNG_BINARY_CONTENT: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xB5, 0x1C, 0x0C,
    0x02, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0x64, 0xF8, 0x0F, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xE3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44,
    0xAE, 0x42, 0x60, 0x82,
];

#[test]
#[cfg(unix)] // od is not available on Windows
fn textconv_output_is_rendered_in_diff() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file.png", PNG_BINARY_CONTENT);
    env.file(".gitattributes", "*.png diff=png");
    env.but("commit -m 'Add binary file'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit xnw on new branch 'a-branch-1'

"#]]);

    env.invoke_git("config --local diff.png.textconv od");

    env.but("diff xnw")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────────────╮
 x:x:2 .gitattributes │
──────────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +*.png diff=png

──────────────╮
 x:t file.png │
──────────────╯

(diff generated from binary-to-text conversion)
@@ -1,0 +1,6 @@
───────────────
  ┊ 1 │ +0000000 050211 043516 005015 005032 000000 006400 044111 051104
  ┊ 2 │ +0000020 000000 000400 000000 000400 002010 000000 132400 006034
  ┊ 3 │ +0000040 000002 000000 044413 040504 074124 061732 174144 000017
  ┊ 4 │ +0000060 002401 000401 014047 063343 000000 000000 042511 042116
  ┊ 5 │ +0000100 041256 101140
  ┊ 6 │ +0000104

"#]]);
}
