use but_testsupport::Sandbox;
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use snapbox::{file, str};

use super::utils::{TestTuiOptions, test_status_tui, test_status_tui_with_options};
use crate::tui::test_utils::{Control, Shift};

fn click(column: u16, row: u16) -> Option<Event> {
    Some(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }))
}

#[test]
fn clicking_status_selects_without_scrolling() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            height: 8,
            ..Default::default()
        },
    );
    tui.reload();
    tui.input(click(15, 6))
        .assert_current_line_eq(str!["┊╭┄ h0 [B]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/clicking_status_selects_without_scrolling_001.svg"
        ]);
    // Connector-only rows and the hotbar aren't selectable.
    tui.input([click(15, 5), click(15, 7)])
        .assert_current_line_eq(str!["┊╭┄ h0 [B]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/clicking_status_selects_without_scrolling_001.svg"
        ]);
}

#[test]
fn clicking_bottom_operation_target_keeps_target_visible() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            height: 9,
            ..Default::default()
        },
    );
    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input('m');
    let result = tui.input(click(15, 7));
    assert_eq!(
        result.app().cursor.index(),
        7,
        "the clicked commit is selected"
    );
    assert_eq!(
        result.app().status_scroll.top(),
        1,
        "scroll only one row to fit the preview and target"
    );
    result.assert_rendered_term_svg_eq(file![
        "snapshots/clicking_bottom_operation_target_keeps_target_visible_001.svg"
    ]);
    // A redraw must retain the minimal scroll instead of restoring context margins.
    tui.input(None).assert_rendered_term_svg_eq(file![
        "snapshots/clicking_bottom_operation_target_keeps_target_visible_001.svg"
    ]);
}

#[test]
fn clicking_bottom_target_keeps_below_preview_visible() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            height: 8,
            ..Default::default()
        },
    );
    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input('m');
    let result = tui.input(click(15, 6));
    assert_eq!(
        result.app().cursor.index(),
        6,
        "the clicked branch is selected"
    );
    assert_eq!(
        result.app().status_scroll.top(),
        1,
        "scroll one row to fit the preview below the target"
    );
    result.assert_rendered_term_svg_eq(file![
        "snapshots/clicking_bottom_target_keeps_below_preview_visible_001.svg"
    ]);
    tui.input(None).assert_rendered_term_svg_eq(file![
        "snapshots/clicking_bottom_target_keeps_below_preview_visible_001.svg"
    ]);
}

#[test]
fn clicking_scrolled_status_uses_visible_rows() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            height: 11,
            ..Default::default()
        },
    );
    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down]);
    tui.input(Control('e'));
    tui.input(click(15, 1))
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/clicking_scrolled_status_uses_visible_rows_001.svg"
        ]);
}

#[test]
fn clicking_status_respects_mode_restrictions() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let mut tui = test_status_tui(env);
    tui.input(KeyCode::Down);
    tui.input('s');
    let selected = tui.input(None).app().cursor;
    // Commits aren't selectable in stack mode, but the other stack's header is.
    let result = tui.input(click(15, 7));
    assert_eq!(
        result.app().cursor,
        selected,
        "clicks cannot bypass mode restrictions"
    );
    tui.input(click(15, 6)).assert_rendered_term_svg_eq(file![
        "snapshots/clicking_status_respects_mode_restrictions_001.svg"
    ]);
}

#[test]
fn clicking_status_accounts_for_operation_preview_rows() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let mut tui = test_status_tui(env);
    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input('m');
    tui.input(click(15, 7)).assert_rendered_term_svg_eq(file![
        "snapshots/clicking_status_accounts_for_operation_preview_rows_001.svg"
    ]);
    // The inserted preview isn't a status item and must not select another row.
    let selected = tui.input(None).app().cursor;
    let result = tui.input(click(15, 7));
    assert_eq!(
        result.app().cursor,
        selected,
        "preview rows aren't click targets"
    );
    result.assert_rendered_term_svg_eq(file![
        "snapshots/clicking_status_accounts_for_operation_preview_rows_001.svg"
    ]);
    tui.input(click(15, 8)).assert_rendered_term_svg_eq(file![
        "snapshots/clicking_status_accounts_for_operation_preview_rows_001.svg"
    ]);
    tui.input('a').assert_rendered_term_svg_eq(file![
        "snapshots/clicking_status_accounts_for_operation_preview_rows_003.svg"
    ]);
    tui.input(click(15, 8)).assert_rendered_term_svg_eq(file![
        "snapshots/clicking_status_accounts_for_operation_preview_rows_003.svg"
    ]);
    let result = tui.input(click(15, 11));
    assert_eq!(
        result.app().cursor.index(),
        10,
        "the merge base is shifted by the preview row"
    );
    result.assert_rendered_term_svg_eq(file![
        "snapshots/clicking_status_accounts_for_operation_preview_rows_004.svg"
    ]);
    tui.input(click(15, 3)).assert_rendered_term_svg_eq(file![
        "snapshots/clicking_status_accounts_for_operation_preview_rows_002.svg"
    ]);
}

#[test]
fn clicks_in_details_do_not_select_status() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let mut tui = test_status_tui(env);
    tui.reload();
    tui.input('d');
    tui.input(click(90, 6));
    tui.input('d')
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/clicks_in_details_do_not_select_status_001.svg"
        ]);
    tui.input(Shift('d'));
    tui.input(click(15, 6));
    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/clicks_in_details_do_not_select_status_002.svg"
        ]);
}

#[test]
fn clicking_status_is_blocked_by_help() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let mut tui = test_status_tui(env);
    tui.input('?');
    tui.input(click(15, 6));
    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/clicking_status_is_blocked_by_help_001.svg"
        ]);
}
