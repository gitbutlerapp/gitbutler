use anyhow::Result;
use but_rebase::graph_rebase::{Editor, GraphEditorOptions, cherry_pick::PickSignMode};
use but_workspace::commit::{commits_list_unsigned, commits_sign};

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments,
    named_writable_scenario_with_description_and_graph as writable_scenario,
};

/// Check if a commit has a PGP/SSH signature by looking for `gpgsig` in the raw data.
fn commit_is_signed(repo: &gix::Repository, id: gix::ObjectId) -> bool {
    repo.find_commit(id)
        .expect("commit exists")
        .decode()
        .expect("commit has valid data")
        .extra_headers()
        .pgp_signature()
        .is_some()
}

#[test]
/// This test case emulates the KISS approach of signing all commits in the workspace by passing
/// all unsigned commits to commits_sign.
///
/// This relies on the "cascade effect" of running the editor with [`PickSignMode::IfChanged`] to
/// re-sign descendant commits that are already signed.
fn sign_all_unsigned_commits_in_workspace() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-with-mixed-signed-unsigned", |meta| {
            add_stack_with_segments(
                meta,
                1,
                "unsigned-top",
                StackState::InWorkspace,
                &["unsigned-bottom"],
            );
            add_stack_with_segments(
                meta,
                2,
                "mixed-top",
                StackState::InWorkspace,
                &["mixed-bottom"],
            );
        })?;

    let mut ws = graph.into_workspace()?;

    let unsigned = commits_list_unsigned(&ws, &repo, None)?;
    assert_eq!(unsigned.unsigned_commits.len(), 3);

    let editor = Editor::create_with_opts(
        &mut ws,
        &mut meta,
        &repo,
        &GraphEditorOptions {
            sign_mode: PickSignMode::IfChanged,
        },
    )?;
    let outcome = commits_sign(editor, unsigned.unsigned_commits)?;
    let materialize_outcome = outcome.rebase.materialize()?;

    let workspace_commit_id = repo.head_id()?.detach();
    let commit_mappings = materialize_outcome.history.commit_mappings();
    assert_eq!(
        commit_mappings.len(),
        5,
        "expected 4 commits + the workspace commit"
    );

    let num_signed_commits: i32 = commit_mappings
        .values()
        .map(|new_id| commit_is_signed(&repo, *new_id) as i32)
        .sum();

    let num_unsigned_commits_in_ws = commits_list_unsigned(&ws, &repo, None)?
        .unsigned_commits
        .len();

    assert!(!commit_is_signed(&repo, workspace_commit_id));
    assert_eq!(num_signed_commits, 4, "expected all 4 commits to be signed");
    assert_eq!(
        num_unsigned_commits_in_ws, 0,
        "expected no unsigned commits to be left in the workspace"
    );

    Ok(())
}

#[test]
/// This test emulates the slightly more surgical approach of passing only the "oldest" unsigned
/// commit we wish to sign to commits_sign, and relying on any descendant unsigned commit to be
/// signed by virtue of the editor running with [`PickSignMode::IfChanged`] on all picks.
///
/// This test case is mostly here to document this behavior, and also show that we don't need to
/// sign the entire workspace: here we just sign the one stack.
///
/// It's important to note that there's no particular efficiency gain of passing just the oldest
/// unsigned commit(s) over passing in _every_ unsigned commit, as they all need to be signed
/// anyway whether that be through cascading signs or force picks, and the brunt of the compute is
/// gonna be the actual signing.
fn sign_entire_stack_by_signing_only_oldest_unsigned_commit() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-with-mixed-signed-unsigned", |meta| {
            add_stack_with_segments(
                meta,
                1,
                "unsigned-top",
                StackState::InWorkspace,
                &["unsigned-bottom"],
            );
            add_stack_with_segments(
                meta,
                2,
                "mixed-top",
                StackState::InWorkspace,
                &["mixed-bottom"],
            );
        })?;
    let unsigned_stack_id = gitbutler_stack::StackId::from_number_for_testing(1);
    let mixed_stack_id = gitbutler_stack::StackId::from_number_for_testing(2);

    let mut ws = graph.into_workspace()?;

    let unsigned_bottom = repo.rev_parse_single("unsigned-bottom")?.detach();

    let editor = Editor::create_with_opts(
        &mut ws,
        &mut meta,
        &repo,
        &GraphEditorOptions {
            sign_mode: PickSignMode::IfChanged,
        },
    )?;
    let materialize_outcome = commits_sign(editor, vec![unsigned_bottom])?
        .rebase
        .materialize()?;

    let commit_mappings = materialize_outcome.history.commit_mappings();
    assert_eq!(
        commit_mappings.len(),
        3,
        "expected both commits in unsigned stack + workspace commit to be updated"
    );

    let num_signed_commits: i32 = commit_mappings
        .values()
        .map(|new_id| commit_is_signed(&repo, *new_id) as i32)
        .sum();
    let num_unsigned_commits_in_unsigned_stack =
        commits_list_unsigned(&ws, &repo, Some(unsigned_stack_id))?
            .unsigned_commits
            .len();
    let num_unsigned_commits_in_mixed_stack =
        commits_list_unsigned(&ws, &repo, Some(mixed_stack_id))?
            .unsigned_commits
            .len();

    assert_eq!(
        num_signed_commits, 2,
        "expected all 2 commits in the unsigned stack to be signed"
    );
    assert_eq!(
        num_unsigned_commits_in_unsigned_stack, 0,
        "expected no unsigned commits in unsigned stack"
    );
    assert_eq!(
        num_unsigned_commits_in_mixed_stack, 1,
        "expected to still have unsigned commit in mixed stack as we were not signing it",
    );

    Ok(())
}

#[test]
fn sign_empty_set_succeeds() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-with-mixed-signed-unsigned", |meta| {
            add_stack_with_segments(
                meta,
                1,
                "unsigned-top",
                StackState::InWorkspace,
                &["unsigned-bottom"],
            );
            add_stack_with_segments(
                meta,
                2,
                "mixed-top",
                StackState::InWorkspace,
                &["mixed-bottom"],
            );
        })?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create_with_opts(
        &mut ws,
        &mut meta,
        &repo,
        &GraphEditorOptions {
            sign_mode: PickSignMode::IfChanged,
        },
    )?;

    let outcome = commits_sign(editor, std::iter::empty::<gix::ObjectId>())?;
    let materialize_outcome = outcome.rebase.materialize()?;

    let commit_mappings = materialize_outcome.history.commit_mappings();

    assert_eq!(
        commit_mappings.len(),
        1,
        "expected only workspace commit in the mappings"
    );
    let (_, new_id) = commit_mappings.iter().next().unwrap();
    let workspace_commit_id = repo.head_id()?.detach();

    assert_eq!(
        *new_id, workspace_commit_id,
        "expected the workspace commit to be the only modified commit"
    );
    assert!(
        !commit_is_signed(&repo, workspace_commit_id),
        "expected workspace commit to be unsigned"
    );

    Ok(())
}
