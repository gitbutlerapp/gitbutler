//! Detect which AI coding agent is invoking the CLI, if any.
//!
//! This checks well-known environment variables set by various AI coding agents
//! when they spawn shell commands. Based on the detection approach used by
//! `@vercel/detect-agent`.

use std::env;
use std::ffi::OsString;

macro_rules! environment_variables {
    ($($name:ident = $value:literal),+ $(,)?) => {
        $(const $name: &str = $value;)+

        /// Every environment variable consulted during agent detection.
        pub const ENVIRONMENT_VARIABLES: &[&str] = &[$($value),+];
    };
}

environment_variables! {
    AI_AGENT = "AI_AGENT",
    CLAUDE_CODE_IS_COWORK = "CLAUDE_CODE_IS_COWORK",
    CLAUDE_CODE = "CLAUDE_CODE",
    CLAUDECODE = "CLAUDECODE",
    CURSOR_AGENT = "CURSOR_AGENT",
    CURSOR_EXTENSION_HOST_ROLE = "CURSOR_EXTENSION_HOST_ROLE",
    CURSOR_TRACE_ID = "CURSOR_TRACE_ID",
    CODEX_SANDBOX = "CODEX_SANDBOX",
    CODEX_CI = "CODEX_CI",
    CODEX_THREAD_ID = "CODEX_THREAD_ID",
    CODEX_SHELL = "CODEX_SHELL",
    AGENT_DISPLAY_OUT = "AGENT_DISPLAY_OUT",
    AGENT_CONTEXT_OUT = "AGENT_CONTEXT_OUT",
    QWEN_CODE = "QWEN_CODE",
    GEMINI_CLI = "GEMINI_CLI",
    COPILOT_AGENT = "COPILOT_AGENT",
    JUNIE_DATA = "JUNIE_DATA",
    JUNIE_SHIM_PATH = "JUNIE_SHIM_PATH",
    KILO_PID = "KILO_PID",
    HERMES_SESSION_ID = "HERMES_SESSION_ID",
    OPENCODE_CLIENT = "OPENCODE_CLIENT",
    OPENCODE = "OPENCODE",
    AUGMENT_AGENT = "AUGMENT_AGENT",
    ANTIGRAVITY_AGENT = "ANTIGRAVITY_AGENT",
    REPL_ID = "REPL_ID",
    DIRAC_ACTIVE = "DIRAC_ACTIVE",
    CLINE_ACTIVE = "CLINE_ACTIVE",
    ROO_CLI_RUNTIME = "ROO_CLI_RUNTIME",
    ROO_ACTIVE = "ROO_ACTIVE",
    TRAE_AI_SHELL_ID = "TRAE_AI_SHELL_ID",
    TABNINE_CLI = "TABNINE_CLI",
    PI_CODING_AGENT = "PI_CODING_AGENT",
    GOOSE_TERMINAL = "GOOSE_TERMINAL",
    AWS_EXECUTION_ENV = "AWS_EXECUTION_ENV",
    CODEBUDDY_SESSION_ID = "CODEBUDDY_SESSION_ID",
    CODEBUDDY_PROJECT_DIR = "CODEBUDDY_PROJECT_DIR",
    GROK_AGENT = "GROK_AGENT",
    OPENCLAW_SHELL = "OPENCLAW_SHELL",
    PS1 = "PS1",
    PROMPT_COMMAND = "PROMPT_COMMAND",
    OZ_HARNESS = "OZ_HARNESS",
    OZ_RUN_ID = "OZ_RUN_ID",
    AGENT = "AGENT",
}

/// An AI coding agent that may be driving the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Agent {
    ClaudeCode,
    ClaudeCodeCowork,
    Cursor,
    CursorCli,
    Codex,
    KiroCli,
    Junie,
    QwenCode,
    GitLabDuoCli,
    KiloCode,
    Hermes,
    Devin,
    Dirac,
    GeminiCli,
    GitHubCopilot,
    OpenCode,
    Poolside,
    Augment,
    Antigravity,
    Replit,
    V0,
    Crush,
    PulumiNeo,
    Goose,
    Amp,
    Cline,
    RooCode,
    Trae,
    TabnineCli,
    Pi,
    AmazonQ,
    CodeBuddy,
    GrokBuild,
    Warp,
    OpenHands,
    OpenClaw,
    Unknown,
}

impl Agent {
    /// A short, stable identifier suitable for telemetry or output-format decisions.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::ClaudeCodeCowork => "claude-code-cowork",
            Self::Cursor => "cursor",
            Self::CursorCli => "cursor-cli",
            Self::Codex => "codex",
            Self::KiroCli => "kiro-cli",
            Self::Junie => "junie",
            Self::QwenCode => "qwen-code",
            Self::GitLabDuoCli => "gitlab-duo-cli",
            Self::KiloCode => "kilo-code",
            Self::Hermes => "hermes-agent",
            Self::Devin => "devin",
            Self::Dirac => "dirac",
            Self::GeminiCli => "gemini-cli",
            Self::GitHubCopilot => "github-copilot",
            Self::OpenCode => "opencode",
            Self::Poolside => "poolside",
            Self::Augment => "augment",
            Self::Antigravity => "antigravity",
            Self::Replit => "replit",
            Self::V0 => "v0",
            Self::Crush => "crush",
            Self::PulumiNeo => "pulumi-neo",
            Self::Goose => "goose",
            Self::Amp => "amp",
            Self::Cline => "cline",
            Self::RooCode => "roo-code",
            Self::Trae => "trae",
            Self::TabnineCli => "tabnine-cli",
            Self::Pi => "pi",
            Self::AmazonQ => "amazon-q",
            Self::CodeBuddy => "codebuddy",
            Self::GrokBuild => "grok-build",
            Self::Warp => "warp",
            Self::OpenHands => "openhands",
            Self::OpenClaw => "openclaw",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when parsing a string that does not name a known agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseAgentError;

impl std::fmt::Display for ParseAgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("unrecognized agent name")
    }
}

impl std::error::Error for ParseAgentError {}

impl std::str::FromStr for Agent {
    type Err = ParseAgentError;

    /// Parse an agent identifier exactly the way detection interprets
    /// `AI_AGENT` values: normalized (`Claude_Code`, `claude code`, and
    /// `claude-code@2` all parse to [`Agent::ClaudeCode`]), accepting known
    /// aliases and version-decorated prefixes such as
    /// `claude-code_2-1-202_agent`.
    ///
    /// Unlike detection this never falls back to [`Agent::Unknown`]:
    /// unrecognized values are an error.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match_normalized_value(&normalize_agent_value(s)).ok_or(ParseAgentError)
    }
}

/// Match an already-normalized identifier against known agents, including
/// decorated forms.
fn match_normalized_value(val: &str) -> Option<Agent> {
    // GitLab decorates Duo CLI with the LSP version between its product
    // names, for example `gitlab-lsp_7.17.0__duo-cli`.
    if val.starts_with("gitlab-lsp-") && val.ends_with("-duo-cli") {
        return Some(Agent::GitLabDuoCli);
    }
    match_agent_name(val).or_else(|| match_agent_name_prefix(val))
}

/// Detect the current AI coding agent from environment variables.
///
/// Returns `None` when the CLI appears to be invoked by a human.
/// Checks the generic `AI_AGENT` variable first, then tool-specific
/// variables in priority order, and finally the generic `AGENT` variable
/// as a last-resort fallback.
pub fn detect() -> Option<Agent> {
    detect_with(|key| env::var_os(key))
}

/// Core detection logic, parameterised over an env-var lookup function.
///
/// [`detect`] is this with [`std::env::var_os`] as the lookup; injecting the
/// lookup keeps detection testable and lets embedders supply a snapshot of the
/// environment instead of the live process environment.
pub fn detect_with(lookup: impl Fn(&str) -> Option<OsString>) -> Option<Agent> {
    let is_set = |var: &str| lookup(var).is_some_and(|v| !v.is_empty());
    let is_value =
        |var: &str, expected: &str| lookup(var).is_some_and(|v| v.to_str() == Some(expected));
    let contains = |var: &str, needle: &str| {
        lookup(var).is_some_and(|v| v.to_str().is_some_and(|value| value.contains(needle)))
    };

    // Generic `AI_AGENT` convention (as documented by `@vercel/detect-agent`).
    if let Some(agent) = parse_ai_agent_var(&lookup) {
        return Some(agent);
    }

    // Tool-specific variables. Order is part of the detection contract: known
    // compatibility markers must follow the more specific agent they can mimic.
    if is_set(CLAUDE_CODE_IS_COWORK) {
        return Some(Agent::ClaudeCodeCowork);
    }
    if is_set(CLAUDE_CODE) || is_set(CLAUDECODE) {
        return Some(Agent::ClaudeCode);
    }
    if is_set(CURSOR_AGENT) || is_value(CURSOR_EXTENSION_HOST_ROLE, "agent-exec") {
        return Some(Agent::CursorCli);
    }
    if is_set(CURSOR_TRACE_ID) {
        return Some(Agent::Cursor);
    }
    if is_set(CODEX_SANDBOX) || is_set(CODEX_CI) || is_set(CODEX_THREAD_ID) || is_set(CODEX_SHELL) {
        return Some(Agent::Codex);
    }
    // Kiro exposes both FIFO paths only while its agent is driving a command.
    if is_set(AGENT_DISPLAY_OUT) && is_set(AGENT_CONTEXT_OUT) {
        return Some(Agent::KiroCli);
    }
    if is_value(QWEN_CODE, "1") {
        return Some(Agent::QwenCode);
    }
    if is_set(GEMINI_CLI) {
        return Some(Agent::GeminiCli);
    }
    if is_set(COPILOT_AGENT) {
        return Some(Agent::GitHubCopilot);
    }
    if is_set(JUNIE_DATA) || is_set(JUNIE_SHIM_PATH) {
        return Some(Agent::Junie);
    }
    // Kilo is an OpenCode fork, so its specific marker must win.
    if is_set(KILO_PID) {
        return Some(Agent::KiloCode);
    }
    if is_set(HERMES_SESSION_ID) {
        return Some(Agent::Hermes);
    }
    if is_set(OPENCODE_CLIENT) || is_set(OPENCODE) {
        return Some(Agent::OpenCode);
    }
    if is_set(AUGMENT_AGENT) {
        return Some(Agent::Augment);
    }
    if is_set(ANTIGRAVITY_AGENT) {
        return Some(Agent::Antigravity);
    }
    if is_set(REPL_ID) {
        return Some(Agent::Replit);
    }
    // Agents that set neither `AI_AGENT` nor `AGENT`, only a private marker.
    // These markers are per-mode: Dirac and Cline set `DIRAC_ACTIVE` and
    // `CLINE_ACTIVE` from their VS Code extensions; Roo Code sets
    // `ROO_CLI_RUNTIME` from its headless CLI and `ROO_ACTIVE` from its
    // extension. Presence is enough to identify the agent when it is set.
    // Dirac's standalone CLI does not currently set an agent marker, so it
    // cannot be distinguished automatically from a human shell.
    if is_set(DIRAC_ACTIVE) {
        return Some(Agent::Dirac);
    }
    if is_set(CLINE_ACTIVE) {
        return Some(Agent::Cline);
    }
    if is_set(ROO_CLI_RUNTIME) || is_set(ROO_ACTIVE) {
        return Some(Agent::RooCode);
    }
    if is_set(TRAE_AI_SHELL_ID) {
        return Some(Agent::Trae);
    }
    if is_set(TABNINE_CLI) {
        return Some(Agent::TabnineCli);
    }
    if is_set(PI_CODING_AGENT) {
        return Some(Agent::Pi);
    }
    if is_value(GOOSE_TERMINAL, "1") {
        return Some(Agent::Goose);
    }
    // These runtime markers are inherited by nested agents, so a nested agent's
    // own marker above must win over the outer command runner.
    if contains(AWS_EXECUTION_ENV, "AmazonQ-For-CLI") {
        return Some(Agent::AmazonQ);
    }
    if is_set(CODEBUDDY_SESSION_ID) || is_set(CODEBUDDY_PROJECT_DIR) {
        return Some(Agent::CodeBuddy);
    }
    if is_value(GROK_AGENT, "1") {
        return Some(Agent::GrokBuild);
    }
    if is_value(OPENCLAW_SHELL, "exec") {
        return Some(Agent::OpenClaw);
    }
    if contains(PS1, "###PS1JSON###") || contains(PROMPT_COMMAND, "###PS1JSON###") {
        return Some(Agent::OpenHands);
    }
    if is_value(OZ_HARNESS, "oz") {
        return Some(Agent::Warp);
    }
    // Keep the run ID as a compatibility fallback only when no harness identity
    // is available; it must not override an explicit delegated harness.
    if !is_set(OZ_HARNESS) && is_set(OZ_RUN_ID) {
        return Some(Agent::Warp);
    }

    // Shorter `AGENT` convention (Goose, Amp, Crush, Codex), checked last: it is
    // a generic variable name and can be stale or inherited from a parent shell,
    // so a fresh tool-specific marker above should win over it.
    if let Some(agent) = parse_agent_var(&lookup) {
        return Some(agent);
    }

    None
}

/// Parse the generic `AI_AGENT` env var into an agent detection.
///
/// A non-empty value we don't recognize still resolves to `Agent::Unknown`:
/// setting `AI_AGENT` is an explicit "an agent is driving this" signal, so we
/// keep it even when we can't name the agent.
fn parse_ai_agent_var(lookup: &impl Fn(&str) -> Option<OsString>) -> Option<Agent> {
    let val = normalize_agent_value(&lookup(AI_AGENT)?.to_string_lossy());
    if val.is_empty() {
        return None;
    }
    Some(match_normalized_value(&val).unwrap_or(Agent::Unknown))
}

/// Match a normalized agent identifier whose leading segments name a known
/// agent, tolerating trailing decorations that embed a version without the
/// `@` separator — Claude Code desktop sets e.g. `claude-code_2-1-202_agent`.
/// Segments are dropped from the end one at a time, so the longest matching
/// prefix wins (`claude-code-...` resolves before the `claude` alias could).
fn match_agent_name_prefix(val: &str) -> Option<Agent> {
    let mut prefix = val;
    while let Some((rest, _)) = prefix.rsplit_once('-') {
        if let Some(agent) = match_agent_name(rest) {
            return Some(agent);
        }
        prefix = rest;
    }
    None
}

/// Parse the shorter `AGENT` convention (distinct from `AI_AGENT`), adopted by
/// Goose (`AGENT=goose`), Amp (`AGENT=amp`), Crush (`AGENT=crush`), and Codex.
///
/// `AGENT` is a generic variable name, so only a strict allowlist of known
/// values counts as an agent signal — anything else is ignored (returns `None`)
/// rather than reported as `Agent::Unknown`, to avoid false positives.
fn parse_agent_var(lookup: &impl Fn(&str) -> Option<OsString>) -> Option<Agent> {
    match normalize_agent_value(&lookup(AGENT)?.to_string_lossy()).as_str() {
        "goose" => Some(Agent::Goose),
        "amp" => Some(Agent::Amp),
        "crush" => Some(Agent::Crush),
        "codex" => Some(Agent::Codex),
        _ => None,
    }
}

/// Map a recognized agent identifier to an [`Agent`], or `None` if unknown.
fn match_agent_name(val: &str) -> Option<Agent> {
    Some(match val {
        "claude" | "claude-code" => Agent::ClaudeCode,
        "cowork" | "claude-code-cowork" => Agent::ClaudeCodeCowork,
        "cursor" => Agent::Cursor,
        "cursor-cli" => Agent::CursorCli,
        "codex" => Agent::Codex,
        "kiro" | "kiro-cli" => Agent::KiroCli,
        "junie" => Agent::Junie,
        "qwen" | "qwen-code" => Agent::QwenCode,
        "gitlab-duo" | "gitlab-duo-cli" => Agent::GitLabDuoCli,
        "kilo" | "kilo-code" => Agent::KiloCode,
        "hermes" | "hermes-agent" => Agent::Hermes,
        "devin" => Agent::Devin,
        "dirac" => Agent::Dirac,
        "gemini" | "gemini-cli" => Agent::GeminiCli,
        "copilot" | "github-copilot" | "github-copilot-cli" | "github-copilot-vscode-agent" => {
            Agent::GitHubCopilot
        }
        "opencode" => Agent::OpenCode,
        "poolside" | "pool" => Agent::Poolside,
        "augment" | "augment-cli" => Agent::Augment,
        "antigravity" => Agent::Antigravity,
        "replit" => Agent::Replit,
        "v0" => Agent::V0,
        "crush" => Agent::Crush,
        "neo" | "pulumi-neo" => Agent::PulumiNeo,
        "goose" => Agent::Goose,
        "amp" => Agent::Amp,
        "cline" => Agent::Cline,
        "roo-code" => Agent::RooCode,
        "trae" => Agent::Trae,
        "tabnine-cli" => Agent::TabnineCli,
        "pi" => Agent::Pi,
        "amazon-q" | "amazon-q-developer" | "amazon-q-developer-cli" => Agent::AmazonQ,
        "codebuddy" | "codebuddy-code" => Agent::CodeBuddy,
        "grok" | "grok-build" => Agent::GrokBuild,
        "warp" | "warp-oz" => Agent::Warp,
        "openhands" | "open-hands" => Agent::OpenHands,
        "openclaw" | "open-claw" => Agent::OpenClaw,
        _ => return None,
    })
}

/// Normalize an agent identifier so casing and separator drift still match a
/// canonical name. Trims, drops any `@version` suffix (the `AI_AGENT` naming
/// convention allows e.g. `claude-code@1`), lowercases, and collapses runs of
/// `_`, `-`, and whitespace into a single `-` (so `Claude_Code`, `claude code`,
/// and `claude-code@2` all resolve to `claude-code`).
fn normalize_agent_value(raw: &str) -> String {
    raw.trim()
        .split('@')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .split(|c: char| c == '_' || c == '-' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests;
