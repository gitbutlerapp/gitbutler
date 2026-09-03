use snapbox::str;

use crate::utils::{CommandExt as _, Sandbox};

fn expand_env() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env
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
fn resolves_cli_id_atom() {
    let env = expand_env();

    env.but("_expand A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Matches: 1

branch: g0 A

"#]]);

    env.but("_expand @")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Matches: 1

uncommitted area

"#]]);
}

#[test]
fn reports_no_matches() {
    let env = expand_env();

    env.but("_expand does-not-exist")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Matches: 0


"#]]);
}

#[test]
fn resolves_duplicated_change_ids() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("commit -m first").assert().success();
    env.but("commit -m second").assert().success();

    env.but("_expand 1#0")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Matches: 0


"#]]);
    env.but("_expand 1#1")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Matches: 0


"#]]);
}

#[test]
fn resolves_distinct_change_id_prefixes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    set_change_id(&env, "123");
    env.but("commit -m first").assert().success();

    set_change_id(&env, "132");
    env.but("commit -m second").assert().success();

    env.but("_expand 12").assert().success().stdout_eq(str![[r#"
Matches: 1

commit: 123 a8954d4c8daf2bc64ad7a62a33dbbad7a920bdb7

"#]]);
    env.but("_expand 13").assert().success().stdout_eq(str![[r#"
Matches: 1

commit: 132 ea3b1e3ff9f628d463fc5d66a20a9d523fb9a95b

"#]]);
}

/// It's important for usability that change IDs on remote commits do not interfere with change IDs
/// on local commits. At the time of writing this test we don't include change IDs for remote
/// commits in ID resolution, but if we do in the future we should take care to put them in a
/// separate namespace from local commits.
#[test]
fn changing_pushed_commit_does_not_cause_change_id_ambiguity() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    set_change_id(&env, "123");
    env.file("file", "some content");
    env.but("commit -m first").assert().success();

    env.file("file", "some other content");
    env.but("commit -m second").assert().success();

    env.invoke_git("update-ref refs/remotes/origin/a-branch-1 refs/heads/a-branch-1");
    env.invoke_git("config branch.a-branch-1.remote origin");
    env.invoke_git("config branch.a-branch-1.merge refs/heads/a-branch-1");

    // Undo to before the second commit
    env.but("undo").assert().success();
    env.but("discard @").assert().success();

    // now reword the first to properly diverge
    env.but("reword 123 -m 'rewritten'").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊┊
┊╭┄┄ (upstream: on origin/a-branch-1)
┊●   a5caff1 second
┊-
┊◐   123 rewritten
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    assert_eq!(
        env.invoke_git("rev-list --count refs/remotes/origin/a-branch-1 ^refs/heads/a-branch-1"),
        "2",
        "remote should have two divergent commits with the local commit's change ID"
    );

    // This should still unambiguously refer to the one local commit with that change ID
    env.but("_expand 123")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Matches: 1

commit: 123 96b6213[..]

"#]]);
}

fn set_change_id(env: &Sandbox, change_id: &str) {
    env.invoke_git(&format!(
        "config --local gitbutler.testing.changeId {change_id}"
    ));
}

#[test]
fn supports_json_output() {
    let env = expand_env();

    env.but("--json _expand @")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "matches": 1,
  "resources": [
    {
      "type": "uncommitted"
    }
  ]
}

"#]]);
}

/// Branch short IDs are allowed to be prefixes of other short IDs. This requires us to prioritize
/// resolving exact matches on branch short IDs over those other IDs, or we can have cases where
/// branch short IDs simply cannot be resolved.
#[test]
fn exact_match_on_branch_short_id_must_prioritize_branch() {
    let env = expand_env();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new tp-branch").assert().success();

    env.but("_expand tp")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Matches: 1

branch: tp tp-branch

"#]]);
}

#[test]
fn resolves_committed_hunk() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "content");

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────╮
 qs:7 file │
───────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +content

"#]]);

    env.but("commit -m 'Add file'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit oln on new branch 'a-branch-1'

"#]]);

    env.but("_expand 1:qs:7")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Matches: 0


"#]]);
}

#[test]
fn resolves_binary_committed_hunk() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("image.png", PNG_BINARY_CONTENT);

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────────╮
 nx:q image.png │
────────────────╯

No diff available - file is either empty, binary, or too large

"#]]);

    env.but("commit -m 'Add binary file'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit nul on new branch 'a-branch-1'

"#]]);

    env.but("_expand 1:nx:q")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Matches: 0


"#]]);
}

#[test]
fn identical_committed_hunks_qualified_by_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let repeated_content = "line\n".repeat(10);

    env.file("file", format!("{repeated_content}{repeated_content}"));

    env.but("commit -m 'Add file'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit lwk on new branch 'a-branch-1'

"#]]);

    env.file(
        "file",
        format!("{repeated_content}new-line\n{repeated_content}"),
    );

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────╮
 qs:b file │
───────────╯

@@ -8,6 +8,7 @@
───────────────
 8 ┊  8 │  line
 9 ┊  9 │  line
10 ┊ 10 │  line
   ┊ 11 │ +new-line
11 ┊ 12 │  line
12 ┊ 13 │  line
13 ┊ 14 │  line

"#]]);

    env.but("commit -m 'Add new line'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit kkx on branch 'a-branch-1'

"#]]);

    // revert to original state so we can get _exactly_ the same hunk again
    env.file("file", format!("{repeated_content}{repeated_content}"));
    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────╮
 qs:2 file │
───────────╯

@@ -8,7 +8,6 @@
───────────────
 8 ┊  8 │  line
 9 ┊  9 │  line
10 ┊ 10 │  line
11 ┊    │ -new-line
12 ┊ 11 │  line
13 ┊ 12 │  line
14 ┊ 13 │  line

"#]]);
    env.but("commit -m 'Revert'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit nsu on branch 'a-branch-1'

"#]]);

    env.file(
        "file",
        format!("{repeated_content}new-line\n{repeated_content}"),
    );
    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────╮
 qs:b file │
───────────╯

@@ -8,6 +8,7 @@
───────────────
 8 ┊  8 │  line
 9 ┊  9 │  line
10 ┊ 10 │  line
   ┊ 11 │ +new-line
11 ┊ 12 │  line
12 ┊ 13 │  line
13 ┊ 14 │  line

"#]]);

    env.but("commit -m 'Add new line'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit twz on branch 'a-branch-1'

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊● twz author 2000-01-01 00:00:00 +0000 (sha 6a6b0b9)
┊│     Add new line
┊│     twz:q M file
┊● nsu author 2000-01-01 00:00:00 +0000 (sha d2b566a)
┊│     Revert
┊│     nsu:q M file
┊● kkx author 2000-01-01 00:00:00 +0000 (sha 55ea192)
┊│     Add new line
┊│     kkx:q M file
┊● lwk author 2000-01-01 00:00:00 +0000 (sha 50ebd78)
┊│     Add file
┊│     lwk:q A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // Without qualifying the collision ID for the change IDs, we get both commits that add the
    // new-line text
    env.but("_expand 1:qs:b")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Matches: 0


"#]]);

    // Can selectively get the tip commit's hunk
    env.but("_expand 1#0:qs:b")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Matches: 0


"#]]);

    // Can selectively get the older commit's hunk
    env.but("_expand 1#2:qs:b")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Matches: 0


"#]]);
}

#[test]
fn resolves_committed_hunk_id_duplicates() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let repeated_content = "line\n".repeat(10);

    env.file(
        "file",
        format!("{repeated_content}{repeated_content}{repeated_content}"),
    );

    env.but("commit -m 'Add file'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit plv on new branch 'a-branch-1'

"#]]);
    env.file(
        "file",
        format!("{repeated_content}new_line\n{repeated_content}new_line\n{repeated_content}"),
    );

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────────╮
 qs:8#0-2 file │
───────────────╯

@@ -8,6 +8,7 @@
───────────────
 8 ┊  8 │  line
 9 ┊  9 │  line
10 ┊ 10 │  line
   ┊ 11 │ +new_line
11 ┊ 12 │  line
12 ┊ 13 │  line
13 ┊ 14 │  line

───────────────╮
 qs:8#1-2 file │
───────────────╯

@@ -18,6 +19,7 @@
─────────────────
18 ┊ 19 │  line
19 ┊ 20 │  line
20 ┊ 21 │  line
   ┊ 22 │ +new_line
21 ┊ 23 │  line
22 ┊ 24 │  line
23 ┊ 25 │  line

"#]]);

    env.but("commit -m 'Edit file'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit luo on branch 'a-branch-1'

"#]]);

    env.file("file", "");
    env.but("commit -m 'Delete content'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   pzr Delete content
┊│     pzr:q M file
┊●   luo Edit file
┊│     luo:q M file
┊●   plv Add file
┊│     plv:q A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_expand 1#1:qs:8")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Matches: 0


"#]]);

    env.but("_expand 1#1:qs:8#0-2")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Matches: 0


"#]]);

    env.but("_expand 1#1:qs:8#1-2")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Matches: 0


"#]]);
}

#[test]
fn requires_exactly_one_argument() {
    let env = Sandbox::empty();

    env.but("_expand").assert().failure();
    env.but("_expand @ extra").assert().failure();
}
