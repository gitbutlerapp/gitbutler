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
Error: No git repository found at .git

Hint: Run `but init` to initialize a Git repository.

"#]]);
}

#[cfg(feature = "legacy")]
#[test]
fn target_configures_distinct_push_remote_for_fork() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    env.invoke_git("remote add upstream .");
    env.invoke_git("update-ref refs/remotes/upstream/main refs/remotes/origin/main");
    env.invoke_git("branch --set-upstream-to=upstream/main main");

    env.but("config target upstream/main --push-remote origin")
        .assert()
        .success();

    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.targetRef"),
        "refs/heads/main",
        "remote input resolves to its local tracking branch"
    );
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.pushRemote"),
        "origin"
    );
}

// regression test for https://linear.app/gitbutler/issue/GB-2242
#[cfg(feature = "legacy")]
#[test]
fn target_remote_shorthand_without_tracking_branch_hints_to_track_it() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    env.invoke_git("remote add upstream .");
    env.invoke_git("update-ref refs/remotes/upstream/main refs/remotes/origin/main");

    // nothing is being migrated, so the error doesn't call upstream/main the current target.
    // The hint names main rather than leaving a branch open to fill in, as an agent fills it with
    // the branch it works on, which then becomes the target and pushes to upstream/main.
    env.but("config target upstream/main")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: No local branch tracks upstream/main

Hint: Make a local branch track upstream/main with `git branch --set-upstream-to=upstream/main main`, or choose a local target branch with `but config target <branch>`

"#]]);

    // run the command the hint suggests
    env.invoke_git("branch --set-upstream-to=upstream/main main");

    env.but("config target upstream/main").assert().success();
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.targetRef"),
        "refs/heads/main",
        "the shorthand resolves to the branch that now tracks it"
    );
    assert_eq!(
        env.invoke_git("rev-parse --abbrev-ref main@{upstream}"),
        "upstream/main",
        "main keeps its upstream although origin has a main too"
    );
}

// regression test for https://linear.app/gitbutler/issue/GB-2244
#[cfg(feature = "legacy")]
#[test]
fn target_remote_shorthand_without_namesake_creates_it_tracking_that_remote() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    env.invoke_bash("git checkout -q -b feature && git branch -q -D main");
    env.invoke_git("remote add upstream .");
    env.invoke_git("update-ref refs/remotes/upstream/main refs/remotes/origin/main");

    // a fresh clone working on a feature branch has no local main to infer the target from
    env.but("status").assert().failure().stderr_eq(str![[r#"
Error: No target branch is configured and none could be inferred

Hint: Run `but config target <remote>/<branch>` to configure one

"#]]);

    // run the command the hint suggests
    env.but("config target upstream/main").assert().success();
    assert_eq!(
        env.invoke_git("rev-parse --abbrev-ref main@{upstream}"),
        "upstream/main",
        "main is created to track the remote branch named, not origin/main of the default remote"
    );
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.targetRef"),
        "refs/heads/main",
    );
    env.but("status").assert().success();
}

#[cfg(feature = "legacy")]
#[test]
fn target_remote_head_without_tracking_branch_hints_to_choose_one() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    env.invoke_bash("git checkout -q -b feature && git branch -q -D main");

    // git doesn't accept HEAD as a branch name, so no command can make one track origin/HEAD
    env.but("config target origin/HEAD")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: No local branch tracks origin/HEAD

Hint: Choose a local target branch with `but config target <branch>`

"#]]);
}

#[cfg(feature = "legacy")]
#[test]
fn target_accepts_local_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.but("config target main").assert().success();
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.targetRef"),
        "refs/heads/main",
        "local input is stored directly"
    );
}

// As 0.23.1 left a target main pushed without `-u`: it tracks nothing, and pulls onto it moved the
// cutoff to its unpublished commit. Choosing it again makes it track origin/main, as
// `git branch --set-upstream-to=origin/main main` would, with the cutoff back on origin/main, so
// pulls don't take the unpublished commit for integrated.
#[cfg(feature = "legacy")]
#[test]
fn target_untracked_local_branch_tracks_remote_branch_named_as_it() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    env.invoke_git("branch --unset-upstream main");
    env.invoke_git("commit --allow-empty -m unpublished");
    env.invoke_git("config gitbutler.project.targetRef refs/heads/main");
    env.invoke_bash("git config gitbutler.project.targetCommitId $(git rev-parse main)");
    env.invoke_git("config gitbutler.project.portedMeta true");

    env.but("config target main").assert().success();
    assert_eq!(
        env.invoke_git("rev-parse --abbrev-ref main@{upstream}"),
        "origin/main",
        "main tracks the branch named as it on the remote"
    );
    assert_eq!(
        env.project_meta().target_commit_id.map(|id| id.to_string()),
        Some(env.invoke_git("rev-parse origin/main")),
        "the cutoff is back on origin/main, below the unpublished commit"
    );
}

#[cfg(feature = "legacy")]
#[test]
fn target_prefers_local_branch_over_remote_shorthand() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.invoke_git("branch origin/main main");
    env.but("config target origin/main").assert().success();
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.targetRef"),
        "refs/heads/origin/main",
        "an exact local branch name takes precedence over remote shorthand"
    );
}

#[cfg(feature = "legacy")]
#[test]
fn target_accepts_qualified_remote_despite_local_name_collision() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.invoke_git("branch origin/main main");
    env.invoke_git("branch --set-upstream-to=refs/remotes/origin/main main");
    env.but("config target refs/remotes/origin/main")
        .assert()
        .success();
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.project.targetRef"),
        "refs/heads/main",
        "a qualified remote name bypasses the colliding local name"
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
    The remote named "missing" did not exist[..]

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
fn feature_config_lists_no_flags() {
    let env = Sandbox::empty();

    env.but("config feature")
        .assert()
        .success()
        .stdout_eq(str![[r#"

Feature Flags:


"#]]);
}

#[test]
fn feature_config_json_lists_no_flags() {
    let env = Sandbox::empty();

    env.but("--json config feature")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(str![[r#"
{}

"#]]);
}

#[test]
fn global_user_configuration_works_outside_a_repository() {
    let env = Sandbox::empty();
    let global_config = env.projects_root().join("global.gitconfig");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let target = env.projects_root().join("actual.gitconfig");
        std::fs::write(&target, "").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink(&target, &global_config).unwrap();
    }

    env.but("config user set --global name 'Ada Lovelace'")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success();
    assert_eq!(
        env.invoke_git("config --file global.gitconfig --get user.name"),
        "Ada Lovelace"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert!(
            std::fs::symlink_metadata(&global_config)
                .unwrap()
                .file_type()
                .is_symlink(),
            "editing global config preserves its symlink"
        );
        assert_eq!(
            std::fs::metadata(env.projects_root().join("actual.gitconfig"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "editing global config preserves its target's permissions"
        );
    }

    env.but("config user")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success();

    env.but("config user unset --global name")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success();
    env.invoke_git_fails(
        "config --file global.gitconfig --get user.name",
        "global user name should be removed from the configured global file",
    );
    env.but("config user set name Byron")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Local Git configuration requires a git repository

"#]]);
}

#[test]
fn global_user_configuration_creates_parent_for_relative_path() {
    let env = Sandbox::empty();

    env.but("config user set --global name 'Ada Lovelace'")
        .env("GIT_CONFIG_GLOBAL", "nested/global.gitconfig")
        .assert()
        .success();
    assert_eq!(
        env.invoke_git("config --file nested/global.gitconfig --get user.name"),
        "Ada Lovelace",
        "relative global config paths are resolved against the command's working directory"
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
