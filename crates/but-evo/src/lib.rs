//! Obsolescence markers record how commits were rewritten. The README is the stable definition
//! of the model and of what's persisted.
#![deny(missing_docs)]

use std::collections::BTreeMap;

use anyhow::Result;
use gix::bstr::BString;

mod effect;
mod record;
mod sqlite;

pub use record::{Provenance, Rewrite, record};

/// A hyperedge from one predecessor to the successors that replace it together.
///
/// A marker has no identity beyond its value, so two equal markers are the same marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    /// The commit that was replaced. It's never one of `succs`.
    pub prec: gix::ObjectId,
    /// The commits that replace `prec`. Empty if it was pruned.
    pub succs: Vec<gix::ObjectId>,
    /// Metadata about the rewrite, as described in the README.
    pub meta: BTreeMap<BString, BString>,
    /// When the rewrite happened.
    pub date: gix::date::Time,
    /// The parents of `prec`. `None` means not recorded, and `Some(vec![])` means `prec` is a
    /// root commit.
    pub parents: Option<Vec<gix::ObjectId>>,
}

/// A place that markers can be persisted to. Implementations find matching markers in at most
/// `O(log n + k)` time, where `k` is the number of matches.
pub trait MarkerStore {
    /// All markers that replace `prec`, in no particular order.
    fn find_by_prec(&self, prec: gix::ObjectId) -> Result<Vec<Marker>>;
    /// All markers that have `succ` as a successor, in no particular order.
    fn find_by_succ(&self, succ: gix::ObjectId) -> Result<Vec<Marker>>;
    /// Atomically store `markers`, ignoring those that are already stored.
    fn write(&mut self, markers: &[Marker]) -> Result<()>;
}
