//! A graph data structure for seeing the Git commit graph as segments.
//!
//! This crate is part of the Correctness and Stability initiative.
//!
//! ### Before the Graph
//!
//! The application traditionally displays commits in lanes while allowing them to be segmented. Today a lane is a
//! stack of segments. This is great for users as it's easy to understand, but there is a problem with it: it's a complete lie.
//!
//! To generate stacks, one performed a graph traversal, following only the first parent, down to the merge-base of the workspace
//! commit with the target branch. It's clear how this degenerates information especially around merges that the application
//! helps to create by allowing to merge with the target branch.
//!
//! When trying to rebase the workspace onto updated target branches though, it still exclusively operates in stacks, ignoring
//! the complexities of the underlying graph, and the illusion starts to break down.
//!
//! Besides that, there is an inherent mental issue when programming with data structures that degenerate information, as the
//! resulting program will inherently be unfit for the task as it's based on over-simplified assumptions baked into its very core.
//!
//! With the current data structures, achieving correctness simply isn't possible.
//!
//! ### The Graph - the Solution
//!
//! The graph solves this problem by simplifying the commit-graph and optimizing it for traversal, without oversimplifying the
//! underlying commit-graph. That way, the notion of stacks with segments is merely a view of the graph specifically prepared
//! for presentation.
//!
//! When operations are supposed to be performed, we can now do things like this `git rebase --preserve-merges origin/main`,
//! creating a correctly ordered list of operations to do the transplantation correctly. Thanks to the additional metadata
//! collected into the Graph I'd expect us to be able to whatever it takes without limitation.
//!
//! Besides that, operating on a graph, despite its own complexities, is finally aligning the mental model of the programmer
//! with what's actually there, for algorithms suitable to perform the job correctly.
//!
//! All this makes the Graph the **new core data-structure** that is the world of GitButler and upon which visualizations and
//! mutation operations are based.
//!
//! ### New Workspace Concepts
//!
//! The workspace is merely a projection of *The Graph*, and as such is mostly useful for display and user interaction.
//! In the end it boils down to passing commit-hashes, or [segment-ids](usize) at most.
//!
//! The workspace has been redesigned from the ground up for flexibility, enabling new user-experiences. To help thinking
//! about these, a few new concepts will be good to know about.
//!
//! #### Entrypoint
//!
//! *The Graph* knows where its traversal started as *Entrypoint*, even though it may extend beyond the entrypoint as it
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
//! For that reason, whenever a commit isn't the end of the graph, but the end traversal as a [limit was hit](init::Options::with_limit_hint),
//! it will be flagged as such.
//!
//! This way one can visualize such Early Ends, and allow the user to extend the traversal selectively the next time it
//! is performed.
//!
//! Despite that, one has to learn how to deal with possible huge graphs, and possible workspaces with a lot of commits,
//! and [a hard limit](init::Options::with_hard_limit()) as long as downstream cannot deal with this on their own.
//!
//! #### Managed Workspaces, and unmanaged ones
//!
//! A Workspace is considered managed if it [has workspace metadata](Workspace::metadata). This is typically
//! only the case for workspaces that have been created by GitButler.
//!
//! Workspaces without such metadata can be anything, and are usually just made up to allow GitButler to work with it based
//! on any `HEAD` position. These should be treated with care, and multi-lane workflows should generally be avoided - these
//! are reserved to managed Workspaces with the managed merge commit that comes with them.
//!
//! #### Optional Targets
//!
//! Even on *Managed Workspaces*, target references are now optional. This makes it possible to have a workspace that doesn't
//! know if it's integrated or not. These are the reason a [soft limit](init::Options::with_limit_hint()) must always be set
//! to assure the traversal doesn't fetch the entire Git history.
//!
//! This, however, also means that the workspace creation doesn't have to be interrupted by a "what's your target" prompt anymore.
//! Instead, this can be prompted once an action first requires it.
//!
//! #### Commit Flags and Segment Flags
//!
//! For convenience, various boolean parameters have been aggregated into [bitflags](Commit::flags). Thanks to the way *The Graph*
//! is traversed, we know that the first commit of any graph segment will always bear the flags that are also used by every other commit
//! contained within it. Thus segment flags are equivalent to the flags of
//! their first commit.
//!
//! The same is *not* true for [stack segments](workspace::StackSegment), i.e. segments within a [workspace projection](Workspace).
//! The reason for this is that they are first-parent aggregations of one *or more* graph segments, and thus have multiple
//! sets of flags, possibly one per segment.
//!
//! #### The 'frozen' Commit-Flag
//!
//! Commits now have a new state that tells for each if it is reachable by *any* remote, and further, if it's reachable
//! by the remote configured for *their segment*.
//!
//! This additional partitioning could be leveraged for enhanced user experiences.
//!
//! ### The Graph - Traversal and more
//!
//! There are two steps to processing the git commit-graph into more usable forms.
//!
//! * **traversal**
//!     - a commit-first walk of the git commit graph that produces a segmented graph, assigning
//!       commits to segments and splitting on incoming and multiple outgoing connections.
//! * **projection**
//!     - transform the segmented graph into an application-specific view — stacks of first-parent
//!       traversed named segments — folding in workspace metadata (which the commit-graph itself
//!       can't hold) as it goes.
//!
//! #### Commits are owned by Segments
//!
//! A commit can only be owned by a single segment. Thus, there are empty *named* segments which point at other segments,
//! effectively representing a reference.
//! Which of these references gets to own a commit depends on the traversal logic, or can be the result of *Reconciliation*.
//!
//! #### Reconciliation
//!
//! *The Graph* is created from traversing the Git commit graph. Thus, information that is not contained in it has to be
//! reconciled with *what was actually traversed*.
//!
//! Nonetheless, we can create *stacks* as independent branches and dependent branches inside of them without having
//! a single commit to differentiate their respective branches from each other.
//!
//! Imagine a repository with a single commit `73a30f8` with the following Git references pointing to it: `gitbutler/workspace`,
//! `stack1-segment1`, `stack1-segment2`, `stack2-segment1`, and `refs/remotes/origin/main`.
//!
//! Right after traversal, a Graph would look like this:
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
//! This is due to `gitbutler/workspace` finding `73a30f8` first, with `origin/main` arriving later, pointing to the
//! first commit in `gitbutler/workspace` effectively. The other references aren't participating in the traversal.
//!
//! The tip that finds the commit first is dependent on various factors, and it could also happen that `origin/main` finds
//! it first. In any case, this needs to be adjusted after traversal in the process called *reconciliation*, so the graph
//! matches what our [workspace metadata](but_core::ref_metadata::Workspace::stacks) says it should be.
//!
//! After reconciling, the graph would become this:
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
//! A projection is a mapping of the graph to any shape an application needs, and for any purpose.
//! The source of truth is the commit graph; segments are themselves a derived view, and projections built on
//! top (like [`Workspace`]) are further simplifications — inherently lossy.
//! Keep links back to the commits/segments the information was extracted from.
//!
//! #### The segment family: two projections of a recorded run
//!
//! A "segment" is a run of commits the walk records (its name/metadata keyed by [`usize`] in the
//! traversal `State`); two consumer-facing projections derive from those records:
//!
//! - [`StackSegment`](workspace::StackSegment) — projected for the **stack** view: linearized
//!   along the first parent and enriched for display; may aggregate several recorded segments.
//! - the [`BranchGraph`] branches — projected for the **rebase step graph**: lean, 1:1 with
//!   recorded segments, keeping every edge so the full topology survives.
//!
//! The two projections are siblings (`StackSegment` : stacks :: `BranchGraph` : steps), differing only in
//! how much topology and detail their consumer needs.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod segment;
/// Use this for basic types like [`petgraph::Direction`], and graph algorithms.
pub use petgraph;
pub use segment::{
    Commit, CommitFlags, RefInfo, SegmentMetadata, StopCondition, Worktree, WorktreeKind,
};

// Whether a traversal follows only first-parent edges.
boolean_enums::gen_boolean_enum!(pub FirstParent);
/// Produce a graph from a Git repository.
pub mod init;

#[path = "projection/mod.rs"]
pub mod workspace;
pub use workspace::workspace::Workspace;

pub mod commit_graph;

pub mod branch_graph;
pub use branch_graph::BranchGraph;

mod debug;
