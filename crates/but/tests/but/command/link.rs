use crate::utils::Sandbox;

fn parse_stdout_json(output: &[u8]) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::from_slice(output)?)
}

#[test]
fn claim_check_release_happy_path() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    let claim = env
        .but("link --agent-id A claim --path src/app.txt --ttl 15m")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(parse_stdout_json(&claim)?["ok"], true);

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
        blocked["blocking_agents"]
            .as_array()
            .is_some_and(|v| v.iter().any(|a| a == "A"))
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
        "link post --type discovery --json '{\"title\":\"hello\",\"evidence\":[{\"kind\":\"note\",\"detail\":\"x\"}],\"suggested_action\":{\"cmd\":\"but link --agent-id A check --path src/app.txt\"}}' --agent-id A",
    )
        .assert()
        .success();

    let read = env
        .but("link read --agent-id tier4-observer --type discovery --since 0")
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
fn read_defaults_to_full_coordination_snapshot() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link --agent-id A claim --path src/app.txt --ttl 15m")
        .assert()
        .success();
    env.but("link --agent-id A status editing src/app.txt")
        .assert()
        .success();
    env.but("link --agent-id A post working on src/app.txt")
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

    assert_eq!(read["kind"], "all");
    assert!(
        read["messages"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["kind"] == "claim"))
    );
    assert!(
        read["claims"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["path"] == "src/app.txt"))
    );
    assert!(
        read["agents"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["agent_id"] == "A"))
    );

    Ok(())
}

#[test]
fn read_type_all_includes_non_discovery_messages() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("link --agent-id A claim --path src/app.txt --ttl 15m")
        .assert()
        .success();
    env.but("link --agent-id A release --path src/app.txt")
        .assert()
        .success();

    let read = env
        .but("link read --agent-id observer --type all")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let read = parse_stdout_json(&read)?;

    assert!(read["messages"].as_array().is_some_and(|items| {
        items.iter().any(|item| item["kind"] == "claim")
            && items.iter().any(|item| item["kind"] == "release")
    }));

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
