//! Tests for the `but claude` hooks.

mod pre_tool {
    use crate::utils::Sandbox;

    #[test]
    fn accepts_valid_json_input() -> anyhow::Result<()> {
        let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
        env.file("test.txt", "content");

        let file_path = env.projects_root().join("test.txt");
        let input = serde_json::json!({
            "session_id": "00000000-0000-0000-0000-000000000001",
            "transcript_path": "/tmp/nonexistent.jsonl",
            "hook_event_name": "pre-tool-use",
            "tool_name": "str_replace_editor",
            "tool_input": {
                "file_path": file_path.to_string_lossy(),
                "new_string": "new",
                "old_string": "old",
                "replace_all": false
            }
        });

        env.but("claude pre-tool")
            .stdin(input.to_string())
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
{"continue":true,"stopReason":"","suppressOutput":true}

"#]]);

        Ok(())
    }
}

mod post_tool {
    use crate::utils::Sandbox;

    #[test]
    fn accepts_valid_json_input() -> anyhow::Result<()> {
        let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
        env.file("test.txt", "content");

        let file_path = env.projects_root().join("test.txt");
        let input = serde_json::json!({
            "session_id": "00000000-0000-0000-0000-000000000001",
            "transcript_path": "/tmp/nonexistent.jsonl",
            "hook_event_name": "post-tool-use",
            "tool_name": "str_replace_editor",
            "tool_input": {
                "file_path": file_path.to_string_lossy(),
                "new_string": "new",
                "old_string": "old",
                "replace_all": false
            },
            "tool_response": {
                "filePath": file_path.to_string_lossy(),
                "oldString": "old",
                "newString": "new",
                "originalFile": null,
                "structuredPatch": [],
                "userModified": false,
                "replaceAll": false
            }
        });

        env.but("claude post-tool")
            .stdin(input.to_string())
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
{"continue":true,"stopReason":"","suppressOutput":true}

"#]]);

        Ok(())
    }
}

mod stop {
    use crate::utils::Sandbox;

    #[test]
    fn handles_no_changes() -> anyhow::Result<()> {
        let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

        // Create a minimal JSONL transcript with a user record containing cwd
        let transcript_path = env.projects_root().join(".claude_transcript.jsonl");
        let cwd = env.projects_root().to_string_lossy();
        let transcript_content = format!(
            r#"{{"type":"user","cwd":"{}","message":{{"role":"user","content":"test"}}}}"#,
            cwd
        );
        env.file(".claude_transcript.jsonl", &transcript_content);

        let input = serde_json::json!({
            "session_id": "00000000-0000-0000-0000-000000000001",
            "transcript_path": transcript_path.to_string_lossy(),
            "hook_event_name": "stop",
            "stop_hook_active": true
        });

        env.but("claude stop")
            .stdin(input.to_string())
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
{"continue":true,"stopReason":"No after-hook behaviour required.","suppressOutput":true}

"#]]);

        Ok(())
    }

    #[test]
    fn commits_changes_when_auto_commit_enabled() -> anyhow::Result<()> {
        let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

        // Enable auto_commit_after_completion in settings
        let settings_path = env.app_data_dir().join("gitbutler/settings.json");
        let settings_content = std::fs::read_to_string(&settings_path)?;
        let mut settings: serde_json::Value = serde_json::from_str(&settings_content)?;
        settings["claude"]["autoCommitAfterCompletion"] = serde_json::json!(true);
        std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;

        // Set up metadata so legacy APIs can find the stacks
        env.setup_metadata(&["A", "B"])?;

        // Create uncommitted changes in the worktree
        env.file("new_file.txt", "new content from claude");

        // Create a JSONL transcript with a user record containing cwd and a summary
        let transcript_path = env.projects_root().join(".claude_transcript.jsonl");
        let cwd = env.projects_root().to_string_lossy();
        let transcript_content = format!(
            r#"{{"type":"user","cwd":"{}","message":{{"role":"user","content":"Add a new file"}}}}
{{"type":"summary","summary":"Added new_file.txt with test content","leafUuid":"00000000-0000-0000-0000-000000000002"}}"#,
            cwd
        );
        env.file(".claude_transcript.jsonl", &transcript_content);

        let input = serde_json::json!({
            "session_id": "00000000-0000-0000-0000-000000000001",
            "transcript_path": transcript_path.to_string_lossy(),
            "hook_event_name": "stop",
            "stop_hook_active": true
        });

        env.but("claude stop")
            .stdin(input.to_string())
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
{"continue":true,"stopReason":"","suppressOutput":true}

"#]]);

        Ok(())
    }
}
