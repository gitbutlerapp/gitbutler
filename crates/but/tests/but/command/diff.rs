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
────────────╮
y:c prefix/a│
────────────╯
     1│+we want this
────────────╮
u:d prefix/b│
────────────╯
     1│+we also want this

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
      "id": "x:b",
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
      "id": "v:5",
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
      "id": "x:4",
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
      "id": "p:b",
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
      "id": "p:b",
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
      "id": "u:a",
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
      "id": "u:6",
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
      "id": "y:c",
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
      "id": "u:d",
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
┊●   1 3f40d29 committed-file-target
┊│     1:k A committed-other.txt
┊│     1:w A committed-target.txt
┊●   9477ae7 add A
┊│     9:t A A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
┊│     d:p A B
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
fn committed_hunk_ids_are_disabled_without_worktree_manipulation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("committed-hunk.txt", "before\n");
    env.but("commit A -m hunk-base").assert().success();
    env.file("committed-hunk.txt", "after\n");
    env.but("commit A -m hunk-source").assert().success();

    let repo = env.open_repo();
    let source = repo.rev_parse_single("A").unwrap().detach();
    let target = repo.rev_parse_single("B").unwrap().detach();
    let hunk = format!("{source}:committed-hunk.txt:#0");

    env.but(format!("rub {hunk} {target}"))
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Rubbed the wrong way. Source '[..]:committed-hunk.txt:#0' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state.

"#]]);

    let repo = env.open_repo();
    assert_eq!(
        repo.rev_parse_single("A").unwrap().detach(),
        source,
        "a disabled committed-hunk action must not rewrite its source"
    );
    assert_eq!(
        repo.rev_parse_single("B").unwrap().detach(),
        target,
        "a disabled committed-hunk action must not rewrite its target"
    );
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
┊●   9477ae7 add A
┊│     9:t A A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
┊│     d:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    let commit_id = "94";

    env.but(format!("diff --format json {commit_id}"))
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
      "id": "n:4",
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
┊   n A unassigned.txt
┊
┊  ╭┄k0 [staged to A]
┊  │ s A assigned.txt
┊  │
┊╭┄g0 [A]
┊●   9477ae7 add A
┊│     9:t A A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
┊│     d:p A B
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
      "id": "s:8",
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
┊●   1#0 dafe86a status-target
┊│     1#0:nx A added.txt
┊│     1#0:nm D deleted.txt
┊│     1#0:u  M modified.txt
┊│     1#0:o  R renamed-after.txt
┊●   1#1 db7d00b status-base
┊│     1#1:n A deleted.txt
┊│     1#1:u A modified.txt
┊│     1#1:z A renamed-before.txt
┊●   9477ae7 add A
┊│     9:t A A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
┊│     d3:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    let commit_id = "da";

    env.but(format!("diff --format json {commit_id}"))
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
