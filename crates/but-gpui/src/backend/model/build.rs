use super::{Branch, Checkout, Commit, Lane, PushState, Workspace, Worktree};

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
        lanes: (stacks.into_iter())
            .map(|branches| Lane {
                branches,
                checkout: Checkout::Main,
            })
            .chain([Lane {
                branches: linked,
                checkout: Checkout::Linked("agent".into()),
            }])
            .collect(),
        worktrees: vec![Worktree {
            name: "agent".into(),
            path: "/agent".into(),
            branch: None,
            uncommitted: Vec::new(),
            git_operation: None,
            tree: None,
            updated_at: None,
        }],
        ..Workspace::default()
    }
}

pub(crate) fn describe(workspace: &Workspace) -> String {
    let names = |branches: &[Branch]| {
        (branches.iter())
            .map(|branch| branch.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let stacks = (workspace.lanes_in(&Checkout::Main))
        .map(|lane| format!("Main: [{}]", names(&lane.branches)));
    let mut worktrees: Vec<_> = workspace.worktrees.iter().collect();
    worktrees.sort_by_key(|worktree| &worktree.name);
    let worktrees = (worktrees.into_iter()).map(|worktree| {
        let lane = workspace.worktree_lane(&worktree.name);
        let branches = lane.map_or(&[][..], |lane| &lane.branches);
        format!("{}: [{}]", worktree.name, names(branches))
    });
    stacks.chain(worktrees).map(|line| line + "\n").collect()
}
