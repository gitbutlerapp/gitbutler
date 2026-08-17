use super::*;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
};

/// Build a lookup function from a set of key-value pairs.
/// `use<>` tells the compiler the returned closure owns all its data and
/// borrows nothing from `vars` (which is consumed into the `HashMap`).
fn env_from(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
    let map: HashMap<String, OsString> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), OsString::from(v)))
        .collect();
    move |key: &str| map.get(key).cloned()
}

#[test]
fn detect_claude_code() {
    assert_eq!(
        detect_with(env_from(&[("CLAUDE_CODE", "1")])),
        Some(Agent::ClaudeCode),
    );
}

#[test]
fn detect_claude_code_legacy_var() {
    assert_eq!(
        detect_with(env_from(&[("CLAUDECODE", "1")])),
        Some(Agent::ClaudeCode),
    );
}

#[test]
fn detect_cowork() {
    assert_eq!(
        detect_with(env_from(&[("CLAUDE_CODE_IS_COWORK", "1")])),
        Some(Agent::ClaudeCodeCowork),
    );
}

#[test]
fn detect_cursor() {
    assert_eq!(
        detect_with(env_from(&[("CURSOR_TRACE_ID", "abc123")])),
        Some(Agent::Cursor),
    );
}

#[test]
fn detect_cursor_cli() {
    assert_eq!(
        detect_with(env_from(&[("CURSOR_AGENT", "1")])),
        Some(Agent::CursorCli),
    );
}

#[test]
fn detect_cursor_cli_extension_host() {
    assert_eq!(
        detect_with(env_from(&[("CURSOR_EXTENSION_HOST_ROLE", "agent-exec")])),
        Some(Agent::CursorCli),
    );
}

#[test]
fn ignores_cursor_extension_host_non_agent_role() {
    assert_eq!(
        detect_with(env_from(&[(
            "CURSOR_EXTENSION_HOST_ROLE",
            "extension-host"
        )])),
        None,
    );
}

#[test]
fn detect_codex() {
    assert_eq!(
        detect_with(env_from(&[("CODEX_SANDBOX", "seatbelt")])),
        Some(Agent::Codex),
    );
}

#[test]
fn detect_codex_shell() {
    assert_eq!(
        detect_with(env_from(&[("CODEX_SHELL", "1")])),
        Some(Agent::Codex),
    );
}

#[test]
fn detect_kiro_cli_when_both_side_channels_are_set() {
    assert_eq!(
        detect_with(env_from(&[
            ("AGENT_DISPLAY_OUT", "/tmp/display"),
            ("AGENT_CONTEXT_OUT", "/tmp/context"),
        ])),
        Some(Agent::KiroCli),
    );
}

#[test]
fn ignore_single_kiro_side_channel() {
    for var in ["AGENT_DISPLAY_OUT", "AGENT_CONTEXT_OUT"] {
        assert_eq!(
            detect_with(env_from(&[(var, "/tmp/fifo")])),
            None,
            "{var} alone is not a sufficiently specific Kiro marker",
        );
    }
}

#[test]
fn detect_qwen_code() {
    assert_eq!(
        detect_with(env_from(&[("QWEN_CODE", "1")])),
        Some(Agent::QwenCode),
    );
}

#[test]
fn ignore_noncanonical_qwen_marker_value() {
    assert_eq!(detect_with(env_from(&[("QWEN_CODE", "true")])), None);
}

#[test]
fn detect_gemini() {
    assert_eq!(
        detect_with(env_from(&[("GEMINI_CLI", "1")])),
        Some(Agent::GeminiCli),
    );
}

#[test]
fn detect_copilot_agent() {
    assert_eq!(
        detect_with(env_from(&[("COPILOT_AGENT", "1")])),
        Some(Agent::GitHubCopilot),
    );
}

#[test]
fn detect_junie_data() {
    assert_eq!(
        detect_with(env_from(&[("JUNIE_DATA", "/tmp/junie")])),
        Some(Agent::Junie),
    );
}

#[test]
fn detect_junie_shim_path() {
    assert_eq!(
        detect_with(env_from(&[("JUNIE_SHIM_PATH", "/tmp/junie/shim")])),
        Some(Agent::Junie),
    );
}

#[test]
fn copilot_config_vars_are_not_agent_signals() {
    for var in ["COPILOT_MODEL", "COPILOT_ALLOW_ALL", "COPILOT_GITHUB_TOKEN"] {
        assert_eq!(
            detect_with(env_from(&[(var, "1")])),
            None,
            "{var} should not be treated as an agent marker",
        );
    }
}

#[test]
fn detect_opencode() {
    assert_eq!(
        detect_with(env_from(&[("OPENCODE_CLIENT", "1")])),
        Some(Agent::OpenCode),
    );
}

#[test]
fn detect_opencode_process_marker() {
    assert_eq!(
        detect_with(env_from(&[("OPENCODE", "1")])),
        Some(Agent::OpenCode),
    );
}

#[test]
fn detect_kilo_code_before_opencode() {
    assert_eq!(
        detect_with(env_from(&[("KILO_PID", "1234"), ("OPENCODE", "1")])),
        Some(Agent::KiloCode),
    );
}

#[test]
fn detect_hermes() {
    assert_eq!(
        detect_with(env_from(&[("HERMES_SESSION_ID", "session-123")])),
        Some(Agent::Hermes),
    );
}

#[test]
fn detect_augment() {
    assert_eq!(
        detect_with(env_from(&[("AUGMENT_AGENT", "1")])),
        Some(Agent::Augment),
    );
}

#[test]
fn detect_antigravity() {
    assert_eq!(
        detect_with(env_from(&[("ANTIGRAVITY_AGENT", "1")])),
        Some(Agent::Antigravity),
    );
}

#[test]
fn detect_replit() {
    assert_eq!(
        detect_with(env_from(&[("REPL_ID", "abc")])),
        Some(Agent::Replit),
    );
}

#[test]
fn detect_dirac_vscode_terminal() {
    assert_eq!(
        detect_with(env_from(&[("DIRAC_ACTIVE", "true")])),
        Some(Agent::Dirac),
    );
}

#[test]
fn detect_none_when_clean() {
    assert_eq!(detect_with(|_| None), None);
}

#[test]
fn environment_variable_inventory_is_unique() {
    let unique: HashSet<_> = ENVIRONMENT_VARIABLES.iter().copied().collect();
    assert_eq!(
        unique.len(),
        ENVIRONMENT_VARIABLES.len(),
        "each agent environment variable should occur once",
    );
}

#[test]
fn test_command_isolation_removes_all_agent_environment_variables() {
    let mut cmd = std::process::Command::new("but");
    for var in ENVIRONMENT_VARIABLES {
        cmd.env(var, "ambient-value");
    }

    but_testsupport::isolate_env_std_cmd_with_additional_removals(&mut cmd, ENVIRONMENT_VARIABLES);

    for var in ENVIRONMENT_VARIABLES {
        let update = cmd.get_envs().find(|(name, _)| *name == OsStr::new(var));
        assert!(
            matches!(update, Some((_, None))),
            "{var} should be explicitly removed",
        );
    }
}

#[test]
fn ai_agent_var_takes_priority() {
    // Even though CLAUDE_CODE is set, AI_AGENT=codex should win.
    assert_eq!(
        detect_with(env_from(&[("AI_AGENT", "codex"), ("CLAUDE_CODE", "1")])),
        Some(Agent::Codex),
    );
}

#[test]
fn empty_var_is_not_detected() {
    assert_eq!(detect_with(env_from(&[("GEMINI_CLI", "")])), None);
}

#[test]
fn empty_ai_agent_var_is_not_detected() {
    assert_eq!(detect_with(env_from(&[("AI_AGENT", "")])), None);
    assert_eq!(detect_with(env_from(&[("AI_AGENT", "  ")])), None);
}

#[test]
fn ai_agent_case_insensitive() {
    assert_eq!(
        detect_with(env_from(&[("AI_AGENT", "Claude-Code")])),
        Some(Agent::ClaudeCode),
    );
}

#[test]
fn ai_agent_accepts_known_aliases() {
    let cases = [
        ("claude", Agent::ClaudeCode),
        ("cowork", Agent::ClaudeCodeCowork),
        ("gemini", Agent::GeminiCli),
        ("augment-cli", Agent::Augment),
        ("github-copilot-cli", Agent::GitHubCopilot),
        ("github_copilot_vscode_agent", Agent::GitHubCopilot),
        ("kiro", Agent::KiroCli),
        ("qwen", Agent::QwenCode),
        ("gitlab-duo", Agent::GitLabDuoCli),
        ("kilo", Agent::KiloCode),
        ("hermes", Agent::Hermes),
        ("devin", Agent::Devin),
        ("pool", Agent::Poolside),
        ("v0", Agent::V0),
        ("amazon-q-developer", Agent::AmazonQ),
        ("amazon-q-developer-cli", Agent::AmazonQ),
        ("codebuddy-code", Agent::CodeBuddy),
        ("grok", Agent::GrokBuild),
        ("warp-oz", Agent::Warp),
        ("open-hands", Agent::OpenHands),
        ("open-claw", Agent::OpenClaw),
        ("dsh", Agent::DeepSeekHarness),
        ("deepseek", Agent::DeepSeekHarness),
    ];

    for (name, agent) in cases {
        assert_eq!(
            detect_with(env_from(&[("AI_AGENT", name)])),
            Some(agent),
            "AI_AGENT alias {name} should resolve to {}",
            agent.as_str(),
        );
    }
}

#[test]
fn ai_agent_unknown_value_still_detects_agent() {
    assert_eq!(
        detect_with(env_from(&[("AI_AGENT", "some-new-agent")])),
        Some(Agent::Unknown),
    );
}

#[test]
fn detect_gitlab_duo_cli_with_decorated_ai_agent_value() {
    assert_eq!(
        detect_with(env_from(&[("AI_AGENT", "gitlab-lsp_7.17.0__duo-cli",)])),
        Some(Agent::GitLabDuoCli),
    );
}

#[test]
fn ai_agent_matches_known_prefix_with_trailing_decorations() {
    // Claude Code desktop embeds its version without the `@` separator.
    for (value, expected) in [
        ("claude-code_2-1-202_agent", Agent::ClaudeCode),
        ("claude-code-2.1.202", Agent::ClaudeCode),
        ("cursor-cli-extra", Agent::CursorCli),
    ] {
        assert_eq!(
            detect_with(env_from(&[("AI_AGENT", value)])),
            Some(expected),
            "AI_AGENT={value} should prefix-match {}",
            expected.as_str(),
        );
    }
}

#[test]
fn ai_agent_strips_version_suffix() {
    // The AI_AGENT naming convention allows a `@version` suffix
    // (e.g. `devin@1`, `custom-agent@2.0`); it must not defeat detection.
    for (value, expected) in [
        ("claude-code@1", Agent::ClaudeCode),
        ("devin@1", Agent::Devin),
        ("cursor-cli@2.0", Agent::CursorCli),
        ("gemini-cli@0.1.2", Agent::GeminiCli),
    ] {
        assert_eq!(
            detect_with(env_from(&[("AI_AGENT", value)])),
            Some(expected),
            "AI_AGENT={value} should strip the version and resolve to {}",
            expected.as_str(),
        );
    }
}

#[test]
fn ai_agent_is_lenient_about_separators() {
    for value in ["claude code", "claude--code", "  Claude-Code  "] {
        assert_eq!(
            detect_with(env_from(&[("AI_AGENT", value)])),
            Some(Agent::ClaudeCode),
            "AI_AGENT={value:?} should normalize to claude-code",
        );
    }
}

#[test]
fn detect_crush() {
    assert_eq!(
        detect_with(env_from(&[("AI_AGENT", "crush")])),
        Some(Agent::Crush),
    );
}

#[test]
fn detect_pulumi_neo() {
    assert_eq!(
        detect_with(env_from(&[("AI_AGENT", "neo")])),
        Some(Agent::PulumiNeo),
    );
}

#[test]
fn detect_goose_via_agent_var() {
    assert_eq!(
        detect_with(env_from(&[("AGENT", "goose")])),
        Some(Agent::Goose),
    );
}

#[test]
fn detect_amp_via_agent_var() {
    assert_eq!(detect_with(env_from(&[("AGENT", "amp")])), Some(Agent::Amp));
}

#[test]
fn agent_var_is_normalized() {
    // The `AGENT` value goes through the same normalization as `AI_AGENT`.
    for value in ["Goose", "GOOSE", "goose@1", "  goose  "] {
        assert_eq!(
            detect_with(env_from(&[("AGENT", value)])),
            Some(Agent::Goose),
            "AGENT={value:?} should normalize to goose",
        );
    }
}

#[test]
fn agent_var_only_matches_known_values() {
    // `AGENT` is generic, so an unrecognized value must not be treated as
    // an agent (unlike `AI_AGENT`, which falls back to `Unknown`).
    assert_eq!(detect_with(env_from(&[("AGENT", "smith")])), None);
    assert_eq!(detect_with(env_from(&[("AGENT", "")])), None);
}

#[test]
fn ai_agent_wins_over_agent_var() {
    assert_eq!(
        detect_with(env_from(&[("AI_AGENT", "codex"), ("AGENT", "goose")])),
        Some(Agent::Codex),
    );
}

#[test]
fn tool_specific_marker_wins_over_stale_agent_var() {
    // A generic `AGENT` can be inherited from a parent shell; a fresh
    // tool-specific marker must still identify the real driver.
    assert_eq!(
        detect_with(env_from(&[("CODEX_THREAD_ID", "t1"), ("AGENT", "goose")])),
        Some(Agent::Codex),
    );
}

#[test]
fn detect_cline() {
    assert_eq!(
        detect_with(env_from(&[("CLINE_ACTIVE", "true")])),
        Some(Agent::Cline),
    );
}

#[test]
fn detect_roo_code() {
    assert_eq!(
        detect_with(env_from(&[("ROO_CLI_RUNTIME", "1")])),
        Some(Agent::RooCode),
    );
    assert_eq!(
        detect_with(env_from(&[("ROO_ACTIVE", "true")])),
        Some(Agent::RooCode),
    );
}

#[test]
fn detect_trae() {
    assert_eq!(
        detect_with(env_from(&[("TRAE_AI_SHELL_ID", "abc123")])),
        Some(Agent::Trae),
    );
}

#[test]
fn detect_tabnine_cli() {
    assert_eq!(
        detect_with(env_from(&[("TABNINE_CLI", "1")])),
        Some(Agent::TabnineCli),
    );
}

#[test]
fn detect_pi() {
    assert_eq!(
        detect_with(env_from(&[("PI_CODING_AGENT", "true")])),
        Some(Agent::Pi),
    );
}

#[test]
fn detect_new_invocation_specific_markers() {
    let cases = [
        ("CODEBUDDY_SESSION_ID", "session-123", Agent::CodeBuddy),
        ("CODEBUDDY_PROJECT_DIR", "/tmp/project", Agent::CodeBuddy),
        ("GROK_AGENT", "1", Agent::GrokBuild),
        ("OZ_HARNESS", "oz", Agent::Warp),
        ("OPENCLAW_SHELL", "exec", Agent::OpenClaw),
        ("DSH_SHELL", "1", Agent::DeepSeekHarness),
        ("GOOSE_TERMINAL", "1", Agent::Goose),
    ];

    for (var, value, expected) in cases {
        assert_eq!(
            detect_with(env_from(&[(var, value)])),
            Some(expected),
            "{var} should detect {}",
            expected.as_str(),
        );
    }
}

#[test]
fn detect_amazon_q_execution_environment() {
    assert_eq!(
        detect_with(env_from(&[(
            "AWS_EXECUTION_ENV",
            "existing AmazonQ-For-CLI Version/1.2.3",
        )])),
        Some(Agent::AmazonQ),
    );
}

#[test]
fn detect_openhands_terminal_metadata() {
    for var in ["PS1", "PROMPT_COMMAND"] {
        assert_eq!(
            detect_with(env_from(&[(var, "prefix ###PS1JSON### suffix")])),
            Some(Agent::OpenHands),
            "{var} should detect OpenHands",
        );
    }
}

#[test]
fn reject_non_agent_runtime_values() {
    for (var, value) in [
        ("GROK_AGENT", "true"),
        ("OPENCLAW_SHELL", "tui-local"),
        ("OPENCLAW_SHELL", "acp-client"),
        ("DSH_SHELL", "true"),
        ("GOOSE_TERMINAL", "true"),
    ] {
        assert_eq!(
            detect_with(env_from(&[(var, value)])),
            None,
            "{var}={value} is not an agent invocation marker",
        );
    }
}

#[test]
fn nested_agent_marker_wins_over_outer_runtime_markers() {
    for (var, value) in [
        ("CODEBUDDY_SESSION_ID", "session-123"),
        ("GROK_AGENT", "1"),
        ("OPENCLAW_SHELL", "exec"),
    ] {
        assert_eq!(
            detect_with(env_from(&[(var, value), ("CLAUDE_CODE", "1")])),
            Some(Agent::ClaudeCode),
            "Claude should win over inherited {var}",
        );
        assert_eq!(
            detect_with(env_from(&[(var, value), ("DSH_SHELL", "1")])),
            Some(Agent::DeepSeekHarness),
            "DeepSeek Harness should win over inherited {var}",
        );
    }
}

#[test]
fn nested_qwen_marker_wins_over_amazon_q_runtime_marker() {
    assert_eq!(
        detect_with(env_from(&[
            ("AWS_EXECUTION_ENV", "AmazonQ-For-CLI Version/1.2.3"),
            ("QWEN_CODE", "1"),
        ])),
        Some(Agent::QwenCode),
    );
}

#[test]
fn selected_harness_marker_wins_over_warp_orchestrator() {
    let vars = [
        ("OZ_HARNESS", "claude"),
        ("OZ_RUN_ID", "task-123"),
        ("CLAUDE_CODE", "1"),
    ];
    assert_eq!(detect_with(env_from(&vars)), Some(Agent::ClaudeCode),);
}

#[test]
fn warp_run_id_is_a_legacy_fallback_without_harness_identity() {
    assert_eq!(
        detect_with(env_from(&[("OZ_RUN_ID", "task-123")])),
        Some(Agent::Warp),
    );
}

#[test]
fn warp_run_id_does_not_override_an_unknown_harness_identity() {
    assert_eq!(
        detect_with(env_from(&[
            ("OZ_HARNESS", "future-harness"),
            ("OZ_RUN_ID", "task-123"),
        ])),
        None,
    );
}

#[test]
fn nested_agent_marker_wins_over_openhands_terminal() {
    assert_eq!(
        detect_with(env_from(&[("PS1", "###PS1JSON###"), ("CLAUDE_CODE", "1"),])),
        Some(Agent::ClaudeCode),
    );
}

#[test]
fn kiro_marker_wins_over_legacy_amazon_q_compatibility_marker() {
    assert_eq!(
        detect_with(env_from(&[
            ("AGENT_DISPLAY_OUT", "/tmp/display"),
            ("AGENT_CONTEXT_OUT", "/tmp/context"),
            ("AWS_EXECUTION_ENV", "AmazonQ-For-CLI Version/1.2.3"),
        ])),
        Some(Agent::KiroCli),
    );
}

#[test]
fn agent_name_roundtrip() {
    let agents = [
        Agent::ClaudeCode,
        Agent::ClaudeCodeCowork,
        Agent::Cursor,
        Agent::CursorCli,
        Agent::Codex,
        Agent::KiroCli,
        Agent::Junie,
        Agent::QwenCode,
        Agent::GitLabDuoCli,
        Agent::KiloCode,
        Agent::Hermes,
        Agent::Devin,
        Agent::Dirac,
        Agent::GeminiCli,
        Agent::GitHubCopilot,
        Agent::OpenCode,
        Agent::Poolside,
        Agent::Augment,
        Agent::Antigravity,
        Agent::Replit,
        Agent::V0,
        Agent::Crush,
        Agent::PulumiNeo,
        Agent::Goose,
        Agent::Amp,
        Agent::Cline,
        Agent::RooCode,
        Agent::Trae,
        Agent::TabnineCli,
        Agent::Pi,
        Agent::AmazonQ,
        Agent::CodeBuddy,
        Agent::GrokBuild,
        Agent::Warp,
        Agent::OpenHands,
        Agent::OpenClaw,
        Agent::DeepSeekHarness,
        Agent::Unknown,
    ];
    for agent in agents {
        // Completeness guard: this wildcard-free match stops compiling when a
        // variant is added to `Agent`, forcing whoever adds one to revisit
        // this test — and, by proximity, the `agents` list above.
        match agent {
            Agent::ClaudeCode
            | Agent::ClaudeCodeCowork
            | Agent::Cursor
            | Agent::CursorCli
            | Agent::Codex
            | Agent::KiroCli
            | Agent::Junie
            | Agent::QwenCode
            | Agent::GitLabDuoCli
            | Agent::KiloCode
            | Agent::Hermes
            | Agent::Devin
            | Agent::Dirac
            | Agent::GeminiCli
            | Agent::GitHubCopilot
            | Agent::OpenCode
            | Agent::Poolside
            | Agent::Augment
            | Agent::Antigravity
            | Agent::Replit
            | Agent::V0
            | Agent::Crush
            | Agent::PulumiNeo
            | Agent::Goose
            | Agent::Amp
            | Agent::Cline
            | Agent::RooCode
            | Agent::Trae
            | Agent::TabnineCli
            | Agent::Pi
            | Agent::AmazonQ
            | Agent::CodeBuddy
            | Agent::GrokBuild
            | Agent::Warp
            | Agent::OpenHands
            | Agent::OpenClaw
            | Agent::DeepSeekHarness
            | Agent::Unknown => {}
        }
        let lookup = env_from(&[("AI_AGENT", agent.as_str())]);
        assert_eq!(
            detect_with(lookup),
            Some(agent),
            "roundtrip failed for {}",
            agent.as_str()
        );
    }
}

#[test]
fn from_str_accepts_aliases_and_normalizes() {
    for (value, expected) in [
        ("claude-code", Agent::ClaudeCode),
        ("claude", Agent::ClaudeCode),
        ("Claude_Code", Agent::ClaudeCode),
        ("claude-code@2", Agent::ClaudeCode),
        ("gemini", Agent::GeminiCli),
    ] {
        assert_eq!(value.parse(), Ok(expected), "{value:?} should parse");
    }
}

#[test]
fn from_str_matches_detection_leniency() {
    // The public parse accepts everything the `AI_AGENT` detection path
    // accepts, including decorated values.
    for (value, expected) in [
        ("claude-code_2-1-202_agent", Agent::ClaudeCode),
        ("gitlab-lsp_7.17.0__duo-cli", Agent::GitLabDuoCli),
    ] {
        assert_eq!(value.parse(), Ok(expected), "{value:?} should parse");
    }
}

#[test]
fn from_str_rejects_unknown_names() {
    // A strict parse: unknown values error instead of resolving to
    // `Agent::Unknown` the way `AI_AGENT` detection does.
    for value in ["some-new-agent", "unknown", ""] {
        assert_eq!(value.parse::<Agent>(), Err(ParseAgentError));
    }
}
