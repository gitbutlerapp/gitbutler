//! Hyperedges between the commits from before a session and the commits it left behind.

use std::collections::BTreeMap;

use anyhow::Result;
use but_core::RefMetadata;
use but_evo::Rewrite;

use crate::graph_rebase::{
    Pick, RevisionHistory, Step, SuccessfulRebase, rebase::order_steps_picking,
    workspace::is_workspace_commit,
};

/// Where a commit came from.
#[derive(Debug, Clone)]
pub(crate) struct Lineage {
    /// The commit this one is a rewrite of, which a commit made in the session doesn't have.
    pub(crate) original: Option<gix::ObjectId>,
    /// What a fold or split turned into this commit, which is empty if it was only rewritten.
    precs: Vec<gix::ObjectId>,
}

impl Lineage {
    pub(crate) fn rewrite_of(original: gix::ObjectId) -> Self {
        Self {
            original: Some(original),
            precs: Vec::new(),
        }
    }

    fn precs(&self) -> &[gix::ObjectId] {
        if self.precs.is_empty() {
            self.original.as_slice()
        } else {
            &self.precs
        }
    }
}

/// A commit and what it turned into.
#[derive(Debug, Clone)]
pub(crate) struct Succession {
    pub(crate) prec: gix::ObjectId,
    pub(crate) succ: gix::ObjectId,
}

impl RevisionHistory {
    /// The commit that `pick` feeds into a fold or split as, which only abbreviates rewrites.
    ///
    /// The output of an earlier fold or split keeps its own place in the hypergraph, under the ID
    /// it has now.
    pub(crate) fn consume(&mut self, pick: &Pick) -> Option<gix::ObjectId> {
        if pick.exclude_from_tracking {
            return None;
        }
        let Some(lineage) = self.lineages.get_mut(&pick.id) else {
            return Some(pick.id);
        };
        if lineage.precs.is_empty() {
            return lineage.original;
        }
        let precs = std::mem::take(&mut lineage.precs);
        self.successions
            .extend(precs.into_iter().map(|prec| Succession {
                prec,
                succ: pick.id,
            }));
        Some(pick.id)
    }

    /// Makes `pick` the output of a fold or split of `precs`, which starts tracking it if it
    /// wasn't yet.
    pub(crate) fn emerge(&mut self, pick: &mut Pick, precs: Vec<gix::ObjectId>) {
        let original = (!pick.exclude_from_tracking).then_some(pick.id);
        pick.exclude_from_tracking = false;
        self.lineages
            .entry(pick.id)
            .or_insert(Lineage {
                original,
                precs: Vec::new(),
            })
            .precs = precs;
    }
}

#[derive(Default)]
struct Incidence {
    succs: BTreeMap<gix::ObjectId, Vec<gix::ObjectId>>,
    precs: BTreeMap<gix::ObjectId, Vec<gix::ObjectId>>,
}

impl Incidence {
    fn connect(&mut self, prec: gix::ObjectId, succ: gix::ObjectId) {
        let succs = self.succs.entry(prec).or_default();
        if !succs.contains(&succ) {
            succs.push(succ);
            self.precs.entry(succ).or_default().push(prec);
        }
    }

    fn is_fold(&self, succ: &gix::ObjectId) -> bool {
        let precs = &self.precs[succ];
        precs.len() > 1 && precs.iter().all(|prec| self.succs[prec].len() == 1)
    }

    fn into_rewrites(self) -> Vec<Rewrite> {
        let folds = self
            .precs
            .iter()
            .filter(|(succ, _)| self.is_fold(succ))
            .map(|(succ, precs)| Rewrite::Fold {
                precs: precs.clone(),
                succ: *succ,
            });
        let others = self
            .succs
            .iter()
            .filter_map(|(prec, succs)| match succs.as_slice() {
                [succ] if succ == prec || self.is_fold(succ) => None,
                [succ] => Some(Rewrite::Replace {
                    prec: *prec,
                    succ: *succ,
                }),
                _ => Some(Rewrite::Split {
                    prec: *prec,
                    succs: succs.clone(),
                }),
            });
        folds.chain(others).collect()
    }
}

impl<M: RefMetadata> SuccessfulRebase<'_, '_, M> {
    /// What this session did to the commits that existed before it, as hyperedges that lead to
    /// the commits that exist now, and that only abbreviate rewrites.
    pub fn rewrites(&self) -> Result<Vec<Rewrite>> {
        let mut incidence = Incidence::default();
        for step in order_steps_picking(&self.graph)? {
            let Step::Pick(pick) = &self.graph[step] else {
                continue;
            };
            let Some(lineage) = self.history.lineages.get(&pick.id) else {
                continue;
            };
            if pick.exclude_from_tracking
                || (lineage.original != Some(pick.id)
                    && is_workspace_commit(&self.graph, &self.repo, step))
            {
                continue;
            }
            for prec in lineage.precs() {
                incidence.connect(*prec, pick.id);
            }
        }
        for Succession { prec, succ } in &self.history.successions {
            incidence.connect(*prec, *succ);
        }
        Ok(incidence.into_rewrites())
    }
}
