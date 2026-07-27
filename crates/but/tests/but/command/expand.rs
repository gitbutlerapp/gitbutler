use snapbox::str;

use crate::utils::{CommandExt as _, Sandbox};

fn expand_env() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env
}

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

    env.but("_expand zz")
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
Matches: 1

commit: 1 e8564e1938a8b7e00c0f3cf88d08f0687d6863d3

"#]]);
    env.but("_expand 1#1")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Matches: 1

commit: 1 71c4380695ab83fc7ff085860bb656ab27ed524e

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
    env.but("discard zz").assert().success();

    // now reword the first to properly diverge
    env.but("reword 123 -m 'rewritten'").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
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

    env.but("--json _expand zz")
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
╭┄ zz [uncommitted] (no changes)
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
fn requires_exactly_one_argument() {
    let env = Sandbox::empty();

    env.but("_expand").assert().failure();
    env.but("_expand zz extra").assert().failure();
}
