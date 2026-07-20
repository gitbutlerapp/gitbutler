use std::fs;

use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn path_prefix() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("prefixx", "don't show this\n");
    env.file("prefix/a", "we want this\n");
    env.file("prefix/b", "we also want this\n");
    env.but("diff prefix/")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
──────────────╮
yzy:c prefix/a│
──────────────╯
     1│+we want this
──────────────╮
uol:d prefix/b│
──────────────╯
     1│+we also want this

"#]]);
}

#[test]
fn partitioned_file_diff_honors_collision_index() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    crate::command::util::commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Split a.txt's hunks between stack A and the uncommitted area: the two
    // partitions share the path-derived ID and carry collision indices.
    env.but("stage a.txt A").assert().success();
    env.but("unstage nk:2").assert().success();

    // Each indexed file ID shows only its own partition's hunks.
    env.but("diff nkn#0")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
─────────────╮
nkn#0:2 a.txt│
─────────────╯
   1  │-first
     1│+firsta
   2 2│ line
   3 3│ line
   4 4│ line

"#]]);
    env.but("diff nkn#1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
─────────────╮
nkn#1:e a.txt│
─────────────╯
    6  6│ line
    7  7│ line
    8  8│ line
    9   │-last
       9│+lasta

"#]]);
}

#[test]
fn json_no_target_empty_worktree() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("diff --format json")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "changes": []
}

"#]]);
}

#[test]
fn json_no_target_all_worktree_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("alpha.txt", "alpha\n");
    env.file("beta.txt", "beta\n");

    env.but("diff --format json")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "id": "xzy:b",
      "path": "alpha.txt",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+alpha/n"
          }
        ]
      }
    },
    {
      "id": "vqr:5",
      "path": "beta.txt",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+beta/n"
          }
        ]
      }
    }
  ]
}

"#]]);
}

#[test]
fn json_target_uncommitted_hunk_or_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("target.txt", "target\n");
    env.file("other.txt", "other\n");

    env.but("diff --format json")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "id": "xwx:4",
      "path": "other.txt",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+other/n"
          }
        ]
      }
    },
    {
      "id": "pky:b",
      "path": "target.txt",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+target/n"
          }
        ]
      }
    }
  ]
}

"#]]);

    let target_id = "pk:b";

    env.but(format!("diff --format json {target_id}"))
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "id": "pky:b",
      "path": "target.txt",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+target/n"
          }
        ]
      }
    }
  ]
}

"#]]);
}

#[test]
fn json_target_uncommitted_whole_file_with_multiple_hunks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file(
        "multi-hunk.txt",
        "line 01\nline 02\nline 03\nline 04\nline 05\nline 06\nline 07\nline 08\nline 09\nline 10\nline 11\nline 12\nline 13\nline 14\nline 15\nline 16\nline 17\nline 18\nline 19\nline 20\n",
    );
    env.but("commit A -m multi-hunk-base").assert().success();

    env.file(
        "multi-hunk.txt",
        "changed 01\nline 02\nline 03\nline 04\nline 05\nline 06\nline 07\nline 08\nline 09\nline 10\nline 11\nline 12\nline 13\nline 14\nline 15\nline 16\nline 17\nline 18\nline 19\nchanged 20\n",
    );

    env.but("diff --format json multi-hunk.txt")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "id": "utt:a",
      "path": "multi-hunk.txt",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 4,
            "newStart": 1,
            "newLines": 4,
            "diff": "@@ -1,4 +1,4 @@/n-line 01/n+changed 01/n line 02/n line 03/n line 04/n"
          }
        ]
      }
    },
    {
      "id": "utt:6",
      "path": "multi-hunk.txt",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 17,
            "oldLines": 4,
            "newStart": 17,
            "newLines": 4,
            "diff": "@@ -17,4 +17,4 @@/n line 17/n line 18/n line 19/n-line 20/n+changed 20/n"
          }
        ]
      }
    }
  ]
}

"#]]);
}

#[test]
fn json_target_path_prefix() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("prefixx", "don't show this\n");
    env.file("prefix/a", "we want this\n");
    env.file("prefix/b", "we also want this\n");

    env.but("diff --format json prefix/")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "id": "yzy:c",
      "path": "prefix/a",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+we want this/n"
          }
        ]
      }
    },
    {
      "id": "uol:d",
      "path": "prefix/b",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+we also want this/n"
          }
        ]
      }
    }
  ]
}

"#]]);
}

#[test]
fn json_target_committed_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("committed-target.txt", "target\n");
    env.file("committed-other.txt", "other\n");
    env.but("commit A -m committed-file-target")
        .assert()
        .success();

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   1 committed-file-target
┊│     1:k A committed-other.txt
┊│     1:w A committed-target.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    let committed_file_id = "3f:wm";

    env.but(format!("diff --format json {committed_file_id}"))
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "path": "committed-target.txt",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+target/n"
          }
        ]
      }
    }
  ]
}

"#]]);
}

#[test]
fn json_target_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("diff --format json A")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
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
            "diff": "@@ -1,0 +1,1 @@/n+A/n"
          }
        ]
      }
    }
  ]
}

"#]]);
}

#[test]
fn json_target_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    let change_id = "tpm";

    env.but(format!("diff --format json {change_id}"))
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
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
            "diff": "@@ -1,0 +1,1 @@/n+A/n"
          }
        ]
      }
    }
  ]
}

"#]]);
}

#[test]
fn json_target_uncommitted_area() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("unassigned.txt", "unassigned\n");
    env.file("assigned.txt", "assigned\n");
    env.but("stage assigned.txt A").assert().success();

    env.but("diff --format json zz")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "id": "nzr:4",
      "path": "unassigned.txt",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+unassigned/n"
          }
        ]
      }
    }
  ]
}

"#]]);
}

#[test]
fn json_target_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("unassigned.txt", "unassigned\n");
    env.file("assigned.txt", "assigned\n");
    env.but("stage assigned.txt A").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   nzr A unassigned.txt
┊
┊  ╭┄k0 [staged to A]
┊  │ xzo A assigned.txt
┊  │
┊╭┄g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit <branch> -m "message" --changes <id>` to commit them

"#]]);

    let stack_id = "k0";

    env.but(format!("diff --format json {stack_id}"))
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "id": "xzo:8",
      "path": "assigned.txt",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+assigned/n"
          }
        ]
      }
    }
  ]
}

"#]]);
}

#[test]
fn json_commit_target_tree_change_statuses() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("modified.txt", "before\n");
    env.file("deleted.txt", "delete me\n");
    env.file("renamed-before.txt", "rename me\n");
    env.but("commit A -m status-base").assert().success();

    env.file("added.txt", "added\n");
    env.file("modified.txt", "after\n");
    fs::remove_file(env.projects_root().join("deleted.txt"))?;
    fs::rename(
        env.projects_root().join("renamed-before.txt"),
        env.projects_root().join("renamed-after.txt"),
    )?;
    env.but("commit A -m status-target").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   1#0 status-target
┊│     1#0:nx A added.txt
┊│     1#0:nm D deleted.txt
┊│     1#0:u  M modified.txt
┊│     1#0:o  R renamed-after.txt
┊●   1#1 status-base
┊│     1#1:n A deleted.txt
┊│     1#1:u A modified.txt
┊│     1#1:z A renamed-before.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    let change_id = "1#0";

    env.but(format!("diff --format json {change_id}"))
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "changes": [
    {
      "path": "added.txt",
      "status": "added",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 0,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,0 +1,1 @@/n+added/n"
          }
        ]
      }
    },
    {
      "path": "deleted.txt",
      "status": "deleted",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 1,
            "newStart": 1,
            "newLines": 0,
            "diff": "@@ -1,1 +1,0 @@/n-delete me/n"
          }
        ]
      }
    },
    {
      "path": "modified.txt",
      "status": "modified",
      "diff": {
        "type": "patch",
        "hunks": [
          {
            "oldStart": 1,
            "oldLines": 1,
            "newStart": 1,
            "newLines": 1,
            "diff": "@@ -1,1 +1,1 @@/n-before/n+after/n"
          }
        ]
      }
    },
    {
      "path": "renamed-after.txt",
      "status": "renamed",
      "oldPath": "renamed-before.txt",
      "diff": {
        "type": "patch",
        "hunks": []
      }
    }
  ]
}

"#]]);

    Ok(())
}
