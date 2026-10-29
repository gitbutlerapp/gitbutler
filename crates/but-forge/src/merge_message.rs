//! Reading the review a forge-authored merge commit message says it landed.

use crate::ForgeName;

/// The review a forge's own merge commit message names, read from the
/// message alone.
#[derive(Debug, PartialEq, Eq)]
pub struct MergedReviewFromMessage<'a> {
    /// The review number, e.g. `123` in `Merge pull request #123 from …`.
    pub number: i64,
    /// The review title, when the message carries it.
    pub title: &'a str,
    /// The branch the review proposed, when the message carries it.
    pub source_branch: Option<&'a str>,
}

/// Best-effort recognition of the commit messages a forge writes when it
/// merges a review, so target commits can name their review without the
/// forge cache knowing about it. Handles GitHub's merge-button message
/// (`Merge pull request #N from owner/branch` with the title as body) and
/// its squash-merge subject (`title (#N)`); rebase merges leave no trace.
/// Being message-based, it trusts what the message says: an edited merge
/// body or a hand-written `(#N)` issue reference is taken at face value.
/// Other forges are not recognised yet.
pub fn merged_review_from_message<'a>(
    forge: &ForgeName,
    message: &'a str,
) -> Option<MergedReviewFromMessage<'a>> {
    match forge {
        ForgeName::GitHub => github_merged_review(message),
        ForgeName::GitLab | ForgeName::Bitbucket | ForgeName::Azure => None,
    }
}

fn github_merged_review(message: &str) -> Option<MergedReviewFromMessage<'_>> {
    let (subject, body) = message.split_once('\n').unwrap_or((message, ""));
    let subject = subject.trim_end();

    if let Some(rest) = subject.strip_prefix("Merge pull request #") {
        let (number, head) = rest.split_once(" from ")?;
        let number = review_number(number)?;
        // The head is `owner:branch` for forks in older messages and
        // `owner/branch` otherwise; either way the branch follows the owner
        // and ends at the first whitespace, should the subject have been edited.
        let head = head.split_whitespace().next()?;
        let source_branch = head.split_once(['/', ':']).map(|(_, branch)| branch);
        // GitHub puts the pull request title as the first paragraph of the body.
        let title = body.trim().lines().next().unwrap_or("").trim();
        return Some(MergedReviewFromMessage {
            number,
            title: if title.is_empty() { subject } else { title },
            source_branch,
        });
    }

    let (title, suffix) = subject.rsplit_once(" (#")?;
    let number = review_number(suffix.strip_suffix(')')?)?;
    (!title.is_empty()).then_some(MergedReviewFromMessage {
        number,
        title,
        source_branch: None,
    })
}

/// Digits only: `i64::from_str` would also accept a sign.
fn review_number(digits: &str) -> Option<i64> {
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}
