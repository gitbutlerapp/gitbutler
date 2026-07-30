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
        picker::{App, PickerItem, build_picker_items, initial_cursor},
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

type Row = (&'static str, Option<&'static str>, bool);

/// Build a picker from `rows`, render its view lines, and flatten each line
/// to plain text (styling dropped) for assertions.
fn render_texts(allow_multiple: bool, cursor: usize, rows: &[Row]) -> Vec<String> {
    let keys: Vec<String> = rows.iter().map(|(key, _, _)| key.to_string()).collect();
    let items = rows
        .iter()
        .copied()
        .enumerate()
        .map(|(i, (_, help, selected))| PickerItem {
            key: &keys[i],
            help: help.map(str::to_owned),
            value: &(),
            selected,
            disabled: false,
        })
        .collect::<Vec<_>>();
    let app = App {
        should_render: true,
        should_quit: false,
        should_confirm: false,
        allow_multiple,
        prompt: "Pick one".to_string(),
        cursor,
        items: NonEmpty::from_vec(items).expect("non-empty rows"),
    };
    app.view_lines()
        .iter()
        .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

#[test]
fn help_is_a_pinned_footer_and_rows_do_not_reflow() {
    let rows: [Row; 3] = [
        ("Codex", Some("Use Codex."), true),
        ("Claude", Some("Use Claude."), false),
        ("Cursor", Some("Use Cursor."), false),
    ];

    let top = render_texts(true, 0, &rows);
    // prompt + 3 rows + blank separator + caption.
    assert_eq!(top.len(), 6);
    assert_eq!(top[0], "Pick one");
    assert!(top[1].contains("Codex"));
    assert!(top[2].contains("Claude"));
    assert!(top[3].contains("Cursor"));
    assert_eq!(top[4], "", "separator line is blank");
    assert!(
        top[5].contains("Use Codex."),
        "footer shows cursor-row help"
    );

    // Moving the cursor keeps every row at the same index (no reflow) and
    // only swaps the footer caption.
    let mid = render_texts(true, 1, &rows);
    assert_eq!(mid.len(), 6);
    assert!(mid[1].contains("Codex"));
    assert!(mid[2].contains("Claude"));
    assert!(mid[3].contains("Cursor"));
    assert!(mid[5].contains("Use Claude."), "footer tracks the cursor");
}

#[test]
fn no_footer_when_no_row_has_help() {
    let rows: [Row; 2] = [("Codex", None, false), ("Claude", None, false)];
    // prompt + 2 rows, no footer reserved.
    assert_eq!(render_texts(true, 0, &rows).len(), 3);
}

#[test]
fn single_select_cursor_starts_at_topmost_default_in_range() {
    assert_eq!(initial_cursor(false, &[2], 5), 2);
    assert_eq!(initial_cursor(false, &[], 5), 0, "no default starts at top");
    assert_eq!(
        initial_cursor(false, &[9], 5),
        0,
        "out-of-range default falls back to top"
    );
    assert_eq!(
        initial_cursor(false, &[3, 1], 5),
        1,
        "picks the top-most selected row, not the first listed"
    );
    assert_eq!(
        initial_cursor(false, &[4, 9], 5),
        4,
        "ignores out-of-range indices when picking the top-most"
    );
    assert_eq!(
        initial_cursor(true, &[2], 5),
        0,
        "multi-select always starts at top"
    );
}

#[test]
fn multi_select_marks_default_indices_selected() {
    let items =
        NonEmpty::from_vec(vec![("a", ()), ("b", ()), ("c", ()), ("d", ())]).expect("non-empty");
    let built = build_picker_items(&items, &[0, 2], &[], |_| None::<&str>);
    let selected = built.iter().map(|item| item.selected).collect::<Vec<_>>();
    assert_eq!(selected, vec![true, false, true, false]);
}

#[test]
fn build_picker_items_marks_disabled_indices() {
    let items = NonEmpty::from_vec(vec![("a", ()), ("b", ()), ("c", ())]).expect("non-empty");
    let built = build_picker_items(&items, &[], &[1], |_| None::<&str>);
    let disabled = built.iter().map(|item| item.disabled).collect::<Vec<_>>();
    assert_eq!(disabled, vec![false, true, false]);
}

#[test]
fn build_picker_items_never_selects_a_disabled_row() {
    let items = NonEmpty::from_vec(vec![("a", ()), ("b", ())]).expect("non-empty");
    // Index 1 is listed as both a default and disabled; disabled wins so it is
    // never returned as a pick.
    let built = build_picker_items(&items, &[0, 1], &[1], |_| None::<&str>);
    let selected = built.iter().map(|item| item.selected).collect::<Vec<_>>();
    let disabled = built.iter().map(|item| item.disabled).collect::<Vec<_>>();
    assert_eq!(selected, vec![true, false]);
    assert_eq!(disabled, vec![false, true]);
}

#[test]
fn single_select_does_not_confirm_on_a_disabled_row() {
    let keys = ["Enabled".to_string(), "Disabled".to_string()];
    let make_app = |cursor| App {
        should_render: true,
        should_quit: false,
        should_confirm: false,
        allow_multiple: false,
        prompt: "Pick".to_string(),
        cursor,
        items: NonEmpty::from_vec(vec![
            PickerItem {
                key: &keys[0],
                help: None,
                value: &(),
                selected: false,
                disabled: false,
            },
            PickerItem {
                key: &keys[1],
                help: None,
                value: &(),
                selected: false,
                disabled: true,
            },
        ])
        .expect("non-empty"),
    };

    // Enter on the disabled row is ignored.
    let mut on_disabled = make_app(1);
    on_disabled.confirm();
    assert!(!on_disabled.should_confirm);

    // Enter on an enabled row still confirms.
    let mut on_enabled = make_app(0);
    on_enabled.confirm();
    assert!(on_enabled.should_confirm);
}

#[test]
fn multi_select_confirms_even_with_cursor_on_a_disabled_row() {
    let keys = ["Enabled".to_string(), "Disabled".to_string()];
    let mut app = App {
        should_render: true,
        should_quit: false,
        should_confirm: false,
        allow_multiple: true,
        prompt: "Pick".to_string(),
        cursor: 1,
        items: NonEmpty::from_vec(vec![
            PickerItem {
                key: &keys[0],
                help: None,
                value: &(),
                selected: true,
                disabled: false,
            },
            PickerItem {
                key: &keys[1],
                help: None,
                value: &(),
                selected: false,
                disabled: true,
            },
        ])
        .expect("non-empty"),
    };

    app.confirm();
    assert!(app.should_confirm);
}

#[test]
fn disabled_row_renders_unavailable_marker_and_cannot_toggle() {
    let keys = ["Enabled".to_string(), "Disabled".to_string()];
    let items = vec![
        PickerItem {
            key: &keys[0],
            help: None,
            value: &(),
            selected: false,
            disabled: false,
        },
        PickerItem {
            key: &keys[1],
            help: None,
            value: &(),
            selected: false,
            disabled: true,
        },
    ];
    let mut app = App {
        should_render: true,
        should_quit: false,
        should_confirm: false,
        allow_multiple: true,
        prompt: "Pick".to_string(),
        cursor: 1,
        items: NonEmpty::from_vec(items).expect("non-empty"),
    };

    let texts = app
        .view_lines()
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    // The togglable row shows an empty checkbox; the disabled row shows the
    // unavailable marker instead.
    assert!(texts[1].contains("[ ]"));
    assert!(texts[2].contains("[-]"));

    // Space (toggle) on a disabled row is a no-op.
    app.toggle_selection();
    assert!(!app.items[1].selected, "disabled row must not toggle on");
}

#[test]
fn single_select_rows_have_no_checkbox() {
    let rows: [Row; 2] = [
        ("Apply", Some("Do it."), false),
        ("Cancel", Some("Stop."), false),
    ];
    let lines = render_texts(false, 0, &rows);
    // prompt + 2 rows + blank separator + caption.
    assert_eq!(lines.len(), 5);
    assert!(!lines[1].contains("[x]") && !lines[1].contains("[ ]"));
    assert!(lines[1].contains("Apply"));
    assert!(
        lines[4].contains("Do it."),
        "footer caption is the last line"
    );
}

#[test]
fn moving_cursor_up_and_down_single() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let items = nonempty![
        ("one", "one value"),
        ("two", "two value"),
        ("three", "three value"),
    ];

    let mut tui = test_tui(env, &items);

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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let items = nonempty![
        ("one", "one value"),
        ("two", "two value"),
        ("three", "three value"),
    ];

    let mut tui = test_tui_with_options(
        env,
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
