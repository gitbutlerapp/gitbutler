use anyhow::Result;
use gitbutler_repo::{signature, SignaturePurpose};

const PREVIOUS_REVIEW_BASE: &str = "refs/gitbutler/review-base";

/// Updates the review base to the given commit.
///
/// If the review base has the same tree as the given commit, the review base
/// is not updated.
fn upsert_review_base(repository: &git2::Repository, stack_base: git2::Oid) -> Result<git2::Oid> {
    let stack_base_commit = repository.find_commit(stack_base)?;
    let previous_review_base = repository
        .find_reference(PREVIOUS_REVIEW_BASE)
        .ok()
        .and_then(|review_base| review_base.peel_to_commit().ok());

    // If the current review base is the same, there is now need to update it.
    if let Some(previous_review_base) = previous_review_base.clone() {
        if previous_review_base.tree_id() == stack_base_commit.tree_id() {
            return Ok(previous_review_base.id());
        }
    }

    let parents = previous_review_base
        .map(|commit| vec![commit])
        .unwrap_or_default();

    let author_signature = signature(SignaturePurpose::Author)?;
    let committer_signature = signature(SignaturePurpose::Committer)?;

    let commit_message = format!(
        "This commit is a stripped down commit used when pushing code to
gitbutler review.


Base-Commit: {}",
        stack_base
    );

    let new_review_base = repository.commit(
        Some(PREVIOUS_REVIEW_BASE),
        &author_signature,
        &committer_signature,
        &commit_message,
        &stack_base_commit.tree()?,
        &parents.iter().collect::<Vec<_>>(),
    )?;

    Ok(new_review_base)
}
