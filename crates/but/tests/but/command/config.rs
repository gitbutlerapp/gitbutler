use crate::utils::{CommandExt as _, Sandbox};
use snapbox::str;

#[cfg(feature = "legacy")]
#[test]
fn github_stacks_configuration_is_repository_local() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");

    env.but("--json config forge github-stacks")
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "mode": "auto"
}

"#]]);
    env.but("config forge github-stacks enable")
        .assert()
        .success()
        .stdout_eq(str![[r#"
✓ Native GitHub stacks are enabled for this repository
The repository must be enrolled in GitHub's stacked pull requests preview.

"#]]);
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.githubStackingMode"),
        "native"
    );
    env.but("--json config forge github-stacks")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "mode": "native"
}

"#]]);
    env.but("--json config forge github-stacks disable")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "mode": "disabled"
}

"#]]);
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.githubStackingMode"),
        "disabled"
    );
    env.but("config forge github-stacks auto")
        .assert()
        .success()
        .stdout_eq(str![[r#"
✓ Native GitHub stacks are automatic for this repository
They are used when the repository is enrolled in GitHub's stacked pull requests preview.

"#]]);
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.githubStackingMode"),
        "auto",
        "selecting auto should persist it in repository-local git config"
    );
}

#[cfg(feature = "legacy")]
#[test]
fn github_stacks_configuration_requires_a_repository() {
    Sandbox::empty()
        .but("config forge github-stacks enable")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: No git repository found at .
Please run 'but setup' to initialize the project.

"#]]);
}

#[cfg(feature = "legacy")]
#[test]
fn target_configures_distinct_push_remote_for_fork() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    env.but("setup").assert().success();
    env.invoke_git("remote add upstream .");
    env.invoke_git("update-ref refs/remotes/upstream/main refs/remotes/origin/main");

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
}

#[test]
fn config_push_remote_sets_push_remote() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.invoke_git("remote add fork .");

    env.but("config push-remote fork")
        .assert()
        .success()
        .stdout_eq(str![[r#"
✓ Push remote set to 'fork'

"#]]);

    assert_eq!(
        env.project_meta().push_remote.as_deref(),
        Some("fork"),
        "the configured push remote should be persisted in project metadata"
    );
}

#[test]
fn config_push_remote_rejects_unknown_remote_without_changing_metadata() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    let before = env.project_meta();

    env.but("config push-remote missing")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: failed to find remote missing

Caused by:
    The remote named "missing" did not exist

"#]]);
    assert_eq!(
        env.project_meta(),
        before,
        "a rejected remote must not alter project metadata"
    );
}

#[test]
fn config_push_remote_shows_effective_remote() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    env.but("config push-remote")
        .assert()
        .success()
        .stdout_eq(str![[r#"

Push Remote:

  origin

"#]]);
}

#[test]
fn feature_config_shell_output_uses_valid_identifiers() {
    let env = Sandbox::empty();

    env.but("config feature")
        .assert()
        .success()
        .stdout_eq(str![[r#"

Feature Flags:

  single-branch: enabled

"#]]);
}

#[test]
fn feature_config_json_output_uses_stable_key() {
    let env = Sandbox::empty();

    env.but("--json config feature single-branch")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "single_branch": true
}

"#]]);
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
fn ai_show_outputs_current_global_configuration_json() {
    let env = Sandbox::empty();
    let global_config = env.projects_root().join("global.gitconfig");

    env.but("config ai openai --key-option butler-api --model gpt-5.4-nano")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success();

    let output = env
        .but("--json config ai show")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .allow_json()
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(json["provider"], "openai");
    assert_eq!(json["openai_key_option"], "butlerAPI");
    assert_eq!(json["openai_model"], "gpt-5.4-nano");
}

#[test]
fn ai_show_outputs_current_local_configuration_json() {
    let env = Sandbox::empty();
    env.invoke_bash("git init repo");
    #[cfg(feature = "legacy")]
    env.but("-C repo setup").assert().success();

    env.but("-C repo config ai --local ollama --endpoint localhost:11434 --model llama3.1")
        .assert()
        .success();

    let output = env
        .but("-C repo --json config ai --local show")
        .allow_json()
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(json["provider"], "ollama");
    assert_eq!(json["ollama_endpoint"], "localhost:11434");
    assert_eq!(json["ollama_model"], "llama3.1");
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
fn ai_openai_byok_without_api_key_fails_non_interactive() {
    let env = Sandbox::empty();
    let global_config = env.projects_root().join("global.gitconfig");

    let output = env
        .but("config ai openai --key-option bring-your-own --model gpt-5.4-nano")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .output()
        .unwrap();

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
}

#[test]
fn ai_anthropic_byok_without_api_key_fails_non_interactive() {
    let env = Sandbox::empty();
    let global_config = env.projects_root().join("global.gitconfig");

    let output = env
        .but("config ai anthropic --key-option bring-your-own --model claude-3-5-haiku-latest")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .output()
        .unwrap();

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
}
