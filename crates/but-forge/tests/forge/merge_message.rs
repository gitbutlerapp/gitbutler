use but_forge::{ForgeName, MergedReviewFromMessage, merged_review_from_message};

#[test]
fn github_merge_button_message() {
    let message = "Merge pull request #15383 from gitbutlerapp/lite-graph-rails\n\nLite: use the new commit line tokens for graph rails\n";
    assert_eq!(
        merged_review_from_message(&ForgeName::GitHub, message),
        Some(MergedReviewFromMessage {
            number: 15383,
            title: "Lite: use the new commit line tokens for graph rails",
            source_branch: Some("lite-graph-rails"),
        })
    );
}

#[test]
fn github_merge_button_message_without_body_keeps_subject_as_title() {
    assert_eq!(
        merged_review_from_message(
            &ForgeName::GitHub,
            "Merge pull request #7 from bob:fix/typo"
        ),
        Some(MergedReviewFromMessage {
            number: 7,
            title: "Merge pull request #7 from bob:fix/typo",
            source_branch: Some("fix/typo"),
        })
    );
}

#[test]
fn github_merge_button_message_with_edited_subject_keeps_the_branch_name() {
    assert_eq!(
        merged_review_from_message(&ForgeName::GitHub, "Merge pull request #12 from a/b (#3)")
            .map(|review| review.source_branch),
        Some(Some("b")),
    );
}

#[test]
fn github_squash_merge_subject() {
    assert_eq!(
        merged_review_from_message(
            &ForgeName::GitHub,
            "Fix the thing (#42)\n\n* first\n* second"
        ),
        Some(MergedReviewFromMessage {
            number: 42,
            title: "Fix the thing",
            source_branch: None,
        })
    );
}

#[test]
fn unrelated_messages_and_other_forges_are_not_recognised() {
    for message in [
        "Fix the thing",
        "Merge branch 'feature' into main",
        "Merge pull request from nowhere",
        "Merge pull request #x from a/b",
        "Merge pull request #-1 from a/b",
        "Refers to (#12) somewhere",
        "Bump deps (#-3)",
        " (#42)",
    ] {
        assert_eq!(
            merged_review_from_message(&ForgeName::GitHub, message),
            None,
            "{message:?} names no review"
        );
    }
    assert_eq!(
        merged_review_from_message(
            &ForgeName::GitLab,
            "Merge pull request #1 from a/b\n\ntitle"
        ),
        None,
        "only GitHub's message shapes are recognised"
    );
}
