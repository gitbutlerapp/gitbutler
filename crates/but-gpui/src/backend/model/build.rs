use super::{Branch, Checkout, Commit, CommitId, Lane, PushState, Workspace, Worktree};

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
                rests_on: None,
            })
            .chain([Lane {
                branches: linked,
                checkout: Checkout::Linked("agent".into()),
                rests_on: None,
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
    let commit = |id: CommitId| {
        (workspace.all_branches())
            .find_map(|branch| {
                let below = branch.commits.iter().position(|commit| commit.id == id)?;
                Some(match below {
                    0 => branch.name.clone(),
                    _ => format!("{}~{below}", branch.name),
                })
            })
            .unwrap_or_else(|| id.short())
    };
    let line = |checkout: &str, lane: Option<&Lane>| {
        let branches = lane.map_or(&[][..], |lane| &lane.branches);
        let rests_on = (lane.and_then(|lane| lane.rests_on))
            .map_or(String::new(), |id| format!(" rests on {}", commit(id)));
        format!("{checkout}: [{}]{rests_on}\n", names(branches))
    };
    let stacks = (workspace.lanes_in(&Checkout::Main)).map(|lane| line("Main", Some(lane)));
    let mut worktrees: Vec<_> = workspace.worktrees.iter().collect();
    worktrees.sort_by_key(|worktree| &worktree.name);
    let worktrees = (worktrees.into_iter())
        .map(|worktree| line(&worktree.name, workspace.worktree_lane(&worktree.name)));
    stacks.chain(worktrees).collect()
}
