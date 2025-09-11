use crate::WorkspaceCommit;
use bstr::ByteSlice;

/// Construction
impl<'repo> WorkspaceCommit<'repo> {
    const GITBUTLER_WORKSPACE_COMMIT_TITLE: &'static str = "GitButler Workspace Commit";

    /// Decode the object at `commit_id` and keep its data for later query.
    pub fn from_id(commit_id: gix::Id<'repo>) -> anyhow::Result<Self> {
        let commit = commit_id.object()?.try_into_commit()?.decode()?.into();
        Ok(WorkspaceCommit {
            id: commit_id,
            inner: commit,
        })
    }

    /// Create a new commit which presents itself as the merge of all the given `stacks`.
    ///
    /// Note that the returned commit lives entirely in memory and would still have to be written to disk.
    /// It still needs its tree set to something non-empty.
    ///
    /// `object_hash` is needed to create an empty tree hash.
    pub(crate) fn create_commit_from_vb_state(
        stacks: &[crate::ui::StackEntryNoOpt],
        object_hash: gix::hash::Kind,
    ) -> gix::objs::Commit {
        // message that says how to get back to where they were
        let mut message = Self::GITBUTLER_WORKSPACE_COMMIT_TITLE.to_string();
        message.push_str("\n\n");
        if !stacks.is_empty() {
            message
                .push_str("This is a merge commit of the virtual branches in your workspace.\n\n");
        } else {
            message.push_str("This is placeholder commit and will be replaced by a merge of your virtual branches.\n\n");
        }
        message.push_str(
            "Due to GitButler managing multiple virtual branches, you cannot switch back and\n",
        );
        message.push_str("forth between git branches and virtual branches easily. \n\n");

        message.push_str(
            "If you switch to another branch, GitButler will need to be reinitialized.\n",
        );
        message.push_str("If you commit on this branch, GitButler will throw it away.\n\n");
        if !stacks.is_empty() {
            message.push_str("Here are the branches that are currently applied:\n");
            for branch in stacks {
                if let Some(name) = branch.name() {
                    message.push_str(" - ");
                    message.push_str(name.to_str_lossy().as_ref());
                    message.push('\n');
                }

                message.push_str("   branch head: ");
                message.push_str(&branch.tip.to_string());
                message.push('\n');
            }
        }
        message.push_str("For more information about what we're doing here, check out our docs:\n");
        message
            .push_str("https://docs.gitbutler.com/features/virtual-branches/integration-branch\n");

        let author = gix::actor::Signature {
            name: "GitButler".into(),
            email: "gitbutler@gitbutler.com".into(),
            time: commit_time("GIT_COMMITTER_DATE"),
        };
        gix::objs::Commit {
            tree: gix::ObjectId::empty_tree(object_hash),
            parents: stacks.iter().map(|s| s.tip).collect(),
            committer: author.clone(),
            author,
            encoding: Some("UTF-8".into()),
            message: message.into(),
            extra_headers: vec![],
        }
    }
}

/// Return the time of a commit as `now` unless the `overriding_variable_name` contains a parseable date,
/// which is used instead.
fn commit_time(overriding_variable_name: &str) -> gix::date::Time {
    std::env::var(overriding_variable_name)
        .ok()
        .and_then(|time| gix::date::parse(&time, Some(std::time::SystemTime::now())).ok())
        .unwrap_or_else(gix::date::Time::now_local_or_utc)
}

/// Query
impl WorkspaceCommit<'_> {
    /// Return `true` if this commit is managed by GitButler.
    /// If `false`, this is the tip of the stack itself which will be put underneath a *managed* workspace commit
    /// once another branch is added to the workspace.
    pub fn is_managed(&self) -> bool {
        but_graph::projection::commit::is_managed_workspace_by_message(self.message.as_bstr())
    }
}

impl std::ops::Deref for WorkspaceCommit<'_> {
    type Target = gix::objs::Commit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for WorkspaceCommit<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
