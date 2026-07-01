//! Expressions of the form "sum of side tree IDs minus sum of base tree IDs".

use std::collections::VecDeque;

use crate::repo_ext::RepositoryExt as _;
use anyhow::Context as _;
use smallvec::{SmallVec, smallvec};

/// Sum of sides minus sum of bases. All functions enforce the invariant that
/// the count of sides is exactly one more than the count of bases.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeExpression {
    /// Base tree IDs.
    pub base_tree_ids: Vec<gix::ObjectId>,
    /// Side tree IDs.
    pub side_tree_ids: SmallVec<[gix::ObjectId; 1]>,
}

impl TryFrom<&crate::Commit<'_>> for TreeExpression {
    type Error = anyhow::Error;

    fn try_from(commit: &crate::Commit<'_>) -> Result<Self, Self::Error> {
        let base_tree_ids = commit.base_tree_ids()?;
        let side_tree_ids = commit.side_tree_ids()?;
        if base_tree_ids.len() + 1 != side_tree_ids.len() {
            anyhow::bail!(
                "in commit {}, number of sides ({}) is not exactly one more than number of bases ({})",
                commit.id.to_hex(),
                side_tree_ids.len(),
                base_tree_ids.len()
            );
        }
        Ok(Self {
            base_tree_ids,
            side_tree_ids,
        })
    }
}

impl From<gix::ObjectId> for TreeExpression {
    fn from(side: gix::ObjectId) -> Self {
        Self {
            base_tree_ids: Vec::new(),
            side_tree_ids: smallvec![side],
        }
    }
}

/// Arithmetic
impl TreeExpression {
    /// Subtracts `base` then adds `side`. This extends self's bases with the
    /// sides of `base` and the bases of `side`, and then extends self's sides
    /// with the bases of `base` and the sides of `side`.
    pub fn subtract_add(&mut self, base: &TreeExpression, side: &TreeExpression) {
        self.base_tree_ids
            .extend(base.side_tree_ids.iter().copied());
        self.base_tree_ids
            .extend(side.base_tree_ids.iter().copied());
        self.side_tree_ids
            .extend(base.base_tree_ids.iter().copied());
        self.side_tree_ids
            .extend(side.side_tree_ids.iter().copied());
    }
}

/// Outcome of [TreeExpression::merge].
pub enum MergeOutcome<'repo> {
    /// There were no merge conflicts.
    Unconflicted {
        /// The lone tree after simplifications and merges.
        tree_id: gix::ObjectId,
    },
    /// There was at least one merge conflict.
    Conflicted {
        /// The tree as returned by the last merge (which resulted in a
        /// conflict).
        tree_id: gix::ObjectId,
        /// The outcome of the last merge (which resulted in a conflict).
        merge: Box<gix::merge::tree::Outcome<'repo>>,
        /// The tree expression representing the parts that could not be merged
        /// without conflict (after possibly some successful simplifications
        /// and/or merges).
        tree_expression: TreeExpression,
    },
}

/// Simplification
impl TreeExpression {
    /// Cancel out terms in the bases and sides wherever possible, then
    /// repeatedly merge until only one side remains or a conflict is
    /// encountered.
    pub fn merge<'repo>(
        self,
        repo: &'repo gix::Repository,
        merge_options: gix::merge::tree::Options,
    ) -> anyhow::Result<MergeOutcome<'repo>> {
        let (mut bases, mut sides) = {
            let Self {
                base_tree_ids,
                side_tree_ids,
            } = self;
            (
                VecDeque::from(base_tree_ids),
                VecDeque::from(side_tree_ids.into_vec()),
            )
        };
        for i in (0..bases.len()).rev() {
            if let Some(position) = sides.iter().position(|side| *side == bases[i]) {
                sides.remove(position);
                bases.remove(i);
            }
        }

        let conflict_kind = gix::merge::tree::TreatAsUnresolved::forced_resolution();
        let mut ours = sides
            .pop_front()
            .context("BUG: side count invariant not enforced (should have at least 1 side)")?;
        while let Some(base) = bases.pop_front() {
            let theirs = sides.pop_front().context(
                "BUG: side count invariant not enforced (no corresponding side to popped base)",
            )?;
            let mut merge = repo.merge_trees(
                base,
                ours,
                theirs,
                repo.default_merge_labels(),
                merge_options.clone(),
            )?;
            let merged_tree_id = merge.tree.write()?.detach();

            if merge.has_unresolved_conflicts(conflict_kind) {
                // Push back the things that conflict when merged. Note that
                // "ours" needs to be the frontmost, so we push "theirs" then
                // "ours".
                sides.push_front(theirs);
                sides.push_front(ours);
                bases.push_front(base);
                return Ok(MergeOutcome::Conflicted {
                    tree_id: merged_tree_id,
                    merge: Box::new(merge),
                    tree_expression: TreeExpression {
                        base_tree_ids: bases.into(),
                        side_tree_ids: Vec::from(sides).into(),
                    },
                });
            }

            ours = merged_tree_id;
        }

        Ok(MergeOutcome::Unconflicted { tree_id: ours })
    }
}
