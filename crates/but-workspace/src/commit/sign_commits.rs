//! An action to forcibly sign all commits on a branch.

use std::borrow::Borrow;

use anyhow::Result;
use but_core::{RefMetadata, commit::SignCommit};
use but_graph::CommitFlags;
use but_rebase::graph_rebase::{
    Editor, GraphEditorOptions, Pick, Step, SuccessfulRebase, cherry_pick::PickMode,
};
use gix::prelude::ObjectIdExt;

/// The result signing all commits on a branch.
#[derive(Debug)]
pub struct SignCommitsOutcome<'ws, 'meta, M: RefMetadata> {
    /// A successful rebase result for continuing operations. This will be always provided
    /// regardless of whether any commits were signed.
    pub rebase: SuccessfulRebase<'ws, 'meta, M>,
}

/// Sign all commits on the branch.
///
/// If all commits are already signed, this is a NOOP.
pub fn sign_commits<'ws, 'meta, M: RefMetadata>(
    ref_name: impl Borrow<gix::refs::FullNameRef>,
    repo: &gix::Repository,
    workspace: &'ws mut but_graph::Workspace,
    meta: &'meta mut M,
) -> Result<SignCommitsOutcome<'ws, 'meta, M>> {
    let graph = but_graph::Graph::from_head(repo, meta, but_graph::init::Options::limited())?;
    let ref_name = ref_name.borrow();

    let Some(segment) = graph.segment_by_ref_name(ref_name) else {
        anyhow::bail!("ref not found in graph: {ref_name}");
    };

    let mut editor = Editor::create_with_opts(
        workspace,
        meta,
        &repo,
        &GraphEditorOptions {
            // SignCommit::Yes makes the signing cascade as necessary, s.t. already signed commits
            // are resigned if any of their parents are forcibly signed. Without this, already
            // signed commits that are parents of unsigned commits may themselves become unsigned
            // as a result of the rebase.
            default_sign_commit: SignCommit::Yes,
            ..GraphEditorOptions::default()
        },
    )?;

    for graph_commit in &segment.commits {
        if graph_commit.flags.contains(CommitFlags::Integrated) {
            continue
        }

        let commit = but_core::Commit::from_id(graph_commit.id.attach(&repo))?;

        if !commit.extra_headers().pgp_signature().is_some() {
            let selector = editor.select_commit(graph_commit.id)?;
            let mut pick = Pick::new_pick(graph_commit.id);
            pick.sign_commit = SignCommit::Yes;
            pick.pick_mode = PickMode::Force;
            editor.replace(selector, Step::Pick(pick))?;
        }
    }

    let rebase = editor.rebase()?;
    Ok(SignCommitsOutcome { rebase })
}
