//! Tests for the `but cursor` hooks.

mod after_edit {
    use crate::utils::Sandbox;

    #[test]
    fn accepts_valid_json_input() -> anyhow::Result<()> {
        let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
        env.file("test.txt", "content");

        // Set up metadata so legacy APIs can find the stacks
        env.setup_metadata(&["A", "B"])?;

        let file_path = env.projects_root().join("test.txt");
        let workspace_root = env.projects_root().to_string_lossy().to_string();
        let input = serde_json::json!({
            "conversation_id": "00000000-0000-0000-0000-000000000001",
            "generation_id": "00000000-0000-0000-0000-000000000002",
            "file_path": file_path.to_string_lossy(),
            "edits": [{
                "old_string": "old",
                "new_string": "new"
            }],
            "hook_event_name": "afterEdit",
            "workspace_roots": [workspace_root]
        });

        env.but("cursor after-edit")
            .stdin(input.to_string())
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
{
  "continue": false,
  "userMessage": "",
  "agentMessage": ""
}

"#]]);

        Ok(())
    }
}

mod stop {
    use crate::utils::Sandbox;

    #[test]
    fn handles_no_changes() -> anyhow::Result<()> {
        let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

        let workspace_root = env.projects_root().to_string_lossy().to_string();
        let input = serde_json::json!({
            "conversation_id": "00000000-0000-0000-0000-000000000001",
            "generation_id": "00000000-0000-0000-0000-000000000002",
            "status": "completed",
            "hook_event_name": "stop",
            "workspace_roots": [workspace_root]
        });

        env.but("cursor stop")
            .stdin(input.to_string())
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
{
  "continue": false,
  "userMessage": "",
  "agentMessage": ""
}

"#]]);

        Ok(())
    }

    #[test]
    fn handles_stop_with_changes() -> anyhow::Result<()> {
        let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

        // Set up metadata so legacy APIs can find the stacks
        env.setup_metadata(&["A", "B"])?;

        // Create uncommitted changes in the worktree
        env.file("new_file.txt", "new content from cursor");

        let workspace_root = env.projects_root().to_string_lossy().to_string();
        let input = serde_json::json!({
            "conversation_id": "00000000-0000-0000-0000-000000000001",
            "generation_id": "00000000-0000-0000-0000-000000000002",
            "status": "completed",
            "hook_event_name": "stop",
            "workspace_roots": [workspace_root]
        });

        env.but("cursor stop")
            .stdin(input.to_string())
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
{
  "continue": false,
  "userMessage": "",
  "agentMessage": ""
}

"#]]);

        Ok(())
    }
}
