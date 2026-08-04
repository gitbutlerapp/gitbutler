use std::sync::Arc;

use bstr::BString;
use but_core::{ChangeId, HunkHeader, ref_metadata::StackId};
use but_rebase::graph_rebase::mutate::InsertSide;
use nonempty::NonEmpty;
use ratatui_textarea::TextArea;

use super::{Cursor, is_selectable_in_mode};
use crate::{
    CliId,
    args::atoms::ResolvedCliIdArg,
    command::legacy::status::{
        CommitClassification, FilesStatusFlag,
        output::{StatusOutputContent, StatusOutputLine, StatusOutputLineData},
        tui::{
            InlineRewordMode, Mode, NormalMode, SelectAfterReload,
            app::{
                CommitMessageComposer, CommitMode, CommitSource, MoveMode, MoveSource,
                MoveStackMode, ReorderStackSource,
                mark::{MarkStore, MarkableRef, Marks},
            },
        },
    },
    id::{BranchId, CommitId, CommittedFileId, IdAndHunk, UncommittedHunkOrFile},
};

fn line(data: StatusOutputLineData) -> StatusOutputLine {
    StatusOutputLine {
        connector: None,
        content: StatusOutputContent::Plain(Vec::new()),
        data,
    }
}

fn uncommitted_area(id: &str) -> Arc<CliId> {
    Arc::new(CliId::Uncommitted { id: id.into() })
}

fn commit_id(hex: &str) -> CommitId {
    CommitId {
        commit_id: gix::ObjectId::from_hex(hex.as_bytes()).unwrap(),
        change_id: None,
    }
}

fn commit_cli_id(hex: &str, id: &str) -> Arc<CliId> {
    Arc::new(CliId::Commit {
        commit: commit_id(hex),
        id: id.into(),
    })
}

fn commit_cli_id_with_change_id(hex: &str, id: &str, change_id: u128) -> Arc<CliId> {
    Arc::new(CliId::Commit {
        commit: CommitId {
            change_id: Some(ChangeId::from_number_for_testing(change_id)),
            ..commit_id(hex)
        },
        id: id.into(),
    })
}

fn committed_file_cli_id(hex: &str, path: &str, id: &str) -> Arc<CliId> {
    Arc::new(CliId::CommittedFile {
        committed_file: CommittedFileId {
            commit_id: commit_id(hex).commit_id,
            path: path.into(),
            change_id: None,
        },
        id: id.into(),
    })
}

fn branch_cli_id(name: &str, id: &str, stack_id: Option<StackId>) -> Arc<CliId> {
    Arc::new(CliId::Branch(BranchId {
        name: name.into(),
        id: id.into(),
        stack_id,
    }))
}

fn branch_line(name: &str, id: &str) -> StatusOutputLine {
    line(StatusOutputLineData::Branch {
        cli_id: branch_cli_id(name, id, None),
        is_merged_upstream: false,
    })
}

fn stack_branch_line(name: &str, id: &str, stack_id: StackId) -> StatusOutputLine {
    line(StatusOutputLineData::Branch {
        cli_id: branch_cli_id(name, id, Some(stack_id)),
        is_merged_upstream: false,
    })
}

fn hunk(path: &str, old_start: u32) -> but_core::SingleHunk {
    but_core::SingleHunk {
        hunk_header: Some(HunkHeader {
            old_start,
            old_lines: 1,
            new_start: old_start,
            new_lines: 1,
        }),
        path: BString::from(path),
        diff: None,
    }
}

fn uncommitted_cli_id(path: &str, id: &str) -> Arc<CliId> {
    uncommitted_cli_id_with_old_start(path, id, 1)
}

fn uncommitted_cli_id_with_old_start(path: &str, id: &str, old_start: u32) -> Arc<CliId> {
    Arc::new(CliId::UncommittedHunkOrFile(UncommittedHunkOrFile {
        id: id.to_owned(),
        hunks: NonEmpty::new(IdAndHunk {
            id: id.to_owned(),
            hunk: hunk(path, old_start),
        }),
        is_entire_file: true,
    }))
}

fn uncommitted_file_line(path: &str, id: &str) -> StatusOutputLine {
    line(StatusOutputLineData::UncommittedFile {
        cli_id: uncommitted_cli_id(path, id),
    })
}

fn uncommitted_source(cli_ids: &[Arc<CliId>]) -> CommitSource {
    let mut cli_ids = cli_ids.iter();
    let first = cli_ids.next().expect("test source should not be empty");
    if cli_ids.len() == 0 {
        match &**first {
            CliId::UncommittedHunkOrFile(uncommitted) => {
                CommitSource::UncommittedHunk(uncommitted.clone())
            }
            CliId::Uncommitted { .. }
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch(BranchId { .. })
            | CliId::Stack { .. }
            | CliId::Commit { .. } => panic!("test cli ID should be uncommitted"),
        }
    } else {
        let CliId::UncommittedHunkOrFile(first) = &**first else {
            panic!("test cli ID should be uncommitted")
        };
        let mut hunks = NonEmpty::new(first.clone());
        for cli_id in cli_ids {
            let CliId::UncommittedHunkOrFile(uncommitted) = &**cli_id else {
                panic!("test cli ID should be uncommitted")
            };
            hunks.push(uncommitted.clone());
        }
        CommitSource::Marks(hunks)
    }
}

fn marks<'a, I>(markables: I) -> Marks
where
    I: IntoIterator<Item = MarkableRef<'a>>,
{
    let mut marks = Marks::default();
    for markable in markables {
        match markable {
            MarkableRef::Uncommitted(hunk) => marks.insert_mark(hunk.clone()).unwrap(),
            MarkableRef::Commit(commit) => marks.insert_mark(commit.to_owned()).unwrap(),
            MarkableRef::CommittedFile(file) => marks.insert_mark(file.to_owned()).unwrap(),
            MarkableRef::Branch(branch) => marks.insert_mark(branch.to_owned()).unwrap(),
        }
    }
    marks
}

#[test]
fn select_resolved_target_selects_committed_file() {
    let commit_id = commit_id("0123456789012345678901234567890123456789").commit_id;
    let committed_file = CommittedFileId {
        commit_id,
        path: "file.txt".into(),
        change_id: None,
    };
    let lines = vec![line(StatusOutputLineData::File {
        cli_id: committed_file_cli_id(
            "0123456789012345678901234567890123456789",
            "file.txt",
            "a:b",
        ),
    })];

    let cursor =
        Cursor::select_resolved_target(ResolvedCliIdArg::CommittedFile(committed_file), &lines)
            .expect("committed file target should be supported");

    assert_eq!(
        cursor,
        Some(Cursor(0)),
        "the committed file row is selected"
    );
}

#[test]
fn select_resolved_target_selects_parent_file_for_hunk() {
    let first_hunk = IdAndHunk {
        id: "fi:a".into(),
        hunk: hunk("file.txt", 1),
    };
    let second_hunk = IdAndHunk {
        id: "fi:b".into(),
        hunk: hunk("file.txt", 20),
    };
    let file = UncommittedHunkOrFile {
        id: "fi".into(),
        hunks: NonEmpty {
            head: first_hunk,
            tail: vec![second_hunk.clone()],
        },
        is_entire_file: true,
    };
    let selected_hunk = UncommittedHunkOrFile {
        id: second_hunk.id.clone(),
        hunks: NonEmpty::new(second_hunk),
        is_entire_file: false,
    };
    let lines = vec![
        uncommitted_file_line("other.txt", "ot"),
        line(StatusOutputLineData::UncommittedFile {
            cli_id: Arc::new(CliId::UncommittedHunkOrFile(file)),
        }),
    ];

    let cursor = Cursor::select_resolved_target(
        ResolvedCliIdArg::UncommittedHunkOrFile(Box::new(selected_hunk)),
        &lines,
    )
    .expect("individual hunk target should be supported");

    assert_eq!(
        cursor,
        Some(Cursor(1)),
        "the row containing the selected hunk is selected"
    );
}

fn markable(cli_id: &Arc<CliId>) -> MarkableRef<'_> {
    MarkableRef::try_from_cli_id(cli_id).expect("test cli ID should be markable")
}

fn commit_line(hex: &str, id: &str) -> StatusOutputLine {
    commit_line_with_classification(hex, id, CommitClassification::LocalOnly)
}

fn commit_line_with_classification(
    hex: &str,
    id: &str,
    classification: CommitClassification,
) -> StatusOutputLine {
    line(StatusOutputLineData::Commit {
        cli_id: commit_cli_id(hex, id),
        stack_id: None,
        classification,
    })
}

fn move_commit_mode(hex: &str) -> Mode {
    Mode::Move(MoveMode {
        source: Arc::new(MoveSource::Commit(commit_id(hex))),
        insert_side: InsertSide::Below,
    })
}

#[expect(dead_code)]
fn stack_cli_id(id: &str, stack_id: StackId) -> Arc<CliId> {
    Arc::new(CliId::Stack {
        id: id.into(),
        stack_id,
    })
}

#[test]
fn new_selects_first_selectable_line() {
    let lines = vec![
        line(StatusOutputLineData::Connector),
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::StagedFile {
            cli_id: uncommitted_area("s0"),
        }),
    ];

    assert_eq!(Cursor::new(&lines), Cursor(2));
}

#[test]
fn new_defaults_to_zero_when_no_line_is_selectable() {
    let lines = vec![
        line(StatusOutputLineData::Connector),
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::UpdateNotice),
    ];

    assert_eq!(Cursor::new(&lines), Cursor(0));
}

#[test]
fn restore_returns_matching_branch_after_short_ids_change() {
    let lines = vec![
        line(StatusOutputLineData::Connector),
        branch_line("main", "b"),
        branch_line("other", "b0"),
    ];

    let selected_cli_id = CliId::Branch(BranchId {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    });

    assert_eq!(
        Cursor::restore(&selected_cli_id, &lines),
        Some(Cursor(1)),
        "the stable branch name should win over matching the previous short ID"
    );
}

#[test]
fn restore_returns_matching_commit_by_change_id_after_object_id_changes() {
    let selected_cli_id =
        commit_cli_id_with_change_id("1111111111111111111111111111111111111111", "c0", 1);
    let lines = vec![line(StatusOutputLineData::Commit {
        cli_id: commit_cli_id_with_change_id("2222222222222222222222222222222222222222", "c1", 1),
        stack_id: None,
        classification: CommitClassification::LocalOnly,
    })];

    assert_eq!(
        Cursor::restore(&selected_cli_id, &lines),
        Some(Cursor(0)),
        "a stable change ID should preserve selection across a commit rewrite"
    );
}

#[test]
fn restore_does_not_match_commits_with_different_change_ids() {
    let object_id = "1111111111111111111111111111111111111111";
    let selected_cli_id = commit_cli_id_with_change_id(object_id, "c0", 1);
    let lines = vec![line(StatusOutputLineData::Commit {
        cli_id: commit_cli_id_with_change_id(object_id, "c1", 2),
        stack_id: None,
        classification: CommitClassification::LocalOnly,
    })];

    assert_eq!(
        Cursor::restore(&selected_cli_id, &lines),
        None,
        "change IDs should take precedence when both commits provide one"
    );
}

#[test]
fn restore_falls_back_to_commit_id_when_a_change_id_is_missing() {
    let object_id = "1111111111111111111111111111111111111111";
    let selected_cli_id = commit_cli_id_with_change_id(object_id, "c0", 1);
    let lines = vec![line(StatusOutputLineData::Commit {
        cli_id: commit_cli_id(object_id, "c1"),
        stack_id: None,
        classification: CommitClassification::LocalOnly,
    })];

    assert_eq!(
        Cursor::restore(&selected_cli_id, &lines),
        Some(Cursor(0)),
        "the object ID should restore selection when either change ID is unavailable"
    );
}

#[test]
fn restore_returns_matching_uncommitted_file_after_short_ids_change() {
    let lines = vec![
        uncommitted_file_line("other.txt", "f0"),
        uncommitted_file_line("wanted.txt", "f"),
    ];
    let selected_cli_id = uncommitted_cli_id("wanted.txt", "f0");

    assert_eq!(
        Cursor::restore(&selected_cli_id, &lines),
        Some(Cursor(1)),
        "the stable file identity should win over matching the previous short ID"
    );
}

#[test]
fn restore_returns_matching_uncommitted_file_after_its_hunks_change() {
    let lines = vec![uncommitted_file_line("wanted.txt", "f")];
    let selected_cli_id = uncommitted_cli_id_with_old_start("wanted.txt", "f0", 2);

    assert_eq!(
        Cursor::restore(&selected_cli_id, &lines),
        Some(Cursor(0)),
        "the file path should identify a whole file as its hunks change"
    );
}

#[test]
fn restore_returns_none_when_cli_id_is_not_present() {
    let lines = vec![line(StatusOutputLineData::UncommittedChanges {
        cli_id: uncommitted_area("u0"),
    })];

    assert_eq!(
        Cursor::restore(
            &CliId::Branch(BranchId {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            }),
            &lines
        ),
        None
    );
}

#[test]
fn restore_selects_first_matching_line_when_cli_id_appears_multiple_times() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: uncommitted_area("u0"),
        }),
    ];

    assert_eq!(
        Cursor::restore(&CliId::Uncommitted { id: "u0".into() }, &lines),
        Some(Cursor(0))
    );
}

#[test]
fn select_finds_commit_line_by_object_id() {
    let wanted = "1111111111111111111111111111111111111111";
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(wanted, "c1"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    assert_eq!(
        Cursor::select_commit(commit_id(wanted).commit_id, &lines),
        Some(Cursor(1))
    );
}

#[test]
fn select_returns_none_when_commit_is_missing() {
    let lines = vec![line(StatusOutputLineData::Commit {
        cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c1"),
        stack_id: None,
        classification: CommitClassification::LocalOnly,
    })];

    assert_eq!(
        Cursor::select_commit(
            commit_id("2222222222222222222222222222222222222222").commit_id,
            &lines
        ),
        None
    );
}

#[test]
fn select_uses_first_matching_commit_when_object_id_appears_multiple_times() {
    let wanted = "1111111111111111111111111111111111111111";
    let lines = vec![
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(wanted, "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(wanted, "c1"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    assert_eq!(
        Cursor::select_commit(commit_id(wanted).commit_id, &lines),
        Some(Cursor(0))
    );
}

#[test]
fn select_after_discarded_commit_selects_commit_below_when_on_top_commit() {
    let top = "1111111111111111111111111111111111111111";
    let below = "2222222222222222222222222222222222222222";
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("main", "b0", None),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(top, "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(below, "c1"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    assert!(matches!(
        Cursor(1).select_after_discarded_commit(&lines),
        Some(SelectAfterReload::Commit(target_commit_id)) if target_commit_id == commit_id(below).commit_id
    ));
}

#[test]
fn select_after_discarded_commit_selects_commit_above_when_on_bottom_commit() {
    let above = "1111111111111111111111111111111111111111";
    let bottom = "2222222222222222222222222222222222222222";
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("main", "b0", None),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(above, "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(bottom, "c1"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    assert!(matches!(
        Cursor(2).select_after_discarded_commit(&lines),
        Some(SelectAfterReload::Commit(target_commit_id)) if target_commit_id == commit_id(above).commit_id
    ));
}

#[test]
fn select_after_discarded_commit_selects_branch_when_commit_is_only_one_in_branch() {
    let only = "1111111111111111111111111111111111111111";
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("main", "b0", None),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(only, "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    assert!(matches!(
        Cursor(1).select_after_discarded_commit(&lines),
        Some(SelectAfterReload::CliId(cli_id))
            if matches!(&*cli_id, CliId::Branch(BranchId { id, .. }) if id == "b0")
    ));
}

#[test]
fn select_after_discarded_commit_selects_commit_below_when_on_middle_commit() {
    let above = "1111111111111111111111111111111111111111";
    let middle = "2222222222222222222222222222222222222222";
    let below = "3333333333333333333333333333333333333333";
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("main", "b0", None),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(above, "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(middle, "c1"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(below, "c2"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    assert!(matches!(
        Cursor(2).select_after_discarded_commit(&lines),
        Some(SelectAfterReload::Commit(target_commit_id)) if target_commit_id == commit_id(below).commit_id
    ));
}

#[test]
fn select_after_discarded_commits_keeps_unmarked_current_commit_selected() {
    let marked = "1111111111111111111111111111111111111111";
    let current = "2222222222222222222222222222222222222222";
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("main", "b0", None),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(marked, "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(current, "c1"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    assert!(matches!(
        Cursor(2).select_after_discarded_commits(&lines, &[commit_id(marked).commit_id]),
        Some(SelectAfterReload::Commit(target_commit_id)) if target_commit_id == commit_id(current).commit_id
    ));
}

#[test]
fn select_after_discarded_commits_skips_marked_commit_below() {
    let top = "1111111111111111111111111111111111111111";
    let marked = "2222222222222222222222222222222222222222";
    let below = "3333333333333333333333333333333333333333";
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("main", "b0", None),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(top, "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(marked, "c1"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(below, "c2"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    assert!(matches!(
        Cursor(1).select_after_discarded_commits(
            &lines,
            &[commit_id(top).commit_id, commit_id(marked).commit_id],
        ),
        Some(SelectAfterReload::Commit(target_commit_id)) if target_commit_id == commit_id(below).commit_id
    ));
}

#[test]
fn select_after_discarded_commits_selects_commit_above_when_no_unmarked_commit_below() {
    let above = "1111111111111111111111111111111111111111";
    let marked = "2222222222222222222222222222222222222222";
    let bottom = "3333333333333333333333333333333333333333";
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("main", "b0", None),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(above, "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(marked, "c1"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(bottom, "c2"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    assert!(matches!(
        Cursor(2).select_after_discarded_commits(
            &lines,
            &[commit_id(marked).commit_id, commit_id(bottom).commit_id],
        ),
        Some(SelectAfterReload::Commit(target_commit_id)) if target_commit_id == commit_id(above).commit_id
    ));
}

#[test]
fn select_after_discarded_commits_selects_branch_when_all_commits_in_section_are_discarded() {
    let top = "1111111111111111111111111111111111111111";
    let bottom = "2222222222222222222222222222222222222222";
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("main", "b0", None),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(top, "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(bottom, "c1"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    assert!(matches!(
        Cursor(1).select_after_discarded_commits(
            &lines,
            &[commit_id(top).commit_id, commit_id(bottom).commit_id],
        ),
        Some(SelectAfterReload::CliId(cli_id))
            if matches!(&*cli_id, CliId::Branch(BranchId { id, .. }) if id == "b0")
    ));
}

#[test]
fn select_after_discarded_marks_keeps_unmarked_current_commit_selected() {
    let marked = "1111111111111111111111111111111111111111";
    let current = "2222222222222222222222222222222222222222";
    let lines = vec![
        branch_line("main", "b0"),
        commit_line(marked, "c0"),
        commit_line(current, "c1"),
    ];
    let discarded_marks = marks([markable(lines[1].data.cli_id().unwrap())]);

    assert!(matches!(
        Cursor(2).select_after_discarded_marks(&lines, &discarded_marks),
        Some(SelectAfterReload::Commit(target_commit_id)) if target_commit_id == commit_id(current).commit_id
    ));
}

#[test]
fn select_after_discarded_marks_keeps_unmarked_current_uncommitted_selected() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        uncommitted_file_line("marked.txt", "f0"),
        uncommitted_file_line("current.txt", "f1"),
    ];
    let discarded_marks = marks([markable(lines[1].data.cli_id().unwrap())]);
    let current_cli_id = lines[2].data.cli_id().unwrap();

    assert!(matches!(
        Cursor(2).select_after_discarded_marks(&lines, &discarded_marks),
        Some(SelectAfterReload::CliId(cli_id)) if cli_id.as_ref() == &**current_cli_id
    ));
}

#[test]
fn select_after_discarded_marks_selects_unmarked_uncommitted_below_marked_top_uncommitted() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        uncommitted_file_line("marked.txt", "f0"),
        uncommitted_file_line("below.txt", "f1"),
    ];
    let discarded_marks = marks([markable(lines[1].data.cli_id().unwrap())]);
    let below_cli_id = lines[2].data.cli_id().unwrap();

    assert!(matches!(
        Cursor(1).select_after_discarded_marks(&lines, &discarded_marks),
        Some(SelectAfterReload::CliId(cli_id)) if cli_id.as_ref() == &**below_cli_id
    ));
}

#[test]
fn select_after_discarded_marks_selects_unmarked_uncommitted_below_marked_middle_uncommitted() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        uncommitted_file_line("above.txt", "f0"),
        uncommitted_file_line("marked.txt", "f1"),
        uncommitted_file_line("below.txt", "f2"),
    ];
    let discarded_marks = marks([markable(lines[2].data.cli_id().unwrap())]);
    let below_cli_id = lines[3].data.cli_id().unwrap();

    assert!(matches!(
        Cursor(2).select_after_discarded_marks(&lines, &discarded_marks),
        Some(SelectAfterReload::CliId(cli_id)) if cli_id.as_ref() == &**below_cli_id
    ));
}

#[test]
fn select_after_discarded_marks_selects_unmarked_uncommitted_above_marked_bottom_uncommitted() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        uncommitted_file_line("above.txt", "f0"),
        uncommitted_file_line("marked.txt", "f1"),
    ];
    let discarded_marks = marks([markable(lines[2].data.cli_id().unwrap())]);
    let above_cli_id = lines[1].data.cli_id().unwrap();

    assert!(matches!(
        Cursor(2).select_after_discarded_marks(&lines, &discarded_marks),
        Some(SelectAfterReload::CliId(cli_id)) if cli_id.as_ref() == &**above_cli_id
    ));
}

#[test]
fn select_after_discarded_marks_selects_header_above_marked_uncommitted() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        uncommitted_file_line("marked.txt", "f0"),
    ];
    let discarded_marks = marks([markable(lines[1].data.cli_id().unwrap())]);
    let header_cli_id = lines[0].data.cli_id().unwrap();

    assert!(matches!(
        Cursor(1).select_after_discarded_marks(&lines, &discarded_marks),
        Some(SelectAfterReload::CliId(cli_id)) if cli_id.as_ref() == &**header_cli_id
    ));
}

#[test]
fn select_after_discarded_marks_selects_commit_below_marked_commit() {
    let marked = "1111111111111111111111111111111111111111";
    let below = "2222222222222222222222222222222222222222";
    let lines = vec![
        branch_line("main", "b0"),
        commit_line(marked, "c0"),
        commit_line(below, "c1"),
    ];
    let discarded_marks = marks([markable(lines[1].data.cli_id().unwrap())]);

    assert!(matches!(
        Cursor(1).select_after_discarded_marks(&lines, &discarded_marks),
        Some(SelectAfterReload::Commit(target_commit_id)) if target_commit_id == commit_id(below).commit_id
    ));
}

#[test]
fn select_after_discarded_branch_selects_branch_below_when_available() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("one", "b0", None),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("two", "b1", None),
            is_merged_upstream: false,
        }),
    ];

    assert!(matches!(
        Cursor(0).select_after_discarded_branch(&lines),
        Some(SelectAfterReload::Branch(name)) if name == "two"
    ));
}

#[test]
fn select_after_discarded_branch_selects_branch_above_when_no_branch_below() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("one", "b0", None),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("two", "b1", None),
            is_merged_upstream: false,
        }),
    ];

    assert!(matches!(
        Cursor(1).select_after_discarded_branch(&lines),
        Some(SelectAfterReload::Branch(name)) if name == "one"
    ));
}

#[test]
fn select_after_discarded_branch_selects_uncommitted_when_it_is_the_only_branch() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("one", "b0", None),
            is_merged_upstream: false,
        }),
    ];

    assert!(matches!(
        Cursor(1).select_after_discarded_branch(&lines),
        Some(SelectAfterReload::Uncommitted)
    ));
}

#[test]
fn select_after_discarded_branch_returns_none_if_selection_is_not_a_branch() {
    let lines = vec![line(StatusOutputLineData::Commit {
        cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
        stack_id: None,
        classification: CommitClassification::LocalOnly,
    })];

    assert!(Cursor(0).select_after_discarded_branch(&lines).is_none());
}

#[test]
fn select_closest_commit_source_selects_current_line_when_it_is_source() {
    let source_cli_id = uncommitted_cli_id("source.txt", "u0");
    let source = uncommitted_source(&[Arc::clone(&source_cli_id)]);
    let lines = vec![
        uncommitted_file_line("other.txt", "u1"),
        line(StatusOutputLineData::UncommittedFile {
            cli_id: source_cli_id,
        }),
        line(StatusOutputLineData::Connector),
    ];

    assert_eq!(
        Cursor(1).select_closest_commit_source(&lines, &source),
        Some(Cursor(1))
    );
}

#[test]
fn select_closest_commit_source_selects_nearest_source_when_current_line_is_not_source() {
    let farther_source_cli_id = uncommitted_cli_id("farther.txt", "u0");
    let nearest_source_cli_id = uncommitted_cli_id("nearest.txt", "u1");
    let source = uncommitted_source(&[
        Arc::clone(&farther_source_cli_id),
        Arc::clone(&nearest_source_cli_id),
    ]);
    let lines = vec![
        line(StatusOutputLineData::UncommittedFile {
            cli_id: farther_source_cli_id,
        }),
        uncommitted_file_line("other.txt", "u2"),
        line(StatusOutputLineData::Connector),
        line(StatusOutputLineData::UncommittedFile {
            cli_id: nearest_source_cli_id,
        }),
    ];

    assert_eq!(
        Cursor(2).select_closest_commit_source(&lines, &source),
        Some(Cursor(3))
    );
}

#[test]
fn select_closest_commit_source_prefers_source_above_on_tie() {
    let above_source_cli_id = uncommitted_cli_id("above.txt", "u0");
    let below_source_cli_id = uncommitted_cli_id("below.txt", "u1");
    let source = uncommitted_source(&[
        Arc::clone(&above_source_cli_id),
        Arc::clone(&below_source_cli_id),
    ]);
    let lines = vec![
        line(StatusOutputLineData::UncommittedFile {
            cli_id: above_source_cli_id,
        }),
        line(StatusOutputLineData::Connector),
        line(StatusOutputLineData::UncommittedFile {
            cli_id: below_source_cli_id,
        }),
    ];

    assert_eq!(
        Cursor(1).select_closest_commit_source(&lines, &source),
        Some(Cursor(0))
    );
}

#[test]
fn select_first_file_in_commit_finds_first_file_for_matching_commit() {
    let wanted = "1111111111111111111111111111111111111111";
    let lines = vec![
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(wanted, "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::File {
            cli_id: committed_file_cli_id(wanted, "src/a.rs", "f0"),
        }),
    ];

    assert_eq!(
        Cursor::select_first_file_in_commit(commit_id(wanted).commit_id, &lines),
        Some(Cursor(1))
    );
}

#[test]
fn select_first_file_in_commit_returns_none_when_commit_file_is_missing() {
    let wanted = "1111111111111111111111111111111111111111";
    let lines = vec![line(StatusOutputLineData::File {
        cli_id: committed_file_cli_id("2222222222222222222222222222222222222222", "src/a.rs", "f0"),
    })];

    assert_eq!(
        Cursor::select_first_file_in_commit(commit_id(wanted).commit_id, &lines),
        None
    );
}

#[test]
fn select_first_file_in_commit_uses_first_matching_file_when_multiple_exist() {
    let wanted = "1111111111111111111111111111111111111111";
    let lines = vec![
        line(StatusOutputLineData::File {
            cli_id: committed_file_cli_id(wanted, "src/a.rs", "f0"),
        }),
        line(StatusOutputLineData::File {
            cli_id: committed_file_cli_id(wanted, "src/b.rs", "f1"),
        }),
    ];

    assert_eq!(
        Cursor::select_first_file_in_commit(commit_id(wanted).commit_id, &lines),
        Some(Cursor(0))
    );
}

#[test]
fn select_branch_finds_branch_line_by_name() {
    let lines = vec![
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
    ];

    assert_eq!(Cursor::select_branch("main", &lines), Some(Cursor(1)));
}

#[test]
fn select_branch_returns_none_when_branch_is_missing() {
    let lines = vec![line(StatusOutputLineData::Branch {
        cli_id: Arc::new(CliId::Branch(BranchId {
            name: "main".into(),
            id: "b0".into(),
            stack_id: None,
        })),
        is_merged_upstream: false,
    })];

    assert_eq!(Cursor::select_branch("feature", &lines), None);
}

#[test]
fn select_branch_uses_first_matching_line_when_branch_appears_multiple_times() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            })),
        }),
    ];

    assert_eq!(Cursor::select_branch("main", &lines), Some(Cursor(0)));
}

#[test]
fn select_uncommitted_finds_uncommitted_line() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
    ];

    assert_eq!(Cursor::select_uncommitted(&lines), Some(Cursor(1)));
}

#[test]
fn select_uncommitted_uses_first_matching_line() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: uncommitted_area("u0"),
        }),
    ];

    assert_eq!(Cursor::select_uncommitted(&lines), Some(Cursor(0)));
}

#[test]
fn select_uncommitted_returns_none_when_missing() {
    let lines = vec![line(StatusOutputLineData::Branch {
        cli_id: Arc::new(CliId::Branch(BranchId {
            name: "main".into(),
            id: "b0".into(),
            stack_id: None,
        })),
        is_merged_upstream: false,
    })];

    assert_eq!(Cursor::select_uncommitted(&lines), None);
}

#[test]
fn select_merge_base_finds_merge_base_line() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::MergeBase),
    ];

    assert_eq!(Cursor::select_merge_base(&lines), Some(Cursor(1)));
}

#[test]
fn select_merge_base_returns_none_when_missing() {
    let lines = vec![line(StatusOutputLineData::Branch {
        cli_id: Arc::new(CliId::Branch(BranchId {
            name: "main".into(),
            id: "b0".into(),
            stack_id: None,
        })),
        is_merged_upstream: false,
    })];

    assert_eq!(Cursor::select_merge_base(&lines), None);
}

#[test]
fn index_returns_the_selected_line_index() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: uncommitted_area("s0"),
        }),
        line(StatusOutputLineData::StagedFile {
            cli_id: uncommitted_area("f0"),
        }),
    ];

    let cursor = Cursor::new(&lines);
    assert_eq!(cursor.index(), 0);
    assert_eq!(Cursor(1).index(), 1);
}

#[test]
fn selected_line_returns_none_when_cursor_out_of_bounds() {
    let lines = vec![line(StatusOutputLineData::UncommittedChanges {
        cli_id: uncommitted_area("u0"),
    })];

    assert!(Cursor(99).selected_line(&lines).is_none());
}

#[test]
fn selected_line_returns_line_when_cursor_is_in_bounds() {
    let lines = vec![
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
    ];

    assert!(matches!(
        Cursor(1).selected_line(&lines).map(|line| &line.data),
        Some(StatusOutputLineData::UncommittedChanges { .. })
    ));
}

#[test]
fn selection_cli_id_for_reload_uses_parent_when_file_is_selected_and_files_are_hidden() {
    let parent = Arc::new(CliId::Branch(BranchId {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    }));
    let lines = vec![
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::Branch {
            cli_id: parent.clone(),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::File {
            cli_id: uncommitted_area("file0"),
        }),
    ];

    assert_eq!(
        Cursor(2).selection_cli_id_for_reload(&lines, FilesStatusFlag::None),
        Some(&parent)
    );
}

#[test]
fn selection_cli_id_for_reload_uses_selected_file_when_files_are_shown() {
    let file_cli = uncommitted_area("file0");
    let lines = vec![line(StatusOutputLineData::File {
        cli_id: file_cli.clone(),
    })];

    assert_eq!(
        Cursor(0).selection_cli_id_for_reload(&lines, FilesStatusFlag::All),
        Some(&file_cli)
    );
}

#[test]
fn selection_cli_id_for_reload_returns_none_when_file_has_no_parent_section() {
    let lines = vec![line(StatusOutputLineData::File {
        cli_id: uncommitted_area("file0"),
    })];

    assert_eq!(
        Cursor(0).selection_cli_id_for_reload(&lines, FilesStatusFlag::None),
        None
    );
}

#[test]
fn selection_cli_id_for_reload_uses_selected_cli_id_for_non_file_lines() {
    let selected = Arc::new(CliId::Branch(BranchId {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    }));
    let lines = vec![line(StatusOutputLineData::Branch {
        cli_id: selected.clone(),
        is_merged_upstream: false,
    })];

    assert_eq!(
        Cursor(0).selection_cli_id_for_reload(&lines, FilesStatusFlag::None),
        Some(&selected)
    );
}

#[test]
fn selection_cli_id_for_reload_returns_none_when_cursor_is_out_of_bounds() {
    let lines = vec![line(StatusOutputLineData::Branch {
        cli_id: Arc::new(CliId::Branch(BranchId {
            name: "main".into(),
            id: "b0".into(),
            stack_id: None,
        })),
        is_merged_upstream: false,
    })];

    assert_eq!(
        Cursor(99).selection_cli_id_for_reload(&lines, FilesStatusFlag::None),
        None
    );
}

#[test]
fn selection_cli_id_for_reload_returns_none_for_non_file_lines_without_cli_id() {
    let lines = vec![line(StatusOutputLineData::Hint)];

    assert_eq!(
        Cursor(0).selection_cli_id_for_reload(&lines, FilesStatusFlag::None),
        None
    );
}

#[test]
fn selection_cli_id_for_reload_uses_nearest_parent_section_for_file() {
    let first_parent = Arc::new(CliId::Branch(BranchId {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    }));
    let nearest_parent = uncommitted_area("u0");
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: first_parent,
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::File {
            cli_id: uncommitted_area("file0"),
        }),
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: nearest_parent.clone(),
        }),
        line(StatusOutputLineData::File {
            cli_id: uncommitted_area("file1"),
        }),
    ];

    assert_eq!(
        Cursor(3).selection_cli_id_for_reload(&lines, FilesStatusFlag::None),
        Some(&nearest_parent)
    );
}

#[test]
fn selection_cli_id_for_reload_uses_commit_as_parent_for_hidden_file() {
    let parent_commit = commit_cli_id("1111111111111111111111111111111111111111", "c0");
    let lines = vec![
        line(StatusOutputLineData::Commit {
            cli_id: parent_commit.clone(),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::File {
            cli_id: uncommitted_area("file0"),
        }),
    ];

    assert_eq!(
        Cursor(1).selection_cli_id_for_reload(&lines, FilesStatusFlag::None),
        Some(&parent_commit)
    );
}

#[test]
fn selection_cli_id_for_reload_uses_staged_changes_as_parent_for_hidden_file() {
    let parent_staged = uncommitted_area("s0");
    let lines = vec![
        line(StatusOutputLineData::StagedChanges {
            cli_id: parent_staged.clone(),
        }),
        line(StatusOutputLineData::File {
            cli_id: uncommitted_area("file0"),
        }),
    ];

    assert_eq!(
        Cursor(1).selection_cli_id_for_reload(&lines, FilesStatusFlag::None),
        Some(&parent_staged)
    );
}

#[test]
fn move_up_moves_to_previous_selectable_line() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::StagedChanges {
            cli_id: uncommitted_area("s0"),
        }),
    ];

    let mut cursor = Cursor(2);
    if let Some(new_cursor) = cursor.move_up(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_up_does_not_move_when_already_at_first_selectable_line() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: uncommitted_area("s0"),
        }),
    ];

    let mut cursor = Cursor(0);
    if let Some(new_cursor) = cursor.move_up(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_down_moves_to_next_selectable_line() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::StagedChanges {
            cli_id: uncommitted_area("s0"),
        }),
    ];

    let mut cursor = Cursor(0);
    if let Some(new_cursor) = cursor.move_down(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(2));
}

#[test]
fn move_down_does_not_move_when_no_selectable_line_below() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::Hint),
    ];

    let mut cursor = Cursor(0);
    if let Some(new_cursor) = cursor.move_down(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_after_mark_moves_between_branches() {
    let lines = vec![
        branch_line("current", "b0"),
        commit_line("1111111111111111111111111111111111111111", "c0"),
        branch_line("next", "b1"),
        commit_line("2222222222222222222222222222222222222222", "c1"),
    ];
    let mode = Mode::Normal(NormalMode {
        marks: marks([markable(lines[0].data.cli_id().unwrap())]),
    });

    assert_eq!(
        Cursor(0).move_after_mark(&lines, &mode, FilesStatusFlag::All),
        Some(Cursor(2)),
        "marking a branch should move to the next branch"
    );
}

#[test]
fn move_up_within_section_stops_at_previous_section() {
    let lines = vec![
        branch_line("previous", "b0"),
        commit_line("1111111111111111111111111111111111111111", "c0"),
        branch_line("current", "b1"),
        commit_line("2222222222222222222222222222222222222222", "c1"),
        commit_line("3333333333333333333333333333333333333333", "c2"),
    ];
    let mode = Mode::Normal(NormalMode {
        marks: marks([markable(lines[4].data.cli_id().unwrap())]),
    });

    assert_eq!(
        Cursor(4).move_up_within_section(&lines, &mode, FilesStatusFlag::All),
        Some(Cursor(3)),
        "upward movement should find a selectable commit in the current branch"
    );
    assert_eq!(
        Cursor(3).move_up_within_section(&lines, &mode, FilesStatusFlag::All),
        None,
        "upward movement should not cross into the previous branch"
    );
}

#[test]
fn move_down_within_section_stops_at_next_section() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("main", "b0", None),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Branch {
            cli_id: branch_cli_id("other", "b1", None),
            is_merged_upstream: false,
        }),
    ];

    assert_eq!(
        Cursor(0).move_down_within_section(
            &lines,
            &Mode::Normal(NormalMode::default()),
            FilesStatusFlag::All
        ),
        Some(Cursor(2))
    );
    assert_eq!(
        Cursor(2).move_down_within_section(
            &lines,
            &Mode::Normal(NormalMode::default()),
            FilesStatusFlag::All
        ),
        None
    );
}

#[test]
fn movement_does_not_panic_or_move_when_cursor_is_out_of_bounds() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: uncommitted_area("s0"),
        }),
    ];

    let mut cursor = Cursor(99);
    if let Some(new_cursor) = cursor.move_up(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }
    if let Some(new_cursor) = cursor.move_down(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }
    if let Some(new_cursor) = cursor.move_next_section(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }
    if let Some(new_cursor) = cursor.move_previous_section(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(99));
}

#[test]
fn move_next_section_moves_to_next_jump_target() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "main".into(),
                id: "a0".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "other".into(),
                id: "a1".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("2222222222222222222222222222222222222222", "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    let mut cursor = Cursor(0);
    if let Some(new_cursor) = cursor.move_next_section(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(2));
}

#[test]
fn move_next_section_does_not_move_when_no_jump_target_below() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::UncommittedFile {
            cli_id: uncommitted_area("u1"),
        }),
    ];

    let mut cursor = Cursor(1);
    if let Some(new_cursor) = cursor.move_next_section(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(1));
}

#[test]
fn move_previous_section_moves_to_current_section_header_when_cursor_is_inside_it() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "main".into(),
                id: "a0".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "other".into(),
                id: "a1".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("2222222222222222222222222222222222222222", "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
    ];

    let mut cursor = Cursor(3);
    if let Some(new_cursor) = cursor.move_previous_section(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(2));
}

#[test]
fn move_previous_section_moves_to_immediate_previous_when_already_on_section_header() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::UncommittedFile {
            cli_id: uncommitted_area("u1"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: uncommitted_area("s0"),
        }),
    ];

    let mut cursor = Cursor(2);
    if let Some(new_cursor) = cursor.move_previous_section(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_previous_section_moves_to_current_header_when_only_current_section_exists_above_cursor() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::UncommittedFile {
            cli_id: uncommitted_area("u1"),
        }),
    ];

    let mut cursor = Cursor(1);
    if let Some(new_cursor) = cursor.move_previous_section(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_previous_section_does_not_move_when_on_first_jump_target() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::StagedFile {
            cli_id: uncommitted_area("s0"),
        }),
    ];

    let mut cursor = Cursor(0);
    if let Some(new_cursor) = cursor.move_previous_section(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_next_section_skips_non_jump_targets_like_commits() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "other".into(),
                id: "a0".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
    ];

    let mut cursor = Cursor(0);
    if let Some(new_cursor) = cursor.move_next_section(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(2));
}

#[test]
fn move_next_section_can_jump_to_merge_base_line() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::MergeBase),
    ];

    let mut cursor = Cursor(0);
    if let Some(new_cursor) = cursor.move_next_section(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(2));
}

#[test]
fn move_previous_section_can_jump_from_merge_base_line() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(BranchId {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            })),
            is_merged_upstream: false,
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        }),
        line(StatusOutputLineData::MergeBase),
    ];

    let mut cursor = Cursor(2);
    if let Some(new_cursor) = cursor.move_previous_section(
        &lines,
        &Mode::Normal(NormalMode::default()),
        FilesStatusFlag::All,
    ) {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn movement_methods_can_move_cursor_in_inline_reword_mode() {
    let lines = vec![
        line(StatusOutputLineData::UncommittedChanges {
            cli_id: uncommitted_area("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: uncommitted_area("s0"),
        }),
    ];

    let mut cursor = Cursor(1);
    let inline_reword = Mode::InlineReword(InlineRewordMode::Commit {
        commit_id: commit_id("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        textarea: Box::new(TextArea::default()),
    });

    // Inline reword keeps lines selectable to avoid dimming the whole UI.
    // Actual user navigation is blocked at the keybinding/event layer, not in these cursor helpers.
    if let Some(new_cursor) = cursor.move_up(&lines, &inline_reword, FilesStatusFlag::All) {
        cursor = new_cursor;
    }
    if let Some(new_cursor) = cursor.move_down(&lines, &inline_reword, FilesStatusFlag::All) {
        cursor = new_cursor;
    }
    if let Some(new_cursor) = cursor.move_next_section(&lines, &inline_reword, FilesStatusFlag::All)
    {
        cursor = new_cursor;
    }
    if let Some(new_cursor) =
        cursor.move_previous_section(&lines, &inline_reword, FilesStatusFlag::All)
    {
        cursor = new_cursor;
    }

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_down_from_move_source_selects_commit_below_source() {
    let source_hex = "2222222222222222222222222222222222222222";
    let lines = vec![
        branch_line("A", "b0"),
        commit_line("1111111111111111111111111111111111111111", "c0"),
        commit_line(source_hex, "c1"),
        commit_line("3333333333333333333333333333333333333333", "c2"),
    ];
    let mode = move_commit_mode(source_hex);

    assert_eq!(
        Cursor(2).move_down(&lines, &mode, FilesStatusFlag::All),
        Some(Cursor(3))
    );
}

#[test]
fn move_stack_skips_noop_target_above_source() {
    let stack_a = StackId::generate();
    let stack_b = StackId::generate();
    let stack_c = StackId::generate();
    let lines = vec![
        line(StatusOutputLineData::BetweenStacks),
        stack_branch_line("A", "b0", stack_a),
        line(StatusOutputLineData::BetweenStacks),
        stack_branch_line("B", "b1", stack_b),
        line(StatusOutputLineData::BetweenStacks),
        stack_branch_line("C", "b2", stack_c),
        line(StatusOutputLineData::BetweenStacks),
    ];
    let mode = Mode::MoveStack(MoveStackMode {
        source: ReorderStackSource {
            branch: BranchId {
                name: "B".into(),
                id: "b1".into(),
                stack_id: Some(stack_b),
            },
        },
    });

    assert_eq!(
        Cursor(3).move_up(&lines, &mode, FilesStatusFlag::All),
        Some(Cursor(0))
    );
}

#[test]
fn move_stack_skips_noop_target_below_source() {
    let stack_a = StackId::generate();
    let stack_b = StackId::generate();
    let stack_c = StackId::generate();
    let lines = vec![
        line(StatusOutputLineData::BetweenStacks),
        stack_branch_line("A", "b0", stack_a),
        line(StatusOutputLineData::BetweenStacks),
        stack_branch_line("B", "b1", stack_b),
        line(StatusOutputLineData::BetweenStacks),
        stack_branch_line("C", "b2", stack_c),
        line(StatusOutputLineData::BetweenStacks),
    ];
    let mode = Mode::MoveStack(MoveStackMode {
        source: ReorderStackSource {
            branch: BranchId {
                name: "B".into(),
                id: "b1".into(),
                stack_id: Some(stack_b),
            },
        },
    });

    assert_eq!(
        Cursor(3).move_down(&lines, &mode, FilesStatusFlag::All),
        Some(Cursor(6))
    );
}

#[test]
fn is_selectable_is_true_in_inline_reword_mode() {
    let selectable_line = line(StatusOutputLineData::StagedChanges {
        cli_id: uncommitted_area("s0"),
    });

    let inline_reword = Mode::InlineReword(InlineRewordMode::Commit {
        commit_id: commit_id("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        textarea: Box::new(TextArea::default()),
    });

    // Inline reword intentionally returns selectable so rows are not dimmed during editing.
    assert!(is_selectable_in_mode(
        &selectable_line,
        inline_reword.as_ref(),
        FilesStatusFlag::All
    ));
}

#[test]
fn is_selectable_in_commit_mode_scopes_commit_targets_to_stack() {
    let scoped_stack_id = StackId::single_branch_id();
    let mode = Mode::Commit(CommitMode {
        source: Arc::new(CommitSource::Uncommitted),
        insert_side: InsertSide::Above,
        scope_to_stack: Some(scoped_stack_id),
        message_composer: CommitMessageComposer::default(),
    });

    let same_stack_commit_line = line(StatusOutputLineData::Commit {
        cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
        stack_id: Some(scoped_stack_id),
        classification: CommitClassification::LocalOnly,
    });
    let other_stack_commit_line = line(StatusOutputLineData::Commit {
        cli_id: commit_cli_id("2222222222222222222222222222222222222222", "c1"),
        stack_id: None,
        classification: CommitClassification::LocalOnly,
    });

    assert!(is_selectable_in_mode(
        &same_stack_commit_line,
        mode.as_ref(),
        FilesStatusFlag::All
    ));
    assert!(!is_selectable_in_mode(
        &other_stack_commit_line,
        mode.as_ref(),
        FilesStatusFlag::All
    ));
}
