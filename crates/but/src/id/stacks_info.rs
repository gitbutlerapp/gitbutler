use bstr::BString;
use but_workspace::branch::Stack;

/// Information extracted from stacks needed for branch and commit CLI ID generation.
/// It's really just a named return value.
pub(crate) struct StacksInfo {
    /// Shortened branch names in unspecified order.
    pub(crate) branch_names: Vec<BString>,
    /// Commit IDs of commits reachable from workspace tips paired with their
    /// first parent IDs in unspecified order. The parent ID is stored to enable
    /// computing diffs upon an invocation of [IdMap::add_file_info].
    pub(crate) workspace_commit_and_first_parent_ids: Vec<(gix::ObjectId, Option<gix::ObjectId>)>,
    /// Commit IDs that are only reachable from remote-tracking branches (not in workspace).
    pub(crate) remote_commit_ids: Vec<gix::ObjectId>,
}

impl StacksInfo {
    /// Extracts branch names and commit IDs from the given `stacks`.
    pub(crate) fn from_stacks(stacks: &[Stack]) -> anyhow::Result<Self> {
        let mut branch_names: Vec<BString> = Vec::new();
        let mut workspace_commit_and_first_parent_ids: Vec<(gix::ObjectId, Option<gix::ObjectId>)> =
            Vec::new();
        let mut remote_commit_ids: Vec<gix::ObjectId> = Vec::new();
        for stack in stacks {
            for segment in &stack.segments {
                if let Some(ref_info) = &segment.ref_info {
                    branch_names.push(ref_info.ref_name.shorten().to_owned());
                }
                for commit in &segment.commits {
                    workspace_commit_and_first_parent_ids
                        .push((commit.id, commit.parent_ids.first().cloned()));
                }
                for commit in &segment.commits_on_remote {
                    remote_commit_ids.push(commit.id);
                }
            }
        }

        Ok(Self {
            branch_names,
            workspace_commit_and_first_parent_ids,
            remote_commit_ids,
        })
    }
}
