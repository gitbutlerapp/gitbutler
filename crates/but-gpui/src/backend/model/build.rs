use bstr::ByteSlice as _;

use super::{Branch, Checkout, Commit, CommitId, FullName, Lane, PushState, Workspace, Worktree};

pub(crate) fn full(name: &str) -> FullName {
    name.try_into().expect("fixtures name valid refs")
}

pub(crate) fn id(n: u8) -> CommitId {
    CommitId(gix::ObjectId::from_bytes_or_panic(&[n; 20]))
}

pub(crate) fn branch(name: &str, commits: &[u8]) -> Branch {
    Branch {
        name: name.into(),
        ref_name: (!name.is_empty()).then(|| full(&format!("refs/heads/{name}"))),
        commits: (commits.iter())
            .map(|&n| Commit {
                key: format!("key{n}"),
                ..Commit::bare(id(n).0)
            })
            .collect(),
        incoming: Vec::new(),
        remote: None,
        push: PushState::NeverPushed,
        review: None,
    }
}

pub(crate) fn worktree(name: &str, rests_on: Option<u8>, branches: Vec<Branch>) -> Lane {
    Lane {
        branches,
        checkout: Checkout::Linked(name.into()),
        rests_on: rests_on.map(id),
    }
}

pub(crate) fn workspace(stacks: Vec<Vec<Branch>>, linked: Vec<Lane>) -> Workspace {
    let worktrees = (linked.iter())
        .filter_map(|lane| lane.checkout.linked())
        .map(|name| Worktree {
            name: name.into(),
            path: format!("/{name}").into(),
            branch: None,
            uncommitted: Vec::new(),
            git_operation: None,
            tree: None,
            updated_at: None,
        })
        .collect();
    let stacks = stacks.into_iter().map(|branches| Lane {
        branches,
        checkout: Checkout::Main,
        rests_on: None,
    });
    Workspace {
        lanes: stacks.chain(linked).collect(),
        worktrees,
        ..Workspace::default()
    }
}

fn names(workspace: &Workspace, branches: &[Branch]) -> String {
    (branches.iter())
        .map(
            |branch| match (branch.ref_name.as_ref()).and_then(|r| workspace.checked_out_in(r)) {
                Some(_) => format!("{} (checked out)", branch.name),
                None => branch.name.clone(),
            },
        )
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn commit_name(workspace: &Workspace, id: CommitId) -> String {
    (workspace.all_branches())
        .find_map(|branch| {
            let below = branch.commits.iter().position(|commit| commit.id == id)?;
            Some(match below {
                0 => branch.name.clone(),
                _ => format!("{}~{below}", branch.name),
            })
        })
        .unwrap_or_else(|| id.short())
}

pub(crate) fn describe(workspace: &Workspace) -> String {
    let line = |checkout: &str, lane: Option<&Lane>| {
        let branches = lane.map_or(&[][..], |lane| &lane.branches);
        let rests_on = (lane.and_then(|lane| lane.rests_on)).map_or(String::new(), |id| {
            format!(" rests on {}", commit_name(workspace, id))
        });
        format!("{checkout}: [{}]{rests_on}\n", names(workspace, branches))
    };
    let stacks = (workspace.lanes_in(&Checkout::Main)).map(|lane| line("Main", Some(lane)));
    let mut worktrees: Vec<_> = workspace.worktrees.iter().collect();
    worktrees.sort_by_key(|worktree| &worktree.name);
    let worktrees = (worktrees.into_iter()).map(|worktree| {
        line(
            &worktree.name.to_str_lossy(),
            workspace.worktree_lane(worktree.name.as_ref()),
        )
    });
    stacks.chain(worktrees).collect()
}

pub(crate) fn describe_chain(workspace: &Workspace, ref_name: &FullName) -> String {
    let chain: Vec<_> = workspace.chain(ref_name).collect();
    let links = chain.iter().map(|link| {
        let checkout =
            (link.lane.checkout.linked()).map_or("Main".into(), |name| name.to_str_lossy());
        let from = (link.from).map_or(String::new(), |id| {
            format!(" from {}", commit_name(workspace, id))
        });
        format!(
            "{checkout}: [{}]{from}\n",
            names(workspace, link.branches())
        )
    });
    let history = (chain.iter().flat_map(|link| link.commits()))
        .map(|commit| commit_name(workspace, commit.id))
        .collect::<Vec<_>>()
        .join(", ");
    links.chain([format!("history: [{history}]\n")]).collect()
}
