use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::{
    command::legacy::status::tui::{BackstackEntry, tests::utils::test_status_tui},
    tui::test_utils::Shift,
};

#[test]
fn marking_individual_commit_toggles_mark_indicator() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input(' ')
        .assert_current_line_eq(str!["┊✔︎   tpm add A"]);

    tui.input(' ')
        .assert_current_line_eq(str!["┊●   tpm add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_individual_commit_toggles_mark_indicator_final.svg"
        ]);
}

#[test]
fn marking_uncommitted_toggles_all_uncommitted_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.file("a.txt", "content");
    env.file("b.txt", "content");

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ zz [uncommitted]"]);

    tui.input(' ')
        .assert_current_line_eq(str!["╭┄ zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊✔︎  nk A a.txt"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊✔︎  pn A b.txt"]);

    tui.input('g');
    tui.input(' ');

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   nk A a.txt"]);
}

#[test]
fn marking_middle_row_uses_neighbor_states() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.file("a.txt", "content");
    env.file("b.txt", "content");
    env.file("c.txt", "content");

    let mut tui = test_status_tui(env);

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down]);

    // Opposite to both neighbors: move down.
    tui.input(' ').assert_current_line_eq(str!["[..]c.txt"]);

    tui.input([KeyCode::Up, KeyCode::Up]);
    tui.input(' ').assert_current_line_eq(str!["[..]a.txt"]);
    tui.input(KeyCode::Down);

    // Same as the next neighbor: move to the previous one.
    tui.input(' ').assert_current_line_eq(str!["[..]a.txt"]);

    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input(' ').assert_current_line_eq(str!["[..]b.txt"]);

    // Same as both neighbors: stay put.
    tui.input(' ').assert_current_line_eq(str!["[..]b.txt"]);

    tui.input(KeyCode::Up);
    tui.input(' ').assert_current_line_eq(str!["[..]b.txt"]);

    // Same as the previous neighbor: move to the next one.
    tui.input(' ').assert_current_line_eq(str!["[..]c.txt"]);
}

#[test]
fn marking_section_edge_moves_only_when_neighbor_differs() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_status_tui(env);

    tui.input('b');
    for message in ["one", "two"] {
        tui.input('n');
        tui.input(KeyCode::Enter);
        tui.input(message);
        tui.input(KeyCode::Enter);
    }

    tui.input(' ').assert_current_line_eq(str!["[..]one[..]"]);
    tui.input(' ').assert_current_line_eq(str!["[..]one[..]"]);
    tui.input(' ').assert_current_line_eq(str!["[..]two[..]"]);
    tui.input(' ').assert_current_line_eq(str!["[..]two[..]"]);
}

#[test]
fn marks_still_show_in_split_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");

    let mut tui = test_status_tui(env);

    // mark some things
    tui.input('j');
    tui.input(' ')
        .assert_rendered_contains("┊✔︎  kl   A one")
        .assert_rendered_contains("┊   twop A two")
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file!["snapshots/marks_still_show_in_split_details_001.svg"]);

    // open details view and still see the marks
    tui.input('d')
        .assert_rendered_contains("+content of two")
        .assert_rendered_contains("┊✔︎  kl   A one")
        .assert_rendered_contains("┊   twop A two")
        .assert_backstack_eq([BackstackEntry::OpenSplitDetailsView, BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file!["snapshots/marks_still_show_in_split_details_002.svg"]);
    tui.input('d')
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file!["snapshots/marks_still_show_in_split_details_003.svg"]);

    // opening and focusing details should still show marks
    tui.input('l')
        .assert_rendered_contains("details")
        .assert_rendered_contains("+content of two")
        .assert_rendered_contains("┊✔︎  kl   A one")
        .assert_rendered_contains("┊   twop A two")
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenSplitDetailsView,
            BackstackEntry::Mark,
        ])
        .assert_rendered_term_svg_eq(file!["snapshots/marks_still_show_in_split_details_004.svg"]);

    // going back to normal mode should retain marks and keep details open
    tui.input('h')
        .assert_rendered_contains("normal")
        .assert_rendered_contains("+content of two")
        .assert_rendered_contains("┊✔︎  kl   A one")
        .assert_rendered_contains("┊   twop A two")
        .assert_backstack_eq([BackstackEntry::OpenSplitDetailsView, BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file!["snapshots/marks_still_show_in_split_details_005.svg"]);
}

#[test]
fn manual_reload_preserves_marks_when_split_details_visible() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('j');
    tui.input(' ')
        .assert_rendered_contains("┊✔︎  kl   A one")
        .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::OpenSplitDetailsView]);

    tui.input((KeyModifiers::CONTROL, 'r'))
        .assert_rendered_contains("┊✔︎  kl   A one")
        .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::OpenSplitDetailsView])
        .assert_rendered_term_svg_eq(file![
            "snapshots/manual_reload_preserves_marks_when_split_details_visible_001.svg"
        ]);
}

#[test]
fn can_only_mark_files_from_one_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");
    env.file("four", "");

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input(' ');
    tui.input(' ');

    tui.input('c');
    tui.input('e');
    tui.input('b');
    tui.input('g');
    tui.input(' ');
    tui.input('c');
    tui.input('e');
    tui.input('j');
    tui.input('j');
    tui.input(KeyCode::Enter);

    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/can_only_mark_files_from_one_commit_001.svg"
        ]);

    tui.input('j');
    tui.input(' ')
        .assert_current_line_eq(str!["┊│     1#0:t A two"])
        .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::ShowFileList])
        .assert_rendered_term_svg_eq(file![
            "snapshots/can_only_mark_files_from_one_commit_002.svg"
        ]);

    // we shouldn't be allowed to select lines outside the commit files
    for _ in 0..10 {
        tui.input('j')
            .assert_current_line_eq(str!["┊│     1#0:t A two"])
            .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::ShowFileList])
            .assert_rendered_term_svg_eq(file![
                "snapshots/can_only_mark_files_from_one_commit_003.svg"
            ]);
    }
    for _ in 0..10 {
        tui.input('k')
            .assert_current_line_eq(str!["┊✔︎     1#0:o A three"])
            .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::ShowFileList])
            .assert_rendered_term_svg_eq(file![
                "snapshots/can_only_mark_files_from_one_commit_004.svg"
            ]);
    }
}

#[test]
fn marking_commit_on_branch_with_one_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('j');

    // there is only one commit on the branch, so marking it should keep the cursor within the
    // branch
    tui.input('j')
        .assert_current_line_eq(str![["┊●   lrm add B"]]);
    tui.input(' ')
        .assert_current_line_eq(str![["┊✔︎   lrm add B"]]);
}

#[test]
fn marking_and_squashing_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");

    let mut tui = test_status_tui(env);

    for _ in 0..3 {
        tui.input('g');
        tui.input('j');
        tui.input('c');
        tui.input('e');
        tui.input('b');
    }
    tui.input('g');
    tui.input(Shift('f'));

    tui.input('j');
    tui.input(' ')
        .assert_current_line_eq(str!["┊╭┄ ra [c-branch-2]"])
        .assert_rendered_term_svg_eq(file!["snapshots/marking_and_squashing_branches_001.svg"]);
    tui.input(' ')
        .assert_current_line_eq(str!["┊╭┄ an [c-branch-1]"])
        .assert_rendered_term_svg_eq(file!["snapshots/marking_and_squashing_branches_002.svg"]);
    tui.input(' ')
        .assert_current_line_eq(str!["┊✔︎  an [c-branch-1]"])
        .assert_rendered_term_svg_eq(file!["snapshots/marking_and_squashing_branches_003.svg"]);

    tui.input('r');
    tui.input('u');
    tui.input('k');
    tui.input('k');
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/marking_and_squashing_branches_004.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/marking_and_squashing_branches_005.svg"]);
}

#[test]
fn fix_backstack_with_marks_in_squash_mode_from_commit_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input(' ');
    tui.input('r').assert_backstack_eq([
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::Mark,
        BackstackEntry::ShowFileList,
    ]);

    tui.input('k').assert_backstack_eq([
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::Mark,
        BackstackEntry::ShowFileList,
    ]);

    tui.input(KeyCode::Esc)
        .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::ShowFileList])
        .assert_current_line_eq(str![["┊✔︎     t:t A A"]]);
}

#[test]
fn marking_branches_shows_checkmark_in_the_right_place() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j').assert_current_line_eq(str![["┊╭┄ g0 [A]"]]);
    tui.input(' ').assert_current_line_eq(str![["┊✔︎  g0 [A]"]]);
}

#[test]
fn marks_cleared_when_closing_commit_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input(' ')
        .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::ShowFileList])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marks_cleared_when_closing_commit_file_list_001.svg"
        ]);
    tui.input('f')
        .assert_marks_count_eq(0)
        .assert_backstack_eq([])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marks_cleared_when_closing_commit_file_list_002.svg"
        ]);
}

#[test]
fn marks_cleared_when_closing_global_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input(Shift('f'));
    tui.input('j');
    tui.input(' ')
        .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::ShowFileList])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marks_cleared_when_closing_global_file_list_001.svg"
        ]);
    tui.input('f')
        .assert_marks_count_eq(0)
        .assert_backstack_eq([])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marks_cleared_when_closing_global_file_list_002.svg"
        ]);
}
