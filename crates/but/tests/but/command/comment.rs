use crate::utils::{CommandExt as _, Sandbox};

/// The full lifecycle on uncommitted changes: add a comment, see it drift with edits above it,
/// then archive it by id prefix.
#[test]
fn add_list_drift_archive_on_uncommitted_changes() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("src/note.ts", "line one\nline two\nline three\n");

    env.but("comment add src/note.ts:2 -m 'rename this variable'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Added comment
[[..]] src/note.ts:2 (uncommitted)
  rename this variable

"#]]);

    // An edit above the anchored line shifts it from line 2 to line 3; the listing re-anchors.
    env.file("src/note.ts", "line zero\nline one\nline two\nline three\n");
    env.but("comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[[..]] src/note.ts:3 (uncommitted)
  rename this variable
  | +line zero
  | +line one
  | +line two
  | +line three

"#]]);

    let id = single_comment_id(&env)?;
    env.but(format!("comment archive {}", &id[..8]))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Archived comment [..]

"#]]);
    env.but("comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
No comments

"#]]);

    Ok(())
}

/// Committing the file removes it from the uncommitted diff, so the comment is auto-archived.
#[test]
fn commenting_and_committing_the_file_auto_archives() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("src/note.ts", "line one\n");
    env.but("comment add src/note.ts:1 -m 'about to disappear'")
        .assert()
        .success();

    env.but("commit -b A -m 'add note'").assert().success();

    env.but("comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
No comments

"#]]);

    Ok(())
}

/// Comments anchored to a commit's diff via its change id survive listing and carry the commit
/// scope in the output.
#[test]
fn add_and_list_on_a_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // The commit `tpm` of the scenario adds the file `A` with a single line `A`.
    env.but("comment add A:1 --commit tpm -m 'commit-anchored'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Added comment
[[..]] A:1 (commit tpm[..])
  commit-anchored

"#]]);

    env.but("comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[[..]] A:1 (commit tpm[..])
  commit-anchored
  | +A

"#]]);

    Ok(())
}

/// Bad anchors and unknown ids are rejected as bad input.
#[test]
fn rejects_bad_input() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("comment add src/note.ts -m 'no line number'")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'src/note.ts' for '<ANCHOR>'

The anchor must have the form `<path>:<line>`

Hint: For example `src/main.rs:42`

"#]]);

    env.but("comment archive deadbeef")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'deadbeef' for '<ID>'

No unarchived comment with this id

Hint: Use `but comment list` to see the ids of all comments

"#]]);

    // Commenting on a line that is not part of the diff is refused.
    env.file("src/note.ts", "line one\n");
    env.but("comment add src/note.ts:7 -m 'beyond the diff'")
        .assert()
        .failure();
}

fn single_comment_id(env: &Sandbox) -> anyhow::Result<String> {
    let stdout = env
        .but("comment list --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&stdout))?;
    let comments = value["comments"]
        .as_array()
        .expect("list output has a comments array");
    assert_eq!(comments.len(), 1, "exactly one comment in this test");
    Ok(comments[0]["id"]
        .as_str()
        .expect("comment ids are strings")
        .to_string())
}
