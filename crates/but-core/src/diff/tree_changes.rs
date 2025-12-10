use gix::{diff::tree_with_rewrites::Change, prelude::TreeDiffChangeExt};

use crate::{ChangeState, Commit, ModeFlags, TreeChange, TreeStatus, commit::TreeKind};

// TODO: use `peel_to_tree()` once special conflict markers aren't needed anymore.
fn id_to_tree(repo: &gix::Repository, id: gix::ObjectId) -> anyhow::Result<gix::Tree<'_>> {
    let object = repo.find_object(id)?;
    if object.kind == gix::object::Kind::Commit {
        let commit = Commit::from_id(object.peel_to_commit()?.id())?;
        let tree = commit.tree_id_or_kind(TreeKind::AutoResolution)?;
        let tree = repo.find_tree(tree)?;
        Ok(tree)
    } else {
        Ok(object.peel_to_tree()?)
    }
}

/// A type representing one or more changes of a tree entry itself, with utility methods for convenience.
#[derive(Debug, Clone)]
pub struct TreeChanges(
    /// All the actual tree-changes, in their original form.
    pub Vec<gix::object::tree::diff::ChangeDetached>,
);

impl TreeChanges {
    /// Produce all changes that are needed to turn the tree of `lhs` into the tree of `rhs`.
    /// If `lhs` is `None`, it will be treated like an empty tree, which is useful if
    /// there was no tree to compare `lhs` to (e.g. in case of the first commit).
    ///
    /// Can be given either a commit or a tree oid.
    ///
    /// Note that we deal with conflicted commits correctly by resolving to the actual tree, not the one with meta-data.
    ///
    /// They are sorted by their current path.
    ///
    /// Additionally, line-stats aggregated for all changes will be computed, which incurs a considerable fraction of the cost
    /// of asking for [UnifiedDiffs](TreeChange::unified_patch()).
    pub fn from_trees(
        repo: &gix::Repository,
        lhs: Option<gix::ObjectId>,
        rhs: gix::ObjectId,
    ) -> anyhow::Result<Self> {
        let lhs_tree = lhs.map(|id| id_to_tree(repo, id)).transpose()?;
        let rhs_tree = id_to_tree(repo, rhs)?;

        let changes = repo.diff_tree_to_tree(lhs_tree.as_ref(), &rhs_tree, None)?;
        let mut out: Vec<_> = changes
            .into_iter()
            .filter(|c| !c.entry_mode().is_tree())
            .collect();
        out.sort_by(|a, b| a.location().cmp(b.location()));
        Ok(TreeChanges(out))
    }
}

/// Utilities
impl TreeChanges {
    /// Compute diff statistics on all blobs in this instance, using `repo` for blob retrieval and diff setup.
    ///
    /// This is expensive.
    pub fn compute_line_stats(
        &self,
        repo: &gix::Repository,
    ) -> anyhow::Result<gix::object::tree::diff::Stats> {
        let mut stats = gix::object::tree::diff::Stats::default();
        let mut resource_cache = repo.diff_resource_cache_for_tree_diff()?;
        for change in self.0.iter() {
            let change = change.attach(repo, repo);
            resource_cache.clear_resource_cache_keep_allocation();
            if let Some(counts) = change
                .diff(&mut resource_cache)
                .ok()
                .and_then(|mut platform| platform.line_counts().ok())
                .flatten()
            {
                stats.files_changed += 1;
                stats.lines_added += u64::from(counts.insertions);
                stats.lines_removed += u64::from(counts.removals);
            }
        }
        Ok(stats)
    }

    /// Convert this instance into a list of [`TreeChange`] instances.
    pub fn into_tree_changes(self) -> Vec<TreeChange> {
        self.0.into_iter().map(Into::into).collect()
    }
}

/// See [TreeChanges::from_trees()].
pub fn tree_changes(
    repo: &gix::Repository,
    lhs: Option<gix::ObjectId>,
    rhs: gix::ObjectId,
) -> anyhow::Result<Vec<TreeChange>> {
    Ok(TreeChanges::from_trees(repo, lhs, rhs)?.into_tree_changes())
}

/// See [TreeChanges::from_trees()], but with [TreeChanges::compute_line_stats()] automatically called.
pub fn tree_changes_with_line_stats(
    repo: &gix::Repository,
    lhs: Option<gix::ObjectId>,
    rhs: gix::ObjectId,
) -> anyhow::Result<(Vec<TreeChange>, gix::object::tree::diff::Stats)> {
    let changes = TreeChanges::from_trees(repo, lhs, rhs)?;
    let stats = changes.compute_line_stats(repo)?;
    let changes = changes.into_tree_changes();
    Ok((changes, stats))
}

impl From<gix::object::tree::diff::ChangeDetached> for TreeChange {
    fn from(value: gix::object::tree::diff::ChangeDetached) -> Self {
        match value {
            Change::Addition {
                location,
                entry_mode,
                id,
                ..
            } => TreeChange {
                path: location,
                status: TreeStatus::Addition {
                    state: ChangeState {
                        id,
                        kind: entry_mode.kind(),
                    },
                    is_untracked: false,
                },
            },
            Change::Deletion {
                location,
                id,
                entry_mode,
                ..
            } => TreeChange {
                path: location,
                status: TreeStatus::Deletion {
                    previous_state: ChangeState {
                        id,
                        kind: entry_mode.kind(),
                    },
                },
            },
            Change::Modification {
                location,
                previous_entry_mode,
                previous_id,
                id,
                entry_mode,
            } => {
                let previous_state = ChangeState {
                    id: previous_id,
                    kind: previous_entry_mode.kind(),
                };
                let state = ChangeState {
                    id,
                    kind: entry_mode.kind(),
                };
                TreeChange {
                    path: location,
                    status: TreeStatus::Modification {
                        previous_state,
                        state,
                        flags: ModeFlags::calculate(&previous_state, &state),
                    },
                }
            }
            Change::Rewrite {
                source_location,
                source_entry_mode,
                source_id,
                entry_mode,
                id,
                location,
                diff: _,
                copy: _,
                ..
            } => {
                let previous_state = ChangeState {
                    id: source_id,
                    kind: source_entry_mode.kind(),
                };
                let state = ChangeState {
                    id,
                    kind: entry_mode.kind(),
                };
                TreeChange {
                    path: location,
                    status: TreeStatus::Rename {
                        previous_path: source_location,
                        previous_state,
                        state,
                        flags: ModeFlags::calculate(&previous_state, &state),
                    },
                }
            }
        }
    }
}
