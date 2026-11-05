use std::{collections::BTreeMap, slice::from_ref};

use anyhow::Result;
use but_core::Commit;
use gix::{bstr::BString, prelude::ObjectIdExt as _};

use crate::{Marker, MarkerStore, effect::effect_flags};

/// What an operation did to a commit, which makes a shape that markers can't express, like a
/// fold with two successors, unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rewrite {
    /// `succ` is the new version of `prec`.
    Replace {
        /// The commit that was rewritten.
        prec: gix::ObjectId,
        /// Its new version.
        succ: gix::ObjectId,
    },
    /// `prec` was turned into all of `succs`.
    Split {
        /// The commit that was split.
        prec: gix::ObjectId,
        /// All the commits that it became.
        succs: Vec<gix::ObjectId>,
    },
    /// All of `precs` were combined into `succ`.
    Fold {
        /// The commits that were combined.
        precs: Vec<gix::ObjectId>,
        /// The commit they became.
        succ: gix::ObjectId,
    },
    /// `prec` was removed from history without a replacement.
    Prune {
        /// The commit that was removed.
        prec: gix::ObjectId,
    },
}

/// Who performed a set of rewrites, when, and as part of what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// The `user` metadata, always written.
    pub user: BString,
    /// The date of all resulting markers.
    pub date: gix::date::Time,
    /// The `operation` metadata, left out if `None`.
    pub operation: Option<BString>,
}

/// Write one marker per `Replace`, `Split` and `Prune`, and one per `prec` of a `Fold`, to `store`.
///
/// `repo` provides the parents and the commits to compute `ef1` from, so all of them have to
/// exist. A `prec` that is also a successor survived the rewrite, so it gets no marker.
pub fn record(
    store: &mut impl MarkerStore,
    repo: &gix::Repository,
    rewrites: &[Rewrite],
    provenance: &Provenance,
) -> Result<()> {
    let mut markers = Vec::new();
    for rewrite in rewrites {
        markers.extend(markers_of(repo, rewrite, provenance)?);
    }
    store.write(&markers)
}

fn markers_of(
    repo: &gix::Repository,
    rewrite: &Rewrite,
    provenance: &Provenance,
) -> Result<Vec<Marker>> {
    let (precs, succs) = match rewrite {
        Rewrite::Replace { prec, succ } => (from_ref(prec), from_ref(succ)),
        Rewrite::Split { prec, succs } => (from_ref(prec), succs.as_slice()),
        Rewrite::Fold { precs, succ } => (precs.as_slice(), from_ref(succ)),
        Rewrite::Prune { prec } => (from_ref(prec), [].as_slice()),
    };
    let replaced: Vec<_> = precs.iter().filter(|prec| !succs.contains(prec)).collect();
    let fold_id = (replaced.len() > 1)
        .then(|| fold_id(&provenance.user, &replaced, succs))
        .transpose()?;
    let succ_commits = succs
        .iter()
        .map(|id| Commit::from_id(id.attach(repo)))
        .collect::<Result<Vec<_>>>()?;

    let mut markers = Vec::with_capacity(replaced.len());
    for (fold_idx, prec) in (1..).zip(&replaced) {
        let prec_commit = Commit::from_id(prec.attach(repo))?;
        let mut meta = BTreeMap::from([
            ("user".into(), provenance.user.clone()),
            (
                "ef1".into(),
                effect_flags(&prec_commit, &succ_commits)?
                    .bits()
                    .to_string()
                    .into(),
            ),
        ]);
        if let Some(operation) = &provenance.operation {
            meta.insert("operation".into(), operation.clone());
        }
        if let Some(fold_id) = &fold_id {
            meta.insert("fold-id".into(), fold_id.clone());
            meta.insert("fold-idx".into(), fold_idx.to_string().into());
            meta.insert("fold-size".into(), replaced.len().to_string().into());
        }
        markers.push(Marker {
            prec: **prec,
            succs: succs.to_vec(),
            meta,
            date: provenance.date,
            parents: Some(prec_commit.parents.to_vec()),
        });
    }
    Ok(markers)
}

fn fold_id(user: &[u8], precs: &[&gix::ObjectId], succs: &[gix::ObjectId]) -> Result<BString> {
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
    hasher.update(user);
    for id in precs.iter().copied().chain(succs) {
        hasher.update(id.as_bytes());
    }
    Ok(hasher.try_finalize()?.to_hex_with_len(8).to_string().into())
}
