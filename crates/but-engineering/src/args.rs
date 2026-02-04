//! CLI argument parsing for but-engineering.

use clap::{Parser, Subcommand, ValueEnum};

/// A coordination system for coding agents working in the same repository.
#[derive(Debug, Parser)]
#[clap(
    name = "but-engineering",
    about = "Coordinate coding agents working in the same repository",
    version = option_env!("VERSION").unwrap_or("dev"),
)]
pub struct Args {
    /// Subcommand to run.
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

/// Which Claude Code hook event to handle.
#[derive(Debug, Clone, ValueEnum)]
pub enum HookEvent {
    /// UserPromptSubmit — fires before every prompt is processed.
    #[value(alias = "prompt")]
    UserPromptSubmit,
    /// PreToolUse — fires before Edit/Write/MultiEdit tool calls.
    #[value(alias = "tool")]
    PreToolUse,
}

#[derive(Debug, Subcommand)]
pub enum Subcommands {
    /// Post a message to the shared channel.
    Post {
        /// The message content to post.
        content: String,

        /// Unique identifier for this agent.
        #[clap(long)]
        agent_id: String,
    },

    /// Read messages from the shared channel.
    Read {
        /// Unique identifier for this agent.
        #[clap(long)]
        agent_id: String,

        /// Read messages since this timestamp (RFC 3339 format).
        #[clap(long)]
        since: Option<String>,

        /// Read only unread messages (based on agent's last_read).
        /// This is the default behavior.
        #[clap(long, default_value = "true")]
        unread: bool,

        /// Block and wait for new messages.
        #[clap(long)]
        wait: bool,

        /// Timeout for waiting (e.g., "30s", "5m"). If not specified, waits indefinitely.
        #[clap(long)]
        timeout: Option<String>,
    },

    /// Set or clear agent status.
    Status {
        /// Unique identifier for this agent.
        #[clap(long)]
        agent_id: String,

        /// Status message to set. If not provided and --clear is not set, returns current status.
        status_message: Option<String>,

        /// Clear the current status.
        #[clap(long)]
        clear: bool,
    },

    /// List active agents.
    Agents {
        /// Filter to agents active within this duration (e.g., "5m", "1h").
        #[clap(long)]
        active_within: Option<String>,
    },

    /// Claim files you're about to edit, so other agents know not to touch them.
    Claim {
        /// File paths to claim.
        #[clap(required = true)]
        paths: Vec<String>,

        /// Unique identifier for this agent.
        #[clap(long)]
        agent_id: String,
    },

    /// Release file claims when you're done editing.
    Release {
        /// File paths to release. Omit if using --all.
        paths: Vec<String>,

        /// Unique identifier for this agent.
        #[clap(long)]
        agent_id: String,

        /// Release all claims for this agent.
        #[clap(long)]
        all: bool,
    },

    /// List active file claims.
    Claims {
        /// Filter to claims from agents active within this duration (e.g., "5m", "1h").
        #[clap(long)]
        active_within: Option<String>,
    },

    /// Check whether editing a file is safe, returning allow/deny JSON.
    ///
    /// This is a read-only coordination API intended for wrappers/orchestrators
    /// (including Codex-equivalent flows) before performing edits.
    Check {
        /// File path to evaluate for potential conflict.
        file_path: String,

        /// Optional explicit identity for this agent.
        /// If omitted, resolution falls back to env/session/heuristic.
        #[clap(long)]
        agent_id: Option<String>,

        /// Include stack/branch dependency analysis from `but status --json`.
        #[clap(long, default_value_t = false)]
        include_stack: bool,

        /// Optional explicit branch intent (used for stack dependency analysis).
        #[clap(long)]
        intent_branch: Option<String>,
    },

    /// Set or clear your plan (what you intend to do). Other agents see this
    /// before you start, so they can flag conflicts early.
    Plan {
        /// Unique identifier for this agent.
        #[clap(long)]
        agent_id: String,

        /// The plan description. If not provided and --clear is not set, returns current plan.
        plan_message: Option<String>,

        /// Clear the current plan.
        #[clap(long)]
        clear: bool,
    },

    /// Share a discovery — a finding, gotcha, or insight other agents should know.
    /// Discoveries get priority in channel summaries so teammates don't miss them.
    Discover {
        /// The discovery content.
        content: String,

        /// Unique identifier for this agent.
        #[clap(long)]
        agent_id: String,
    },

    /// Announce task completion and clean up coordination state.
    ///
    /// This releases all your claims, clears your plan, and posts a completion
    /// message in one command.
    Done {
        /// Short summary of what was completed.
        summary: String,

        /// Unique identifier for this agent.
        #[clap(long)]
        agent_id: String,
    },

    /// Watch agent chat in a live terminal UI (read-only).
    Lurk,

    /// Handle a Claude Code hook event. Reads hook JSON from stdin and outputs
    /// hook response to stdout. Used in .claude/settings.json hook configuration.
    ///
    /// Example: but-engineering eval user-prompt-submit
    Eval {
        /// Which hook event to handle.
        hook: HookEvent,
    },

    /// Alias for `eval user-prompt-submit` (backwards compatibility).
    #[clap(hide = true)]
    EvalPrompt,
}

impl Subcommands {
    /// Extract the agent_id from any variant that has one.
    pub fn agent_id(&self) -> Option<&str> {
        match self {
            Subcommands::Post { agent_id, .. }
            | Subcommands::Read { agent_id, .. }
            | Subcommands::Status { agent_id, .. }
            | Subcommands::Claim { agent_id, .. }
            | Subcommands::Release { agent_id, .. }
            | Subcommands::Plan { agent_id, .. }
            | Subcommands::Discover { agent_id, .. }
            | Subcommands::Done { agent_id, .. } => Some(agent_id),
            Subcommands::Check { agent_id, .. } => agent_id.as_deref(),
            Subcommands::Agents { .. }
            | Subcommands::Claims { .. }
            | Subcommands::Lurk
            | Subcommands::Eval { .. }
            | Subcommands::EvalPrompt => None,
        }
    }
}
