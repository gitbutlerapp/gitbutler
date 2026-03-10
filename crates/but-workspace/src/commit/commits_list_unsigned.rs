//! An action to list unsigned commit IDs in the workspace.

pub(crate) mod function {
    use anyhow::Result;
    use but_graph::projection::Workspace;
    use gitbutler_stack::StackId;
    use gix::{ObjectId, hashtable::HashSet};

    /// The result of listing unsigned commits in the workspace.
    #[derive(Debug)]
    pub struct CommitsListUnsignedOutcome {
        /// IDs of all unsigned commits in the workspace
        pub unsigned_commits: HashSet<ObjectId>,
    }

    /// List unsigned commits in the workspace.
    ///
    /// `ws` the workspace to operate in.
    ///
    /// `repo` associated repository so signature information can be extracted for commits
    ///
    /// `stack_id` if provided, only commits for this stack are fetched.
    pub fn commits_list_unsigned(
        ws: &Workspace,
        repo: &gix::Repository,
        stack_id: Option<StackId>,
    ) -> Result<CommitsListUnsignedOutcome> {
        let commits = ws
            .stacks
            .iter()
            .filter(|stack| stack_id.is_none() || stack_id == stack.id)
            .flat_map(|stack| stack.segments.iter())
            .flat_map(|segment| segment.commits.iter());

        let mut unsigned_commits = HashSet::default();
        for commit in commits {
            if commit
                .attach(repo)?
                .extra_headers()
                .pgp_signature()
                .is_none()
            {
                unsigned_commits.insert(commit.id);
            }
        }

        Ok(CommitsListUnsignedOutcome { unsigned_commits })
    }
}
