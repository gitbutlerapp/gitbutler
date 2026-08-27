//! A place for each command, i.e. `but foo` as `pub mod foo` here.
#[cfg(feature = "legacy")]
pub mod legacy;

pub mod agent;
pub mod alias;
pub mod branch;
pub mod comment;
pub mod completions;
pub mod config;
pub mod expand;
pub(crate) mod external;
pub(crate) mod git_config;
pub mod gui;
pub mod help;
pub mod mcp;
pub mod onboarding;
pub mod open;
pub mod push;
pub mod skill;
pub mod update;
pub mod worktree;

/// The durable result of a command, independent of rendering and transport.
#[derive(Debug, Clone)]
pub enum CommandOutcome {
    AgentSetupPrintOnly,
    AgentSetupCancelled,
    AgentSetupCompleted {
        manual_instructions_required: bool,
    },
    #[cfg(feature = "legacy")]
    Commit(legacy::commit::CommitOutcome),
}
