use crate::utils::{CommandExt as _, Sandbox};

/// The full lifecycle on uncommitted changes: add a comment, see it drift with edits above it,
/// then archive it by id prefix.
#[test]
fn add_list_drift_archive_on_uncommitted_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("src/note.ts", "line one\nline two\nline three\n");

    env.but(
        "_comment add src/note.ts:2 -m 'rename this variable' --author Sam --author-kind human",
    )
    .assert()
    .success()
    .stdout_eq(snapbox::str![[r#"
Added comment
[[..]] src/note.ts:2 (uncommitted)
  [[..]] Sam (human)
    rename this variable

"#]]);

    // An edit above the anchored line shifts it from line 2 to line 3; the listing re-anchors.
    env.file("src/note.ts", "line zero\nline one\nline two\nline three\n");
    env.but("_comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[[..]] src/note.ts:3 (uncommitted)
  [[..]] Sam (human)
    rename this variable
  | +line zero
  | +line one
  | +line two
  | +line three

"#]]);

    let id = single_comment_id(&env);
    env.but(format!("_comment archive {}", &id[..8]))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Archived comment [..]

"#]]);
    env.but("_comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
No comments

"#]]);
}

/// Committing the file removes it from the uncommitted diff, so the comment is auto-archived —
/// and archiving it afterwards (the typical "agent finishes its fix" race) is a success no-op.
#[test]
fn commenting_and_committing_the_file_auto_archives() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("src/note.ts", "line one\n");
    env.but("_comment add src/note.ts:1 -m 'about to disappear' --author Sam --author-kind human")
        .assert()
        .success();
    let id = single_comment_id(&env);

    env.but("commit -b A -m 'add note'").assert().success();

    env.but("_comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
No comments

"#]]);

    env.but(format!("_comment archive {}", &id[..8]))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Comment [..] was already archived; nothing to do

"#]]);
}

/// Comments anchored to a commit's diff via its change id survive listing and carry the commit
/// scope in the output.
#[test]
fn add_and_list_on_a_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // The commit `tpm` of the scenario adds the file `A` with a single line `A`.
    env.but(
        "_comment add A:1 --commit tpm -m 'commit-anchored' --author Codex --author-kind agent --client-id codex-commit",
    )
    .assert()
    .success()
    .stdout_eq(snapbox::str![[r#"
Added comment
[[..]] A:1 (commit tpm[..])
  [[..]] Codex (agent)
    commit-anchored

"#]]);

    env.but("_comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[[..]] A:1 (commit tpm[..])
  [[..]] Codex (agent)
    commit-anchored
  | +A

"#]]);
}

/// `list --wait` returns immediately with the listing when comments already exist, and reports
/// the timeout when none appear within the bound.
#[test]
fn wait_returns_existing_comments_or_times_out() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("_comment list --wait --timeout 0 --client-id codex-1 --author Codex --title 'Implement foo' --author-kind agent")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
No comments appeared within 0s. Run the same `but _comment list --wait --client-id ... --author ... --author-kind agent` command again to keep waiting.

"#]]);

    env.but("_comment list --wait --timeout 0")
        .assert()
        .failure();

    env.file("src/note.ts", "line one\n");
    env.but("_comment add src/note.ts:1 -m 'already here' --author Sam --author-kind human --mention codex-1")
        .assert()
        .success();

    env.but(
        "_comment list --wait --timeout 0 --client-id codex-1 --author Codex --author-kind agent",
    )
    .assert()
    .success()
    .stdout_eq(snapbox::str![[r#"
[[..]] src/note.ts:1 (uncommitted)
  [[..]] Sam (human)
    already here
  | +line one

"#]]);

    // --timeout only makes sense while waiting.
    env.but("_comment list --timeout 5").assert().failure();
}

/// Mentions invite a stable agent workstream, and explicit receipts prevent a later agent reply
/// from masking an unseen human follow-up.
#[test]
fn invited_agent_receipts_are_explicit_and_durable() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("src/note.ts", "line one\n");
    env.but("_comment add src/note.ts:1 -m 'please rename this' --author Sam --author-kind human --mention codex-1")
        .assert()
        .success();
    let id = single_comment_id(&env);
    let initial_message_id = comment_message_ids(&env)[0].clone();

    env.but(format!(
        "_comment reply {} -m 'Renamed it.' --author Codex --author-kind agent --client-id codex-1 --ack-through {}",
        &id[..8], &initial_message_id[..8]
    ))
    .assert()
    .success()
    .stdout_eq(snapbox::str![[r#"
Replied to comment [..]
  [[..]] Codex (agent)
    Renamed it.

"#]]);

    env.but("_comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[[..]] src/note.ts:1 (uncommitted)
  [[..]] Sam (human)
    please rename this
  [[..]] Codex (agent)
    Renamed it.
  | +line one

"#]]);
    env.but("_comment list --wait --timeout 0 --client-id codex-1 --author Codex --author-kind agent")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
No comments appeared within 0s. Run the same `but _comment list --wait --client-id ... --author ... --author-kind agent` command again to keep waiting.

"#]]);

    env.but(format!(
        "_comment reply {} -m 'One more thing.' --author Sam --author-kind human",
        &id[..8]
    ))
    .assert()
    .success();
    let human_follow_up_id = comment_message_ids(&env)[2].clone();

    // A completion posted without acknowledging the human follow-up cannot hide it.
    env.but(format!(
        "_comment reply {} -m 'Finished everything.' --author Codex --author-kind agent --client-id codex-1",
        &id[..8]
    ))
    .assert()
    .success();
    let agent_wait = env
        .but("_comment list --wait --timeout 0 --client-id codex-1 --author Codex --author-kind agent")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&agent_wait).contains("One more thing."),
        "the human reply is actionable to the agent client"
    );

    env.but(format!(
        "_comment ack {} --message {} --client-id codex-1",
        &id[..8],
        &human_follow_up_id[..8]
    ))
    .assert()
    .success();
    env.but("_comment list --wait --timeout 0 --client-id codex-1 --author Codex --author-kind agent")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
No comments appeared within 0s. Run the same `but _comment list --wait --client-id ... --author ... --author-kind agent` command again to keep waiting.

"#]]);

    let stdout = env
        .but("_comment list --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(
        value["comments"][0]["messages"][1]["payload"],
        "Renamed it."
    );
    assert_eq!(value["comments"][0]["messages"][2]["authorKind"], "human");
    assert_eq!(value["comments"][0]["agentParticipantIds"][0], "codex-1");
}

/// A comment written by another process unblocks a wait that is already sleeping.
#[test]
fn wait_unblocks_when_a_comment_appears_mid_wait() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("src/note.ts", "line one\n");

    let add = env
        .but("_comment add src/note.ts:1 -m 'mid-wait arrival' --author Sam --author-kind human --mention codex-1");
    let wait = env.but("_comment list --wait --timeout 30 --client-id codex-1 --author Codex --title 'Waiting test' --author-kind agent");
    std::thread::scope(|scope| {
        scope.spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            add.assert().success();
        });
        wait.assert().success().stdout_eq(snapbox::str![[r#"
[[..]] src/note.ts:1 (uncommitted)
  [[..]] Sam (human)
    mid-wait arrival
  | +line one

"#]]);
    });
}

/// Comments created by the GUI's gutter click start with an empty payload; agents must not see
/// them until text is typed.
#[test]
fn blank_comments_are_hidden_from_the_cli() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("src/note.ts", "line one\n");
    env.but("_comment add src/note.ts:1 -m '' --author Sam --author-kind human --mention codex-1")
        .assert()
        .success();

    env.but("_comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
No comments

"#]]);
    env.but("_comment list --wait --timeout 0 --client-id codex-1 --author Codex --author-kind agent")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
No comments appeared within 0s. Run the same `but _comment list --wait --client-id ... --author ... --author-kind agent` command again to keep waiting.

"#]]);
}

/// Unapplying the branch that holds a commented commit hides the comment instead of destroying
/// it: re-applying the branch brings it back.
#[test]
fn comments_on_unapplied_branches_survive() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("_comment add A:1 --commit tpm -m 'still wanted' --author Sam --author-kind human")
        .assert()
        .success();

    env.but("unapply A").assert().success();
    env.but("_comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
No comments

"#]]);

    env.but("apply A").assert().success();
    env.but("_comment list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[[..]] A:1 (commit tpm[..])
  [[..]] Sam (human)
    still wanted
  | +A

"#]]);
}

/// Bad anchors and unknown ids are rejected as bad input.
#[test]
fn rejects_bad_input() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("_comment add src/note.ts -m 'no line number' --author Sam --author-kind human")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'src/note.ts' for '<ANCHOR>'

The anchor must have the form `<path>:<line>`

Hint: For example `src/main.rs:42`

"#]]);

    env.but("_comment archive deadbeef")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'deadbeef' for '<ID>'

No comment with this id

Hint: Use `but _comment list` to see the ids of all comments

"#]]);

    env.but(
        "_comment reply deadbeef -m nope --author Codex --author-kind agent --client-id codex-1",
    )
    .assert()
    .failure()
    .stderr_eq(snapbox::str![[r#"
Error: Bad input 'deadbeef' for '<ID>'

No comment with this id

Hint: Use `but _comment list` to see the ids of all comments

"#]]);

    // Commenting on a line that is not part of the diff is refused.
    env.file("src/note.ts", "line one\n");
    env.but("_comment add src/note.ts:7 -m 'beyond the diff' --author Sam --author-kind human")
        .assert()
        .failure();
}

fn single_comment_id(env: &Sandbox) -> String {
    let stdout = env
        .but("_comment list --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&stdout)).unwrap();
    let comments = value["comments"]
        .as_array()
        .expect("list output has a comments array");
    assert_eq!(comments.len(), 1, "exactly one comment in this test");
    comments[0]["id"]
        .as_str()
        .expect("comment ids are strings")
        .to_string()
}

fn comment_message_ids(env: &Sandbox) -> Vec<String> {
    let stdout = env
        .but("_comment list --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
    value["comments"][0]["messages"]
        .as_array()
        .expect("the comment has messages")
        .iter()
        .map(|message| {
            message["id"]
                .as_str()
                .expect("message ids are strings")
                .to_string()
        })
        .collect()
}
