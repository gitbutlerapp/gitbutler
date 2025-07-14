//! A way to represent the graph in a simplified (but more usable) form.
//!
//! This is the current default way of GitButler to perceive its world, but most inexpensively generated to stay
//! close to the source of truth, [The Graph](crate::Graph).
//!
//! These types are not for direct consumption, but should be processed further for consumption by the user.

/// Types related to the stack representation for graphs.
///
/// Note that these are always a simplification, degenerating information, while maintaining a link back to the graph.
mod stack;
pub use stack::{Stack, StackCommit, StackCommitDebugFlags, StackCommitFlags, StackSegment};

mod workspace;
pub use workspace::{HeadLocation, Target, Workspace};
