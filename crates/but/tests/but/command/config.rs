use crate::utils::{CommandExt as _, Sandbox};
use snapbox::str;

#[cfg(feature = "legacy")]
#[test]
fn target_configures_distinct_push_remote_for_fork() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    env.but("setup").assert().success();
    env.invoke_git("remote add upstream .");
    env.invoke_git("update-ref refs/remotes/upstream/main refs/remotes/origin/main");
    let expected_target_commit_id = env.invoke_git("merge-base HEAD refs/remotes/upstream/main");
    let stale_target_commit_id = env.invoke_git("commit-tree HEAD^{tree} -p HEAD -m stale-target");
    assert_ne!(
        stale_target_commit_id, expected_target_commit_id,
        "the seeded target commit must expose accidental preservation"
    );
    env.invoke_git(&format!(
        "config --local gitbutler.project.targetCommitId {stale_target_commit_id}"
    ));

    env.but("config target upstream/main --push-remote origin")
        .assert()
        .success();

    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.targetRef"),
        "refs/remotes/upstream/main"
    );
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.pushRemote"),
        "origin"
    );
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.targetCommitId"),
        expected_target_commit_id,
        "switching target refs recomputes the merge-base"
    );
}

#[test]
fn ai_openai_defaults_to_global_config() {
    let env = Sandbox::empty();
    env.invoke_bash("git init repo");
    let global_config = env.projects_root().join("global.gitconfig");

    env.but("-C repo config ai openai --key-option butler-api --model gpt-5.4-nano")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success();

    assert_eq!(
        env.invoke_git("config --file global.gitconfig --get gitbutler.aiModelProvider"),
        "openai"
    );
    assert_eq!(
        env.invoke_git("config --file global.gitconfig --get gitbutler.aiOpenAIKeyOption"),
        "butlerAPI"
    );
    assert_eq!(
        env.invoke_git("config --file global.gitconfig --get gitbutler.aiOpenAIModelName"),
        "gpt-5.4-nano"
    );

    env.invoke_git_fails(
        "-C repo config --local --get gitbutler.aiModelProvider",
        "default AI config should not write repo-local keys",
    );
}

#[test]
fn ai_ollama_local_writes_repo_config() {
    let env = Sandbox::empty();
    env.invoke_bash("git init repo");
    #[cfg(feature = "legacy")]
    env.but("-C repo setup").assert().success();

    env.but("-C repo config ai --local ollama --endpoint localhost:11434 --model llama3.1")
        .assert()
        .success();

    assert_eq!(
        env.invoke_git("-C repo config --local --get gitbutler.aiModelProvider"),
        "ollama"
    );
    assert_eq!(
        env.invoke_git("-C repo config --local --get gitbutler.aiOllamaEndpoint"),
        "localhost:11434"
    );
    assert_eq!(
        env.invoke_git("-C repo config --local --get gitbutler.aiOllamaModelName"),
        "llama3.1"
    );
}

#[test]
fn ai_global_config_works_outside_repository() {
    let env = Sandbox::empty();
    let global_config = env.projects_root().join("global.gitconfig");

    env.but("config ai lmstudio --endpoint http://localhost:1234/v1 --model local-model")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success();

    assert_eq!(
        env.invoke_git("config --file global.gitconfig --get gitbutler.aiModelProvider"),
        "lmstudio"
    );
    assert_eq!(
        env.invoke_git("config --file global.gitconfig --get gitbutler.aiLMStudioEndpoint"),
        "http://localhost:1234/v1"
    );
    assert_eq!(
        env.invoke_git("config --file global.gitconfig --get gitbutler.aiLMStudioModelName"),
        "local-model"
    );
}

#[test]
fn ai_show_outputs_current_global_configuration_json() -> anyhow::Result<()> {
    let env = Sandbox::empty();
    let global_config = env.projects_root().join("global.gitconfig");

    env.but("config ai openai --key-option butler-api --model gpt-5.4-nano")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success();

    let output = env
        .but("--format json config ai show")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .allow_json()
        .output()?;
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    assert_eq!(json["provider"], "openai");
    assert_eq!(json["openai_key_option"], "butlerAPI");
    assert_eq!(json["openai_model"], "gpt-5.4-nano");

    Ok(())
}

#[test]
fn ai_show_outputs_current_local_configuration_json() -> anyhow::Result<()> {
    let env = Sandbox::empty();
    env.invoke_bash("git init repo");
    #[cfg(feature = "legacy")]
    env.but("-C repo setup").assert().success();

    env.but("-C repo config ai --local ollama --endpoint localhost:11434 --model llama3.1")
        .assert()
        .success();

    let output = env
        .but("-C repo --format json config ai --local show")
        .allow_json()
        .output()?;
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    assert_eq!(json["provider"], "ollama");
    assert_eq!(json["ollama_endpoint"], "localhost:11434");
    assert_eq!(json["ollama_model"], "llama3.1");

    Ok(())
}

#[test]
fn ai_show_outputs_current_global_configuration_human() {
    let env = Sandbox::empty();
    let global_config = env.projects_root().join("global.gitconfig");

    env.but("config ai openai --key-option butler-api --model gpt-5.4-nano")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success();

    env.but("config ai show")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success()
        .stdout_eq(str![[r#"
AI Configuration (global)

  Provider: openai
  OpenAI key option: butlerAPI
  OpenAI model: gpt-5.4-nano
  OpenAI endpoint: (not set)
  Anthropic key option: (not set)
  Anthropic model: (not set)
  Ollama endpoint: (not set)
  Ollama model: (not set)
  LM Studio endpoint: (not set)
  LM Studio model: (not set)

"#]]);
}

#[test]
fn ai_openai_byok_without_api_key_fails_non_interactive() -> anyhow::Result<()> {
    let env = Sandbox::empty();
    let global_config = env.projects_root().join("global.gitconfig");

    let output = env
        .but("config ai openai --key-option bring-your-own --model gpt-5.4-nano")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .output()?;

    assert!(!output.status.success(), "command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "OpenAI with --key-option bring-your-own requires --api-key or --api-key-env"
        ),
        "unexpected stderr: {stderr}"
    );

    env.invoke_git_fails(
        "config --file global.gitconfig --get gitbutler.aiModelProvider",
        "provider should not be written when BYOK key is missing",
    );

    Ok(())
}

#[test]
fn ai_anthropic_byok_without_api_key_fails_non_interactive() -> anyhow::Result<()> {
    let env = Sandbox::empty();
    let global_config = env.projects_root().join("global.gitconfig");

    let output = env
        .but("config ai anthropic --key-option bring-your-own --model claude-3-5-haiku-latest")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .output()?;

    assert!(!output.status.success(), "command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "Anthropic with --key-option bring-your-own requires --api-key or --api-key-env"
        ),
        "unexpected stderr: {stderr}"
    );

    env.invoke_git_fails(
        "config --file global.gitconfig --get gitbutler.aiModelProvider",
        "provider should not be written when BYOK key is missing",
    );

    Ok(())
}
