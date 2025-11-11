//! * Select something, like picking things.
//!     - Can this be a series of picks with information for reference updates.
//!     - `make_steps()` to rebase everything above `this`.
//!     - Can the thing you edit be a `graph` that is edited, instead of a `vector` (sequence)?
//!     - It outputs all edits that would be done.
//!     - commit all given edits
//! * How to optimise for usability?
//!     - Use the `but_graph::Graph` and make it possible to change it.
//!     - For instance a squash, remove two commits, replace it with the new one.
//!     - Now `solve` it, tell me what would be done, dry-run. Be able to commit it when done.
//! * The status quo
//!     - `but-rebase`
//!
//! ### Output of the edit
//!
//! - **worktrees to update**
//!     - path to worktree, enough info to do a safe-checkout
//! - **workspace metadata**
//!     - Edits to apply to metadata
//! - **reference updates**
//!     - `Vec<gix::refs::RefEdit>`
//! - **commit cherry-picking**
//!     - auto-conflict resolution
//!     - Just write as in-memory objects
//! - **workspace re-merge**
//!     - Outcome of in-memory workspace commit merge.
//!
//! All of the above can be **materialised**.
//! All of the above can be visualised with a new `Graph` instance,
//! from which a workspace can be created as well.
//!
//! ### Kinds of Edits
//!
//! See the sketches below for how this could look like.
//! In theory, just sketch the API as you would like it based on what we can do today.
//! Maybe it can boil down to `replace` under the hood.
//! * **commit**
//!     - [Select::Point]:[`Place::Commit`]
//! * **squash**
//! * **rebase subgraph**
//! TBC
#![allow(missing_docs)]
use but_graph::{CommitIndex, SegmentIndex};

fn squash_two_commits(repo: &gix::Repository) {
    let editor = Editor::new(repo, workspace);
    // let selection = editor.selection_for_range(top_commit_id, bottom_commit_id);
    let squashed_commit = squash(editor.repo(), first_commit_id, into_commit_id);

    let first = editor.select_commit(first_commit_id);
    let into = editor.select_commit(into_commit_id);

    let _removed = editor.replace(first, None);
    let _replaced = editor.replace(into, squashed_commit);

    let (graph_with_squash, edits) = editor.into_edits(&mut meta);

    fail_if_there_is_weird_stuff(edits);
    let ws = graph_with_squash.to_workspace();

    edits.materialize();
}

fn reorder_commit(repo: &gix::Repository) {
    let editor = Editor::new(repo, workspace);

    let commit_to_move = editor.select_commit(commit_id);
    let removed = editor.replace(commit_to_move, None);

    // let destination = editor.select_commit(destination_commit_id);
    // editor.replace(destination, Place::from_selections(destination, removed));
    editor.insert(AboveCommit(destination_commit_id), removed);
    editor.insert(AboveReference(reference_name), removed);
}

/// As an extension to `but-rebase`.
pub fn kiril_squash(
    ctx: &CommandContext,
    perm: &mut WorktreeWritePermission,
    subject: ObjectId,
    target: StackId,
    from: ObjectId,
    to: ObjectId,
) -> Result<()> {
    let repo = ctx.gix_repo_for_merging()?;
    let but_graph = but_graph::gimme_graph(ctx)?;
    // but_graph.find(start);

    let lower = todo!();// of from & to
    let upper = todo!();// of from & to

    let subgraph_of_steps = but_graph.rebase_steps(lower); // This is a directed graph of all the commits needed to rebase above lower's parent/s

    let new_commit= todo!(); // using the upper commit's tree

    subgraph_of_steps.remove_pick(from);
    subgraph_of_steps.replace_pick(to, new_commit);
    // Ops:
    // add above pick
    // add below pick
    // remove pick
    // replace pick

    // This writes out new commits into the repo (this _could_ be in memory if it's an in memory gix repo).
    let output = subgraph_of_steps.rebase();

    output.status // is tree conflicting, is workspace commit conflicted, causes conflicted commits (vec of what ends up conflicted)
    // This is useful for two-step rebase operations
    let updated_subgraph_of_steps = output.steps;

    // This updates any references that need to change.
    let updated_but_graph = ctx.apply_rebase(output);

    Ok(())
}

/// * Given a commit or Segment, find the next possible `Selection`.
/// * replace range: section of commits, to become a new set of commits
///     - `<empty-range>:<one-or-more>` insert one or more commits
///     - `<range>:<empty-range>` drop/delete a range of commits.
///     - as the above returns the removed commits, these can be inserted elsewhere.
pub struct Editor<'a, 'graph> {
    workspace: &'a but_graph::projection::Workspace<'graph>,
}

impl<'a, 'graph> Editor<'a, 'graph> {
    pub fn new(workspace: &'a but_graph::projection::Workspace<'graph>) -> Self {
        Editor { workspace }
    }
}

/// A way to mark a portion of a workspace.
pub enum AnchorPoint {
    Segment(SegmentIndex),
    Commit {
        segment: SegmentIndex,
        commit: CommitIndex,
    },
}

pub enum Select {
    Range {
        top: AnchorPoint,
        bottom: AnchorPoint,
    },
    Point(AnchorPoint),
}

pub enum Place {
    Commit,
}
