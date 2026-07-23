use but_core::worktree::contains_conflict_markers;
use but_testsupport::writable_scenario;

#[test]
fn detects_a_complete_marker_sequence() {
    let marked = b"1\n<<<<<<< ours\ntheir line\n=======\nour line\n>>>>>>> theirs\n2\n";
    assert!(contains_conflict_markers(marked));

    let crlf = b"<<<<<<< ours\r\na\r\n=======\r\nb\r\n>>>>>>> theirs\r\n";
    assert!(contains_conflict_markers(crlf));

    let diff3 = b"<<<<<<< ours\na\n||||||| base\nc\n=======\nb\n>>>>>>> theirs\n";
    assert!(contains_conflict_markers(diff3));

    let unlabelled = b"<<<<<<<\na\n=======\nb\n>>>>>>>\n";
    assert!(contains_conflict_markers(unlabelled));
}

#[test]
fn incomplete_or_lookalike_sequences_are_not_detected() {
    assert!(!contains_conflict_markers(b""));
    assert!(!contains_conflict_markers(b"plain content\n"));
    // A separator line alone, like in setext markdown headers or tables.
    assert!(!contains_conflict_markers(b"header\n=======\ncontent\n"));
    // Start marker without the rest.
    assert!(!contains_conflict_markers(b"<<<<<<< ours\ncontent\n"));
    // Out of order.
    assert!(!contains_conflict_markers(
        b">>>>>>> theirs\n=======\n<<<<<<< ours\n"
    ));
    // Markers must start the line and be exactly 7 characters wide.
    assert!(!contains_conflict_markers(
        b" <<<<<<< ours\n=======\n>>>>>>> theirs\n"
    ));
    assert!(!contains_conflict_markers(
        b"<<<<<<<< ours\n=======\n>>>>>>>> theirs\n"
    ));
    // Binary content is never considered marked.
    assert!(!contains_conflict_markers(
        b"\x00<<<<<<< ours\n=======\n>>>>>>> theirs\n"
    ));
}

#[test]
fn worktree_changes_flag_files_with_conflict_markers() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");

    std::fs::write(
        repo.workdir_path("file").unwrap(),
        "1\n<<<<<<< ours\nupdated\n=======\noriginal\n>>>>>>> theirs\n",
    )?;

    let changes: but_core::ui::WorktreeChanges = but_core::diff::worktree_changes(&repo)?.into();
    assert_eq!(
        changes.conflict_marker_paths,
        Vec::<but_serde::BStringForFrontend>::new(),
        "the plain conversion does not scan for markers"
    );

    let changes = changes.with_conflict_marker_paths(&repo);
    assert_eq!(
        changes.conflict_marker_paths,
        vec![but_serde::BStringForFrontend::from("file")],
        "only the file with a complete marker sequence is flagged, \
         other dirty files stay unflagged"
    );

    Ok(())
}
