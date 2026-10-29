//! The commits the Graph page draws, each with every parent: the workspace's and its linked
//! worktrees', what their remotes and the target have that it doesn't, and the history below
//! them, as far back as the page reaches.

use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;
use bstr::ByteSlice;

use super::{
    Agent, CommitId, FullName, commit_byline, open, ref_tip, subject_and_body, target_source,
};

#[derive(Debug, Clone)]
pub struct GraphCommit {
    pub id: CommitId,
    /// First parent first. A shallow clone's oldest commits name parents it doesn't have,
    /// which are left out.
    pub parents: Vec<CommitId>,
    pub title: String,
    /// The rest of its message, after the title.
    pub body: String,
    /// Who it's shown by, as [`super::byline`] picks.
    pub author: String,
    pub email: String,
    pub agent: Option<Agent>,
    /// When it was written, in seconds since the epoch.
    pub time: i64,
}

#[derive(Debug, Clone, Default)]
pub struct Graph {
    /// `seeds` first, as asked for, then the history below them newest first.
    pub commits: Vec<GraphCommit>,
    /// The target's newest commit, where pulls come from.
    pub target: Option<CommitId>,
    /// The local target branch's newest commit and its short name, like `master`, when the
    /// target is the remote branch it tracks.
    pub local: Option<(CommitId, String)>,
    /// Where each of `refs` points, in their order, `None` for one that's gone.
    pub refs: Vec<Option<CommitId>>,
}

/// Each of `seeds` that's still there, then up to `history` commits more, newest first, from
/// below them, from where `refs` point and from the target's and the local target branch's
/// tips down. A commit whose parents aren't among them is where the graph stops.
pub fn graph(path: &Path, seeds: &[CommitId], refs: &[FullName], history: usize) -> Result<Graph> {
    use gix::revision::walk::Sorting;
    use gix::traverse::commit::simple::CommitTimeOrder;
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let meta = ctx.project_meta()?;
    let source = target_source(&repo, &meta);
    let target = (source.as_ref()).and_then(|name| ref_tip(&repo, name.as_ref()));
    let local = (meta.target_ref.as_ref())
        .map(|name| name.as_local_full_name_ref())
        .filter(|name| {
            source
                .as_ref()
                .is_none_or(|source| source.as_ref() != *name)
        })
        .and_then(|name| {
            let tip = ref_tip(&repo, name)?;
            Some((CommitId(tip), name.shorten().to_str_lossy().into_owned()))
        });
    let refs: Vec<Option<CommitId>> = (refs.iter())
        .map(|name| ref_tip(&repo, name.as_ref()).map(CommitId))
        .collect();
    let mailmap = repo.open_mailmap();
    let mut seen = HashSet::new();
    let mut commits = Vec::new();
    for seed in seeds {
        if !seen.insert(seed.0) {
            continue;
        }
        // Gone since the workspace was read, as an agent's amend leaves the old one.
        let Ok(commit) = repo.find_commit(seed.0) else {
            continue;
        };
        commits.push(graph_commit(&mailmap, &commit)?);
    }
    let mut tips: Vec<gix::ObjectId> = (commits.iter())
        .flat_map(|commit| &commit.parents)
        .map(|parent| parent.0)
        .chain(target)
        .chain(local.as_ref().map(|(tip, _)| tip.0))
        .chain(refs.iter().flatten().map(|tip| tip.0))
        .filter(|id| !seen.contains(id))
        .collect();
    tips.sort();
    tips.dedup();
    if tips.is_empty() {
        return Ok(Graph {
            commits,
            target: target.map(CommitId),
            local,
            refs,
        });
    }
    let walk = (repo.rev_walk(tips))
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
        .all()?;
    let mut taken = 0;
    // A shallow clone's walk ends in an error at its edge, which is where the graph does.
    for info in walk.map_while(Result::ok) {
        if taken == history {
            break;
        }
        if !seen.insert(info.id) {
            continue;
        }
        commits.push(graph_commit(&mailmap, &info.object()?)?);
        taken += 1;
    }
    Ok(Graph {
        commits,
        target: target.map(CommitId),
        local,
        refs,
    })
}

fn graph_commit(mailmap: &gix::mailmap::Snapshot, commit: &gix::Commit<'_>) -> Result<GraphCommit> {
    let (title, body) = subject_and_body(&commit.message_raw_sloppy().to_str_lossy());
    let ((author, email), agent) = commit_byline(mailmap, commit)?;
    let parents = (commit.parent_ids())
        .map(|id| id.detach())
        .filter(|id| commit.repo.has_object(id))
        .map(CommitId)
        .collect();
    Ok(GraphCommit {
        id: CommitId(commit.id),
        parents,
        title,
        body,
        author,
        email,
        agent,
        time: commit.author()?.time()?.seconds,
    })
}
