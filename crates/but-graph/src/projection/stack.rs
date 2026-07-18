use std::fmt::Formatter;

use bitflags::bitflags;
use gix::prelude::ObjectIdExt;

use crate::CommitFlags;

/// A commit prepared for workspace presentation.
#[derive(Clone, Eq, PartialEq)]
pub struct StackCommit {
    /// The hash of the commit.
    pub id: gix::ObjectId,
    /// The IDs of the parent commits, or empty for a root commit.
    pub parent_ids: Vec<gix::ObjectId>,
    /// Additional properties used by workspace consumers.
    pub flags: StackCommitFlags,
    /// References pointing to this commit, including peeled tags.
    pub refs: Vec<crate::RefInfo>,
}

impl StackCommit {
    /// Attach this commit to `repo` for decoded access.
    pub fn attach<'repo>(
        &self,
        repo: &'repo gix::Repository,
    ) -> anyhow::Result<but_core::Commit<'repo>> {
        but_core::Commit::from_id(self.id.attach(repo))
    }

    /// Return all reference names that point to this commit.
    pub fn ref_iter(&self) -> impl Iterator<Item = &gix::refs::FullNameRef> + Clone {
        self.refs.iter().map(|info| info.ref_name.as_ref())
    }

    /// Return a concise single-line representation.
    pub fn debug_string(&self, flags: StackCommitDebugFlags) -> String {
        use StackCommitDebugFlags as F;
        format!(
            "{end}{kind}{hex}{flags}{refs}",
            end = if self.flags.contains(StackCommitFlags::EarlyEnd) {
                if flags.contains(F::HardLimitReached) {
                    "❌"
                } else {
                    "✂️"
                }
            } else {
                ""
            },
            kind = if flags.contains(F::RemoteOnly) {
                "🟣"
            } else if self
                .flags
                .contains(StackCommitFlags::ReachableByMatchingRemote)
            {
                "❄️"
            } else if self.flags.contains(StackCommitFlags::ReachableByRemote) {
                "❄"
            } else {
                "·"
            },
            flags = {
                let flags = self.flags.debug_string();
                if flags.is_empty() {
                    String::new()
                } else {
                    format!(" ({flags})")
                }
            },
            hex = self.id.to_hex_with_len(7),
            refs = if self.refs.is_empty() {
                String::new()
            } else {
                format!(
                    " {}",
                    self.refs
                        .iter()
                        .map(crate::RefInfo::debug_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        )
    }
}

impl std::fmt::Debug for StackCommit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.debug_string(Default::default()).fmt(f)
    }
}

bitflags! {
    /// Configure concise commit rendering.
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct StackCommitDebugFlags: u8 {
        /// Render the commit as remote-only.
        const RemoteOnly = 1 << 0;
        /// Render an early end as a hard traversal limit.
        const HardLimitReached = 1 << 1;
    }
}

bitflags! {
    /// Information about a commit in a workspace projection.
    #[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
    pub struct StackCommitFlags: u8 {
        /// The commit is reachable from a remote.
        const ReachableByRemote = 1 << 0;
        /// The commit is reachable from a workspace.
        const InWorkspace = 1 << 1;
        /// The commit is reachable from a target branch.
        const Integrated = 1 << 2;
        /// The commit is listed in the repository's shallow boundary file.
        const ShallowBoundary = 1 << 3;
        /// The commit is reachable from its matching remote.
        const ReachableByMatchingRemote = 1 << 4;
        /// The commit contains unresolved conflicts.
        const HasConflicts = 1 << 5;
        /// Traversal stopped before all parents were followed.
        const EarlyEnd = 1 << 6;
    }
}

impl StackCommitFlags {
    /// Return a concise symbolic representation of presentation flags.
    pub fn debug_string(&self) -> String {
        let flags = *self & (Self::InWorkspace | Self::Integrated | Self::ShallowBoundary);
        if flags.is_empty() {
            String::new()
        } else {
            let string = format!("{flags:?}");
            let out = &string["StackCommitFlags(".len()..string.len() - 1];
            out.replace("InWorkspace", "🏘️")
                .replace("Integrated", "✓")
                .replace("ShallowBoundary", "⛰")
                .replace(' ', "")
        }
    }
}

impl From<CommitFlags> for StackCommitFlags {
    fn from(value: CommitFlags) -> Self {
        StackCommitFlags::from_bits_retain(
            (value
                & (CommitFlags::Integrated
                    | CommitFlags::InWorkspace
                    | CommitFlags::ShallowBoundary))
                .bits() as u8,
        )
    }
}
