use but_testsupport::Sandbox;
use crossterm::event::{Event, KeyModifiers, MouseEvent, MouseEventKind};
use snapbox::file;

use super::utils::{TestTuiOptions, test_status_tui, test_status_tui_with_options};
use crate::tui::test_utils::Control;

#[test]
fn scrolling_reveals_merge_base_in_single_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("single-branch-mode");
    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            height: 5,
            ..Default::default()
        },
    );
    tui.reload();
    let result = tui.input([Control('e'), Control('e'), Control('e'), Control('e')]);
    assert_eq!(
        result.app().status_scroll.top(),
        result.app().status_lines.len().saturating_sub(3),
        "scrolling reaches the final row above the hotbar and single-branch footer",
    );
    result.assert_rendered_term_svg_eq(file![
        "snapshots/scrolling_reveals_merge_base_in_single_branch_mode_001.svg"
    ]);
    tui.input([Control('y'), Control('y'), Control('y')]);
    let scroll_down = Some(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    }));
    tui.input([scroll_down.clone(), scroll_down.clone(), scroll_down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/scrolling_reveals_merge_base_in_single_branch_mode_001.svg"
        ]);
}

#[test]
fn shows_single_branch_mode_label_in_hot_bar() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("single-branch-mode");

    let mut tui = test_status_tui(env);

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/shows_single_branch_mode_label_in_hot_bar_001.svg"
    ]);
}
