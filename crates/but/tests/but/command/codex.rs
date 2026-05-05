//! Tests for the `but codex` hooks.

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
