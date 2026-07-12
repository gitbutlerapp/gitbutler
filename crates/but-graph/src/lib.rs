//! A graph data structure for seeing the Git commit graph as a workspace.
//!
//! ### The pipeline
//!
//! Everything in this crate flows through one pipeline with a single substrate, the
//! [`CommitGraph`]:
//!
//! ```text
//! repository ──walk──▶ CommitGraph (+ ref layout) ──project──▶ Workspace
//!                          ▲                                       │
//!                          └────────── but-rebase edits ◀──────────┘
//! ```
//!
//! * **walk** ([`walk`], entered through `CommitGraph::from_head` and `CommitGraph::from_tip`):
//!   the traversal seeds from `HEAD`, the workspace ref, the target, and the stack branches;
//!   obeys goals and limits; propagates flags; and accumulates the [`CommitGraph`] — an arena
//!   of commits with ordered parent arrays, the connectivity the traversal actually followed,
//!   and every encountered ref attached as data.
//! * **build** (the private `build` module, entered through `graph_from_repository`):
//!   gather-then-build. Pure fact and planning passes decide the workspace's ref structure as
//!   data — boundaries, chains from workspace metadata, the name lifecycle — authored as
//!   build-internal segments whose only lasting outputs are the stored [`ref_layout`] (every
//!   surfaced ref with its position over the commit graph) and the context the projection
//!   reads (enrichment details, the entry's resolution verdict).
//! * **project** ([`workspace`], entered through the `Workspace::from_*` constructors): the
//!   application view — stacks of first-parent chains with integration status against the
//!   target — derived from the commit graph, its layout and the build context alone.
//! * **edit** (`but-rebase`): the editor is created FROM the [`CommitGraph`] (mutability follows
//!   reachability), and an edited commit graph re-enters the same build via
//!   `workspace_from_commit_graph` — edit previews and fresh walks share one code path.
//!   `but-workspace` sits on top of both as the operations layer.
//!
//! ### New Workspace Concepts
//!
//! The workspace is a projection of the commit graph, and as such is mostly useful for display and user interaction.
//! In the end it boils down to passing commit-hashes.
//!
//! The workspace has been redesigned from the ground up for flexibility, enabling new user-experiences. To help thinking
//! about these, a few new concepts will be good to know about.
//!
//! #### Entrypoint
//!
//! The graph knows where its traversal started as *Entrypoint*, even though it may extend beyond the entrypoint as it
//! needs to discover possible surrounding workspaces and the target branches that come with them.
//! In practice, the entrypoint relates to the position of the Git `HEAD` reference, and with that it relates to what
//! the user currently sees in their worktree.
//!
//! #### Early End of Traversal
//!
//! During traversal there are mandatory goals, but when reached the traversal usually obeys a limit, if configured.
//! This is particularly relevant in open-ended traversals outside of workspaces, they can go on until the end of history,
//! literally.
//!
//! For that reason, whenever a commit isn't the end of the graph, but the end traversal as a [limit was hit](walk::Options::with_limit_hint),
//! it will be flagged as such.
//!
//! This way one can visualize such Early Ends, and allow the user to extend the traversal selectively the next time it
//! is performed.
//!
//! Despite that, one has to learn how to deal with possible huge graphs, and possible workspaces with a lot of commits,
//! and [a hard limit](walk::Options::with_hard_limit()) as long as downstream cannot deal with this on their own.
//!
//! #### Managed Workspaces, and unmanaged ones
//!
//! A Workspace is considered managed if it [has workspace metadata](Workspace::metadata). This is typically
//! only the case for workspaces that have been created by GitButler.
//!
//! Workspaces without such metadata can be anything, and are usually just made up to allow GitButler to work with it based
//! on any `HEAD` position. These should be treated with care, and multi-stack workflows should generally be avoided - these
//! are reserved to managed Workspaces with the managed merge commit that comes with them.
//!
//! #### Optional Targets
//!
//! Even on *Managed Workspaces*, target references are now optional. This makes it possible to have a workspace that doesn't
//! know if it's integrated or not. These are the reason a [soft limit](walk::Options::with_limit_hint()) must always be set
//! to assure the traversal doesn't fetch the entire Git history.
//!
//! This, however, also means that the workspace creation doesn't have to be interrupted by a "what's your target" prompt anymore.
//! Instead, this can be prompted once an action first requires it.
//!
//! #### Commit Flags
//!
//! For convenience, various boolean parameters have been aggregated into [bitflags](Commit::flags),
//! propagated per commit during traversal. [Stack segments](workspace::StackSegment) aggregate
//! first-parent runs of commits, so a segment's commits may carry multiple distinct flag sets —
//! read flags off commits, not segments.
//!
//! #### The 'frozen' Commit-Flag
//!
//! [`CommitFlags::NotInRemote`] marks commits NOT reachable from any remote-seeded tip. The
//! workspace projection inverts it into
//! [`StackCommitFlags::ReachableByRemote`](workspace::StackCommitFlags): commits others may
//! already have observed, to be treated as frozen and not manipulated casually.
//!
//! ### Build decisions
//!
//! #### Commits are owned by Segments
//!
//! A commit can only be owned by a single segment. Thus, there are empty *named* segments which point at other segments,
//! effectively representing a reference.
//! Which of these references gets to own a commit is a *planning* decision.
//!
//! #### Planning chains from metadata
//!
//! The graph is created from traversing the Git commit graph. Thus, information that is not contained in it,
//! like workspace metadata, has to shape the segmented graph as it is built.
//!
//! That way, we can create *stacks* as independent branches and dependent branches inside of them without having
//! a single commit to differentiate their respective branches from each other.
//!
//! Imagine a repository with a single commit `73a30f8` with the following Git references pointing to it: `gitbutler/workspace`,
//! `stack1-segment1`, `stack1-segment2`, `stack2-segment1`, and `refs/remotes/origin/main`.
//!
//! A naive segmentation of the traversal would look like this:
//!
//! ```text
//!   ┌────────────────────┐
//!   │    origin/main     │
//!   └────────────────────┘
//!              │
//!              ▼
//! ┌────────────────────────┐
//! │gitbutler/workspace     │
//! │------------------------│
//! │73a30f8 ►stack1-segment1│
//! │        ►stack1-segment2│
//! │        ►stack2-segment1│
//! │        ►main           │
//! └────────────────────────┘
//! ```
//!
//! This is because `gitbutler/workspace` owns `73a30f8`, with `origin/main` merely pointing to
//! that commit; the other references would be plain refs on it.
//!
//! The chain plan instead reads [workspace metadata](but_core::ref_metadata::Workspace::stacks) before any
//! segment exists and decides which refs form chains of empty segments. Materialization then mints this
//! shape directly:
//!
//! ```text
//! ┌───────────────────┐
//! │    origin/main    │
//! └───────────────────┘
//!            │            ┌────────────────────┐
//!            │            │gitbutler/workspace │
//!            │            └────────────────────┘
//!            │                       │
//!            │             ┌─────────┴─────────┐
//!            │             │                   │
//!            │             ▼                   │
//!            │     ┌───────────────┐           │
//!            │     │stack1-segment1│           ▼
//!            │     └───────────────┘   ┌───────────────┐
//!            │             │           │stack2-segment1│
//!            │             ▼           └───────────────┘
//!            │     ┌───────────────┐           │
//!            │     │stack1-segment2│           │
//!            │     └───────────────┘           │
//!            │             │                   │
//!            │             └─────────┬─────────┘
//!            │                       │
//!            │                       ▼
//!            │                  ┌────────┐
//!            │                  │  main  │
//!            └─────────────────▶│ ------ │
//!                               │ 73a30f │
//!                               └────────┘
//! ```
//!
//! #### Projection
//!
//! A projection maps the commit graph and its stored ref layout to any shape an application
//! needs. Projections are inherently lossy and speak in commit ids and ref names — manipulation
//! never operates on a projection: the rebase editor edits the [`CommitGraph`], and re-projecting
//! the edited substrate yields the next [`Workspace`].
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod records;
pub use records::{
    Commit, CommitFlags, RefInfo, SegmentMetadata, StopCondition, Worktree, WorktreeKind,
};

boolean_enums::gen_boolean_enum!(pub FirstParent);
/// Produce a graph from a Git repository.
pub mod walk;

#[path = "projection/mod.rs"]
pub mod workspace;
pub use workspace::Workspace;

/// The commit-first graph flattened out of the raw traversal — the substrate every graph build
/// starts from. See the module docs.
mod commit_graph;
mod commit_graph_diagnostics;
pub use commit_graph::CommitGraph;
pub use commit_graph_diagnostics::CommitGraphStatistics;
/// The graph builders: derive the workspace's ref layout and context from a [`CommitGraph`]. See the module docs.
mod build;
/// The metadata-driven ref placement table stored on the commit graph. See the module docs.
pub mod ref_layout;
pub(crate) use build::graph_from_repository;
pub(crate) use build::workspace_from_commit_graph;
pub(crate) use build::{graph_from_repository_seeds, graph_from_repository_unmanaged};

pub(crate) mod debug;
