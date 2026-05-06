//! Tests for the `but codex` hooks.

use super::util;
use crate::utils::Sandbox;

#[test]
fn pre_tool_uses_payload_cwd_when_current_dir_is_not_project() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.file("test.txt", "content");

    let input = serde_json::json!({
        "turn_id": "turn-1",
        "cwd": env.projects_root().to_string_lossy(),
        "tool_name": "str_replace_editor",
        "tool_input": {
            "file_path": "test.txt",
            "new_string": "new",
            "old_string": "old",
            "replace_all": false
        }
    });

    env.but("codex pre-tool")
        .current_dir(env.app_data_dir())
        .stdin(input.to_string())
        .assert()
        .success()
        .stdout_eq("")
        .stderr_eq("");

    Ok(())
}

#[test]
fn post_tool_does_not_initialize_context_from_process_current_dir() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    let input = serde_json::json!({
        "turn_id": "turn-1",
        "cwd": env.projects_root().to_string_lossy(),
        "tool_name": "shell_command",
        "tool_input": {}
    });

    env.but("codex post-tool")
        .current_dir(env.app_data_dir())
        .stdin(input.to_string())
        .assert()
        .success()
        .stdout_eq("")
        .stderr_eq("");

    Ok(())
}
#[test]
fn stop_accepts_turn_id_without_session_id() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    let input = serde_json::json!({
        "turn_id": "turn-1",
        "cwd": env.projects_root().to_string_lossy(),
        "last_assistant_message": "done"
    });

    env.but("codex stop")
        .current_dir(env.app_data_dir())
        .stdin(input.to_string())
        .assert()
        .success()
        .stdout_eq("")
        .stderr_eq("");

    Ok(())
}

#[test]
fn post_tool_assigns_apply_patch_file_without_structured_patch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("codex-patch.txt", "new content from codex\n");

    let input = serde_json::json!({
        "turn_id": "turn-1",
        "cwd": env.projects_root().to_string_lossy(),
        "tool_name": "apply_patch",
        "tool_input": {
            "cmd": "*** Begin Patch\n*** Add File: codex-patch.txt\n+new content from codex\n*** End Patch\n"
        },
        "tool_response": {
            "status": "completed"
        }
    });

    env.but("codex post-tool")
        .stdin(input.to_string())
        .assert()
        .success()
        .stdout_eq("")
        .stderr_eq("");

    let status = util::status_json(&env)?;
    let unassigned = status["unassignedChanges"]
        .as_array()
        .expect("status should contain unassigned changes");
    assert!(
        !unassigned
            .iter()
            .any(|change| change_path_eq(change, "codex-patch.txt")),
        "codex-patch.txt should be assigned to the Codex session stack"
    );

    Ok(())
}

#[test]
fn post_tool_ignores_read_only_path_bearing_tools() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("existing-unassigned.txt", "already unassigned\n");
    let status = util::status_json(&env)?;
    let unassigned = status["unassignedChanges"]
        .as_array()
        .expect("status should contain unassigned changes");
    assert!(
        unassigned
            .iter()
            .any(|change| change_path_eq(change, "existing-unassigned.txt")),
        "test setup should start with existing-unassigned.txt in unassigned changes"
    );

    let input = serde_json::json!({
        "turn_id": "turn-1",
        "cwd": env.projects_root().to_string_lossy(),
        "tool_name": "mcp__filesystem__read_file",
        "tool_input": {
            "path": "existing-unassigned.txt"
        },
        "tool_response": {
            "content": "already unassigned\n"
        }
    });

    env.but("codex post-tool")
        .stdin(input.to_string())
        .assert()
        .success()
        .stdout_eq("")
        .stderr_eq("");

    let status = util::status_json(&env)?;
    let unassigned = status["unassignedChanges"]
        .as_array()
        .expect("status should contain unassigned changes");
    assert!(
        unassigned
            .iter()
            .any(|change| change_path_eq(change, "existing-unassigned.txt")),
        "read-only tools must not assign existing unassigned changes to the Codex session stack"
    );

    Ok(())
}

#[test]
fn stop_commits_apply_patch_file_without_structured_patch_or_llm_provider() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("repo-with-remote-and-head")?;
    enable_auto_commit(&env)?;

    env.file("codex-stop.txt", "new content from codex\n");
    let post_input = serde_json::json!({
        "turn_id": "turn-1",
        "cwd": env.projects_root().to_string_lossy(),
        "tool_name": "apply_patch",
        "tool_input": {
            "cmd": "*** Begin Patch\n*** Add File: codex-stop.txt\n+new content from codex\n*** End Patch\n"
        },
        "tool_response": {
            "status": "completed"
        }
    });

    env.but("codex post-tool")
        .stdin(post_input.to_string())
        .assert()
        .success()
        .stdout_eq("")
        .stderr_eq("");

    let stop_input = serde_json::json!({
        "turn_id": "turn-1",
        "cwd": env.projects_root().to_string_lossy(),
        "last_assistant_message": "Created codex-stop.txt"
    });
    env.but("codex stop")
        .stdin(stop_input.to_string())
        .assert()
        .success()
        .stdout_eq("")
        .stderr_eq("");

    let status = util::status_json(&env)?;
    let committed_count = status["stacks"]
        .as_array()
        .into_iter()
        .flat_map(|stacks| stacks.iter())
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .flat_map(|branch| branch["commits"].as_array().into_iter().flatten())
        .count();
    assert_eq!(
        committed_count, 1,
        "Codex stop should commit the file assigned by post-tool"
    );
    let unassigned = status["unassignedChanges"]
        .as_array()
        .expect("status should contain unassigned changes");
    assert!(
        !unassigned
            .iter()
            .any(|change| change_path_eq(change, "codex-stop.txt")),
        "codex-stop.txt should not remain unassigned after stop"
    );

    Ok(())
}

#[test]
fn stop_commits_shell_created_file_from_transcript_without_post_tool() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("repo-with-remote-and-head")?;
    enable_auto_commit(&env)?;

    env.file("codex-touch.txt", "");
    let transcript_path = env.projects_root().join(".codex_transcript.jsonl");
    let transcript_line = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "function_call",
            "name": "exec_command",
            "arguments": serde_json::json!({
                "cmd": "touch codex-touch.txt",
                "workdir": env.projects_root().to_string_lossy()
            }).to_string()
        }
    });
    env.file(".codex_transcript.jsonl", &format!("{transcript_line}\n"));

    let stop_input = serde_json::json!({
        "turn_id": "turn-1",
        "cwd": env.projects_root().to_string_lossy(),
        "transcript_path": transcript_path.to_string_lossy(),
        "last_assistant_message": "Created codex-touch.txt"
    });
    env.but("codex stop")
        .stdin(stop_input.to_string())
        .assert()
        .success()
        .stdout_eq("")
        .stderr_eq("");

    let status = util::status_json(&env)?;
    assert_eq!(
        committed_count(&status),
        1,
        "Codex stop should commit shell-created files recorded in the transcript"
    );
    let unassigned = status["unassignedChanges"]
        .as_array()
        .expect("status should contain unassigned changes");
    assert!(
        !unassigned
            .iter()
            .any(|change| change_path_eq(change, "codex-touch.txt")),
        "codex-touch.txt should not remain unassigned after stop"
    );

    Ok(())
}

#[test]
fn stop_does_not_commit_read_only_shell_paths_from_transcript() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("repo-with-remote-and-head")?;
    enable_auto_commit(&env)?;

    env.file("existing-unassigned.txt", "already unassigned\n");
    let transcript_path = env.projects_root().join(".codex_transcript.jsonl");
    let transcript_line = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "function_call",
            "name": "exec_command",
            "arguments": serde_json::json!({
                "cmd": "ls -l existing-unassigned.txt",
                "workdir": env.projects_root().to_string_lossy()
            }).to_string()
        }
    });
    env.file(".codex_transcript.jsonl", &format!("{transcript_line}\n"));

    let stop_input = serde_json::json!({
        "turn_id": "turn-1",
        "cwd": env.projects_root().to_string_lossy(),
        "transcript_path": transcript_path.to_string_lossy(),
        "last_assistant_message": "Listed existing-unassigned.txt"
    });
    env.but("codex stop")
        .stdin(stop_input.to_string())
        .assert()
        .success()
        .stdout_eq("")
        .stderr_eq("");

    let status = util::status_json(&env)?;
    assert_eq!(
        committed_count(&status),
        0,
        "read-only shell commands must not commit existing unassigned changes"
    );
    let unassigned = status["unassignedChanges"]
        .as_array()
        .expect("status should contain unassigned changes");
    assert!(
        unassigned
            .iter()
            .any(|change| change_path_eq(change, "existing-unassigned.txt")),
        "existing-unassigned.txt should remain unassigned"
    );

    Ok(())
}

fn enable_auto_commit(env: &Sandbox) -> anyhow::Result<()> {
    let settings_path = env.app_data_dir().join("gitbutler/settings.json");
    let settings_content = std::fs::read_to_string(&settings_path)?;
    let mut settings: serde_json::Value = serde_json::from_str(&settings_content)?;
    settings["claude"]["autoCommitAfterCompletion"] = serde_json::json!(true);
    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
    Ok(())
}

fn committed_count(status: &serde_json::Value) -> usize {
    status["stacks"]
        .as_array()
        .into_iter()
        .flat_map(|stacks| stacks.iter())
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .flat_map(|branch| branch["commits"].as_array().into_iter().flatten())
        .count()
}

fn change_path_eq(change: &serde_json::Value, expected_path: &str) -> bool {
    ["path", "filePath"].into_iter().any(|key| {
        change
            .get(key)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|path| path == expected_path)
    })
}

#[test]
fn stop_hook_errors_do_not_fail_codex_process() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    let input = serde_json::json!({
        "turn_id": "turn-1",
        "cwd": env.app_data_dir().to_string_lossy(),
        "last_assistant_message": "done"
    });

    env.but("codex stop")
        .current_dir(env.app_data_dir())
        .stdin(input.to_string())
        .assert()
        .success()
        .stdout_eq("");

    Ok(())
}
