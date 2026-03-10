use anyhow::Result;
use but_workspace::commit::commits_list_unsigned;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
};

#[test]
fn filter_by_stack_id() -> Result<()> {
    let (_tmp, graph, repo, _meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-with-mixed-signed-unsigned",
            |meta| {
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
            },
        )?;

    let unsigned_stack_id = gitbutler_stack::StackId::from_number_for_testing(1);
    let mixed_stack_id = gitbutler_stack::StackId::from_number_for_testing(2);

    let ws = graph.into_workspace()?;
    let outcome_unsigned_stack = commits_list_unsigned(&ws, &repo, Some(unsigned_stack_id))?;
    let outcome_mixed_stack = commits_list_unsigned(&ws, &repo, Some(mixed_stack_id))?;

    assert_eq!(
        outcome_unsigned_stack.unsigned_commits.len(),
        2,
        "expected 2 unsigned commits for unsigned stack, got {}: {:?}",
        outcome_unsigned_stack.unsigned_commits.len(),
        outcome_unsigned_stack.unsigned_commits,
    );

    assert_eq!(
        outcome_mixed_stack.unsigned_commits.len(),
        1,
        "expected 1 unsigned commits for mixed stack, got {}: {:?}",
        outcome_mixed_stack.unsigned_commits.len(),
        outcome_mixed_stack.unsigned_commits,
    );

    Ok(())
}

#[test]
fn all_unsigned_commits_are_listed() -> Result<()> {
    let (_tmp, graph, repo, _meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-with-mixed-signed-unsigned",
            |meta| {
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
            },
        )?;

    let ws = graph.into_workspace()?;
    let outcome = commits_list_unsigned(&ws, &repo, None)?;

    assert_eq!(
        outcome.unsigned_commits.len(),
        3,
        "expected 3 unsigned commits (signed mixed excluded), got {}: {:?}",
        outcome.unsigned_commits.len(),
        outcome.unsigned_commits
    );

    let unsigned_bottom = repo.rev_parse_single("unsigned-bottom")?.detach();
    let unsigned_top = repo.rev_parse_single("unsigned-top")?.detach();
    let mixed_bottom = repo.rev_parse_single("mixed-bottom")?.detach();
    assert!(
        outcome.unsigned_commits.contains(&unsigned_bottom),
        "unsigned-bottom commit should be unsigned"
    );
    assert!(
        outcome.unsigned_commits.contains(&unsigned_top),
        "unsigned-top commit should be unsigned"
    );
    assert!(
        outcome.unsigned_commits.contains(&mixed_bottom),
        "mixed bottom commit should be unsigned",
    );

    // just to be on the safe side, we'll also explicitly check that the signed top-commit of the
    // mixed stack is _not_ in the result set.
    let mixed_top = repo.rev_parse_single("mixed-top")?.detach();
    assert!(
        !outcome.unsigned_commits.contains(&mixed_top),
        "signed mixed-top commit should not be in the unsigned set"
    );

    Ok(())
}
