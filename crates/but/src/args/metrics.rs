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
    Rub,
    Move,
    Diff,
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
    Gui,
    BaseFetch,
    BaseCheck,
    Pull,
    BranchNew,
    BranchDelete,
    BranchList,
    BranchShow,
    BranchUnapply,
    BranchApply,
    BranchMove,
    BranchTearOff,
    ClaudePreTool,
    ClaudePostTool,
    ClaudeStop,
    CursorAfterEdit,
    CursorStop,
    Worktree,
    Mark,
    Unmark,
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
    Merge,
    SkillInstall,
    SkillCheck,
    Pick,
    Clean,
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
