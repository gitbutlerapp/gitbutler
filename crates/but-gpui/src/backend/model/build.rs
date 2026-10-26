use super::{Branch, Commit, PushState, Stack, Workspace, Worktree};

pub(crate) fn branch(name: &str, commits: &[u8]) -> Branch {
    let id = |n: u8| gix::ObjectId::from_bytes_or_panic(&[n; 20]);
    Branch {
        name: name.into(),
        ref_name: if name.is_empty() {
            String::new()
        } else {
            format!("refs/heads/{name}")
        },
        commits: (commits.iter())
            .map(|&n| Commit {
                key: format!("key{n}"),
                ..Commit::bare(id(n))
            })
            .collect(),
        incoming: Vec::new(),
        remote: None,
        push: PushState::NeverPushed,
        review: None,
    }
}

pub(crate) fn workspace(stacks: Vec<Vec<Branch>>, linked: Vec<Branch>) -> Workspace {
    Workspace {
        stacks: stacks
            .into_iter()
            .map(|branches| Stack { branches })
            .collect(),
        worktrees: vec![Worktree {
            name: "agent".into(),
            path: "/agent".into(),
            branch: None,
            branches: linked,
            uncommitted: Vec::new(),
            git_operation: None,
            tree: None,
            updated_at: None,
        }],
        ..Workspace::default()
    }
}
