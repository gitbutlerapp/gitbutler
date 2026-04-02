//! Utilities for deterministic commit ordering.

mod commit_parentage;
pub use commit_parentage::order_commit_selectors_by_parentage;
