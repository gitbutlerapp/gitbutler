use std::fmt::Display;

use but_testsupport::Sandbox;
use crossterm::event::KeyCode;
use nonempty::{NonEmpty, nonempty};
use ratatui::{Terminal, backend::TestBackend};
use snapbox::file;

use crate::{
    args::OutputFormat,
    tui::{
        Clipboard,
        picker::{App, build_picker_items, initial_cursor},
        test_utils::{TestTui, configure_test_repo},
    },
    utils::OutputChannel,
};

struct TestTuiOptions<Key> {
    allow_multiple: bool,
    default_selected: &'static [usize],
    disabled: &'static [usize],
    help: fn(&Key) -> Option<&str>,
    prompt: &'static str,
}

impl<Key> Default for TestTuiOptions<Key> {
    fn default() -> Self {
        fn help<Key>(_: &Key) -> Option<&'static str> {
            None
        }

        Self {
            allow_multiple: false,
            default_selected: &[],
            disabled: &[],
            prompt: "Pick something",
            help: help::<Key>,
        }
    }
}

fn test_env() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env
}

fn test_tui<'a, Key, Value>(
    env: Sandbox,
    items: &'a NonEmpty<(Key, Value)>,
) -> TestTui<App<'a, Key, Value>>
where
    Key: Display,
{
    test_tui_with_options(env, items, Default::default())
}

fn test_tui_with_options<'a, Key, Value>(
    env: Sandbox,
    items: &'a NonEmpty<(Key, Value)>,
    options: TestTuiOptions<Key>,
) -> TestTui<App<'a, Key, Value>>
where
    Key: Display,
{
    let TestTuiOptions {
        allow_multiple,
        default_selected,
        disabled,
        help,
        prompt,
    } = options;

    configure_test_repo(&env);
    let ctx = env.context();

    let (_clipboard, clipboard_text) = Clipboard::test();

    let width = 100;
    let height = 20;

    let terminal =
        Terminal::new(TestBackend::new(width, height)).expect("failed to create test terminal");

    let out = OutputChannel::new(OutputFormat::Human { agent: false });

    let picker_items = build_picker_items(items, default_selected, disabled, help);
    let default_cursor = initial_cursor(allow_multiple, default_selected, picker_items.len());

    let app = App {
        should_render: true,
        should_quit: false,
        should_confirm: false,
        allow_multiple,
        prompt: prompt.to_owned(),
        cursor: default_cursor,
        items: picker_items,
    };

    TestTui::new(
        app,
        ctx,
        terminal,
        env,
        out,
        width,
        height,
        None,
        clipboard_text,
    )
}

#[test]
fn moving_cursor_up_and_down_single() {
    let items = nonempty![("one", ()), ("two", ()), ("three", ())];

    let mut tui = test_tui(test_env(), &items);

    tui.input(None)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_single_001.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_single_002.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_single_003.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_single_004.svg"]);
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_single_005.svg"]);
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_single_006.svg"]);
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_single_007.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_single_008.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_single_009.svg"]);
}

#[test]
fn moving_cursor_up_and_down_multi() {
    let items = nonempty![("one", ()), ("two", ()), ("three", ())];

    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            allow_multiple: true,
            ..Default::default()
        },
    );

    tui.input(None)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_001.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_002.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_003.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_004.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_005.svg"]);
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_006.svg"]);
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_007.svg"]);
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_008.svg"]);
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_009.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_010.svg"]);
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_011.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_cursor_up_and_down_multi_012.svg"]);
}

#[test]
fn help_is_a_pinned_footer_and_rows_do_not_reflow() {
    let items = nonempty![
        ("Codex", "codex"),
        ("Claude", "claude"),
        ("Cursor", "cursor"),
    ];
    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            allow_multiple: true,
            default_selected: &[0],
            help: |key: &&str| match *key {
                "Codex" => Some("Use Codex."),
                "Claude" => Some("Use Claude."),
                "Cursor" => Some("Use Cursor."),
                _ => unreachable!(),
            },
            prompt: "Pick one",
            ..Default::default()
        },
    );

    tui.input(None).assert_rendered_term_svg_eq(file![
        "snapshots/help_is_a_pinned_footer_and_rows_do_not_reflow_001.svg"
    ]);
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/help_is_a_pinned_footer_and_rows_do_not_reflow_002.svg"
    ]);
}

#[test]
fn no_footer_when_no_row_has_help() {
    let items = nonempty![("Codex", "codex"), ("Claude", "claude")];
    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            allow_multiple: true,
            prompt: "Pick one",
            ..Default::default()
        },
    );

    tui.input(None)
        .assert_rendered_term_svg_eq(file!["snapshots/no_footer_when_no_row_has_help_001.svg"]);
}

#[test]
fn single_select_cursor_starts_at_topmost_default_in_range() {
    let items = nonempty![
        ("one", "one value"),
        ("two", "two value"),
        ("three", "three value"),
        ("four", "four value"),
        ("five", "five value"),
    ];

    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            default_selected: &[2],
            ..Default::default()
        },
    );
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/single_select_cursor_starts_at_topmost_default_in_range_001.svg"
    ]);

    let mut tui = test_tui(test_env(), &items);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/single_select_cursor_starts_at_topmost_default_in_range_002.svg"
    ]);

    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            default_selected: &[9],
            ..Default::default()
        },
    );
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/single_select_cursor_starts_at_topmost_default_in_range_003.svg"
    ]);

    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            default_selected: &[3, 1],
            ..Default::default()
        },
    );
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/single_select_cursor_starts_at_topmost_default_in_range_004.svg"
    ]);

    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            default_selected: &[4, 9],
            ..Default::default()
        },
    );
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/single_select_cursor_starts_at_topmost_default_in_range_005.svg"
    ]);

    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            allow_multiple: true,
            default_selected: &[2],
            ..Default::default()
        },
    );
    tui.input(None).assert_rendered_term_svg_eq(file![
        "snapshots/single_select_cursor_starts_at_topmost_default_in_range_006.svg"
    ]);
}

#[test]
fn multi_select_marks_default_indices_selected() {
    let items = nonempty![("a", ()), ("b", ()), ("c", ()), ("d", ())];
    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            allow_multiple: true,
            default_selected: &[0, 2],
            ..Default::default()
        },
    );

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/multi_select_marks_default_indices_selected_001.svg"
    ]);
}

#[test]
fn build_picker_items_marks_disabled_indices() {
    let items = nonempty![("a", ()), ("b", ()), ("c", ())];
    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            allow_multiple: true,
            disabled: &[1],
            ..Default::default()
        },
    );

    tui.input(None).assert_rendered_term_svg_eq(file![
        "snapshots/build_picker_items_marks_disabled_indices_001.svg"
    ]);
}

#[test]
fn build_picker_items_never_selects_a_disabled_row() {
    let items = nonempty![("a", ()), ("b", ())];
    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            allow_multiple: true,
            default_selected: &[0, 1],
            disabled: &[1],
            ..Default::default()
        },
    );

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/build_picker_items_never_selects_a_disabled_row_001.svg"
    ]);
}

#[test]
fn single_select_does_not_confirm_on_a_disabled_row() {
    let items = nonempty![("Enabled", ()), ("Disabled", ())];
    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            default_selected: &[1],
            disabled: &[1],
            prompt: "Pick",
            ..Default::default()
        },
    );

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/single_select_does_not_confirm_on_a_disabled_row_001.svg"
    ]);
    tui.input('k');
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/single_select_does_not_confirm_on_a_disabled_row_002.svg"
    ]);
}

#[test]
fn multi_select_confirms_even_with_cursor_on_a_disabled_row() {
    let items = nonempty![("Enabled", ()), ("Disabled", ())];
    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            allow_multiple: true,
            default_selected: &[0],
            disabled: &[1],
            prompt: "Pick",
            ..Default::default()
        },
    );

    tui.input('j');
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/multi_select_confirms_even_with_cursor_on_a_disabled_row_001.svg"
    ]);
}

#[test]
fn disabled_row_renders_unavailable_marker_and_cannot_toggle() {
    let items = nonempty![("Enabled", ()), ("Disabled", ())];
    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            allow_multiple: true,
            disabled: &[1],
            prompt: "Pick",
            ..Default::default()
        },
    );

    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/disabled_row_renders_unavailable_marker_and_cannot_toggle_001.svg"
    ]);
    tui.input(' ');
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/disabled_row_renders_unavailable_marker_and_cannot_toggle_002.svg"
    ]);
}

#[test]
fn single_select_rows_have_no_checkbox() {
    let items = nonempty![("Apply", ()), ("Cancel", ())];
    let mut tui = test_tui_with_options(
        test_env(),
        &items,
        TestTuiOptions {
            help: |key: &&str| match *key {
                "Apply" => Some("Do it."),
                "Cancel" => Some("Stop."),
                _ => unreachable!(),
            },
            prompt: "Pick one",
            ..Default::default()
        },
    );

    tui.input(None).assert_rendered_term_svg_eq(file![
        "snapshots/single_select_rows_have_no_checkbox_001.svg"
    ]);
}
