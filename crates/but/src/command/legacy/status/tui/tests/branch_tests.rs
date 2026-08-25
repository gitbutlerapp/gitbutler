use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::{
    command::legacy::status::tui::{
        Message, ReloadCause, SelectAfterReload, backstack::BackstackEntry,
        tests::utils::test_status_tui,
    },
    tui::test_utils::{Control, Shift},
};

#[test]
fn branch_key_from_uncommitted_creates_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ zz [uncommitted] (no changes)"]);

    tui.input('b');
    tui.input('n')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-1] (no commits)"]);
}

#[test]
fn branch_key_from_commit_is_noop() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   lrm add B"]);

    tui.input('b');
    tui.input('n')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-1] (no commits)"]);
}

#[test]
fn branch_key_from_branch_creates_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('b');
    tui.input('n')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-1] (no commits)"]);
}

#[test]
fn branch_key_keeps_global_file_list_open() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_contains("t:t A A");

    tui.input('b');
    tui.input('n')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-1] (no commits)"])
        .assert_rendered_contains("t:t A A");
}

#[test]
fn focus_reload_preserves_branch_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.render_with_messages(Some(Event::FocusGained), Vec::new())
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
}

#[test]
fn deleted_branch_name_can_be_reused_without_restoring_old_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('x').assert_rendered_contains("Discard branch A?");

    tui.input('y');

    tui.reload()
        .assert_current_line_eq(str!["╭┄ zz [uncommitted] (no changes)"]);

    tui.input('b');
    tui.input('n')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-1] (no commits)"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-1 ] (no commits)"]);

    for _ in 0..10 {
        tui.input(KeyCode::Backspace);
    }

    tui.input("A")
        .assert_current_line_eq(str!["┊╭┄ br [A ] (no commits)"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ g0 [A] (no commits)"]);

    let mut tui = tui.recreate();
    tui.reload().assert_rendered_contains("[A] (no commits)");
}

#[test]
fn focus_reload_preserves_merge_base_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);

    tui.render_with_messages(Some(Event::FocusGained), Vec::new())
        .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);
}

#[test]
fn inline_branch_reword_confirm_renames_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ g0 [A ]"]);

    tui.input(KeyCode::Backspace)
        .assert_current_line_eq(str!["┊╭┄ g0 [ ]"]);

    tui.input("new")
        .assert_current_line_eq(str!["┊╭┄ g0 [new ]"]);

    // spaces get mapped to dashes
    tui.input(" ")
        .assert_current_line_eq(str!["┊╭┄ g0 [new- ]"]);

    tui.input("name")
        .assert_current_line_eq(str!["┊╭┄ g0 [new-name ]"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ ne [new-name]"]);
}

#[test]
fn inline_branch_reword_esc_cancels() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ g0 [A ]"]);

    tui.input("new-name")
        .assert_current_line_eq(str!["┊╭┄ g0 [Anew-name ]"]);

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
}

#[test]
fn inline_branch_reword_preserves_selection_after_reload_with_multiple_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ g0 [A ]"]);

    tui.input(KeyCode::Backspace)
        .assert_current_line_eq(str!["┊╭┄ g0 [ ]"]);

    tui.input("renamed-a")
        .assert_current_line_eq(str!["┊╭┄ g0 [renamed-a ]"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ re [renamed-a]"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄ g0 [B]"]);
}

#[test]
fn inline_branch_reword_space_before_close_bracket() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');

    // when the insertion point is at the end show a space before `]`
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ g0 [A ]"]);

    // dont show a space when the cursor isn't at the end
    tui.input(KeyCode::Left)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
}

#[test]
fn cannot_select_merged_branches() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata(&["A", "B"]);
    env.set_target_sha("refs/heads/base");

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/cannot_select_merged_branches_001.svg"]);

    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/cannot_select_merged_branches_002.svg"]);
}

#[test]
fn reload_moves_selection_off_merged_branch() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata(&["A", "B"]);
    env.set_target_sha("refs/heads/base");

    let mut tui = test_status_tui(env);

    tui.render_with_messages(
        None,
        vec![Message::Reload(
            Some(SelectAfterReload::Branch("A".into())),
            ReloadCause::Mutation,
        )],
    )
    .assert_current_line_eq(str!["┊╭┄ h0 [B]"]);
}

#[test]
fn switch_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    // switch to a branch
    tui.input('b')
        .assert_rendered_term_svg_eq(file!["snapshots/switch_branch_001.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/switch_branch_002.svg"]);
    tui.input(Shift('s'))
        .assert_rendered_term_svg_eq(file!["snapshots/switch_branch_003.svg"]);

    // apply a different branch
    tui.input('s');
    tui.input('a')
        .assert_rendered_term_svg_eq(file!["snapshots/switch_branch_004.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/switch_branch_005.svg"]);

    // switch to the newly applied branch
    tui.input('b')
        .assert_rendered_term_svg_eq(file!["snapshots/switch_branch_006.svg"]);
    tui.input(Shift('s'))
        .assert_rendered_term_svg_eq(file!["snapshots/switch_branch_007.svg"]);
}

#[test]
fn create_new_branches_from_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_status_tui(env);

    tui.input('b');
    tui.input('n').assert_rendered_term_svg_eq(file![
        "snapshots/create_new_branches_from_branch_mode_001.svg"
    ]);

    tui.input('b');
    tui.input('n').assert_rendered_term_svg_eq(file![
        "snapshots/create_new_branches_from_branch_mode_002.svg"
    ]);

    tui.input('j');
    tui.input('b');
    tui.input('n').assert_rendered_term_svg_eq(file![
        "snapshots/create_new_branches_from_branch_mode_003.svg"
    ]);
}

#[test]
fn discard_branches_from_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('b');
    tui.input('x')
        .assert_rendered_term_svg_eq(file!["snapshots/discard_branches_from_branch_mode_001.svg"]);
    tui.input('y')
        .assert_rendered_term_svg_eq(file!["snapshots/discard_branches_from_branch_mode_002.svg"]);
}

#[test]
fn discard_marked_branches_from_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    // we can enter branch mode on `zz [uncommitted]` so ensure it isn't markable
    tui.input('b');
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/discard_marked_branches_from_branch_mode_001.svg"
    ]);

    tui.input('j');
    tui.input(' ');
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/discard_marked_branches_from_branch_mode_002.svg"
    ]);

    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/discard_marked_branches_from_branch_mode_003.svg"
    ]);
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/discard_marked_branches_from_branch_mode_004.svg"
    ]);
}

#[test]
fn marks_carry_from_normal_mode_to_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input(' ');
    tui.input(' ')
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marks_carry_from_normal_mode_to_branch_mode_001.svg"
        ]);
    tui.input('b')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marks_carry_from_normal_mode_to_branch_mode_002.svg"
        ]);

    tui.input(KeyCode::Esc)
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marks_carry_from_normal_mode_to_branch_mode_003.svg"
        ]);
}

#[test]
fn cannot_enter_branch_mode_with_non_branch_marks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/cannot_enter_branch_mode_with_non_branch_marks_001.svg"
    ]);
    tui.input('b').assert_rendered_term_svg_eq(file![
        "snapshots/cannot_enter_branch_mode_with_non_branch_marks_001.svg"
    ]);
}

#[test]
fn clearing_marks_from_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('b');
    tui.input('j');
    tui.input(' ');
    tui.input(' ')
        .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::LeaveNormalMode])
        .assert_rendered_term_svg_eq(file!["snapshots/clearing_marks_from_branch_mode_001.svg"]);
    tui.input(KeyCode::Esc)
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode])
        .assert_rendered_term_svg_eq(file!["snapshots/clearing_marks_from_branch_mode_002.svg"]);
    tui.input(KeyCode::Esc)
        .assert_backstack_eq([])
        .assert_rendered_term_svg_eq(file!["snapshots/clearing_marks_from_branch_mode_003.svg"]);
}

#[test]
fn create_and_switch_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('b');
    tui.input(Shift('n'))
        .assert_rendered_term_svg_eq(file!["snapshots/create_and_switch_to_branch_001.svg"]);

    snapbox::assert_data_eq!(
        tui.env().git_log(),
        snapbox::str![[r#"
*   cc54560 (gitbutler/workspace) GitButler Workspace Commit
|/  
| * 9477ae7 (A) add A
|/  
* 0dc3733 (HEAD -> c-branch-1, origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn create_and_switch_to_stacked_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('b');
    tui.input(Shift('n')).assert_rendered_term_svg_eq(file![
        "snapshots/create_and_switch_to_stacked_branch_001.svg"
    ]);

    snapbox::assert_data_eq!(
        tui.env().git_log(),
        snapbox::str![[r#"
*   c128bce (gitbutler/workspace) GitButler Workspace Commit
|/  
| * 9477ae7 (HEAD -> c-branch-1, A) add A
* | d3e2ba3 (B) add B
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn pick_and_switch_to_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('b');
    tui.input('s')
        .assert_rendered_term_svg_eq(file!["snapshots/pick_and_switch_to_branches_001.svg"]);
    tui.input(Control('n'))
        .assert_rendered_term_svg_eq(file!["snapshots/pick_and_switch_to_branches_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/pick_and_switch_to_branches_003.svg"]);
}

#[test]
fn pick_and_switch_to_stacked_branches() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-dependent-branches");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('b');
    tui.input('s').assert_rendered_term_svg_eq(file![
        "snapshots/pick_and_switch_to_stacked_branches_001.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/pick_and_switch_to_stacked_branches_002.svg"
    ]);

    tui.input('b');
    tui.input('s').assert_rendered_term_svg_eq(file![
        "snapshots/pick_and_switch_to_stacked_branches_003.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/pick_and_switch_to_stacked_branches_004.svg"
    ]);
}
