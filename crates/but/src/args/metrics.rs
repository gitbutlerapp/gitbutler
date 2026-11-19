use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, strum::Display, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
pub enum CommandName {
    Init,
    Setup,
    Absorb,
    Discard,
    Status,
    Tui,
    Uncommit,
    Amend,
    Squash,
    Move,
    Split,
    Diff,
    Edit,
    Show,
    Commit,
    Push,
    Reword,
    OplogList,
    OplogSnapshot,
    Restore,
    Undo,
    Redo,
    Gui,
    Open,
    Pull,
    BranchNew,
    BranchDelete,
    BranchList,
    BranchShow,
    BranchUnapply,
    BranchApply,
    BranchUpdate,
    BranchMove,
    WorktreeList,
    WorktreeArchive,
    WorktreeUnarchive,
    WorktreeRemove,
    WorktreeNew,
    WorktreeRecycle,
    Switch,
    Config,
    ForgeAuth,
    ForgeListUsers,
    ForgeForget,
    PrNew,
    PrTemplate,
    DisableAutoMerge,
    EnableAutoMerge,
    SetReviewReady,
    SetReviewDraft,
    AliasCheck,
    AliasAdd,
    AliasRemove,
    RefreshRemoteData,
    Resolve,
    Update,
    UpdateCheck,
    UpdateSuppress,
    UpdateInstall,
    Land,
    Teardown,
    #[cfg(feature = "nightly")]
    Expand,
    Comment,
    #[cfg(feature = "nightly")]
    Ui,
    Completions,
    Mcp,
    Metrics,
    Help,
    Onboarding,
    AgentLog,
    Skill,
    SkillInstall,
    SkillCheck,
    AgentSetup,
    AgentUninstall,
    Pick,
    Clean,
    External,
}

impl CommandName {
    /// Whether this command reports through the shared CLI event.
    ///
    /// Comments are still experimental, completions are shell-startup noise, and MCP and the
    /// transport child own their own events.
    pub fn emits_cli_event(&self) -> bool {
        !matches!(
            self,
            Self::Comment | Self::Completions | Self::Mcp | Self::Metrics
        )
    }

    /// Sampling rate for successful events, in (0, 1].
    ///
    /// Most failures bypass this policy and are emitted with a rate of 1; `sample_props` names
    /// the exception. In PostHog, estimate command totals, failures included, with
    /// `sum(1 / coalesce(samplingRate, 1))`.
    pub fn sample_rate(&self) -> f32 {
        match self {
            Self::Status => 0.01,
            Self::Diff | Self::RefreshRemoteData => 0.05,
            Self::BranchList => 0.10,
            _ => 1.0,
        }
    }
}
