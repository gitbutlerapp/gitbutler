use snapbox::IntoData as _;
use std::fs;

use crate::{
    command::util::{add_dirty_worktree, enable_worktree_manipulation},
    utils::{CommandExt, Sandbox},
};

#[test]
fn query_file_glob_filters_human_and_json() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("src/main.rs", "fn main() {}\n");
    env.file("src/nested/lib.rs", "pub fn example() {}\n");
    env.file("src/readme.md", "not Rust\n");
    env.but(r#"diff --query '(file :glob "src/**/*.rs")'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────────────╮
 vo:d A src/main.rs │
────────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +fn main() {}

──────────────────────────╮
 kw:7 A src/nested/lib.rs │
──────────────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +pub fn example() {}

"#]]);
    env.but(r#"diff --json --query '(file :glob "src/**/*.rs")'"#)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "id": "vo:d",
      "path": "src/main.rs",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@\n+fn main() {}\n"
          }
        ]
      }
    },
    {
      "id": "kw:7",
      "path": "src/nested/lib.rs",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@\n+pub fn example() {}\n"
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
fn query_partial_lines_preserve_context_and_recalculate_headers() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("file", "context\nold\nlast\n");
    env.but("commit -b A --no-message").assert().success();
    env.file("file", "context\nnew\nTODO\nlast\n");
    env.but(r#"diff --query '(line-added :regex "^new$")'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────╮
 qs:8 M file │
─────────────╯

@@ -1,3 +1,4 @@
───────────────
1 ┊ 1 │  context
2 ┊ 2 │  old
  ┊ 3 │ +new
3 ┊ 4 │  last

"#]]);
    env.but(r#"diff --json --query '(line-added :regex "^new$")'"#)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "id": "qs:8",
      "path": "file",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 3,
            "newStart": 1,
            "newLines": 4,
            "diff": "@@ -1,3 +1,4 @@\n context\n old\n+new\n last\n"
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
fn query_hunks_ignore_context_and_compose_with_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("file.rs", "TODO context\nold\n");
    env.but("commit -b A --no-message").assert().success();
    env.file("file.rs", "TODO context\nnew\n");
    env.file("other.rs", "TODO added\nleave this too\n");
    env.file("file.txt", "leave\n");
    env.but(r#"diff --query '(difference (file :extension "rs") (hunk-added :contains "TODO"))'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────────╮
 oz:5 M file.rs │
────────────────╯

@@ -1,2 +1,2 @@
───────────────
1 ┊ 1 │  TODO context
2 ┊   │ -old
  ┊ 2 │ +new

"#]]);
    env.but(r#"diff --json --query '(difference (file :extension "rs") (hunk-added :contains "TODO"))'"#).allow_json().assert().success().stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "id": "oz:5",
      "path": "file.rs",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 2,
            "newStart": 1,
            "newLines": 2,
            "diff": "@@ -1,2 +1,2 @@\n TODO context\n-old\n+new\n"
          }
        ]
      }
    }
  ]
}

"#]].raw());
}

#[test]
fn query_range_and_multiple_predicates() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file(
        "file",
        "keep outside\nkeep inside\nleave inside\nkeep outside again\n",
    );
    env.but("diff")
        .args(["--query", r#"(line-added :contains "keep" :range '(2 3))"#])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────╮
 qs:b A file │
─────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +keep inside

"#]]);
    env.but("diff --json")
        .args(["--query", r#"(line-added :contains "keep" :range '(2 3))"#])
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "id": "qs:b",
      "path": "file",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@\n+keep inside\n"
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
fn query_narrows_positional_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("one.rs", "one\n");
    env.file("two.rs", "two\n");
    env.but(r#"diff one.rs --query '(file :extension "rs")'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────────╮
 pu:0 A one.rs │
───────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +one

"#]]);
    env.but(r#"diff --json one.rs --query '(file :extension "rs")'"#)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "id": "pu:0",
      "path": "one.rs",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@\n+one\n"
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
fn query_no_matches_is_empty() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("file", "text\n");
    env.but(r#"diff --query '(file :status :deleted)'"#)
        .assert()
        .success()
        .stdout_eq("");
    env.but(r#"diff --json --query '(file :status :deleted)'"#)
        .allow_json()
        .assert()
        .success()
        .stdout_eq("{\n  \"changes\": []\n}\n");
}

#[test]
fn query_invalid_regex_has_labels() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.but(r#"diff --query '(line :regex "[")'"#)
        .assert()
        .failure()
        .stdout_eq("")
        .stderr_eq(snapbox::str![[r#"
Error:   × Invalid diff query: invalid regex: regex parse error:
  │     [
  │     ^
  │ error: unclosed character class
   ╭─[query:1:14]
 1 │ (line :regex "[")
   ·              ─┬─
   ·               ╰─┤ invalid regex: regex parse error:
   ·                 │     [
   ·                 │     ^
   ·                 │ error: unclosed character class
   ╰────


"#]]);
}

#[test]
fn query_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but(r#"diff A --query '(intersection (file :status :added) (line :contains "A"))'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────╮
 A A │
─────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +A

"#]]);
    env.but(r#"diff --json A --query '(intersection (file :status :added) (line :contains "A"))'"#)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "path": "A",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@\n+A\n"
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
fn query_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but(r#"diff tpm --query '(file :path "A")'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────╮
 t:t:6 A A │
───────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +A

"#]]);
    env.but(r#"diff --json tpm --query '(file :path "A")'"#)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "path": "A",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@\n+A\n"
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
fn query_committed_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but(r#"diff tpm:t --query '(hunk :regex "^A$")'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────╮
 t:t:6 A A │
───────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +A

"#]]);
    env.but(r#"diff --json tpm:t --query '(hunk :regex "^A$")'"#)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "path": "A",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@\n+A\n"
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
fn query_non_text_complements_keep_binary_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("image.png", PNG_BINARY_CONTENT);
    env.file("file", "text\n");
    env.but(r#"diff --query '(not (line :regex ".*"))'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────────╮
 nx:e A image.png │
──────────────────╯

No diff available - file is either empty, binary, or too large

"#]]);
    env.but(r#"diff --json --query '(not (line :regex ".*"))'"#)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "id": "nx:e",
      "path": "image.png",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": []
      }
    }
  ]
}

"#]]
            .raw(),
        );
}

#[test]
fn query_rename_without_text_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("old.rs", "contents\n");
    env.but("commit -b A --no-message").assert().success();
    env.rename_file("old.rs", "new.rs");
    env.but(r#"diff --query '(file :status :renamed :path "new.rs")'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────────╮
 lt:e R new.rs │
───────────────╯

No diff available - file is either empty, binary, or too large

"#]]);
    env.but("commit -b A --no-message").assert().success();
    let commit = env.invoke_git("rev-parse A");
    env.but(format!(
        r#"diff {commit} --query '(file :status :renamed :path "new.rs")'"#
    ))
    .assert()
    .success()
    .stdout_eq(snapbox::str![[r#"
────────────────╮
 p:l:e R new.rs │
────────────────╯

No diff available - file is either empty, binary, or too large

"#]]);
    env.but(format!(
        r#"diff --json {commit} --query '(file :status :renamed :path "new.rs")'"#
    ))
    .allow_json()
    .assert()
    .success()
    .stdout_eq(
        snapbox::str![[r#"
{
  "changes": [
    {
      "path": "new.rs",
      "status": "renamed",
      "oldPath": "old.rs",
      "diff": {
        "type": "patch",
        "hunks": []
      }
    }
  ]
}

"#]]
        .raw(),
    );
}

#[test]
fn query_branch_rename_without_text_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.rename_file("M", "renamed.rs");
    env.but("commit -b A --no-message").assert().success();
    env.but(r#"diff A --query '(file :status :renamed)'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────╮
 R renamed.rs │
──────────────╯

No text diff available

"#]]);
    env.but(r#"diff --json A --query '(file :status :renamed)'"#)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "path": "renamed.rs",
      "status": "renamed",
      "oldPath": "M",
      "diff": {
        "type": "patch",
        "hunks": []
      }
    }
  ]
}

"#]]
            .raw(),
        );
}

#[test]
fn query_path_prefix() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("src/a.rs", "first\n");
    env.file("src/b.txt", "second\n");
    env.file("other.rs", "third\n");
    env.but(r#"diff src/ --query '(file :extension "rs")'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────────╮
 nv:3 A src/a.rs │
─────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +first

"#]]);
    env.but(r#"diff --json src/ --query '(file :extension "rs")'"#)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "id": "nv:3",
      "path": "src/a.rs",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@\n+first\n"
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
fn query_remote_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("remote-local-divergence");
    env.setup_metadata(&["main", "A"]);
    let commit = env.invoke_git("rev-parse refs/remotes/origin/A");
    env.but(format!(
        r#"diff {commit} --query '(hunk-added :regex "remote")'"#
    ))
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
    env.but(format!(
        r#"diff --json {commit} --query '(hunk-added :regex "remote")'"#
    ))
    .allow_json()
    .assert()
    .success()
    .stdout_eq(
        snapbox::str![[r#"
{
  "changes": [
    {
      "path": "only-on-remote",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@\n+only-on-remote\n"
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
fn query_narrows_positional_hunk_and_preserves_its_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("file", "keep\nTODO: leave\n");
    env.file("other", "keep this unselected file\n");
    env.but(r#"diff qs:2 --query '(not (line :contains "TODO"))'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────╮
 qs:2 A file │
─────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +keep

"#]]);
    env.but(r#"diff --json qs:2 --query '(not (line :contains "TODO"))'"#)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "id": "qs:2",
      "path": "file",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@\n+keep\n"
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
fn query_uses_linked_worktree_source() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    enable_worktree_manipulation(&env);
    env.but("status").assert().success();
    let wt_dir = add_dirty_worktree(&env, "wt-feature", "A");
    env.file("main.txt", "dirty in main\n");
    env.but(r#"diff --query '(file :extension "txt")'"#)
        .current_dir(&wt_dir)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────────╮
 nl:a A note.txt │
─────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +dirty

"#]]);
    env.but(r#"diff --json --query '(file :extension "txt")'"#)
        .current_dir(&wt_dir)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(
            snapbox::str![[r#"
{
  "changes": [
    {
      "id": "nl:a",
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
            "diff": "@@ -1,0 +1,1 @@\n+dirty\n"
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
fn query_deleted_files_and_removed_lines() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("file", "context\nold\n");
    env.but("commit -b A --no-message").assert().success();
    env.file("file", "context\nnew\n");
    fs::remove_file(env.projects_root().join("M")).unwrap();
    env.but(r#"diff --query '(union (file :status :deleted) (line-removed :regex "^old$"))'"#)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────╮
 nt:5 D M │
──────────╯

@@ -1,1 +1,0 @@
───────────────
1 ┊   │ -M

─────────────╮
 qs:a M file │
─────────────╯

@@ -1,2 +1,1 @@
───────────────
1 ┊ 1 │  context
2 ┊   │ -old

"#]]);
    env.but(
        r#"diff --json --query '(union (file :status :deleted) (line-removed :regex "^old$"))'"#,
    )
    .allow_json()
    .assert()
    .success()
    .stdout_eq(
        snapbox::str![[r#"
{
  "changes": [
    {
      "id": "nt:5",
      "path": "M",
      "status": "deleted",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 1,
            "newStart": 1,
            "newLines": 0,
            "diff": "@@ -1,1 +1,0 @@\n-M\n"
          }
        ]
      }
    },
    {
      "id": "qs:a",
      "path": "file",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 2,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,2 +1,1 @@\n context\n-old\n"
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
┊●   txl Add files to modify
┊│     txl:usv A to-delete.txt
┊│     txl:lou A to-modify.txt
┊│     txl:ztt A to-rename.txt
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
─────────────────────╮
 nxx:213 A added.txt │
─────────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +added

───────────────────────╮
 szk:emp R renamed.txt │
───────────────────────╯

No diff available - file is either empty, binary, or too large

─────────────────────────╮
 usv:0a5 D to-delete.txt │
─────────────────────────╯

@@ -1,1 +1,0 @@
───────────────
1 ┊   │ -deleted

─────────────────────────╮
 lou:b28 M to-modify.txt │
─────────────────────────╯

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
─────────────────────────╮
 vxw:nxx:213 A added.txt │
─────────────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +added

───────────────────────────╮
 vxw:szk:emp R renamed.txt │
───────────────────────────╯

No diff available - file is either empty, binary, or too large

─────────────────────────────╮
 vxw:usv:0a5 D to-delete.txt │
─────────────────────────────╯

@@ -1,1 +1,0 @@
───────────────
1 ┊   │ -deleted

─────────────────────────────╮
 vxw:lou:b28 M to-modify.txt │
─────────────────────────────╯

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
fn bare_diff_in_a_linked_worktree_shows_its_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    enable_worktree_manipulation(&env);
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
    let wt_dir = add_dirty_worktree(&env, "wt-feature", "A");
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
