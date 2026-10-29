use but_testsupport::Sandbox;
use crossterm::event::{Event, KeyCode};
use nonempty::NonEmpty;
use ratatui::{Terminal, backend::TestBackend};
use snapbox::file;

use super::App;
use crate::{
    args::OutputFormat,
    command::legacy::switch::SwitchBranchItem,
    tui::{
        Clipboard,
        test_utils::{Control, TestTui, configure_test_repo},
    },
    utils::OutputChannel,
};

fn test_tui() -> TestTui<App> {
    // The harness requires a context, but the picker only uses the fixed items below.
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    configure_test_repo(&env);
    let ctx = env.context();
    let (_, clipboard_text) = Clipboard::test();
    let width = 50;
    let height = 4;
    let terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let items = NonEmpty {
        head: SwitchBranchItem::Workspace,
        tail: Vec::from([
            branch("feature-one", "1h ago"),
            branch("feature-two", "2h ago"),
            branch("main", "1d ago"),
        ]),
    };

    TestTui::new(
        App::new(items),
        ctx,
        terminal,
        env,
        OutputChannel::new(OutputFormat::Human { agent: false }),
        width,
        height,
        None,
        clipboard_text,
    )
}

fn branch(name: &str, updated_at_display: &str) -> SwitchBranchItem {
    SwitchBranchItem::Branch {
        name: name.into(),
        updated_at: None,
        updated_at_display: updated_at_display.into(),
    }
}

#[test]
fn resize_preserves_selection() {
    let mut tui = test_tui();
    tui.input("feat");
    tui.input(KeyCode::Down);
    let rendered = tui.input(Some(Event::Resize(50, 4)));
    assert!(
        matches!(rendered.app().state.selected_item(), Some(SwitchBranchItem::Branch { name, .. }) if name == "feature-two"),
        "events that do not change the query must preserve the selected match"
    );
    rendered.assert_rendered_term_svg_eq(file!["snapshots/resize_preserves_selection_001.svg"]);
}

#[test]
fn focus_preserves_selection() {
    let mut tui = test_tui();
    tui.input("feat");
    tui.input(KeyCode::Down);
    let rendered = tui.input([Some(Event::FocusLost), Some(Event::FocusGained)]);
    assert!(
        matches!(rendered.app().state.selected_item(), Some(SwitchBranchItem::Branch { name, .. }) if name == "feature-two"),
        "events that do not change the query must preserve the selected match"
    );
    rendered.assert_rendered_term_svg_eq(file!["snapshots/focus_preserves_selection_001.svg"]);
}

#[test]
fn query_cursor_movement_preserves_selection() {
    let mut tui = test_tui();
    tui.input("feat");
    tui.input(KeyCode::Down);
    let rendered = tui.input(KeyCode::Left);
    assert!(
        matches!(rendered.app().state.selected_item(), Some(SwitchBranchItem::Branch { name, .. }) if name == "feature-two"),
        "events that do not change the query must preserve the selected match"
    );
    rendered.assert_rendered_term_svg_eq(file![
        "snapshots/query_cursor_movement_preserves_selection_001.svg"
    ]);
}

#[test]
fn initial_selection_and_confirmation() {
    let mut tui = test_tui();
    let rendered = tui.input(None).assert_rendered_term_svg_eq(file![
        "snapshots/initial_selection_and_confirmation_001.svg"
    ]);
    assert!(
        matches!(
            rendered.app().state.selected_item(),
            Some(SwitchBranchItem::Workspace)
        ),
        "the first item is selected before any navigation"
    );

    let rendered = tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/initial_selection_and_confirmation_002.svg"
    ]);
    assert!(
        rendered.app().should_confirm,
        "Enter confirms the default selection"
    );
    assert!(
        !rendered.app().should_quit,
        "confirmation is not cancellation"
    );
}

#[test]
fn arrow_navigation_scrolls_and_clamps() {
    let mut tui = test_tui();
    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/arrow_navigation_scrolls_and_clamps_001.svg"
        ]);
    tui.input([KeyCode::Up, KeyCode::Up, KeyCode::Up, KeyCode::Up])
        .assert_rendered_term_svg_eq(file![
            "snapshots/arrow_navigation_scrolls_and_clamps_002.svg"
        ]);
}

#[test]
fn control_navigation_and_confirmation() {
    let mut tui = test_tui();
    tui.input([Control('n'), Control('n')])
        .assert_rendered_term_svg_eq(file![
            "snapshots/control_navigation_and_confirmation_001.svg"
        ]);
    tui.input(Control('p')).assert_rendered_term_svg_eq(file![
        "snapshots/control_navigation_and_confirmation_002.svg"
    ]);
    let rendered = tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/control_navigation_and_confirmation_003.svg"
    ]);
    assert!(
        rendered.app().should_confirm,
        "Enter confirms the navigated selection"
    );
    assert!(
        matches!(rendered.app().state.selected_item(), Some(SwitchBranchItem::Branch { name, .. }) if name == "feature-one"),
        "Ctrl-N and Ctrl-P navigate without entering search text"
    );
}

#[test]
fn fuzzy_search_and_confirmation() {
    let mut tui = test_tui();
    tui.input("ftw")
        .assert_rendered_term_svg_eq(file!["snapshots/fuzzy_search_and_confirmation_001.svg"]);
    let rendered = tui
        .input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/fuzzy_search_and_confirmation_002.svg"]);
    assert!(
        rendered.app().should_confirm,
        "Enter confirms a fuzzy match"
    );
    assert!(
        matches!(rendered.app().state.selected_item(), Some(SwitchBranchItem::Branch { name, .. }) if name == "feature-two"),
        "confirmation uses the filtered item, not its original row index"
    );
}

#[test]
fn no_matches_prevents_confirmation_and_can_recover() {
    let mut tui = test_tui();
    tui.input("zzz").assert_rendered_term_svg_eq(file![
        "snapshots/no_matches_prevents_confirmation_and_can_recover_001.svg"
    ]);
    let rendered = tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/no_matches_prevents_confirmation_and_can_recover_002.svg"
    ]);
    assert!(
        !rendered.app().should_confirm,
        "an empty result cannot be confirmed"
    );
    assert!(
        rendered.app().state.selected_item().is_none(),
        "there is no hidden selection"
    );

    tui.input([KeyCode::Backspace, KeyCode::Backspace, KeyCode::Backspace])
        .assert_rendered_term_svg_eq(file![
            "snapshots/no_matches_prevents_confirmation_and_can_recover_003.svg"
        ]);
}

#[test]
fn escape_cancels_and_clears() {
    let mut tui = test_tui();
    tui.input("feat")
        .assert_rendered_term_svg_eq(file!["snapshots/escape_cancels_and_clears_001.svg"]);
    let rendered = tui
        .input(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file!["snapshots/escape_cancels_and_clears_002.svg"]);
    assert!(rendered.app().should_quit, "Escape cancels the picker");
    assert!(
        !rendered.app().should_confirm,
        "cancellation does not confirm a selection"
    );
}

#[test]
fn control_c_cancels() {
    let mut tui = test_tui();
    tui.input(None);
    let rendered = tui
        .input(Control('c'))
        .assert_rendered_term_svg_eq(file!["snapshots/control_c_cancels_001.svg"]);
    assert!(rendered.app().should_quit, "Ctrl-C cancels the picker");
    assert!(
        !rendered.app().should_confirm,
        "Ctrl-C does not confirm a selection"
    );
}

#[test]
fn control_d_cancels() {
    let mut tui = test_tui();
    tui.input(None);
    let rendered = tui
        .input(Control('d'))
        .assert_rendered_term_svg_eq(file!["snapshots/control_d_cancels_001.svg"]);
    assert!(rendered.app().should_quit, "Ctrl-D cancels the picker");
    assert!(
        !rendered.app().should_confirm,
        "Ctrl-D does not confirm a selection"
    );
}
