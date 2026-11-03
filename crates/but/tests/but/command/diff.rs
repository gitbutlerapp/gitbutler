use std::fs;

use snapbox::IntoData as _;

use crate::{
    command::util::add_dirty_worktree,
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
fn git_revision_target_hints_git_diff() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    // Git revisions and ranges are not CLI IDs, so point at the Git command that diffs them.
    env.but("diff HEAD~1")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'HEAD~1'

Hint: To diff it as a Git revision, run `git diff HEAD~1`. Run `but status` for applicable targets.

"#]]);

    env.but("diff main...HEAD")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'main...HEAD'

Hint: To diff it as a Git revision, run `git diff main...HEAD`. Run `but status` for applicable targets.

"#]]);

    // Two revisions are one comparison, so the hint keeps them together.
    env.but("diff HEAD~1 HEAD")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'HEAD~1'

Hint: To diff it as a Git revision, run `git diff HEAD~1 HEAD`. Run `but status` for applicable targets.

"#]]);

    // Anything Git can't resolve either keeps the generic hint.
    env.but("diff nonexistent")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'nonexistent'

Hint: Run `but status` for applicable targets.

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
──────────────────╮
 up:2 A a/b/c.txt │
──────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +content of c

──────────────────╮
 oz:8 A a/b/d.txt │
──────────────────╯

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
┊●   txlq Add files to modify
┊│     txlq:usvy A to-delete.txt
┊│     txlq:louy A to-modify.txt
┊│     txlq:ztty A to-rename.txt
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

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
───────────────────────╮
 nxxm:213a A added.txt │
───────────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +added

─────────────────────────╮
 szkp:empt R renamed.txt │
─────────────────────────╯

No diff available - file is either empty, binary, or too large

───────────────────────────╮
 usvy:0a51 D to-delete.txt │
───────────────────────────╯

@@ -1,1 +1,0 @@
───────────────
1 ┊   │ -deleted

───────────────────────────╮
 louy:b28b M to-modify.txt │
───────────────────────────╯

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
────────────────────────────╮
 vxwu:nxxm:213a A added.txt │
────────────────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +added

──────────────────────────────╮
 vxwu:szkp:empt R renamed.txt │
──────────────────────────────╯

No diff available - file is either empty, binary, or too large

────────────────────────────────╮
 vxwu:usvy:0a51 D to-delete.txt │
────────────────────────────────╯

@@ -1,1 +1,0 @@
───────────────
1 ┊   │ -deleted

────────────────────────────────╮
 vxwu:louy:b28b M to-modify.txt │
────────────────────────────────╯

@@ -1,1 +1,1 @@
───────────────
1 ┊   │ -initial
  ┊ 1 │ +modified

"#]]);
}

#[test]
fn committed_hunk() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file.txt", "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");
    env.but("commit -m base").assert().success();
    env.file("file.txt", "one\n2\n3\n4\n5\n6\n7\n8\n9\nten\n");
    env.file("new.txt", "new\n");
    env.but("commit -m change")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit ryk on branch 'a-branch-1'

"#]]);

    // The commit diff prints one committed hunk ID per hunk.
    env.but("diff ryk")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────────╮
 r:u:1 M file.txt │
──────────────────╯

@@ -1,4 +1,4 @@
───────────────
1 ┊   │ -1
  ┊ 1 │ +one
2 ┊ 2 │  2
3 ┊ 3 │  3
4 ┊ 4 │  4

──────────────────╮
 r:u:9 M file.txt │
──────────────────╯

@@ -7,4 +7,4 @@
───────────────
 7 ┊  7 │  7
 8 ┊  8 │  8
 9 ┊  9 │  9
10 ┊    │ -10
   ┊ 10 │ +ten

─────────────────╮
 r:s:b A new.txt │
─────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +new

"#]]);

    // Passing one of those IDs back shows only that hunk, under the same ID.
    env.but("diff r:u:9")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────────╮
 r:u:9 M file.txt │
──────────────────╯

@@ -7,4 +7,4 @@
───────────────
 7 ┊  7 │  7
 8 ┊  8 │  8
 9 ┊  9 │  9
10 ┊    │ -10
   ┊ 10 │ +ten

"#]]);
    // JSON keeps the file's status.
    env.but("diff --json r:s:b")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "id": "r:s:b",
      "path": "new.txt",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@\n+new\n"
          }
        ]
      }
    }
  ]
}

"#]]
            .raw(),
        );
}

#[test]
fn worktree() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("status").assert().success();
    add_dirty_worktree(&env, "wt-feature", "A");
    env.file("main.txt", "dirty in main\n");

    env.but("diff wt-feature:@")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
─────────────────╮
 nl:a A note.txt │
─────────────────╯
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
      "status": "added",
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
fn bare_diff_in_a_linked_worktree_shows_its_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("status").assert().success();
    let wt_dir = add_dirty_worktree(&env, "wt-feature", "A");
    env.file("main.txt", "dirty in main\n");

    // Only the worktree's note.txt shows; main.txt is dirty in the main worktree.
    env.but("diff")
        .current_dir(&wt_dir)
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
─────────────────╮
 nl:a A note.txt │
─────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +dirty

"#]]);
}

#[test]
fn bare_diff_in_an_unmanaged_worktree_is_refused() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let wt_dir = super::util::add_unmanaged_worktree(&env, "wt-feature");
    env.file("main.txt", "dirty in main\n");

    env.but("diff")
        .current_dir(&wt_dir)
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Worktree wt-feature is not managed by GitButler

Hint: Run `but worktree list` to see the worktrees GitButler manages

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
──────────────────╮
 A only-on-remote │
──────────────────╯

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

    env.invoke_git("config --local diff.png.textconv 'git hash-object --no-filters'");

    env.but("diff xnw")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────────────────╮
 x:x:2 A .gitattributes │
────────────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +*.png diff=png

────────────────╮
 x:t A file.png │
────────────────╯

(diff generated from binary-to-text conversion)
@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +26a8c68efad2a094e8fe6d850426b651d353c568

"#]]);
}

#[test]
fn several_targets_show_each_in_turn() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("a.txt", "a\n");
    env.file("b.txt", "b\n");
    env.file("c.txt", "c\n");

    // b.txt isn't named, so it doesn't show.
    env.but("diff a.txt c.txt")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────╮
 nk:2 A a.txt │
──────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +a

──────────────╮
 kl:9 A c.txt │
──────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +c

"#]]);
    env.but("diff --json a.txt c.txt")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "id": "nk:2",
      "path": "a.txt",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+a/n"
          }
        ]
      }
    },
    {
      "id": "kl:9",
      "path": "c.txt",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+c/n"
          }
        ]
      }
    }
  ]
}

"#]]);
}

#[test]
fn a_commit_or_branch_must_be_the_only_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("a.txt", "a\n");

    // `git diff A B` compares the two, so showing each in turn would read like a comparison.
    for targets in ["A B", "A a.txt", "a.txt A"] {
        env.but(format!("diff {targets}"))
            .assert()
            .failure()
            .stdout_eq(snapbox::str![])
            .stderr_eq(snapbox::str![[r#"
Error: A commit or branch must be the only target of `but diff`

Hint: To compare two revisions, run `git diff <a> <b>`. To see each target on its own, run `but diff <target>` once per target.

"#]]);
    }
}

#[test]
fn clean_path_has_no_uncommitted_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("src/a.txt", "a\n");
    env.but("commit -m x src/a.txt").assert().success();

    let root = env.projects_root().display().to_string();
    for (dir, spelling) in [
        (".", "src/a.txt".to_owned()),
        (".", "./src/a.txt".to_owned()),
        (".", format!("{root}/src/a.txt")),
        (".", "src/".to_owned()),
        (".", ".".to_owned()),
        ("src", "a.txt".to_owned()),
        ("src", "src/a.txt".to_owned()),
    ] {
        env.but(format!("diff {spelling}"))
            .current_dir(env.projects_root().join(dir))
            .assert()
            .success()
            .stderr_eq(snapbox::str![])
            .stdout_eq(format!("'{spelling}' has no uncommitted changes\n"));
    }

    // A path outside of the checkout isn't one that could have changes.
    let outside = env.projects_root().parent().unwrap().join("outside.txt");
    std::fs::write(&outside, "outside\n").unwrap();
    env.but("diff ../outside.txt")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: '../outside.txt'

Hint: Run `but status` for applicable targets.

"#]]);

    // A clean path doesn't get in the way of a dirty one.
    env.file("b.txt", "b\n");
    env.but("diff b.txt src/a.txt")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────╮
 pn:2 A b.txt │
──────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +b

'src/a.txt' has no uncommitted changes

"#]]);
    env.but("diff --json src/a.txt")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "changes": []
}

"#]]);
}

#[test]
fn clean_path_in_a_linked_worktree() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("status").assert().success();
    let wt_dir = add_dirty_worktree(&env, "wt-feature", "A");

    env.but("diff M")
        .current_dir(&wt_dir)
        .assert()
        .success()
        .stdout_eq("'M' has no uncommitted changes\n");
}

#[test]
fn path_in_an_unmanaged_worktree_is_refused() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let wt_dir = super::util::add_unmanaged_worktree(&env, "wt-feature");

    // note.txt is dirty, but GitButler doesn't list this checkout's changes to know that.
    env.but("diff note.txt")
        .current_dir(&wt_dir)
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Worktree wt-feature is not managed by GitButler

Hint: Run `but worktree list` to see the worktrees GitButler manages

"#]]);
}

#[test]
fn stat_lists_changed_files_and_line_counts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    let lines: String = (1..=20).map(|n| format!("{n}\n")).collect();
    env.file("long.txt", &lines);
    env.file("old-name.txt", "keep\n");
    env.but("commit -m x long.txt old-name.txt")
        .assert()
        .success();
    env.invoke_git("mv old-name.txt new-name.txt");
    env.but("commit -m rename new-name.txt").assert().success();

    // long.txt has two hunks, which count as one file.
    env.file(
        "long.txt",
        lines
            .replace("\n2\n", "\ntwo\n")
            .replace("\n19\n", "\nnineteen\n"),
    );
    env.file("a-new-file.txt", "a\nb\n");
    env.but("diff --stat")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
a-new-file.txt +2 -0
long.txt       +2 -2
2 files changed, +4 -2

"#]]);
    env.but("diff --json --stat")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "files": [
    {
      "path": "a-new-file.txt",
      "status": "added",
      "insertions": 2,
      "deletions": 0
    },
    {
      "path": "long.txt",
      "status": "modified",
      "insertions": 2,
      "deletions": 2
    }
  ]
}

"#]]);

    // A named file, and a clean one that has nothing to count.
    env.but("diff --stat long.txt new-name.txt")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
long.txt +2 -2
1 file changed, +2 -2
'new-name.txt' has no uncommitted changes

"#]]);

    // A branch counts what it adds to its base.
    env.but("diff --stat A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
A            +1 -0
long.txt     +20 -0
new-name.txt +1 -0
3 files changed, +22 -0

"#]]);

    // A commit shows where a renamed file came from.
    let rename_commit = env.invoke_git("rev-parse A");
    env.but(format!("diff --stat {rename_commit}"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
old-name.txt => new-name.txt +0 -0
1 file changed, +0 -0

"#]]);
}

#[test]
fn stat_keeps_one_path_in_two_checkouts_apart() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("status").assert().success();
    add_dirty_worktree(&env, "wt-feature", "A");
    env.file("note.txt", "dirty\nin main\n");

    env.but("diff --stat @ wt:@")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
note.txt +2 -0
note.txt +1 -0
2 files changed, +3 -0

"#]]);
}

#[test]
fn git_flags_get_examples_of_what_works() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // Clap's tips would pass these after `--`, and read `--staged` as a typo of `--stat`.
    for flag in ["--name-only", "--staged"] {
        env.but(format!("diff {flag}"))
            .assert()
            .failure()
            .stdout_eq(snapbox::str![])
            .stderr_eq(snapbox::str![[r#"
error: unexpected argument '--[..]' found

Usage: but diff [..]

For more information, try '--help'.

Examples:
  but diff                    # all uncommitted changes, staged or not
  but diff <file-or-hunk>...  # only the given changes
  but diff --stat             # only the changed files, with line counts
  git diff <flag>...          # any other Git diff flag

"#]]);
    }
}

#[test]
fn a_path_with_unresolved_conflicts_is_not_clean() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("status").assert().success();
    let wt_dir = add_dirty_worktree(&env, "wt-feature", "A");

    // Leave behind what a conflicting update produces for an uncommitted file.
    let conflict = r#"mkdir -p src &&
printf '<<<<<<< ours\nours\n=======\ntheirs\n>>>>>>> theirs\n' > src/c.txt &&
base=$(echo base | git hash-object -w --stdin) &&
ours=$(echo ours | git hash-object -w --stdin) &&
theirs=$(echo theirs | git hash-object -w --stdin) &&
printf '100644 %s 1\tsrc/c.txt\n100644 %s 2\tsrc/c.txt\n100644 %s 3\tsrc/c.txt\n' "$base" "$ours" "$theirs" | git update-index --index-info"#;
    env.invoke_bash(conflict);
    but_testsupport::invoke_bash_at_dir(conflict, &wt_dir);

    for (dir, target) in [
        (env.projects_root(), "src"),
        (wt_dir.as_path(), "src/c.txt"),
        (wt_dir.as_path(), "src"),
    ] {
        env.but(format!("diff {target}"))
            .current_dir(dir)
            .assert()
            .failure()
            .stdout_eq(snapbox::str![])
            .stderr_eq(format!(
                "Error: Could not find target: '{target}'\n\nHint: Run `but status` for applicable targets.\n"
            ));
    }
}

#[test]
fn show_stat_counts_the_lines_of_each_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("show --stat tpm")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
...
Files changed:
  A A  (+1, -0)

"#]]);
}
