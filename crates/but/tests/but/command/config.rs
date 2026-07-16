#[cfg(feature = "legacy")]
use std::fs;

#[cfg(feature = "legacy")]
use but_core::RepositoryExt as _;

use crate::utils::{CommandExt as _, Sandbox};
use snapbox::str;

#[cfg(feature = "legacy")]
#[test]
fn target_configures_distinct_push_remote_for_fork() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    env.but("setup").assert().success();
    env.invoke_git("remote add upstream .");
    env.invoke_git("update-ref refs/remotes/upstream/main refs/remotes/origin/main");

    env.but("config target refs/remotes/upstream/main --push-remote origin")
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

#[cfg(feature = "legacy")]
#[test]
fn target_refresh_updates_only_metadata_and_is_idempotent() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    env.but("setup").assert().success();
    env.invoke_git("remote add upstream .");
    env.invoke_git("update-ref refs/remotes/upstream/main refs/remotes/origin/main");
    env.but("config target upstream/main --push-remote origin")
        .assert()
        .success();
    env.invoke_bash(
        r#"
new_target=$(printf 'advance target\n' | git commit-tree refs/remotes/upstream/main^{tree} -p refs/remotes/upstream/main)
git update-ref refs/remotes/upstream/main "$new_target"
"#,
    );

    let expected_target = env.invoke_git("rev-parse refs/remotes/upstream/main");
    let refs_before = env.invoke_git("show-ref");
    let head_before = env.invoke_git("rev-parse HEAD");
    let index_before = env.invoke_git("ls-files --stage");
    let worktree_before = env.invoke_git("status --porcelain=v2 --untracked-files=all");

    env.but("config target --refresh").assert().success();

    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.targetRef"),
        "refs/remotes/upstream/main"
    );
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.targetCommitId"),
        expected_target
    );
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.pushRemote"),
        "origin"
    );
    assert_eq!(
        env.invoke_git("show-ref"),
        refs_before,
        "refresh does not move local, remote, or workspace refs"
    );
    assert_eq!(
        env.invoke_git("rev-parse HEAD"),
        head_before,
        "refresh does not move HEAD"
    );
    assert_eq!(
        env.invoke_git("ls-files --stage"),
        index_before,
        "refresh does not alter the index"
    );
    assert_eq!(
        env.invoke_git("status --porcelain=v2 --untracked-files=all"),
        worktree_before,
        "refresh does not alter the worktree"
    );

    let repo = env.open_repo();
    let config_path = repo.path().join("config");
    let metadata_path = repo
        .gitbutler_storage_path()
        .unwrap()
        .join("virtual_branches.toml");
    let config_after_first_refresh = fs::read(&config_path).unwrap();
    let metadata_after_first_refresh = fs::read(&metadata_path).unwrap();

    env.but("config target --refresh").assert().success();

    assert_eq!(
        fs::read(config_path).unwrap(),
        config_after_first_refresh,
        "a current target does not rewrite repository config"
    );
    assert_eq!(
        fs::read(metadata_path).unwrap(),
        metadata_after_first_refresh,
        "a current target does not rewrite legacy metadata"
    );
}

#[cfg(feature = "legacy")]
#[test]
fn target_refresh_rejects_named_stacks_before_metadata_writes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    assert_refresh_rejected_without_metadata_writes(&env);
}

#[cfg(feature = "legacy")]
#[test]
fn target_refresh_rejects_anonymous_stacks_before_metadata_writes() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-anonymous-segment");

    assert_refresh_rejected_without_metadata_writes(&env);
}

#[cfg(feature = "legacy")]
fn assert_refresh_rejected_without_metadata_writes(env: &Sandbox) {
    let repo = env.open_repo();
    let config_path = repo.path().join("config");
    let metadata_path = repo
        .gitbutler_storage_path()
        .unwrap()
        .join("virtual_branches.toml");
    let config_before = fs::read(&config_path).unwrap();
    let metadata_before = fs::read(&metadata_path).unwrap();
    let refs_before = env.invoke_git("show-ref");
    let head_before = env.invoke_git("rev-parse HEAD");
    let index_before = env.invoke_git("ls-files --stage");
    let worktree_before = env.invoke_git("status --porcelain=v2 --untracked-files=all");

    let output = env.but("config target --refresh").output().unwrap();
    assert!(
        !output.status.success(),
        "refresh should reject applied branches"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "Cannot refresh target metadata while there are applied branches. Please unapply all branches first."
        ),
        "unexpected stderr: {stderr}"
    );

    assert_eq!(
        fs::read(config_path).unwrap(),
        config_before,
        "rejection happens before repository config is written"
    );
    assert_eq!(
        fs::read(metadata_path).unwrap(),
        metadata_before,
        "rejection happens before legacy metadata is written"
    );
    assert_eq!(
        env.invoke_git("show-ref"),
        refs_before,
        "rejection does not move refs"
    );
    assert_eq!(
        env.invoke_git("rev-parse HEAD"),
        head_before,
        "rejection does not move HEAD"
    );
    assert_eq!(
        env.invoke_git("ls-files --stage"),
        index_before,
        "rejection does not alter the index"
    );
    assert_eq!(
        env.invoke_git("status --porcelain=v2 --untracked-files=all"),
        worktree_before,
        "rejection does not alter the worktree"
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

    env.but("--format shell config push-remote")
        .assert()
        .success()
        .stdout_eq(str![[r#"
origin

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
