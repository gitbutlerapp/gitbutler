use crate::StackEntry;
use bstr::ByteSlice;
use but_core::WorkspaceCommit;

/// Create a new commit which presents itself as the merge of all the given `stacks`.
///
/// Note that the returned commit lives entirely in memory and would still have to be written to disk.
/// It still needs its tree set to something non-empty.
///
/// `object_hash` is needed to create an empty tree hash.
pub fn create_workspace_commit_from_vb_state(
    stacks: &[StackEntry],
    object_hash: gix::hash::Kind,
) -> gix::objs::Commit {
    // message that says how to get back to where they were
    let mut message = WorkspaceCommit::GITBUTLER_WORKSPACE_COMMIT_TITLE.to_string();
    message.push_str("\n\n");
    if !stacks.is_empty() {
        message.push_str("This is a merge commit the virtual branches in your workspace.\n\n");
    } else {
        message.push_str("This is placeholder commit and will be replaced by a merge of your virtual branches.\n\n");
    }
    message.push_str(
        "Due to GitButler managing multiple virtual branches, you cannot switch back and\n",
    );
    message.push_str("forth between git branches and virtual branches easily. \n\n");

    message.push_str("If you switch to another branch, GitButler will need to be reinitialized.\n");
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
    message.push_str("https://docs.gitbutler.com/features/virtual-branches/integration-branch\n");

    let author = gix::actor::Signature {
        name: "GitButler".into(),
        email: "gitbutler@gitbutler.com".into(),
        time: gix::date::Time::now_local_or_utc(),
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
