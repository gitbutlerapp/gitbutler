use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, strum::Display, clap::ValueEnum, Default)]
#[serde(rename_all = "camelCase")]
pub enum CommandName {
    Init,
    Absorb,
    Discard,
    Status,
    Tui,
    Stf,
    Uncommit,
    Amend,
    Squash,
    Move,
    Diff,
    Diff2,
    Edit,
    Show,
    Commit,
    CommitEmpty,
    Push,
    Reword,
    OplogList,
    OplogSnapshot,
    Restore,
    Undo,
    Redo,
    Gui,
    Open,
    BaseFetch,
    BaseCheck,
    Pull,
    BranchNew,
    BranchDelete,
    BranchList,
    BranchShow,
    BranchUnapply,
    BranchApply,
    BranchUpdate,
    BranchMove,
    BranchTearOff,
    WorktreeList,
    WorktreeArchive,
    WorktreeUnarchive,
    WorktreeRemove,
    Switch,
    ForgeAuth,
    ForgeListUsers,
    ForgeForget,
    PrNew,
    PrTemplate,
    DisableAutoMerge,
    EnableAutoMerge,
    SetReviewReady,
    SetReviewDraft,
    Completions,
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
    SkillInstall,
    SkillCheck,
    AgentSetup,
    Pick,
    Clean,
    External,
    #[default]
    Unknown,
}

impl CommandName {
    /// Percentage sample rate, between 0 and 1.
    ///
    /// 1 indicates that the command should always be submitted to posthog, and
    /// 0 should never be submitted to posthog.
    pub fn sample_rate(&self) -> f32 {
        match self {
            Self::Unknown | Self::Completions | Self::Status => 0.05,
            _ => 1.0,
        }
    }
}
