use std::{thread, time::Duration};

use crate::utils::Sandbox;

fn parse_stdout_json(output: &[u8]) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::from_slice(output)?)
}

#[test]
fn acquire_claim_release_happy_path() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    let acquire = env
        .but("link --agent-id A acquire --path src/app.txt --ttl 15m")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let acquire = parse_stdout_json(&acquire)?;
    assert_eq!(
        acquire["acquired_paths"],
        serde_json::json!(["src/app.txt"])
    );

    let blocked = env
        .but("link --agent-id B check --path src/app.txt")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let blocked = parse_stdout_json(&blocked)?;
    assert_eq!(blocked["decision"], "warn");
    assert!(
        blocked["blocking_claims"]
            .as_array()
            .is_some_and(|claims| { claims.iter().any(|claim| claim["agent_id"] == "A") })
    );

    let release = env
        .but("link --agent-id A release --path src/app.txt")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(parse_stdout_json(&release)?["ok"], true);

    let allowed = env
        .but("link --agent-id B check --path src/app.txt")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(parse_stdout_json(&allowed)?["decision"], "allow");

    Ok(())
}

#[test]
fn advisory_block_denies_only_in_strict_mode() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but(
        "link --agent-id A block --path src/app.txt --reason \"shared refactor\" --mode advisory",
    )
    .assert()
    .success();

    let warned = env
        .but("link --agent-id B check --path src/app.txt")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(parse_stdout_json(&warned)?["decision"], "warn");

    let denied = env
        .but("link --agent-id B check --path src/app.txt --strict")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(parse_stdout_json(&denied)?["decision"], "deny");

    let acquire = env
        .but("link --agent-id B acquire --path src/app.txt --ttl 15m")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let acquire = parse_stdout_json(&acquire)?;
    assert_eq!(
        acquire["acquired_paths"],
        serde_json::json!(["src/app.txt"])
    );

    Ok(())
}

#[test]
fn accepts_agent_id_before_and_after_subcommand() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link --agent-id Z claim --path src/a.txt --ttl 15m")
        .assert()
        .success();

    env.but("link claim --path src/b.txt --ttl 15m --agent-id Z")
        .assert()
        .success();

    Ok(())
}

#[test]
fn structured_post_json_payload_is_supported() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but(
        "link post --type discovery '{\"title\":\"hello\",\"evidence\":[{\"kind\":\"note\",\"detail\":\"x\"}],\"suggested_action\":{\"cmd\":\"but link --agent-id A check --path src/app.txt\"}}' --agent-id A",
    )
    .assert()
    .success();

    let read = env
        .but("link read --agent-id tier4-observer --view discoveries --since 0")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let items = parse_stdout_json(&read)?;
    assert!(items.as_array().is_some_and(|arr| !arr.is_empty()));
    assert!(
        items
            .as_array()
            .is_some_and(|arr| arr.iter().any(|entry| entry["kind"] == "discovery"))
    );

    Ok(())
}

#[test]
fn observer_commands_work_without_explicit_agent_id() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link --agent-id A claim --path src/app.txt --ttl 15m")
        .assert()
        .success();

    let claims = env
        .but("link claims")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let claims_json = parse_stdout_json(&claims)?;
    assert!(
        claims_json
            .get("claims")
            .and_then(|v| v.as_array())
            .is_some_and(|arr| !arr.is_empty())
    );

    let agents = env
        .but("link agents")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let agents_json = parse_stdout_json(&agents)?;
    assert!(
        agents_json
            .get("agents")
            .and_then(|v| v.as_array())
            .is_some_and(|arr| !arr.is_empty())
    );

    Ok(())
}

#[test]
fn claims_accepts_absolute_in_repo_path_prefix() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link --agent-id A claim --path src/app.txt --ttl 15m")
        .assert()
        .success();

    let absolute_prefix = env.projects_root().join("src");
    let claims = env
        .but("")
        .arg("link")
        .arg("claims")
        .arg("--path-prefix")
        .arg(&absolute_prefix)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let claims_json = parse_stdout_json(&claims)?;
    assert!(
        claims_json
            .get("claims")
            .and_then(|v| v.as_array())
            .is_some_and(|arr| arr.iter().any(|item| item["path"] == "src/app.txt"))
    );

    Ok(())
}

#[test]
fn read_defaults_to_inbox_snapshot() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link --agent-id A acquire --path src/app.txt --ttl 15m")
        .assert()
        .success();
    env.but("link --agent-id A status editing src/app.txt")
        .assert()
        .success();
    env.but("link --agent-id B post \"@observer: please review src/app.txt\"")
        .assert()
        .success();
    env.but(
        "link --agent-id C block --path src/other.txt --reason \"shared refactor\" --mode advisory",
    )
    .assert()
    .success();

    let read = env
        .but("link read --agent-id observer")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let read = parse_stdout_json(&read)?;

    assert_eq!(read["view"], "inbox");
    assert!(
        read["mentions_or_directed_updates"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(
        read["open_blocks_relevant_to_me"]
            .as_array()
            .is_some_and(|items| items.is_empty())
    );
    assert!(
        read["recent_advisories"]
            .as_array()
            .is_some_and(|items| items.is_empty())
    );

    Ok(())
}

#[test]
fn read_view_full_includes_transcript_and_blocks() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link --agent-id A claim --path src/app.txt --ttl 15m")
        .assert()
        .success();
    env.but("link --agent-id A block --path src/app.txt --reason \"shared refactor\" --mode hard")
        .assert()
        .success();

    let read = env
        .but("link read --agent-id observer --view full")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let read = parse_stdout_json(&read)?;

    assert_eq!(read["view"], "full");
    assert!(
        read["messages"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["kind"] == "block"))
    );
    assert!(
        read["blocks"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );

    Ok(())
}

#[test]
fn typed_ack_is_recorded_and_visible_in_inbox() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but(
        "link --agent-id A block --path src/app.txt --reason \"shared refactor\" --mode advisory",
    )
    .assert()
    .success();
    env.but("link --agent-id B ack --agent A --path src/app.txt --note \"saw it\"")
        .assert()
        .success();

    let read = env
        .but("link read --agent-id A")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let read = parse_stdout_json(&read)?;
    assert!(
        read["mentions_or_directed_updates"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["kind"] == "ack"))
    );

    Ok(())
}

#[test]
fn resolve_is_directed_back_to_block_owner() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    let block = env
        .but("link --agent-id A block --path src/app.txt --reason \"shared refactor\" --mode advisory")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let block = parse_stdout_json(&block)?;
    let block_id = block["block_id"]
        .as_i64()
        .expect("block id must be present");

    env.but(format!("link --agent-id B resolve --block-id {block_id}"))
        .assert()
        .success();

    let read = env
        .but("link read --agent-id A")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let read = parse_stdout_json(&read)?;
    assert!(
        read["mentions_or_directed_updates"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["kind"] == "resolve"))
    );

    Ok(())
}

#[test]
fn failed_progress_command_does_not_refresh_last_progress() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link --agent-id A post hello").assert().success();
    let agents_before = env
        .but("link agents")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let agents_before = parse_stdout_json(&agents_before)?;
    let agent_before = agents_before["agents"]
        .as_array()
        .and_then(|agents| agents.iter().find(|agent| agent["agent_id"] == "A"))
        .expect("agent A must exist after a successful post");
    let before_progress = agent_before["last_progress_at_ms"]
        .as_i64()
        .expect("progress timestamp must be present");
    let before_seen = agent_before["last_seen_at_ms"]
        .as_i64()
        .expect("seen timestamp must be present");

    thread::sleep(Duration::from_millis(20));

    env.but("link --agent-id A resolve --block-id 999")
        .assert()
        .failure();

    let agents_after = env
        .but("link agents")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let agents_after = parse_stdout_json(&agents_after)?;
    let agent_after = agents_after["agents"]
        .as_array()
        .and_then(|agents| agents.iter().find(|agent| agent["agent_id"] == "A"))
        .expect("agent A must still exist after the failed resolve");
    let after_progress = agent_after["last_progress_at_ms"]
        .as_i64()
        .expect("progress timestamp must be present");
    let after_seen = agent_after["last_seen_at_ms"]
        .as_i64()
        .expect("seen timestamp must be present");

    assert_eq!(after_progress, before_progress);
    assert!(after_seen >= before_seen);

    Ok(())
}

#[test]
fn link_tui_non_tty_fails_fast() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link --agent-id A post hello").assert().success();

    let stdout = env
        .but("link tui")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let out = parse_stdout_json(&stdout)?;
    assert_eq!(out["ok"], false);

    Ok(())
}

#[test]
fn invalid_link_command_returns_json_error_and_nonzero() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    let stdout = env
        .but("link claim --path src/app.txt")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let out = parse_stdout_json(&stdout)?;
    assert_eq!(out["ok"], false);
    assert!(
        out["error"]
            .as_str()
            .is_some_and(|error| !error.trim().is_empty())
    );

    Ok(())
}
