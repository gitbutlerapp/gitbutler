use std::sync::Arc;

use but_rebase::graph_rebase::mutate::InsertSide;
use gitbutler_stack::StackId;
use ratatui_textarea::TextArea;

use super::{Cursor, is_selectable_in_mode};
use crate::{
    CliId,
    command::legacy::status::{
        output::{StatusOutputContent, StatusOutputLine, StatusOutputLineData},
        tui::{CommitMode, CommitSource, InlineRewordMode, Mode, RubMode},
    },
};

fn line(data: StatusOutputLineData) -> StatusOutputLine {
    StatusOutputLine {
        connector: None,
        content: StatusOutputContent::Plain(Vec::new()),
        data,
    }
}

fn unassigned(id: &str) -> Arc<CliId> {
    Arc::new(CliId::Unassigned { id: id.into() })
}

fn commit_id(hex: &str) -> gix::ObjectId {
    gix::ObjectId::from_hex(hex.as_bytes()).unwrap()
}

fn commit_cli_id(hex: &str, id: &str) -> Arc<CliId> {
    Arc::new(CliId::Commit {
        commit_id: commit_id(hex),
        id: id.into(),
    })
}

#[test]
fn new_selects_first_selectable_line() {
    let lines = vec![
        line(StatusOutputLineData::Connector),
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::StagedFile {
            cli_id: unassigned("s0"),
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
fn restore_returns_matching_line_by_cli_id() {
    let lines = vec![
        line(StatusOutputLineData::Connector),
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            }),
        }),
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
    ];

    let selected_cli_id = CliId::Branch {
        name: "any-other-name".into(),
        id: "b0".into(),
        stack_id: None,
    };

    assert_eq!(Cursor::restore(&selected_cli_id, &lines), Some(Cursor(1)));
}

#[test]
fn restore_returns_none_when_cli_id_is_not_present() {
    let lines = vec![line(StatusOutputLineData::UnstagedChanges {
        cli_id: unassigned("u0"),
    })];

    assert_eq!(
        Cursor::restore(
            &CliId::Branch {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            },
            &lines
        ),
        None
    );
}

#[test]
fn restore_selects_first_matching_line_when_cli_id_appears_multiple_times() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("u0"),
        }),
    ];

    assert_eq!(
        Cursor::restore(&CliId::Unassigned { id: "u0".into() }, &lines),
        Some(Cursor(0))
    );
}

#[test]
fn select_finds_commit_line_by_object_id() {
    let wanted = "1111111111111111111111111111111111111111";
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            }),
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(wanted, "c1"),
            stack_id: None,
        }),
    ];

    assert_eq!(
        Cursor::select_commit(commit_id(wanted), &lines),
        Some(Cursor(1))
    );
}

#[test]
fn select_returns_none_when_commit_is_missing() {
    let lines = vec![line(StatusOutputLineData::Commit {
        cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c1"),
        stack_id: None,
    })];

    assert_eq!(
        Cursor::select_commit(
            commit_id("2222222222222222222222222222222222222222"),
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
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id(wanted, "c1"),
            stack_id: None,
        }),
    ];

    assert_eq!(
        Cursor::select_commit(commit_id(wanted), &lines),
        Some(Cursor(0))
    );
}

#[test]
fn select_branch_finds_branch_line_by_name() {
    let lines = vec![
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
        }),
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            }),
        }),
    ];

    assert_eq!(
        Cursor::select_branch("main".into(), &lines),
        Some(Cursor(1))
    );
}

#[test]
fn select_branch_returns_none_when_branch_is_missing() {
    let lines = vec![line(StatusOutputLineData::Branch {
        cli_id: Arc::new(CliId::Branch {
            name: "main".into(),
            id: "b0".into(),
            stack_id: None,
        }),
    })];

    assert_eq!(Cursor::select_branch("feature".into(), &lines), None);
}

#[test]
fn select_branch_uses_first_matching_line_when_branch_appears_multiple_times() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            }),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: Arc::new(CliId::Branch {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            }),
        }),
    ];

    assert_eq!(
        Cursor::select_branch("main".into(), &lines),
        Some(Cursor(0))
    );
}

#[test]
fn select_unassigned_finds_unassigned_line() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            }),
        }),
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
    ];

    assert_eq!(Cursor::select_unassigned(&lines), Some(Cursor(1)));
}

#[test]
fn select_unassigned_uses_first_matching_line() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("u0"),
        }),
    ];

    assert_eq!(Cursor::select_unassigned(&lines), Some(Cursor(0)));
}

#[test]
fn select_unassigned_returns_none_when_missing() {
    let lines = vec![line(StatusOutputLineData::Branch {
        cli_id: Arc::new(CliId::Branch {
            name: "main".into(),
            id: "b0".into(),
            stack_id: None,
        }),
    })];

    assert_eq!(Cursor::select_unassigned(&lines), None);
}

#[test]
fn iter_lines_marks_only_the_selected_line() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("s0"),
        }),
        line(StatusOutputLineData::StagedFile {
            cli_id: unassigned("f0"),
        }),
    ];

    let selected: Vec<bool> = Cursor(1)
        .iter_lines(&lines)
        .map(|(_, selected)| selected)
        .collect();

    assert_eq!(selected, vec![false, true, false]);
}

#[test]
fn selected_line_returns_none_when_cursor_out_of_bounds() {
    let lines = vec![line(StatusOutputLineData::UnstagedChanges {
        cli_id: unassigned("u0"),
    })];

    assert!(Cursor(99).selected_line(&lines).is_none());
}

#[test]
fn selected_line_returns_line_when_cursor_is_in_bounds() {
    let lines = vec![
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
    ];

    assert!(matches!(
        Cursor(1).selected_line(&lines).map(|line| &line.data),
        Some(StatusOutputLineData::UnstagedChanges { .. })
    ));
}

#[test]
fn selection_cli_id_for_reload_uses_parent_when_file_is_selected_and_files_are_hidden() {
    let parent = Arc::new(CliId::Branch {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    });
    let lines = vec![
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::Branch {
            cli_id: parent.clone(),
        }),
        line(StatusOutputLineData::File {
            cli_id: unassigned("file0"),
        }),
    ];

    assert_eq!(
        Cursor(2).selection_cli_id_for_reload(&lines, false),
        Some(&parent)
    );
}

#[test]
fn selection_cli_id_for_reload_uses_selected_file_when_files_are_shown() {
    let file_cli = unassigned("file0");
    let lines = vec![line(StatusOutputLineData::File {
        cli_id: file_cli.clone(),
    })];

    assert_eq!(
        Cursor(0).selection_cli_id_for_reload(&lines, true),
        Some(&file_cli)
    );
}

#[test]
fn selection_cli_id_for_reload_returns_none_when_file_has_no_parent_section() {
    let lines = vec![line(StatusOutputLineData::File {
        cli_id: unassigned("file0"),
    })];

    assert_eq!(Cursor(0).selection_cli_id_for_reload(&lines, false), None);
}

#[test]
fn selection_cli_id_for_reload_uses_selected_cli_id_for_non_file_lines() {
    let selected = Arc::new(CliId::Branch {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    });
    let lines = vec![line(StatusOutputLineData::Branch {
        cli_id: selected.clone(),
    })];

    assert_eq!(
        Cursor(0).selection_cli_id_for_reload(&lines, false),
        Some(&selected)
    );
}

#[test]
fn selection_cli_id_for_reload_returns_none_when_cursor_is_out_of_bounds() {
    let lines = vec![line(StatusOutputLineData::Branch {
        cli_id: Arc::new(CliId::Branch {
            name: "main".into(),
            id: "b0".into(),
            stack_id: None,
        }),
    })];

    assert_eq!(Cursor(99).selection_cli_id_for_reload(&lines, false), None);
}

#[test]
fn selection_cli_id_for_reload_returns_none_for_non_file_lines_without_cli_id() {
    let lines = vec![line(StatusOutputLineData::Hint)];

    assert_eq!(Cursor(0).selection_cli_id_for_reload(&lines, false), None);
}

#[test]
fn selection_cli_id_for_reload_uses_nearest_parent_section_for_file() {
    let first_parent = Arc::new(CliId::Branch {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    });
    let nearest_parent = unassigned("u0");
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: first_parent,
        }),
        line(StatusOutputLineData::File {
            cli_id: unassigned("file0"),
        }),
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: nearest_parent.clone(),
        }),
        line(StatusOutputLineData::File {
            cli_id: unassigned("file1"),
        }),
    ];

    assert_eq!(
        Cursor(3).selection_cli_id_for_reload(&lines, false),
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
        }),
        line(StatusOutputLineData::File {
            cli_id: unassigned("file0"),
        }),
    ];

    assert_eq!(
        Cursor(1).selection_cli_id_for_reload(&lines, false),
        Some(&parent_commit)
    );
}

#[test]
fn selection_cli_id_for_reload_uses_staged_changes_as_parent_for_hidden_file() {
    let parent_staged = unassigned("s0");
    let lines = vec![
        line(StatusOutputLineData::StagedChanges {
            cli_id: parent_staged.clone(),
        }),
        line(StatusOutputLineData::File {
            cli_id: unassigned("file0"),
        }),
    ];

    assert_eq!(
        Cursor(1).selection_cli_id_for_reload(&lines, false),
        Some(&parent_staged)
    );
}

#[test]
fn move_up_moves_to_previous_selectable_line() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("s0"),
        }),
    ];

    let mut cursor = Cursor(2);
    cursor.move_up(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_up_does_not_move_when_already_at_first_selectable_line() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("s0"),
        }),
    ];

    let mut cursor = Cursor(0);
    cursor.move_up(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_down_moves_to_next_selectable_line() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::Hint),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("s0"),
        }),
    ];

    let mut cursor = Cursor(0);
    cursor.move_down(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(2));
}

#[test]
fn move_down_does_not_move_when_no_selectable_line_below() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::Hint),
    ];

    let mut cursor = Cursor(0);
    cursor.move_down(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn movement_does_not_panic_or_move_when_cursor_is_out_of_bounds() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("s0"),
        }),
    ];

    let mut cursor = Cursor(99);
    cursor.move_up(&lines, &Mode::Normal);
    cursor.move_down(&lines, &Mode::Normal);
    cursor.move_next_section(&lines, &Mode::Normal);
    cursor.move_previous_section(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(99));
}

#[test]
fn move_next_section_moves_to_next_jump_target() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::UnstagedFile {
            cli_id: unassigned("u1"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("s0"),
        }),
    ];

    let mut cursor = Cursor(0);
    cursor.move_next_section(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(2));
}

#[test]
fn move_next_section_does_not_move_when_no_jump_target_below() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::UnstagedFile {
            cli_id: unassigned("u1"),
        }),
    ];

    let mut cursor = Cursor(1);
    cursor.move_next_section(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(1));
}

#[test]
fn move_previous_section_skips_current_section_when_cursor_is_inside_it() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::UnstagedFile {
            cli_id: unassigned("u1"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("s0"),
        }),
        line(StatusOutputLineData::StagedFile {
            cli_id: unassigned("s1"),
        }),
    ];

    let mut cursor = Cursor(3);
    cursor.move_previous_section(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_previous_section_moves_to_immediate_previous_when_already_on_section_header() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::UnstagedFile {
            cli_id: unassigned("u1"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("s0"),
        }),
    ];

    let mut cursor = Cursor(2);
    cursor.move_previous_section(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_previous_section_does_not_move_when_only_current_section_exists_above_cursor() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::UnstagedFile {
            cli_id: unassigned("u1"),
        }),
    ];

    let mut cursor = Cursor(1);
    cursor.move_previous_section(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(1));
}

#[test]
fn move_previous_section_does_not_move_when_on_first_jump_target() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::StagedFile {
            cli_id: unassigned("s0"),
        }),
    ];

    let mut cursor = Cursor(0);
    cursor.move_previous_section(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_up_in_rub_mode_skips_unavailable_targets() {
    let allowed = Arc::new(CliId::Branch {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    });
    let blocked = Arc::new(CliId::Branch {
        name: "feature".into(),
        id: "b1".into(),
        stack_id: None,
    });
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: allowed.clone(),
        }),
        line(StatusOutputLineData::StagedChanges { cli_id: blocked }),
        line(StatusOutputLineData::StagedFile {
            cli_id: allowed.clone(),
        }),
    ];
    let mode = Mode::Rub(RubMode {
        source: unassigned("source"),
        available_targets: vec![allowed],
    });

    let mut cursor = Cursor(2);
    cursor.move_up(&lines, &mode);

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_down_in_rub_mode_skips_unavailable_targets() {
    let allowed = Arc::new(CliId::Branch {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    });
    let blocked = Arc::new(CliId::Branch {
        name: "feature".into(),
        id: "b1".into(),
        stack_id: None,
    });
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: allowed.clone(),
        }),
        line(StatusOutputLineData::StagedChanges { cli_id: blocked }),
        line(StatusOutputLineData::StagedFile {
            cli_id: allowed.clone(),
        }),
    ];
    let mode = Mode::Rub(RubMode {
        source: unassigned("source"),
        available_targets: vec![allowed],
    });

    let mut cursor = Cursor(0);
    cursor.move_down(&lines, &mode);

    assert_eq!(cursor, Cursor(2));
}

#[test]
fn movement_in_rub_mode_handles_starting_on_unavailable_line() {
    let allowed_a = Arc::new(CliId::Branch {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    });
    let allowed_b = Arc::new(CliId::Branch {
        name: "release".into(),
        id: "b2".into(),
        stack_id: None,
    });
    let blocked = Arc::new(CliId::Branch {
        name: "feature".into(),
        id: "b1".into(),
        stack_id: None,
    });
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: allowed_a.clone(),
        }),
        line(StatusOutputLineData::StagedFile {
            cli_id: allowed_a.clone(),
        }),
        line(StatusOutputLineData::Branch {
            cli_id: blocked.clone(),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: allowed_b.clone(),
        }),
    ];
    let mode = Mode::Rub(RubMode {
        source: unassigned("source"),
        available_targets: vec![allowed_a, allowed_b],
    });

    let mut cursor = Cursor(2);
    cursor.move_up(&lines, &mode);
    assert_eq!(cursor, Cursor(1));

    let mut cursor = Cursor(2);
    cursor.move_down(&lines, &mode);
    assert_eq!(cursor, Cursor(3));

    let mut cursor = Cursor(2);
    cursor.move_next_section(&lines, &mode);
    assert_eq!(cursor, Cursor(3));
}

#[test]
fn move_next_section_skips_non_jump_targets_like_commits() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            }),
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("s0"),
        }),
    ];

    let mut cursor = Cursor(0);
    cursor.move_next_section(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(2));
}

#[test]
fn move_next_section_can_jump_to_merge_base_line() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            }),
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
        }),
        line(StatusOutputLineData::MergeBase),
    ];

    let mut cursor = Cursor(0);
    cursor.move_next_section(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(2));
}

#[test]
fn move_previous_section_can_jump_from_merge_base_line() {
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch {
                name: "main".into(),
                id: "b0".into(),
                stack_id: None,
            }),
        }),
        line(StatusOutputLineData::Commit {
            cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
            stack_id: None,
        }),
        line(StatusOutputLineData::MergeBase),
    ];

    let mut cursor = Cursor(2);
    cursor.move_previous_section(&lines, &Mode::Normal);

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_next_section_in_rub_mode_skips_unavailable_sections() {
    let allowed = Arc::new(CliId::Branch {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    });
    let blocked = Arc::new(CliId::Branch {
        name: "feature".into(),
        id: "b1".into(),
        stack_id: None,
    });
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: blocked.clone(),
        }),
        line(StatusOutputLineData::UnstagedChanges { cli_id: blocked }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: allowed.clone(),
        }),
    ];
    let mode = Mode::Rub(RubMode {
        source: unassigned("source"),
        available_targets: vec![allowed],
    });

    let mut cursor = Cursor(0);
    cursor.move_next_section(&lines, &mode);

    assert_eq!(cursor, Cursor(2));
}

#[test]
fn move_previous_section_in_rub_mode_skips_unavailable_sections() {
    let allowed = Arc::new(CliId::Branch {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    });
    let blocked = Arc::new(CliId::Branch {
        name: "feature".into(),
        id: "b1".into(),
        stack_id: None,
    });
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: allowed.clone(),
        }),
        line(StatusOutputLineData::StagedChanges { cli_id: blocked }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: allowed.clone(),
        }),
        line(StatusOutputLineData::StagedFile {
            cli_id: allowed.clone(),
        }),
    ];
    let mode = Mode::Rub(RubMode {
        source: unassigned("source"),
        available_targets: vec![allowed],
    });

    let mut cursor = Cursor(3);
    cursor.move_previous_section(&lines, &mode);

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn move_previous_section_in_rub_mode_from_unavailable_section_header_goes_to_previous_available_section()
 {
    let allowed_a = Arc::new(CliId::Branch {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    });
    let allowed_b = Arc::new(CliId::Branch {
        name: "release".into(),
        id: "b2".into(),
        stack_id: None,
    });
    let blocked = Arc::new(CliId::Branch {
        name: "feature".into(),
        id: "b1".into(),
        stack_id: None,
    });
    let lines = vec![
        line(StatusOutputLineData::Branch {
            cli_id: allowed_a.clone(),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: allowed_b.clone(),
        }),
        line(StatusOutputLineData::UnstagedChanges { cli_id: blocked }),
    ];
    let mode = Mode::Rub(RubMode {
        source: unassigned("source"),
        available_targets: vec![allowed_a, allowed_b],
    });

    let mut cursor = Cursor(2);
    cursor.move_previous_section(&lines, &mode);

    assert_eq!(cursor, Cursor(1));
}

#[test]
fn movement_methods_can_move_cursor_in_inline_reword_mode() {
    let lines = vec![
        line(StatusOutputLineData::UnstagedChanges {
            cli_id: unassigned("u0"),
        }),
        line(StatusOutputLineData::StagedChanges {
            cli_id: unassigned("s0"),
        }),
    ];

    let mut cursor = Cursor(1);
    let inline_reword = Mode::InlineReword(InlineRewordMode {
        commit_id: commit_id("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        textarea: Box::new(TextArea::default()),
    });

    // Inline reword keeps lines selectable to avoid dimming the whole UI.
    // Actual user navigation is blocked at the keybinding/event layer, not in these cursor helpers.
    cursor.move_up(&lines, &inline_reword);
    cursor.move_down(&lines, &inline_reword);
    cursor.move_next_section(&lines, &inline_reword);
    cursor.move_previous_section(&lines, &inline_reword);

    assert_eq!(cursor, Cursor(0));
}

#[test]
fn is_selectable_in_rub_mode_requires_available_target() {
    let allowed = Arc::new(CliId::Branch {
        name: "main".into(),
        id: "b0".into(),
        stack_id: None,
    });
    let blocked = Arc::new(CliId::Branch {
        name: "feature".into(),
        id: "b1".into(),
        stack_id: None,
    });
    let selectable_line = line(StatusOutputLineData::StagedFile {
        cli_id: allowed.clone(),
    });
    let blocked_line = line(StatusOutputLineData::UnstagedFile { cli_id: blocked });
    let not_selectable_line = line(StatusOutputLineData::Hint);

    let mode = Mode::Rub(RubMode {
        source: unassigned("source"),
        available_targets: vec![allowed],
    });

    assert!(is_selectable_in_mode(&selectable_line, &mode));
    assert!(!is_selectable_in_mode(&blocked_line, &mode));
    assert!(!is_selectable_in_mode(&not_selectable_line, &mode));
}

#[test]
fn is_selectable_is_true_in_inline_reword_mode() {
    let selectable_line = line(StatusOutputLineData::StagedChanges {
        cli_id: unassigned("s0"),
    });

    let inline_reword = Mode::InlineReword(InlineRewordMode {
        commit_id: commit_id("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        textarea: Box::new(TextArea::default()),
    });

    // Inline reword intentionally returns selectable so rows are not dimmed during editing.
    assert!(is_selectable_in_mode(&selectable_line, &inline_reword));
}

#[test]
fn is_selectable_in_commit_mode_scopes_commit_targets_to_stack() {
    let scoped_stack_id = StackId::single_branch_id();
    let mode = Mode::Commit(CommitMode {
        source: Arc::new(CommitSource::Unassigned { id: "zz".into() }),
        scope_to_stack: Some(scoped_stack_id),
        insert_side: InsertSide::Above,
    });

    let same_stack_commit_line = line(StatusOutputLineData::Commit {
        cli_id: commit_cli_id("1111111111111111111111111111111111111111", "c0"),
        stack_id: Some(scoped_stack_id),
    });
    let other_stack_commit_line = line(StatusOutputLineData::Commit {
        cli_id: commit_cli_id("2222222222222222222222222222222222222222", "c1"),
        stack_id: None,
    });

    assert!(is_selectable_in_mode(&same_stack_commit_line, &mode));
    assert!(!is_selectable_in_mode(&other_stack_commit_line, &mode));
}
