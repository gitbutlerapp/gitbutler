//! The branch structure the rebase engine reads: the [`BranchGraph`], a flat adjacency list of the
//! workspace's branches carrying the full commit topology (per-commit, on each [`Branch`]) plus what
//! the commit graph alone cannot express — empty branches, branch names, the entrypoint.
//!
//! Assembled losslessly from the traversal's [branch records](crate::Workspace::branches), so the
//! structure is exact, not reconstructed from the linearized display
//! [stacks](crate::workspace::StackSegment). The display is still built separately from the
//! traversal; unifying both views onto this structure is future work.

/// A flat adjacency list of the workspace's [branches](Branch) — named runs of commits (including
/// empty ones) whose per-commit parentage and [`outgoing`](Branch::outgoing) edges carry the full
/// topology — plus the workspace commit.
#[derive(Debug, Clone)]
pub struct BranchGraph {
    /// The branches, in traversal record order. Edges in [`Branch::outgoing`] index into this list.
    /// Empty branches (no commits) are retained — they are the routing nodes the commit graph
    /// cannot express. Per-commit parentage lives on each [`Branch::commits`] entry.
    pub branches: Vec<Branch>,
    /// The managed workspace ("octopus") commit, if the entrypoint sits on one — resolved by commit
    /// message, so it is set even for an unmanaged head checked out onto a workspace commit. Its
    /// pick is special in the rebase.
    pub workspace_commit: Option<gix::ObjectId>,
}

/// One branch: an optional name, its exclusive commits (tip → base), and edges to the branches it
/// rests on. A named run of commits, or an empty named position carrying only its attachment.
#[derive(Debug, Clone)]
pub struct Branch {
    /// The branch reference, if named (anonymous runs have none).
    pub ref_name: Option<gix::refs::FullName>,
    /// The commits exclusive to this branch, tip → base, each carrying its references. Empty for an
    /// empty branch.
    pub commits: Vec<crate::Commit>,
    /// Edges to other branches: `(index into [`BranchGraph::branches`], parent order)`.
    pub outgoing: Vec<(usize, u32)>,
    /// Whether this branch is the traversal entrypoint (selects `HEAD`).
    pub is_entrypoint: bool,
}

impl crate::Workspace {
    /// Derive the [`BranchGraph`] from this workspace projection.
    ///
    /// Assembled from the carried [branch records](Self::branches) — the traversal's full-topology
    /// view — so the branch structure (selection, empty branches, naming, entrypoint) is exact, not
    /// reconstructed from the linearized display stacks. `repo` is read only to resolve the
    /// [workspace commit](BranchGraph::workspace_commit) by message. A later step produces this
    /// directly from the commit graph and metadata.
    pub fn branch_graph(&self, repo: &gix::Repository) -> BranchGraph {
        let branches = self.branches().unwrap_or_default().to_vec();
        // The managed workspace pick: the entrypoint's tip when its message marks a managed
        // workspace commit. A message check, not a `kind` check — an unmanaged head (no workspace
        // metadata) can still sit on a workspace commit.
        let workspace_commit = branches
            .iter()
            .find(|b| b.is_entrypoint)
            .and_then(|b| b.commits.first().map(|c| c.id))
            .filter(|id| {
                repo.find_commit(*id)
                    .ok()
                    .and_then(|c| c.message_raw().ok().map(|m| m.to_owned()))
                    .is_some_and(|message| {
                        crate::workspace::commit::is_managed_workspace_by_message(message.as_ref())
                    })
            });
        BranchGraph {
            branches,
            workspace_commit,
        }
    }
}
