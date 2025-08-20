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
//! All this makes the Graph the **new core data-structure** that is the world of GitButler and upon which visualisations and
//! mutation operations are based.
//!
//! ### New Workspace Concepts
//!
//! The workspace is merely a projection of *The Graph*, and as such is mostly useful for display and user interaction.
//! In the end it boils down to passing commit-hashes, or [segment-ids](SegmentIndex) at most.
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
//! A Workspace is considered managed if it [has workspace metadata](projection::Workspace::metadata). This is typically
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
//! is traversed, we know that the first commit of any [graph segment](Segment) will always bear the flags that are also used by every other commit
//! contained within it. Thus, [segment flags](Segment::non_empty_flags_of_first_commit()) are equivalent to the flags of
//! their first commit.
//!
//! The same is *not* true for [stack segments](projection::StackSegment), i.e. segments within a [workspace projection](projection::Workspace).
//! The reason for this is that they are first-parent aggregations of one *or more* [graph segments](Segment), and thus have multiple
//! sets of flags, possibly one per [segment](Segment).
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
//! There are three distinct steps to processing the git commit-graph into more usable forms.
//!
//! * **traversal**
//!     - walk the git commit graph to produce a segmented graph, which assigns commits to segments,
//!       but also splits segments on incoming and multiple outgoing connections.
//! * **reconciliation**
//!     - a post-processing step which adds workspace metadata into the segmented graph, as such information
//!       can't be held in the commit-graph itself.
//! * **projection**
//!     - transform the segmented and reconciled graph into a view that is application-specific, i.e. see
//!       stacks of first-parent traversed named segments.
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
//! it first. In any case, this needs to be adjusted after traversal in the process called *reconiliation*, so the graph
//! matches what our [workspace metadata](but_core::ref_metadata::Workspace::stacks) says it should be.
//!
//! After reconciling, the graph would become this:
//!
//! ```text
//! ┌────────────────────┐
//! │    origin/main     │
//! └────────────────────┘
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
//!            │                  ┌─────────┐
//!            │                  │  main   │
//!            └─────────────────▶│ ------- │
//!                               │ 73a30f8 │
//!                               └─────────┘
//! ```
//!
//! #### Projection
//!
//! A projection is a mapping of the segmented graph to any shape an application needs, and for any purpose.
//! It cannot be stressed enough that the source of truth for all commit-graph manipulation must be the segmented graph,
//! as projections are inherently lossy.
//! Thus, it's useful create projects with links back to the segments that the information was extracted from.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod segment;
pub use segment::{Commit, CommitFlags, Segment, SegmentMetadata};

/// Use this for basic types like [`petgraph::Direction`], and graph algorithms.
pub use petgraph;

mod api;
/// Produce a graph from a Git repository.
pub mod init;
pub mod projection;

mod ref_metadata_legacy;
pub use ref_metadata_legacy::{VirtualBranchesTomlMetadata, is_workspace_ref_name};

pub mod virtual_branches_legacy_types;

mod statistics;
pub use statistics::Statistics;

mod debug;

/// Edges to other segments are the index into the list of local commits of the parent segment.
/// That way we can tell where a segment branches off, despite the graph only connecting segments, and not commits.
pub type CommitIndex = usize;

/// A graph of connected segments that represent a section of the actual commit-graph.
#[derive(Default, Debug, Clone)]
#[must_use]
pub struct Graph {
    inner: init::PetGraph,
    /// From where the graph was created. This is useful if one wants to focus on a subset of the graph.
    ///
    /// The [`CommitIndex`] is empty if the entry point is an empty segment, one that is supposed to receive
    /// commits later.
    entrypoint: Option<(SegmentIndex, Option<CommitIndex>)>,
    /// The segment index of the extra target as provided for traversal.
    extra_target: Option<SegmentIndex>,
    /// It's `true` only if we have stopped the traversal due to a hard limit.
    hard_limit_hit: bool,
    /// The options used to create the graph, which allows it to regenerate itself after something
    /// possibly changed. This can also be used to simulate changes by injecting would-be information.
    options: init::Options,
}

/// A resolved entry point into the graph for easy access to the segment, commit,
/// and the respective indices for later traversal.
#[derive(Debug, Copy, Clone)]
pub struct EntryPoint<'graph> {
    /// The index to the segment that served starting point for the traversal into this graph.
    pub segment_index: SegmentIndex,
    /// If present, the index of the commit that started the traversal in the segment denoted by `segment_index`.
    pub commit_index: Option<CommitIndex>,
    /// The segment that served starting point for the traversal into this graph.
    pub segment: &'graph Segment,
    /// If present, the commit that started the traversal in the `segment`.
    ///
    /// It's usually the first commit of the segment due to the way we split segments, and even though
    /// downstream code relies on this properly, the graph itself does not.
    pub commit: Option<&'graph Commit>,
}

/// This structure is used as data associated with each edge and is mainly for collecting
/// the intent of an edge, which should always represent the connection of a commit to another.
/// Sometimes, it represents the connection from a commit (or segment) to an empty segment which
/// doesn't yet have a commit.
/// The idea is to write code that keeps edge information consistent, and our visualization tools hightlights
/// issues with the inherent invariants.
#[derive(Debug, Copy, Clone)]
pub struct Edge {
    /// `None` if the source segment has no commit.
    src: Option<CommitIndex>,
    /// The commit id at `src` in the segment commit list.
    src_id: Option<gix::ObjectId>,
    dst: Option<CommitIndex>,
    /// The commit id at `dst` in the segment commit list.
    dst_id: Option<gix::ObjectId>,
}

/// An index into the [`Graph`].
pub type SegmentIndex = petgraph::graph::NodeIndex;
