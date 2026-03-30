use crate::utils::{CommandExt as _, Sandbox};
use snapbox::str;

#[test]
fn ai_openai_defaults_to_global_config() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
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

    Ok(())
}

#[test]
fn ai_ollama_local_writes_repo_config() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    env.invoke_bash("git init repo");
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

    Ok(())
}

#[test]
fn ai_global_config_works_outside_repository() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
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

    Ok(())
}

#[test]
fn ai_show_outputs_current_global_configuration_json() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    let global_config = env.projects_root().join("global.gitconfig");

    env.but("config ai openai --key-option butler-api --model gpt-5.4-nano")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success();

    let output = env
        .but("--json config ai show")
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
    let env = Sandbox::empty()?;
    env.invoke_bash("git init repo");
    env.but("-C repo setup").assert().success();

    env.but("-C repo config ai --local ollama --endpoint localhost:11434 --model llama3.1")
        .assert()
        .success();

    let output = env
        .but("-C repo --json config ai --local show")
        .allow_json()
        .output()?;
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    assert_eq!(json["provider"], "ollama");
    assert_eq!(json["ollama_endpoint"], "localhost:11434");
    assert_eq!(json["ollama_model"], "llama3.1");

    Ok(())
}

#[test]
fn ai_show_outputs_current_global_configuration_human() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
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

    Ok(())
}

#[test]
fn ai_openai_byok_without_api_key_fails_non_interactive() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
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
    let env = Sandbox::empty()?;
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
