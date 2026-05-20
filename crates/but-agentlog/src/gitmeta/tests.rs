use super::*;
use crate::agent::Agent;
use crate::capture::record_transcript;
use crate::environment::{EnvironmentObservation, ObservedTargets, capture_environment};
use crate::transcript::TranscriptBatch;
use but_core::worktree::safe_checkout;
use git_meta_lib::{MetaEdit, MetaValue, Session, Target};
use gix::object::tree::EntryKind;
use std::{fs, path::Path};
use tempfile::TempDir;

const TEST_SESSION_KEY: &str = "sha256-11111111111111111111111111111111";
const TEST_SOURCE_KEY: &str = "sha256-22222222222222222222222222222222";
const TEST_BRANCH_KEY: &str = "ref:refs/heads/main";
const TEST_REVIEW_KEY: &str = "gitbutler-review:review-1";
const TEST_CHANGE_KEY: &str = "gitbutler-change:change-1";

fn setup_repo() -> TempDir {
    let dir = TempDir::new().expect("temp repo");
    gix::ThreadSafeRepository::init_opts(
        dir.path(),
        gix::create::Kind::WithWorktree,
        gix::create::Options::default(),
        gix::open::Options::isolated().config_overrides(["init.defaultBranch=main"]),
    )
    .expect("gitoxide repo init");
    dir
}

fn commit_file(repo: &Path, path: &str, body: &str) {
    let repo = but_testsupport::open_repo(repo).expect("open gitoxide test repo");
    let parent = repo.head_commit().ok();
    let current_tree_id = parent
        .as_ref()
        .map(|commit| commit.tree_id().expect("head commit tree").detach())
        .unwrap_or(repo.empty_tree().id);
    let mut editor = parent
        .as_ref()
        .map(|commit| commit.tree().expect("head commit tree").edit())
        .unwrap_or_else(|| Ok(repo.empty_tree().edit().expect("empty tree editor")))
        .expect("tree editor");
    let blob_id = repo
        .write_blob(body.as_bytes())
        .expect("write committed file blob");
    editor
        .upsert(path, EntryKind::Blob, blob_id)
        .expect("add committed file to tree");
    let tree_id = editor.write().expect("write committed file tree").detach();
    let parents = parent
        .as_ref()
        .map(|commit| vec![commit.id])
        .unwrap_or_default();
    let commit = repo
        .new_commit("test commit", tree_id, parents)
        .expect("write test commit");
    safe_checkout(current_tree_id, commit.id, &repo, Default::default())
        .expect("checkout test commit");
}

fn write_transcript(repo: &Path, body: &str) -> std::path::PathBuf {
    let path = repo.join("session.jsonl");
    fs::write(&path, body).expect("write transcript");
    path
}

fn project_target() -> Target {
    Target::parse("project").expect("project target")
}

fn capture_project(repo: &Path, agent: Agent, transcript: &Path) -> usize {
    record_transcript(repo, agent, transcript)
        .expect("capture")
        .0
}

fn target_value(repo: &Path, target: &Target, key: &str) -> Option<MetaValue> {
    let session = Session::open(repo).expect("open session");
    session
        .target(target)
        .get_value(key)
        .expect("read GitMeta value")
}

fn project_value(repo: &Path, key: &str) -> Option<MetaValue> {
    target_value(repo, &project_target(), key)
}

fn jsonl(records: impl IntoIterator<Item = serde_json::Value>) -> String {
    let mut output = String::new();
    for record in records {
        output.push_str(&record.to_string());
        output.push('\n');
    }
    output
}

fn codex_fixture() -> String {
    jsonl([
        serde_json::json!({
            "timestamp": "2026-05-07T09:00:00Z",
            "type": "session_meta",
            "payload": {
                "id": "session-1",
                "model_provider": "openai",
                "cli_version": "0.1.0",
            },
        }),
        serde_json::json!({
            "timestamp": "2026-05-07T09:00:01Z",
            "type": "turn_context",
            "payload": {
                "model": "gpt-5.5",
            },
        }),
        serde_json::json!({
            "timestamp": "2026-05-07T09:00:02Z",
            "type": "response_item",
            "payload": {
                "type": "message",
                "turn_id": "turn-1",
                "role": "assistant",
                "content": "Implemented change",
            },
        }),
    ])
}

fn only_session_key(repo: &Path) -> String {
    let index = project_value(repo, "gitbutler:agent-sessions").expect("session index value");
    let MetaValue::Set(values) = index else {
        panic!("expected session index set");
    };
    assert_eq!(values.len(), 1);
    values.into_iter().next().expect("session key")
}

fn only_source_key(repo: &Path, session_key: &str) -> String {
    let session_prefix = format!("gitbutler:agent-session:{session_key}");
    let sources =
        project_value(repo, &format!("{session_prefix}:sources")).expect("source index value");
    let MetaValue::Set(values) = sources else {
        panic!("expected source index set");
    };
    assert_eq!(values.len(), 1);
    values.into_iter().next().expect("source key")
}

fn transcript_entries(repo: &Path, session_key: &str) -> Vec<String> {
    let transcript_value = project_value(
        repo,
        &format!("gitbutler:agent-session:{session_key}:transcript"),
    )
    .expect("transcript list");
    let MetaValue::List(entries) = transcript_value else {
        panic!("expected transcript list");
    };
    entries.into_iter().map(|entry| entry.value).collect()
}

fn record_hashes(repo: &Path, session_key: &str) -> Vec<String> {
    let hashes = project_value(
        repo,
        &format!("gitbutler:agent-session:{session_key}:record-hashes"),
    )
    .expect("record hash set");
    let MetaValue::Set(hashes) = hashes else {
        panic!("expected record hash set");
    };
    hashes.into_iter().collect()
}

fn turn_summaries(repo: &Path, session_key: &str) -> Vec<serde_json::Value> {
    let turns = project_value(
        repo,
        &format!("gitbutler:agent-session:{session_key}:turns"),
    )
    .expect("turn summaries list");
    let MetaValue::List(entries) = turns else {
        panic!("expected turn summaries list");
    };
    entries
        .into_iter()
        .map(|entry| serde_json::from_str(&entry.value).expect("turn summary json"))
        .collect()
}

fn turn_detail(repo: &Path, session_key: &str, turn_key: &str) -> serde_json::Value {
    let detail = project_value(
        repo,
        &format!("gitbutler:agent-session:{session_key}:turn:{turn_key}"),
    )
    .expect("turn detail");
    let MetaValue::String(detail) = detail else {
        panic!("expected turn detail string");
    };
    serde_json::from_str(&detail).expect("turn detail json")
}

fn index_hits(repo: &Path, index_key: &str) -> Vec<serde_json::Value> {
    let value = project_value(repo, index_key).expect("index value");
    let MetaValue::Set(values) = value else {
        panic!("expected index set");
    };
    values
        .into_iter()
        .map(|value| serde_json::from_str(&value).expect("index hit json"))
        .collect()
}

fn add_index_hit(repo: &Path, kind: &str, target_key: &str, session_key: &str, turn_key: &str) {
    let hit = serde_json::to_string(&IndexHit {
        session_key: session_key.to_owned(),
        turn_key: turn_key.to_owned(),
    })
    .expect("index hit json");
    add_index_member(repo, kind, target_key, hit);
}

fn add_index_member(repo: &Path, kind: &str, target_key: &str, member: String) {
    let session = Session::open(repo).expect("open session");
    let target = project_target();
    let key = index_key(kind, target_key);
    let hits = [member];
    session
        .target(&target)
        .apply_edits(vec![MetaEdit::set_add(&key, &hits)])
        .expect("add index hit");
}

fn only_related(repo: &Path, target: RelatedTarget<'_>) -> RelatedSession {
    let mut sessions =
        find_related_sessions_limited(repo, target, None).expect("find related sessions");
    assert_eq!(sessions.len(), 1);
    sessions.pop().expect("related session")
}

fn write_turn_with_targets(repo: &Path, text: &str, observed_targets: ObservedTargets) -> String {
    write_turn_for_session(
        repo,
        TEST_SESSION_KEY,
        TEST_SOURCE_KEY,
        text,
        observed_targets,
    )
}

fn write_turn_for_session(
    repo: &Path,
    session_key: &str,
    source_key: &str,
    text: &str,
    observed_targets: ObservedTargets,
) -> String {
    let transcript = jsonl([serde_json::json!({
        "timestamp": "2026-05-07T09:00:00Z",
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "assistant",
            "content": text,
        },
    })]);
    let batch =
        TranscriptBatch::parse(Agent::Codex, transcript.as_bytes()).expect("parse transcript");
    write_transcript_batch(repo, Agent::Codex, session_key, source_key, batch, || {
        EnvironmentObservation::from_observed_targets_for_testing(observed_targets)
    })
    .expect("write transcript batch");
    turn_summaries(repo, session_key)
        .last()
        .and_then(|turn| turn["turn_key"].as_str())
        .expect("latest turn key")
        .to_owned()
}

#[test]
fn codex_capture_can_be_read_back_and_is_idempotent() {
    let repo = setup_repo();
    let transcript = write_transcript(repo.path(), &codex_fixture());

    let report = capture_project(repo.path(), Agent::Codex, &transcript);
    let report_again = capture_project(repo.path(), Agent::Codex, &transcript);

    assert_eq!(report, 1);
    assert_eq!(report_again, 0);

    let session_key = only_session_key(repo.path());
    let source_key = only_source_key(repo.path(), &session_key);
    let session = Session::open(repo.path()).expect("open session");
    let target = project_target();
    let source_prefix = format!("gitbutler:agent-session:{session_key}:source:{source_key}");
    let source: serde_json::Value = session
        .target(&target)
        .get_record(&source_prefix)
        .expect("read source record")
        .expect("source record");
    assert_eq!(source["agent"], "codex");
    assert_eq!(source["provider"], "openai");
    assert_eq!(source["model"], "gpt-5.5");
    assert_eq!(source["tool-version"], "0.1.0");

    let records = transcript_entries(repo.path(), &session_key)
        .into_iter()
        .map(|entry| {
            serde_json::from_str::<serde_json::Value>(&entry).expect("transcript record json")
        })
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    assert_eq!(record_hashes(repo.path(), &session_key).len(), 1);
    assert_eq!(records[0]["source_key"], source_key);
    assert_eq!(records[0]["kind"], "message");
    assert_eq!(
        records[0]["source_event_kind"],
        "codex:response_item:message"
    );
    assert_eq!(records[0]["role"], "assistant");
    assert_eq!(records[0]["text"], "Implemented change");
    assert_eq!(records[0]["source_record"]["type"], "response_item");
    assert_eq!(records[0]["source_record"]["payload"]["type"], "message");
    assert!(
        !records[0]["source_record"]
            .to_string()
            .contains("Implemented change")
    );

    let turns = turn_summaries(repo.path(), &session_key);
    assert_eq!(turns.len(), 1);
    let turn_key = turns[0]["turn_key"].as_str().expect("turn key");
    assert_eq!(turns[0]["capture_kind"], "backfill");
    assert!(turns[0].get("previous_turn_key").is_none());

    let detail = turn_detail(repo.path(), &session_key, turn_key);
    assert_eq!(detail["schema"], "gitbutler.agent-session-turn.v1");
    assert_eq!(detail["turn_key"], turn_key);
    assert_eq!(detail["session_key"], session_key);
    assert_eq!(detail["source_key"], source_key);
    assert_eq!(detail["capture_kind"], "backfill");
    assert!(detail.get("previous_turn_key").is_none());
    assert_eq!(detail["records"].as_array().expect("turn records").len(), 1);
    assert!(detail["records"][0].get("source_key").is_none());
    assert_eq!(
        detail["records"][0]["record_hash"],
        records[0]["record_hash"]
    );
}

#[test]
fn find_related_sessions_returns_verified_turn_keys() {
    let repo = setup_repo();
    let observed_targets = || {
        ObservedTargets::from_index_keys_for_testing(
            TEST_BRANCH_KEY,
            TEST_REVIEW_KEY,
            TEST_CHANGE_KEY,
        )
    };
    let turn_key = write_turn_with_targets(repo.path(), "Related work", observed_targets());

    let branch = only_related(repo.path(), RelatedTarget::Branch(TEST_BRANCH_KEY));
    assert_eq!(branch.session_key, TEST_SESSION_KEY);
    assert_eq!(branch.turn_keys.as_slice(), std::slice::from_ref(&turn_key));

    for target in [
        RelatedTarget::Review(TEST_REVIEW_KEY),
        RelatedTarget::Change(TEST_CHANGE_KEY),
    ] {
        let related = only_related(repo.path(), target);
        assert_eq!(
            related.turn_keys.as_slice(),
            std::slice::from_ref(&turn_key)
        );
    }

    write_turn_with_targets(repo.path(), "Related follow-up", observed_targets());
    let turn_keys = turn_summaries(repo.path(), TEST_SESSION_KEY)
        .iter()
        .map(|turn| turn["turn_key"].as_str().expect("turn key").to_owned())
        .collect::<Vec<_>>();

    let branch = only_related(repo.path(), RelatedTarget::Branch(TEST_BRANCH_KEY));
    assert_eq!(branch.turn_keys, turn_keys);
    let review = only_related(repo.path(), RelatedTarget::Review(TEST_REVIEW_KEY));
    assert_eq!(review.turn_keys, branch.turn_keys);
}

#[test]
fn find_related_sessions_returns_all_verified_sessions_for_target() {
    let repo = setup_repo();
    let other_session_key = "sha256-33333333333333333333333333333333";
    let other_source_key = "sha256-44444444444444444444444444444444";
    let observed_targets = || {
        ObservedTargets::from_index_keys_for_testing(
            TEST_BRANCH_KEY,
            TEST_REVIEW_KEY,
            TEST_CHANGE_KEY,
        )
    };
    let first_turn_key =
        write_turn_with_targets(repo.path(), "First related work", observed_targets());
    let second_turn_key = write_turn_for_session(
        repo.path(),
        other_session_key,
        other_source_key,
        "Second related work",
        observed_targets(),
    );

    let sessions = find_related_sessions(repo.path(), RelatedTarget::Branch(TEST_BRANCH_KEY))
        .expect("find related sessions");

    assert_eq!(
        sessions,
        vec![
            RelatedSession {
                session_key: TEST_SESSION_KEY.to_owned(),
                turn_keys: vec![first_turn_key],
            },
            RelatedSession {
                session_key: other_session_key.to_owned(),
                turn_keys: vec![second_turn_key],
            },
        ]
    );
}

#[test]
fn find_related_sessions_ignores_stale_index_hits() {
    let repo = setup_repo();
    let stale_branch_key = "ref:refs/heads/stale";
    let turn_key = write_turn_with_targets(
        repo.path(),
        "Related work",
        ObservedTargets::from_index_keys_for_testing(
            TEST_BRANCH_KEY,
            TEST_REVIEW_KEY,
            TEST_CHANGE_KEY,
        ),
    );
    add_index_hit(
        repo.path(),
        "branch",
        stale_branch_key,
        TEST_SESSION_KEY,
        &turn_key,
    );

    let sessions =
        find_related_sessions_limited(repo.path(), RelatedTarget::Branch(stale_branch_key), None)
            .expect("find related sessions");
    assert!(
        sessions.is_empty(),
        "stale index hit must not become evidence"
    );
}

#[test]
fn read_helpers_deduplicate_merged_turn_summaries() {
    let repo = setup_repo();
    let turn_key = write_turn_with_targets(repo.path(), "Merged once", ObservedTargets::default());
    let turns_key = format!("gitbutler:agent-session:{TEST_SESSION_KEY}:turns");
    let Some(MetaValue::List(mut turn_entries)) = project_value(repo.path(), &turns_key) else {
        panic!("expected turn summary list");
    };
    turn_entries.push(turn_entries[0].clone());
    let target = project_target();
    let turns_value = MetaValue::List(turn_entries);
    Session::open(repo.path())
        .expect("open session")
        .target(&target)
        .apply_edits(vec![MetaEdit::set_value(&turns_key, &turns_value)])
        .expect("duplicate stored turn summary");

    let timeline =
        get_session_timeline(repo.path(), TEST_SESSION_KEY, None).expect("read timeline");
    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].turn_key, turn_key);

    let sessions = list_sessions(repo.path(), None).expect("list sessions");
    assert_eq!(sessions[0].turn_count, 1);
    assert_eq!(
        sessions[0].latest_turn_key.as_deref(),
        Some(turn_key.as_str())
    );
}

#[test]
fn list_sessions_returns_recent_session_summaries() {
    let repo = setup_repo();
    assert!(
        list_sessions(repo.path(), None)
            .expect("list empty sessions")
            .is_empty()
    );
    let first_session_key = "sha256-11111111111111111111111111111111";
    let first_source_key = "sha256-22222222222222222222222222222222";
    let second_session_key = "sha256-33333333333333333333333333333333";
    let second_source_key = "sha256-44444444444444444444444444444444";
    let second_source_key_next = "sha256-55555555555555555555555555555555";
    let first_turn_key = write_turn_for_session(
        repo.path(),
        first_session_key,
        first_source_key,
        "First session",
        ObservedTargets::default(),
    );
    write_turn_for_session(
        repo.path(),
        second_session_key,
        second_source_key,
        "Second session",
        ObservedTargets::default(),
    );
    let second_turn_key_next = write_turn_for_session(
        repo.path(),
        second_session_key,
        second_source_key_next,
        "Second session follow-up",
        ObservedTargets::default(),
    );
    set_session_updated_at(repo.path(), first_session_key, "2026-05-07T09:00:00.000Z");
    set_session_updated_at(repo.path(), second_session_key, "2026-05-07T10:00:00.000Z");

    let sessions = list_sessions(repo.path(), None).expect("list sessions");

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].session_key, second_session_key);
    assert_eq!(sessions[0].updated_at, "2026-05-07T10:00:00.000Z");
    assert_eq!(
        sessions[0].source_keys,
        [second_source_key, second_source_key_next]
    );
    assert_eq!(sessions[0].turn_count, 2);
    assert_eq!(
        sessions[0].latest_turn_key.as_deref(),
        Some(second_turn_key_next.as_str())
    );
    assert_eq!(sessions[1].session_key, first_session_key);
    assert_eq!(
        sessions[1].latest_turn_key.as_deref(),
        Some(first_turn_key.as_str())
    );

    let latest = list_sessions(repo.path(), Some(1)).expect("list latest session");
    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0].session_key, second_session_key);
}

#[test]
fn get_related_session_detail_merges_windows_for_multiple_highlighted_turns() {
    let repo = setup_repo();
    let before_turn_key = write_turn_with_targets(
        repo.path(),
        "Before related work",
        ObservedTargets::default(),
    );
    let related_turn_key = write_turn_with_targets(
        repo.path(),
        "Related work",
        ObservedTargets::from_index_keys_for_testing(
            TEST_BRANCH_KEY,
            TEST_REVIEW_KEY,
            TEST_CHANGE_KEY,
        ),
    );
    let middle_turn_key = write_turn_with_targets(
        repo.path(),
        "Between related turns",
        ObservedTargets::default(),
    );
    let next_related_turn_key = write_turn_with_targets(
        repo.path(),
        "Related follow-up",
        ObservedTargets::from_index_keys_for_testing(
            TEST_BRANCH_KEY,
            TEST_REVIEW_KEY,
            TEST_CHANGE_KEY,
        ),
    );
    let after_turn_key = write_turn_with_targets(
        repo.path(),
        "After related work",
        ObservedTargets::default(),
    );

    let detail = get_related_session_detail(
        repo.path(),
        RelatedTarget::Branch(TEST_BRANCH_KEY),
        TEST_SESSION_KEY,
        RelatedTurnWindow {
            context_before: 1,
            context_after: 1,
        },
    )
    .expect("read related session detail")
    .expect("related session detail");

    let turn_window = detail
        .iter()
        .map(|turn| {
            (
                turn.turn.turn_key.as_str(),
                turn.highlighted,
                turn.turn.records[0]["text"].as_str().expect("record text"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        turn_window,
        [
            (before_turn_key.as_str(), false, "Before related work"),
            (related_turn_key.as_str(), true, "Related work"),
            (middle_turn_key.as_str(), false, "Between related turns"),
            (next_related_turn_key.as_str(), true, "Related follow-up"),
            (after_turn_key.as_str(), false, "After related work"),
        ]
    );
}

#[test]
fn get_related_session_detail_returns_none_for_unrelated_target_or_session() {
    let repo = setup_repo();
    write_turn_with_targets(
        repo.path(),
        "Related work",
        ObservedTargets::from_index_keys_for_testing(
            TEST_BRANCH_KEY,
            TEST_REVIEW_KEY,
            TEST_CHANGE_KEY,
        ),
    );
    let missing = get_related_session_detail(
        repo.path(),
        RelatedTarget::Branch("ref:refs/heads/missing"),
        TEST_SESSION_KEY,
        RelatedTurnWindow {
            context_before: 0,
            context_after: 0,
        },
    )
    .expect("read missing related session detail");
    assert!(missing.is_none());

    let wrong_session = get_related_session_detail(
        repo.path(),
        RelatedTarget::Branch(TEST_BRANCH_KEY),
        "sha256-33333333333333333333333333333333",
        RelatedTurnWindow {
            context_before: 0,
            context_after: 0,
        },
    )
    .expect("read wrong-session related detail");
    assert!(wrong_session.is_none());
}

#[test]
fn duplicate_capture_enriches_matching_failed_turn_without_new_records() {
    let repo = setup_repo();
    let branch_index_key = index_key("branch", TEST_BRANCH_KEY);
    let review_index_key = index_key("review", TEST_REVIEW_KEY);
    let change_index_key = index_key("change", TEST_CHANGE_KEY);
    let missing_repo_path = repo.path().join("missing-repo");
    let transcript = codex_fixture();
    let mut grown_transcript = transcript.clone();
    grown_transcript.push_str("{\"timestamp\":\"2026-05-07T09:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"content\":\"Follow-up\"}}\n");
    let batch =
        TranscriptBatch::parse(Agent::Codex, transcript.as_bytes()).expect("parse transcript");

    let failed_environment_write = write_transcript_batch(
        repo.path(),
        Agent::Codex,
        TEST_SESSION_KEY,
        TEST_SOURCE_KEY,
        batch,
        || capture_environment(&missing_repo_path),
    )
    .expect("write failed-environment turn");
    assert_eq!(failed_environment_write.records_written, 1);
    assert!(failed_environment_write.metadata_changed);
    let turns = turn_summaries(repo.path(), TEST_SESSION_KEY);
    let turn_key = turns[0]["turn_key"].as_str().expect("turn key").to_owned();
    assert_eq!(turns[0]["environment_snapshot_status"], "failed");
    assert!(project_value(repo.path(), &branch_index_key).is_none());

    let grown_batch = TranscriptBatch::parse(Agent::Codex, grown_transcript.as_bytes())
        .expect("parse grown transcript");
    write_transcript_batch(
        repo.path(),
        Agent::Codex,
        TEST_SESSION_KEY,
        TEST_SOURCE_KEY,
        grown_batch,
        || capture_environment(&missing_repo_path),
    )
    .expect("write newer failed turn");
    let turns = turn_summaries(repo.path(), TEST_SESSION_KEY);
    let newer_turn_key = turns[1]["turn_key"]
        .as_str()
        .expect("newer turn key")
        .to_owned();
    assert_eq!(turns[1]["environment_snapshot_status"], "failed");

    let batch = TranscriptBatch::parse(Agent::Codex, transcript.as_bytes())
        .expect("parse duplicate transcript");
    let enriched = write_transcript_batch(
        repo.path(),
        Agent::Codex,
        TEST_SESSION_KEY,
        TEST_SOURCE_KEY,
        batch,
        || {
            EnvironmentObservation::from_observed_targets_for_testing(
                ObservedTargets::from_index_keys_for_testing(
                    TEST_BRANCH_KEY,
                    TEST_REVIEW_KEY,
                    TEST_CHANGE_KEY,
                ),
            )
        },
    )
    .expect("enrich failed turn");
    assert_eq!(enriched.records_written, 0);
    assert!(enriched.metadata_changed);

    let turns = turn_summaries(repo.path(), TEST_SESSION_KEY);
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0]["turn_key"], turn_key);
    assert_eq!(turns[1]["turn_key"], newer_turn_key);
    assert_eq!(turns[0]["environment_snapshot_status"], "complete");
    assert_eq!(turns[1]["environment_snapshot_status"], "failed");
    assert_eq!(transcript_entries(repo.path(), TEST_SESSION_KEY).len(), 2);
    assert_eq!(record_hashes(repo.path(), TEST_SESSION_KEY).len(), 2);

    let detail = turn_detail(repo.path(), TEST_SESSION_KEY, &turn_key);
    assert_eq!(detail["environment"]["snapshot_status"], "complete");
    assert!(
        detail["observed_targets"]["branches"]
            .as_array()
            .expect("observed branches")
            .iter()
            .any(|branch| branch["key"] == TEST_BRANCH_KEY),
        "enriched turn stores observed branch target"
    );
    for index_key in [&branch_index_key, &review_index_key, &change_index_key] {
        let hits = index_hits(repo.path(), index_key);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0]["session_key"], TEST_SESSION_KEY);
        assert_eq!(hits[0]["turn_key"], turn_key);
    }

    let grown_batch = TranscriptBatch::parse(Agent::Codex, grown_transcript.as_bytes())
        .expect("parse duplicate grown transcript");
    let enriched = write_transcript_batch(
        repo.path(),
        Agent::Codex,
        TEST_SESSION_KEY,
        TEST_SOURCE_KEY,
        grown_batch,
        || {
            EnvironmentObservation::from_observed_targets_for_testing(
                ObservedTargets::from_index_keys_for_testing(
                    TEST_BRANCH_KEY,
                    TEST_REVIEW_KEY,
                    TEST_CHANGE_KEY,
                ),
            )
        },
    )
    .expect("enrich newer failed turn");
    assert_eq!(enriched.records_written, 0);
    assert!(enriched.metadata_changed);

    let turns = turn_summaries(repo.path(), TEST_SESSION_KEY);
    assert_eq!(turns[0]["environment_snapshot_status"], "complete");
    assert_eq!(turns[1]["environment_snapshot_status"], "complete");
    let hits = index_hits(repo.path(), &branch_index_key);
    assert_eq!(hits.len(), 2);
    assert!(
        hits.iter().any(|hit| hit["turn_key"] == turn_key),
        "older enriched turn remains indexed"
    );
    assert!(
        hits.iter().any(|hit| hit["turn_key"] == newer_turn_key),
        "grown duplicate transcript enriches the newer matching turn"
    );
}

#[test]
fn capture_turn_stores_environment_worktree_paths() {
    let repo = setup_repo();
    commit_file(repo.path(), "src/lib.rs", "pub fn before() {}\n");
    fs::write(repo.path().join("src/lib.rs"), "pub fn after() {}\n").expect("modify source");
    fs::write(repo.path().join("src/new.rs"), "pub fn new() {}\n").expect("write new source");
    let transcript = write_transcript(repo.path(), &codex_fixture());

    assert_eq!(capture_project(repo.path(), Agent::Codex, &transcript), 1);

    let session_key = only_session_key(repo.path());
    let turns = turn_summaries(repo.path(), &session_key);
    let turn_key = turns[0]["turn_key"].as_str().expect("turn key");
    let detail = turn_detail(repo.path(), &session_key, turn_key);
    let worktree_files = detail["environment"]["worktree"]["files"]
        .as_array()
        .expect("worktree files");
    let has_path = |expected| {
        worktree_files
            .iter()
            .any(|path| path.as_str() == Some(expected))
    };
    assert!(
        has_path("src/lib.rs"),
        "worktree snapshot includes tracked modification"
    );
    assert!(
        has_path("src/new.rs"),
        "worktree snapshot includes untracked file"
    );
    assert_ne!(turns[0]["environment_snapshot_status"], "failed");
    assert_eq!(
        turns[0]["environment_snapshot_status"],
        detail["environment"]["snapshot_status"]
    );
    assert_eq!(
        detail["environment"]["worktree"]["file_count"],
        worktree_files.len()
    );
}

#[test]
fn capture_turn_stores_branch_observations_without_synthetic_changes() {
    let repo = setup_repo();
    commit_file(repo.path(), "src/lib.rs", "pub fn committed() {}\n");
    let transcript = write_transcript(repo.path(), &codex_fixture());

    assert_eq!(capture_project(repo.path(), Agent::Codex, &transcript), 1);

    let session_key = only_session_key(repo.path());
    let turns = turn_summaries(repo.path(), &session_key);
    let turn_key = turns[0]["turn_key"].as_str().expect("turn key");
    let detail = turn_detail(repo.path(), &session_key, turn_key);
    let branch = detail["observed_targets"]["branches"]
        .as_array()
        .expect("observed branches")
        .iter()
        .find(|branch| branch["key"] == "ref:refs/heads/main")
        .expect("main branch observation");
    assert_eq!(branch["name"], "main");
    assert!(
        detail["observed_targets"]["changes"]
            .as_array()
            .expect("observed changes")
            .is_empty(),
        "plain Git commits must not synthesize GitButler change targets"
    );
}

#[test]
fn transcript_records_redact_secrets_and_copied_scalar_fields() {
    let repo = setup_repo();
    let session_id = "550e8400-e29b-41d4-a716-446655440000";
    let secret = "Nf9K2pLm8QwEr7TyUi4OzXa3Bv6Cn0Md";
    let record = serde_json::json!({
        "timestamp": "2026-05-07T09:00:00Z",
        "type": "response_item",
        "payload": {
            "type": "message",
            "id": session_id,
            "message_id": session_id,
            "api_key": secret,
            "content": format!("token: {secret}"),
        },
    });
    let transcript = write_transcript(repo.path(), &jsonl([record]));

    let report = capture_project(repo.path(), Agent::Codex, &transcript);

    assert_eq!(report, 1);
    let session_key = only_session_key(repo.path());
    let entries = transcript_entries(repo.path(), &session_key);
    let record: serde_json::Value =
        serde_json::from_str(&entries[0]).expect("transcript record json");
    assert_eq!(record["text"], "token: [REDACTED:entropy]");
    let source_record = record["source_record"].to_string();
    assert!(!source_record.contains(secret));
    assert!(!source_record.contains(session_id));
    assert_eq!(record["source_record"]["type"], "response_item");
    assert_eq!(record["source_record"]["payload"]["type"], "message");
    assert_eq!(
        record["source_record"]["payload"]["api_key"],
        "[REDACTED:entropy]"
    );
    assert!(
        record["source_record"]["payload"]
            .get("content")
            .is_none_or(serde_json::Value::is_null),
        "source_record keeps non-secret structure while pruning copied text"
    );
}

#[test]
fn tool_payloads_are_slimmed_before_storage() {
    let repo = setup_repo();
    let long_output = format!(
        "start:{}:end",
        "x".repeat(MAX_TOOL_RESULT_TEXT_BYTES + 1024)
    );
    let transcript = write_transcript(
        repo.path(),
        &jsonl([
            serde_json::json!({
                "timestamp": "2026-05-07T09:00:00Z",
                "type": "session_meta",
                "payload": { "id": "session-1" },
            }),
            serde_json::json!({
                "timestamp": "2026-05-07T09:00:01Z",
                "type": "response_item",
                "payload": {
                    "type": "function_call",
                    "name": "exec_command",
                    "arguments": "{\"cmd\":\"cargo test\"}",
                },
            }),
            serde_json::json!({
                "timestamp": "2026-05-07T09:00:02Z",
                "type": "response_item",
                "payload": {
                    "type": "function_call_output",
                    "output": long_output,
                },
            }),
            serde_json::json!({
                "timestamp": "2026-05-07T09:00:03Z",
                "type": "response_item",
                "payload": {
                    "type": "function_call_output",
                    "output": {
                        "raw": "x".repeat(MAX_TOOL_RESULT_TEXT_BYTES + 1024),
                    },
                },
            }),
        ]),
    );

    assert_eq!(capture_project(repo.path(), Agent::Codex, &transcript), 3);
    let session_key = only_session_key(repo.path());
    let turns = turn_summaries(repo.path(), &session_key);
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0]["capture_kind"], "backfill");
    let turn_key = turns[0]["turn_key"].as_str().expect("turn key");
    let detail = turn_detail(repo.path(), &session_key, turn_key);
    assert_eq!(detail["records"].as_array().expect("turn records").len(), 3);

    let records = transcript_entries(repo.path(), &session_key)
        .into_iter()
        .map(|entry| {
            serde_json::from_str::<serde_json::Value>(&entry).expect("transcript record json")
        })
        .collect::<Vec<_>>();

    assert_eq!(records[0]["kind"], "tool_call");
    assert_eq!(records[0]["tool_input"]["cmd"], "cargo test");
    assert_eq!(records[1]["kind"], "tool_result");
    let text = records[1]["text"].as_str().expect("tool result text");
    assert!(text.len() <= MAX_TOOL_RESULT_TEXT_BYTES);
    assert!(text.starts_with("start:"));
    assert!(text.contains(TRUNCATION_MARKER.trim()));
    assert!(text.ends_with(":end"));
    assert!(records[1]["source_record"]["payload"]["output"].is_null());
    assert_eq!(records[2]["kind"], "tool_result");
    assert!(records[2]["text"].is_null());
    assert!(records[2]["source_record"]["payload"]["output"].is_null());
}

#[test]
fn transcript_without_capturable_records_does_not_create_session_metadata() {
    let repo = setup_repo();
    let transcript = write_transcript(
        repo.path(),
        &jsonl([
            serde_json::json!({
                "timestamp": "2026-05-07T09:00:00Z",
                "type": "session_meta",
                "payload": { "id": "session-1" },
            }),
            serde_json::json!({
                "timestamp": "2026-05-07T09:00:01Z",
                "type": "event_msg",
                "payload": { "type": "info" },
            }),
        ]),
    );

    let report = capture_project(repo.path(), Agent::Codex, &transcript);

    assert_eq!(report, 0);
    assert!(project_value(repo.path(), "gitbutler:agent-sessions").is_none());
}

#[test]
fn growing_fixture_captures_only_new_records() {
    let repo = setup_repo();
    let mut fixture = codex_fixture();
    let transcript = write_transcript(repo.path(), &fixture);
    capture_project(repo.path(), Agent::Codex, &transcript);

    fixture.push_str("{\"timestamp\":\"2026-05-07T09:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"content\":\"Follow-up\"}}\n");
    fixture.push_str("{\"timestamp\":\"2026-05-07T09:00:04Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"content\":\"Second follow-up\"}}\n");
    fs::write(&transcript, fixture).expect("grow transcript");
    let report = capture_project(repo.path(), Agent::Codex, &transcript);
    let report_again = capture_project(repo.path(), Agent::Codex, &transcript);

    assert_eq!(report, 2);
    assert_eq!(report_again, 0);
    let session_key = only_session_key(repo.path());
    assert_eq!(transcript_entries(repo.path(), &session_key).len(), 3);
    let turns = turn_summaries(repo.path(), &session_key);
    assert_eq!(turns.len(), 2);
    let first_turn_key = turns[0]["turn_key"].as_str().expect("first turn key");
    let second_turn_key = turns[1]["turn_key"].as_str().expect("second turn key");
    assert_ne!(first_turn_key, second_turn_key);
    assert_eq!(turns[1]["capture_kind"], "incremental");
    assert_eq!(turns[1]["previous_turn_key"], first_turn_key);

    let detail = turn_detail(repo.path(), &session_key, second_turn_key);
    assert_eq!(detail["capture_kind"], "incremental");
    assert_eq!(detail["previous_turn_key"], first_turn_key);
    assert_eq!(detail["records"].as_array().expect("turn records").len(), 2);

    let mut fixture = fs::read_to_string(&transcript).expect("read transcript");
    fixture.push_str("{\"timestamp\":\"2026-05-07T09:00:05Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"content\":\"Third follow-up\"}}\n");
    fs::write(&transcript, fixture).expect("grow transcript again");
    assert_eq!(capture_project(repo.path(), Agent::Codex, &transcript), 1);

    let turns = turn_summaries(repo.path(), &session_key);
    assert_eq!(turns.len(), 3);
    assert_eq!(turns[2]["previous_turn_key"], second_turn_key);
}

#[test]
fn incremental_capture_links_previous_turn_by_entry_timestamp() {
    let repo = setup_repo();
    let first_turn_key = write_turn_for_session(
        repo.path(),
        TEST_SESSION_KEY,
        TEST_SOURCE_KEY,
        "First turn",
        ObservedTargets::default(),
    );
    let second_turn_key = write_turn_for_session(
        repo.path(),
        TEST_SESSION_KEY,
        TEST_SOURCE_KEY,
        "Second turn",
        ObservedTargets::default(),
    );
    let turns_key = format!("gitbutler:agent-session:{TEST_SESSION_KEY}:turns");
    let Some(MetaValue::List(mut turn_entries)) = project_value(repo.path(), &turns_key) else {
        panic!("expected turn summary list");
    };
    let mut duplicate_first = turn_entries[0].clone();
    duplicate_first.timestamp = turn_entries
        .iter()
        .map(|entry| entry.timestamp)
        .max()
        .expect("turn timestamp")
        + 1;
    turn_entries.reverse();
    turn_entries.push(duplicate_first);
    let target = project_target();
    let turns_value = MetaValue::List(turn_entries);
    Session::open(repo.path())
        .expect("open session")
        .target(&target)
        .apply_edits(vec![MetaEdit::set_value(&turns_key, &turns_value)])
        .expect("rewrite stored turn order");

    let third_turn_key = write_turn_for_session(
        repo.path(),
        TEST_SESSION_KEY,
        TEST_SOURCE_KEY,
        "Third turn",
        ObservedTargets::default(),
    );

    let turns = turn_summaries(repo.path(), TEST_SESSION_KEY);
    let third_turn = turns
        .iter()
        .find(|turn| turn["turn_key"] == third_turn_key)
        .expect("third turn summary");
    assert_eq!(third_turn["previous_turn_key"], second_turn_key);
    assert_ne!(third_turn["previous_turn_key"], first_turn_key);

    let timeline =
        get_session_timeline_outline(repo.path(), TEST_SESSION_KEY, None).expect("read timeline");
    let timeline_turn_keys = timeline
        .turns
        .iter()
        .map(|turn| turn.turn_key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        timeline_turn_keys,
        [
            first_turn_key.as_str(),
            second_turn_key.as_str(),
            third_turn_key.as_str()
        ]
    );
}

#[test]
fn association_is_idempotent_and_can_link_multiple_targets() {
    let repo = setup_repo();
    commit_file(repo.path(), "src/lib.rs", "pub fn committed() {}\n");
    let transcript = write_transcript(repo.path(), &codex_fixture());
    capture_project(repo.path(), Agent::Codex, &transcript);
    let session_key = only_session_key(repo.path());
    let main_target = Target::branch("main");
    let change_target = Target::change_id("change-123");
    let commit_id = but_testsupport::open_repo(repo.path())
        .expect("open gitoxide repo")
        .head_id()
        .expect("head id")
        .detach()
        .to_string();
    let short_commit_target = Target::commit(&commit_id[..7]).expect("short commit target");
    let full_commit_target = Target::commit(&commit_id).expect("full commit target");

    assert!(associate_session(repo.path(), &main_target, &session_key).expect("associate main"));
    assert!(
        !associate_session(repo.path(), &main_target, &session_key).expect("associate main again"),
        "duplicate association should not report a metadata change"
    );
    assert!(
        associate_session(repo.path(), &change_target, &session_key).expect("associate change")
    );
    assert!(
        associate_session(repo.path(), &short_commit_target, &session_key)
            .expect("associate short commit")
    );

    assert_eq!(
        session_index(repo.path(), &main_target),
        vec![session_key.clone()]
    );
    assert!(
        target_value(
            repo.path(),
            &main_target,
            &format!("gitbutler:agent-session:{session_key}:transcript"),
        )
        .is_none()
    );
    assert_eq!(
        session_index(repo.path(), &change_target),
        vec![session_key.clone()]
    );
    assert_eq!(
        session_index(repo.path(), &full_commit_target),
        vec![session_key]
    );
    assert!(
        target_value(
            repo.path(),
            &short_commit_target,
            "gitbutler:agent-sessions",
        )
        .is_none(),
        "short commit target should resolve before writing association metadata"
    );
}
