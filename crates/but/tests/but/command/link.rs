use std::{thread, time::Duration};

use crate::utils::Sandbox;

fn parse_stdout_json(output: &[u8]) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::from_slice(output)?)
}

#[test]
fn acquire_dry_run_done_happy_path() -> anyhow::Result<()> {
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
        .but("link --agent-id B acquire --path src/app.txt --dry-run")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let blocked = parse_stdout_json(&blocked)?;
    assert_eq!(blocked["dry_run"], true);
    assert_eq!(blocked["decisions"][0]["decision"], "warn");
    assert_eq!(blocked["warn_paths"], serde_json::json!(["src/app.txt"]));
    assert!(
        blocked["decisions"][0]["retry_after_ms"]
            .as_i64()
            .is_some_and(|retry_after_ms| retry_after_ms > 0)
    );
    assert!(
        blocked["decisions"][0]["retry_at_ms"]
            .as_i64()
            .is_some_and(|retry_at_ms| retry_at_ms > 0)
    );
    assert!(
        blocked["decisions"][0]["blocking_claims"]
            .as_array()
            .is_some_and(|claims| claims.iter().any(|claim| claim["agent_id"] == "A"))
    );

    let done = env
        .but("link --agent-id A done \"released src/app.txt\"")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(parse_stdout_json(&done)?["ok"], true);

    let allowed = env
        .but("link --agent-id B acquire --path src/app.txt --dry-run")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let allowed = parse_stdout_json(&allowed)?;
    assert_eq!(allowed["decisions"][0]["decision"], "allow");
    assert!(
        allowed["warn_paths"]
            .as_array()
            .is_some_and(|warn_paths| warn_paths.is_empty())
    );
    assert!(allowed["decisions"][0]["retry_after_ms"].is_null());
    assert!(allowed["decisions"][0]["retry_at_ms"].is_null());

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
        .but("link --agent-id B acquire --path src/app.txt --dry-run")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        parse_stdout_json(&warned)?["decisions"][0]["decision"],
        "warn"
    );
    assert_eq!(
        parse_stdout_json(&warned)?["warn_paths"],
        serde_json::json!(["src/app.txt"])
    );
    assert!(
        parse_stdout_json(&warned)?["decisions"][0]["retry_after_ms"]
            .as_i64()
            .is_some_and(|retry_after_ms| retry_after_ms > 0)
    );

    let denied = env
        .but("link --agent-id B acquire --path src/app.txt --dry-run --strict")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        parse_stdout_json(&denied)?["decisions"][0]["decision"],
        "deny"
    );
    assert!(
        parse_stdout_json(&denied)?["decisions"][0]["retry_after_ms"]
            .as_i64()
            .is_some_and(|retry_after_ms| retry_after_ms > 0)
    );

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

    env.but("link --agent-id Z acquire --path src/a.txt --ttl 15m")
        .assert()
        .success();

    env.but("link acquire --path src/b.txt --ttl 15m --agent-id Z")
        .assert()
        .success();

    Ok(())
}

#[test]
fn typed_discovery_command_is_supported() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but(
        "link discovery hello --evidence x --action \"but link acquire --path src/app.txt --dry-run --agent-id A\" --agent-id A",
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
    assert!(
        items
            .as_array()
            .is_some_and(|arr| arr.iter().any(|entry| entry["kind"] == "discovery"))
    );

    Ok(())
}

#[test]
fn canonical_read_views_work_without_explicit_agent_id() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link --agent-id A acquire --path src/app.txt --ttl 15m")
        .assert()
        .success();

    let claims = env
        .but("link read --view claims")
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
        .but("link read --view agents")
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

    env.but("link --agent-id A acquire --path src/app.txt --ttl 15m")
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
            .is_some_and(|items| items.is_empty())
    );
    assert!(
        read["blocks"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(
        read["surfaces"]
            .as_array()
            .is_some_and(|items| items.is_empty())
    );

    Ok(())
}

#[test]
fn discovery_read_uses_format_field_instead_of_mode() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but(
        "link discovery hello --evidence x --action \"but link acquire --path src/app.txt --dry-run --agent-id A\" --agent-id A",
    )
    .assert()
    .success();

    let full = env
        .but("link read --agent-id observer --view discoveries")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let full = parse_stdout_json(&full)?;
    assert_eq!(full["view"], "discoveries");
    assert_eq!(full["format"], "full");
    assert!(full.get("mode").is_none());

    let brief = env
        .but("link read --agent-id observer --view discoveries --format brief")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let brief = parse_stdout_json(&brief)?;
    assert_eq!(brief["view"], "discoveries");
    assert_eq!(brief["format"], "brief");
    assert!(brief.get("mode").is_none());

    let digest = env
        .but("link read --agent-id observer --view discoveries --format digest")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let digest = parse_stdout_json(&digest)?;
    assert_eq!(digest["view"], "discoveries");
    assert_eq!(digest["format"], "digest");
    assert!(digest.get("mode").is_none());

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
fn read_since_rejects_unsupported_views() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link read --agent-id observer --view claims --since 0")
        .assert()
        .failure()
        .stderr_eq("Error: --since is only supported for --view discoveries or --view messages\n");

    Ok(())
}

#[test]
fn read_since_rejects_non_discovery_formats_for_other_views() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link read --agent-id observer --view messages --since 0 --format digest")
        .assert()
        .failure()
        .stderr_eq("Error: --format only applies to --view discoveries\n");

    Ok(())
}

#[test]
fn read_since_rejects_discovery_formats_other_than_full() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link read --agent-id observer --view discoveries --since 0 --format digest")
        .assert()
        .failure()
        .stderr_eq(
            "Error: --format is not supported with --since; incremental reads always use full payloads\n",
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
        .but("link read --view agents")
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
        .but("link read --view agents")
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
    assert!(agent_after.get("updated_at_ms").is_none());
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

    env.but("link claim --path src/app.txt").assert().failure();
    Ok(())
}

#[test]
fn removed_compatibility_commands_and_flags_fail() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link claims").assert().failure();
    env.but("link agents").assert().failure();
    env.but("link --agent-id A release --path src/app.txt")
        .assert()
        .failure();
    env.but("link read --agent-id A --type all")
        .assert()
        .failure();

    Ok(())
}

#[test]
fn positional_path_arguments_fail() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link --agent-id A acquire src/app.txt --ttl 15m")
        .assert()
        .failure();
    env.but("link --agent-id A block src/app.txt --reason \"shared refactor\"")
        .assert()
        .failure();
    env.but("link --agent-id B ack --agent A src/app.txt --note \"saw it\"")
        .assert()
        .failure();

    Ok(())
}
