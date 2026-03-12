//! Detect which AI coding agent is invoking the CLI, if any.
//!
//! This checks well-known environment variables set by various AI coding agents
//! when they spawn shell commands. Based on the detection approach used by
//! `@vercel/detect-agent`.

use std::env;
use std::ffi::OsString;

/// An AI coding agent that may be driving the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    ClaudeCode,
    ClaudeCodeCowork,
    Cursor,
    CursorCli,
    Codex,
    GeminiCli,
    GitHubCopilot,
    OpenCode,
    Augment,
    Antigravity,
    Replit,
}

impl Agent {
    /// A short, stable identifier suitable for telemetry or output-format decisions.
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::ClaudeCodeCowork => "claude-code-cowork",
            Self::Cursor => "cursor",
            Self::CursorCli => "cursor-cli",
            Self::Codex => "codex",
            Self::GeminiCli => "gemini-cli",
            Self::GitHubCopilot => "github-copilot",
            Self::OpenCode => "opencode",
            Self::Augment => "augment",
            Self::Antigravity => "antigravity",
            Self::Replit => "replit",
        }
    }
}

impl std::fmt::Display for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Detect the current AI coding agent from environment variables.
///
/// Returns `None` when the CLI appears to be invoked by a human.
/// Checks the generic `AI_AGENT` variable first, then falls back to
/// tool-specific variables in priority order.
pub fn detect() -> Option<Agent> {
    detect_with(|key| env::var_os(key))
}

/// Core detection logic, parameterised over an env-var lookup function for testability.
fn detect_with(lookup: impl Fn(&str) -> Option<OsString>) -> Option<Agent> {
    let is_set = |var: &str| lookup(var).is_some_and(|v| !v.is_empty());

    // Generic AI_AGENT standard (see https://github.com/anthropics/agent-env).
    if let Some(agent) = parse_ai_agent_var(&lookup) {
        return Some(agent);
    }

    // Tool-specific variables, roughly ordered by popularity.
    if is_set("CLAUDE_CODE_IS_COWORK") {
        return Some(Agent::ClaudeCodeCowork);
    }
    if is_set("CLAUDE_CODE") || is_set("CLAUDECODE") {
        return Some(Agent::ClaudeCode);
    }
    if is_set("CURSOR_AGENT") {
        return Some(Agent::CursorCli);
    }
    if is_set("CURSOR_TRACE_ID") {
        return Some(Agent::Cursor);
    }
    if is_set("CODEX_SANDBOX") || is_set("CODEX_CI") || is_set("CODEX_THREAD_ID") {
        return Some(Agent::Codex);
    }
    if is_set("GEMINI_CLI") {
        return Some(Agent::GeminiCli);
    }
    if is_set("COPILOT_MODEL") || is_set("COPILOT_ALLOW_ALL") || is_set("COPILOT_GITHUB_TOKEN") {
        return Some(Agent::GitHubCopilot);
    }
    if is_set("OPENCODE_CLIENT") {
        return Some(Agent::OpenCode);
    }
    if is_set("AUGMENT_AGENT") {
        return Some(Agent::Augment);
    }
    if is_set("ANTIGRAVITY_AGENT") {
        return Some(Agent::Antigravity);
    }
    if is_set("REPL_ID") {
        return Some(Agent::Replit);
    }

    None
}

/// Parse the generic `AI_AGENT` env var into a known agent, if possible.
fn parse_ai_agent_var(lookup: &impl Fn(&str) -> Option<OsString>) -> Option<Agent> {
    let val = lookup("AI_AGENT")?;
    let val = val.to_str()?.trim().to_ascii_lowercase();
    match val.as_str() {
        "claude-code" => Some(Agent::ClaudeCode),
        "claude-code-cowork" => Some(Agent::ClaudeCodeCowork),
        "cursor" => Some(Agent::Cursor),
        "cursor-cli" => Some(Agent::CursorCli),
        "codex" => Some(Agent::Codex),
        "gemini-cli" => Some(Agent::GeminiCli),
        "github-copilot" => Some(Agent::GitHubCopilot),
        "opencode" => Some(Agent::OpenCode),
        "augment" => Some(Agent::Augment),
        "antigravity" => Some(Agent::Antigravity),
        "replit" => Some(Agent::Replit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
    fn detect_codex() {
        assert_eq!(
            detect_with(env_from(&[("CODEX_SANDBOX", "seatbelt")])),
            Some(Agent::Codex),
        );
    }

    #[test]
    fn detect_gemini() {
        assert_eq!(
            detect_with(env_from(&[("GEMINI_CLI", "1")])),
            Some(Agent::GeminiCli),
        );
    }

    #[test]
    fn detect_copilot() {
        assert_eq!(
            detect_with(env_from(&[("COPILOT_MODEL", "gpt-4")])),
            Some(Agent::GitHubCopilot),
        );
    }

    #[test]
    fn detect_opencode() {
        assert_eq!(
            detect_with(env_from(&[("OPENCODE_CLIENT", "1")])),
            Some(Agent::OpenCode),
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
    fn detect_none_when_clean() {
        assert_eq!(detect_with(|_| None), None);
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
    fn ai_agent_case_insensitive() {
        assert_eq!(
            detect_with(env_from(&[("AI_AGENT", "Claude-Code")])),
            Some(Agent::ClaudeCode),
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
            Agent::GeminiCli,
            Agent::GitHubCopilot,
            Agent::OpenCode,
            Agent::Augment,
            Agent::Antigravity,
            Agent::Replit,
        ];
        for agent in agents {
            let lookup = env_from(&[("AI_AGENT", agent.name())]);
            assert_eq!(
                detect_with(lookup),
                Some(agent),
                "roundtrip failed for {}",
                agent.name()
            );
        }
    }
}
