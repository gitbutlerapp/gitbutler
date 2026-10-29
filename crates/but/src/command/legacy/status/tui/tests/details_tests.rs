use but_testsupport::Sandbox;
use crossterm::event::{KeyCode, KeyModifiers};
use snapbox::{file, str};
use temp_env::with_var;

use crate::{
    command::legacy::status::{
        TuiRunOptions,
        tui::{
            DetailsLayoutMessage, Message, ReloadCause,
            backstack::BackstackEntry,
            tests::utils::{
                TestTuiOptions, commit_with_two_hunks_in_file, test_status_tui,
                test_status_tui_with_options,
            },
        },
    },
    tui::test_utils::Shift,
};

mod binds {
    use crate::tui::test_utils::Shift;

    pub const SCROLL_DOWN: char = 'j';
    pub const SCROLL_UP: char = 'k';

    pub const NEXT_HUNK: Shift = Shift('j');
    pub const PREV_HUNK: Shift = Shift('k');
}

#[test]
fn toggle_details_view_for_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file!["snapshots/toggle_details_view_for_commit_001.svg"]);

    tui.input('d')
        .assert_rendered_term_svg_eq(file!["snapshots/toggle_details_view_for_commit_002.svg"]);

    tui.input('d')
        .assert_rendered_term_svg_eq(file!["snapshots/toggle_details_view_for_commit_003.svg"]);
}

#[test]
fn details_view_updates_with_selection_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('d').assert_rendered_term_svg_eq(file![
        "snapshots/details_view_updates_with_selection_changes_001.svg"
    ]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_updates_with_selection_changes_002.svg"
        ]);

    tui.input(binds::NEXT_HUNK)
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_updates_with_selection_changes_003.svg"
        ]);
}

#[test]
fn manual_reload_does_not_highlight_details_when_status_is_focused() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("uncommitted.txt", "changed\n");

    let mut tui = test_status_tui(env);

    tui.input('d');

    tui.input((KeyModifiers::CONTROL, 'r'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/manual_reload_does_not_highlight_details_when_status_is_focused_001.svg"
        ]);

    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(KeyCode::Esc);

    tui.input((KeyModifiers::CONTROL, 'r'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/manual_reload_does_not_highlight_details_when_status_is_focused_001.svg"
        ]);
}

#[test]
fn watcher_reload_does_not_highlight_previously_focused_details() {
    for remove_selected_hunk in [false, true] {
        let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
        env.setup_metadata(&["A"]);
        env.file("uncommitted.txt", "changed\n");
        if remove_selected_hunk {
            env.file("removed.txt", "removed\n");
        }

        let mut tui = test_status_tui(env);
        tui.input('l');
        tui.input(binds::SCROLL_DOWN);
        tui.input(KeyCode::Esc);

        if remove_selected_hunk {
            tui.env().remove_file("removed.txt");
        }

        // Restoring a remembered hunk or failing to find it must both leave the pane unfocused.
        tui.render_with_messages(
            None,
            Vec::from([Message::Reload(
                None,
                ReloadCause::Watcher {
                    details_selection_changed: true,
                },
            )]),
        )
        .assert_rendered_term_svg_eq(file![
            "snapshots/watcher_reload_does_not_highlight_previously_focused_details_001.svg"
        ]);
    }
}

#[test]
fn details_view_supports_scroll_controls() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let file_contents = (1..=120)
        .map(|line| format!("line-{line:03}\n"))
        .collect::<String>();
    env.file("first file.txt", file_contents.clone());
    env.file("second file.txt", file_contents);

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 12,
            ..Default::default()
        },
    );

    tui.input('l').assert_rendered_term_svg_eq(file![
        "snapshots/details_view_supports_scroll_controls_001.svg"
    ]);

    // scroll by single lines
    tui.render_with_messages(binds::NEXT_HUNK, Vec::new());
    tui.render_with_messages(binds::NEXT_HUNK, Vec::new());
    tui.render_with_messages(binds::NEXT_HUNK, Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_002.svg"
        ]);
    tui.render_with_messages(binds::PREV_HUNK, Vec::new());
    tui.render_with_messages(binds::PREV_HUNK, Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_003.svg"
        ]);

    // jump
    tui.render_with_messages((KeyModifiers::CONTROL, 'd'), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_004.svg"
        ]);
    tui.render_with_messages((KeyModifiers::CONTROL, 'u'), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_005.svg"
        ]);

    // navigate by hunk
    tui.render_with_messages(binds::SCROLL_DOWN, Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_006.svg"
        ]);

    tui.render_with_messages(binds::SCROLL_UP, Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_007.svg"
        ]);
}

#[test]
fn details_scroll_down_updates_selection_when_selected_hunk_leaves_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("alpha.txt", numbered_lines("alpha", 8));
    env.file("bravo.txt", numbered_lines("bravo", 8));
    env.file("charlie.txt", numbered_lines("charlie", 8));

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 16,
            ..Default::default()
        },
    );

    tui.input((KeyModifiers::SHIFT, 'D'));
    tui.render_with_messages(None, Vec::new());

    tui.render_with_messages([binds::NEXT_HUNK; 8], Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_scroll_down_updates_selection_when_selected_hunk_leaves_view_001.svg"
        ]);
}

#[test]
fn details_scroll_up_updates_selection_to_previous_visible_hunk() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("alpha.txt", numbered_lines("alpha", 8));
    env.file("bravo.txt", numbered_lines("bravo", 8));
    env.file("charlie.txt", numbered_lines("charlie", 8));

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 16,
            ..Default::default()
        },
    );

    tui.input((KeyModifiers::SHIFT, 'D'));
    tui.render_with_messages(None, Vec::new());
    tui.render_with_messages(binds::SCROLL_DOWN, Vec::new());
    tui.render_with_messages(binds::SCROLL_DOWN, Vec::new());

    tui.render_with_messages([binds::PREV_HUNK; 8], Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_scroll_up_updates_selection_to_previous_visible_hunk_001.svg"
        ]);
}

fn numbered_lines(prefix: &str, count: usize) -> String {
    (1..=count)
        .map(|line| format!("{prefix}-{line:02}\n"))
        .collect::<String>()
}

#[test]
fn commit_message_wraps_in_details_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 80,
            height: 22,
            ..Default::default()
        },
    );

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_message_wraps_in_details_view_001.svg"
        ]);

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/commit_message_wraps_in_details_view_002.svg"
    ]);

    tui.input(" this commit message is intentionally long so the details pane has to wrap the text across multiple visual lines")
        .assert_rendered_term_svg_eq(file!["snapshots/commit_message_wraps_in_details_view_003.svg"]);

    with_var("GIT_AUTHOR_DATE", Some("2000-01-01T00:00:00Z"), || {
        with_var("GIT_COMMITTER_DATE", Some("2000-01-01T00:00:00Z"), || {
            tui.input(KeyCode::Enter);
        });
    });

    tui.input('d');

    tui.render_with_messages(None, Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_message_wraps_in_details_view_005.svg"
        ]);
}

#[test]
fn details_view_renders_multiple_hunks_and_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let first_file = (1..=8)
        .map(|line| format!("alpha-{line}\n"))
        .collect::<String>();
    let second_file = (1..=8)
        .map(|line| format!("beta-{line}\n"))
        .collect::<String>();

    env.file("alpha.txt", first_file);
    env.file("beta.txt", second_file);

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 24,
            ..Default::default()
        },
    );

    tui.input('d').assert_rendered_term_svg_eq(file![
        "snapshots/details_view_renders_multiple_hunks_and_files_001.svg"
    ]);
}

#[test]
fn details_diff_svg_shows_plus_and_minus_backgrounds() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("A", "A-changed\n");

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 12,
            ..Default::default()
        },
    );

    tui.input('d').assert_rendered_term_svg_eq(file![
        "snapshots/details_diff_svg_shows_plus_and_minus_backgrounds_001.svg"
    ]);
}

#[test]
fn details_view_renders_tab_indented_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file(
        "tab-indented.svelte",
        "\t<CardGroup.Item>\n\t\t{#snippet title()}\n\t\t\tWorktree manipulation\n\t\t{/snippet}\n\t</CardGroup.Item>\n",
    );

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 14,
            ..Default::default()
        },
    );

    tui.input('d').assert_rendered_term_svg_eq(file![
        "snapshots/details_view_renders_tab_indented_file_001.svg"
    ]);
}

#[test]
fn toggling_details_off_and_on_resets_scroll_position() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    let file_contents = (1..=80)
        .map(|line| format!("line-{line:03}\n"))
        .collect::<String>();
    env.file("large.txt", file_contents);

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 12,
            ..Default::default()
        },
    );

    tui.input('l').assert_rendered_term_svg_eq(file![
        "snapshots/toggling_details_off_and_on_resets_scroll_position_001.svg"
    ]);

    tui.input((KeyModifiers::CONTROL, 'd'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggling_details_off_and_on_resets_scroll_position_002.svg"
        ]);

    tui.input('h');
    tui.input('d').assert_rendered_term_svg_eq(file![
        "snapshots/toggling_details_off_and_on_resets_scroll_position_003.svg"
    ]);

    tui.input('d');
    tui.render_with_messages(None, Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggling_details_off_and_on_resets_scroll_position_004.svg"
        ]);
}

#[test]
fn details_view_syntax_highlighting_survives_scrolling() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let rust_code = (1..=120)
        .map(|line| {
            format!(
                "fn function_{line:03}(value: i32) -> i32 {{ let answer = match value {{ 0 => 41, _ => value + 1 }}; println!(\"line-{line:03}: {{answer}}\"); answer }} // comment-{line:03}\n"
            )
        })
        .collect::<String>();
    env.file("syntax.rs", rust_code);

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 12,
            ..Default::default()
        },
    );

    tui.input('l').assert_rendered_term_svg_eq(file![
        "snapshots/details_view_syntax_highlighting_survives_scrolling_001.svg"
    ]);

    tui.render_with_messages((KeyModifiers::CONTROL, 'd'), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_syntax_highlighting_survives_scrolling_002.svg"
        ]);

    tui.render_with_messages((KeyModifiers::CONTROL, 'u'), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_syntax_highlighting_survives_scrolling_003.svg"
        ]);
}

#[test]
fn details_view_highlights_swift() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file(
        "AppModel.swift",
        r#"import Foundation

@MainActor
final class AppModel: ObservableObject {
    @Published private(set) var isRefreshing = false
    private let refreshInterval: Int = 5

    func refresh() async throws {
        print("Refreshing every \(refreshInterval) minutes")
    }
}
"#,
    );

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 18,
            ..Default::default()
        },
    );

    tui.input('l')
        .assert_rendered_term_svg_eq(file!["snapshots/details_view_highlights_swift_001.svg"]);
}

#[test]
fn details_view_can_grow_and_shrink() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 16,
            ..Default::default()
        },
    );

    tui.input('d');
    tui.input("++-")
        .assert_rendered_term_svg_eq(file!["snapshots/details_view_can_grow_and_shrink_001.svg"]);
}

#[test]
fn details_view_resize_clamps_to_max_and_min_width() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 16,
            ..Default::default()
        },
    );

    tui.input('d');
    tui.input("++++++++++++++++++++");
    tui.input("--------------------")
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_resize_clamps_to_max_and_min_width_001.svg"
        ]);
}

#[test]
fn details_cursor_stays_visible_after_resizing() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let long_lines = (1..=80)
        .map(|line| format!("this is a deliberately long line in alpha.txt #{line:03} that should wrap in narrow detail panes\n"))
        .collect::<String>();

    env.file("alpha.txt", long_lines);
    env.file("beta.txt", "beta\n");

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 80,
            height: 10,
            ..Default::default()
        },
    );

    tui.input('d');
    tui.input('l');
    tui.input("----------");
    tui.input(binds::NEXT_HUNK);
    tui.input(binds::NEXT_HUNK);

    tui.input("++++++++++").assert_rendered_term_svg_eq(file![
        "snapshots/details_cursor_stays_visible_after_resizing_001.svg"
    ]);
}

#[test]
fn toggle_full_screen_details_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "content");

    let mut tui = test_status_tui(env);

    // can open details with shift+d
    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_001_open_full_screen.svg"
        ]);

    // full screen details don't close when pressing h
    tui.input('h').assert_rendered_term_svg_eq(file![
        "snapshots/toggle_full_screen_details_view_for_commit_002_h_keeps_full_screen_open.svg"
    ]);

    // can close details with shift+d
    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_003_shift_d_closes_full_screen.svg"
        ]);

    // can close full screen details with escape
    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_004_reopen_full_screen.svg"
        ]);
    tui.input(KeyCode::Esc).assert_rendered_term_svg_eq(file![
        "snapshots/toggle_full_screen_details_view_for_commit_005_escape_closes_full_screen.svg"
    ]);

    // can close full screen details with d
    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_006_reopen_full_screen.svg"
        ]);
    tui.input('d').assert_rendered_term_svg_eq(file![
        "snapshots/toggle_full_screen_details_view_for_commit_007_d_closes_full_screen.svg"
    ]);

    // shift+d with split details in normal mode opens full screen details
    tui.input('d').assert_rendered_term_svg_eq(file![
        "snapshots/toggle_full_screen_details_view_for_commit_008_split_details.svg"
    ]);
    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_009_split_to_full_screen.svg"
        ]);
    tui.input(KeyCode::Esc).assert_rendered_term_svg_eq(file![
        "snapshots/toggle_full_screen_details_view_for_commit_010_escape_closes_from_split.svg"
    ]);

    // shift+d with split details in details mode opens full screen details
    tui.input('d').assert_rendered_term_svg_eq(file![
        "snapshots/toggle_full_screen_details_view_for_commit_011_split_details.svg"
    ]);
    tui.input('l').assert_rendered_term_svg_eq(file![
        "snapshots/toggle_full_screen_details_view_for_commit_012_split_details_mode.svg"
    ]);
    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_013_details_mode_to_full_screen.svg"
        ]);
    tui.input(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_014_escape_closes_from_details_mode.svg"
        ]);
}

#[test]
fn switch_full_screen_details_to_split() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'D'));
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ').assert_backstack_eq([
        BackstackEntry::Mark,
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenFullScreenDetailsView,
    ]);

    tui.render_with_messages(
        None,
        vec![Message::DetailsLayout(DetailsLayoutMessage::SwitchToSplit)],
    )
    .assert_backstack_eq([
        BackstackEntry::Mark,
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenSplitDetailsView,
    ])
    .assert_rendered_term_svg_eq(file![
        "snapshots/switch_full_screen_details_to_split_001.svg"
    ]);

    tui.input((KeyModifiers::SHIFT, 'D')).assert_backstack_eq([
        BackstackEntry::Mark,
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenSplitDetailsView,
    ]);
    tui.render_with_messages(
        None,
        vec![Message::DetailsLayout(DetailsLayoutMessage::SwitchToSplit)],
    )
    .assert_backstack_eq([
        BackstackEntry::Mark,
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenSplitDetailsView,
    ])
    .assert_rendered_term_svg_eq(file![
        "snapshots/switch_full_screen_details_to_split_002.svg"
    ]);
}

#[test]
fn back_from_details_switched_to_split_unfocuses_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("file.txt", "content");

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'D')).assert_backstack_eq([
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenFullScreenDetailsView,
    ]);
    tui.render_with_messages(
        None,
        vec![Message::DetailsLayout(DetailsLayoutMessage::SwitchToSplit)],
    )
    .assert_backstack_eq([
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenSplitDetailsView,
    ]);
    tui.input(KeyCode::Esc)
        .assert_backstack_eq([BackstackEntry::OpenSplitDetailsView])
        .assert_rendered_term_svg_eq(file![
            "snapshots/back_from_details_switched_to_split_unfocuses_details_001.svg"
        ]);
}

#[test]
fn details_view_with_no_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'D'));

    tui.render_with_messages(None, Vec::new())
        .assert_rendered_contains("0 files changed, +0 -0");
}

#[test]
fn unfocusing_split_details_with_escape() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "content");

    let mut tui = test_status_tui(env);

    tui.input('l').assert_rendered_term_svg_eq(file![
        "snapshots/unfocusing_split_details_with_escape_focused.svg"
    ]);

    tui.input(KeyCode::Esc).assert_rendered_term_svg_eq(file![
        "snapshots/unfocusing_split_details_with_escape_unfocused.svg"
    ]);
}

#[test]
fn close_split_details_with_escape() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "content");

    let mut tui = test_status_tui(env);

    tui.input('d')
        .assert_rendered_term_svg_eq(file!["snapshots/close_split_details_with_escape_open.svg"]);

    tui.input(KeyCode::Esc).assert_rendered_term_svg_eq(file![
        "snapshots/close_split_details_with_escape_closed.svg"
    ]);
}

#[test]
fn escape_after_toggling_split_details_closed_does_not_reopen_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "content");

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('d');

    tui.input(KeyCode::Esc).assert_rendered_term_svg_eq(file![
        "snapshots/close_split_details_with_escape_closed.svg"
    ]);
}

#[test]
fn escape_after_toggling_full_screen_details_closed_does_not_reopen_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "content");

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'D'));
    tui.input((KeyModifiers::SHIFT, 'D'));

    tui.input(KeyCode::Esc).assert_rendered_term_svg_eq(file![
        "snapshots/toggle_full_screen_details_view_for_commit_closed_full_screen_details.svg"
    ]);
}

#[test]
fn open_and_focus_details_split_can_be_closed_with_esc() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "content");

    let mut tui = test_status_tui(env);

    tui.input('l').assert_rendered_term_svg_eq(file![
        "snapshots/open_and_focus_details_split_can_be_closed_with_esc_focused.svg"
    ]);

    tui.input(KeyCode::Esc).assert_rendered_term_svg_eq(file![
        "snapshots/open_and_focus_details_split_can_be_closed_with_esc_open.svg"
    ]);

    tui.input(KeyCode::Esc);
}

#[test]
fn viewing_empty_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("empty file", "");

    let mut tui = test_status_tui(env);

    tui.input('d')
        .assert_rendered_term_svg_eq(file!["snapshots/viewing_empty_file_001.svg"]);
}

#[test]
fn discard_hunk_from_detail_view_via_uncommitted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('l');
    tui.input((KeyModifiers::SHIFT, 'G'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_hunk_from_detail_view_via_uncommitted_001.svg"
        ]);

    tui.input('x')
        .assert_rendered_contains("Discard hunk twop:b two?")
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_hunk_from_detail_view_via_uncommitted_002.svg"
        ]);
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/discard_hunk_from_detail_view_via_uncommitted_003.svg"
    ]);

    tui.input('x')
        .assert_rendered_contains("Discard hunk or:f three?")
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_hunk_from_detail_view_via_uncommitted_004.svg"
        ]);
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/discard_hunk_from_detail_view_via_uncommitted_005.svg"
    ]);

    tui.input('x')
        .assert_rendered_contains("Discard hunk kl:f one?")
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_hunk_from_detail_view_via_uncommitted_006.svg"
        ]);
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/discard_hunk_from_detail_view_via_uncommitted_007.svg"
    ]);
}

#[test]
fn discard_hunk_from_detail_view_via_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    env.file(
        "x-file",
        Vec::from([
            "line 1", "line 2", "line 3", "line 4", "line 5", "line 6", "line 7", "line 8",
            "line 9",
        ])
        .join("\n"),
    );

    let mut tui = test_status_tui(env);

    tui.input(binds::SCROLL_DOWN);
    tui.input(binds::SCROLL_DOWN);
    tui.input(binds::SCROLL_DOWN);
    tui.input(binds::SCROLL_DOWN);
    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.env().prepend_file("x-file", "new first line");
    tui.env().append_file("x-file", "new first line");

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/discard_hunk_from_detail_view_via_file_001.svg"
    ]);

    tui.input('g');
    tui.input(binds::SCROLL_DOWN);
    tui.input(binds::SCROLL_DOWN);
    tui.input(binds::SCROLL_DOWN);
    tui.input(binds::SCROLL_DOWN)
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_hunk_from_detail_view_via_file_002.svg"
        ]);
    tui.input('d');
    tui.input('l');

    tui.input((KeyModifiers::SHIFT, 'G'));
    tui.input('x')
        .assert_rendered_contains("Discard hunk px:0 x-file?");
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/discard_hunk_from_detail_view_via_file_003.svg"
    ]);

    tui.input('x');
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/discard_hunk_from_detail_view_via_file_004.svg"
    ]);
}

#[test]
fn highlighting_multiline_things_work() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one.py", include_str!("fixtures/python_with_shebang.py"));

    let mut tui = test_status_tui(env);

    tui.input('d').assert_rendered_term_svg_eq(file![
        "snapshots/highlighting_multiline_things_work_001.svg"
    ]);
}

#[test]
fn marking_and_discarding_multiple_uncommitted_hunks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_uncommitted_hunks_001.svg"
    ]);
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_uncommitted_hunks_002.svg"
    ]);
    tui.input('x');
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_uncommitted_hunks_003.svg"
    ]);
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_uncommitted_hunks_004.svg"
    ]);
    tui.input('x');
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_uncommitted_hunks_005.svg"
    ]);
}

#[test]
fn detail_marks_use_the_backstack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN)
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenSplitDetailsView,
        ])
        .assert_rendered_term_svg_eq(file!["snapshots/detail_marks_use_the_backstack_001.svg"]);
    tui.input(' ');
    tui.input(' ')
        .assert_backstack_eq([
            BackstackEntry::Mark,
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenSplitDetailsView,
        ])
        .assert_rendered_term_svg_eq(file!["snapshots/detail_marks_use_the_backstack_002.svg"]);
    tui.input('g');
    tui.input(KeyCode::Esc)
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenSplitDetailsView,
        ])
        .assert_rendered_term_svg_eq(file!["snapshots/detail_marks_use_the_backstack_003.svg"]);
}

#[test]
fn detail_marks_stay_when_leaving_detail_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN).assert_backstack_eq([
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenSplitDetailsView,
    ]);
    tui.input(' ');
    tui.input(' ').assert_backstack_eq([
        BackstackEntry::Mark,
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenSplitDetailsView,
    ]);
    tui.input('h')
        .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::OpenSplitDetailsView])
        .assert_rendered_term_svg_eq(file![
            "snapshots/detail_marks_stay_when_leaving_detail_mode_001.svg"
        ]);
}

#[test]
fn detail_marks_stay_when_closing_detail_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN).assert_backstack_eq([
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenSplitDetailsView,
    ]);
    tui.input(' ');
    tui.input(' ').assert_backstack_eq([
        BackstackEntry::Mark,
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenSplitDetailsView,
    ]);
    tui.input('d').assert_backstack_eq([BackstackEntry::Mark]);
    tui.input('d')
        .assert_backstack_eq([BackstackEntry::OpenSplitDetailsView, BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file![
            "snapshots/detail_marks_stay_when_closing_detail_view_001.svg"
        ]);
}

#[test]
fn detail_marks_stay_when_closing_full_screen_detail_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'D'));
    tui.render_with_messages(None, Vec::new());
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input(' ')
        .assert_backstack_eq([
            BackstackEntry::Mark,
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenFullScreenDetailsView,
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/detail_marks_stay_when_closing_full_screen_detail_view_001.svg"
        ]);
    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file![
            "snapshots/detail_marks_stay_when_closing_full_screen_detail_view_002.svg"
        ]);
    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenFullScreenDetailsView,
            BackstackEntry::Mark,
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/detail_marks_stay_when_closing_full_screen_detail_view_003.svg"
        ]);
}

#[test]
fn marks_stay_when_going_straight_from_split_to_fullscreen() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ').assert_backstack_eq([
        BackstackEntry::Mark,
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenSplitDetailsView,
    ]);

    // go directly from split to full screen, the marks should be maintained
    // the top entry in the backstack should be the mark
    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_backstack_eq([
            BackstackEntry::Mark,
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenSplitDetailsView,
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marks_stay_when_going_straight_from_split_to_fullscreen_001.svg"
        ]);

    // Closing full screen details should retain the marks.
    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marks_stay_when_going_straight_from_split_to_fullscreen_002.svg"
        ]);
}

#[test]
fn normal_and_detail_marks_coexist_in_split_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input(binds::SCROLL_DOWN)
        .assert_rendered_contains("┊   kl   A one");
    tui.input(' ')
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_contains("┊✔︎  kl   A one");

    tui.input('d');

    // focusing the details should preserve the marks
    tui.input('l')
        .assert_rendered_contains("┊✔︎  kl   A one")
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenSplitDetailsView,
            BackstackEntry::Mark,
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/normal_and_detail_marks_coexist_in_split_details_001.svg"
        ]);

    // Marking from the details view retains the normal mode marks.
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ')
        .assert_rendered_contains("┊✔︎  kl   A one")
        .assert_backstack_eq([
            BackstackEntry::Mark,
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenSplitDetailsView,
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/normal_and_detail_marks_coexist_in_split_details_002.svg"
        ]);
}

#[test]
fn normal_and_detail_marks_coexist_in_full_screen_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input(binds::SCROLL_DOWN);
    tui.input(' ')
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_contains("┊✔︎  kl   A one");

    tui.input((KeyModifiers::SHIFT, 'D'));
    tui.render_with_messages(None, Vec::new())
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenFullScreenDetailsView,
            BackstackEntry::Mark,
        ]);

    tui.input(binds::SCROLL_DOWN);
    tui.input(' ')
        .assert_backstack_eq([
            BackstackEntry::Mark,
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenFullScreenDetailsView,
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/normal_and_detail_marks_coexist_in_full_screen_details_001.svg"
        ]);

    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_rendered_contains("┊✔︎  kl   A one")
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file![
            "snapshots/normal_and_detail_marks_coexist_in_full_screen_details_002.svg"
        ]);
}

#[test]
fn keeps_normal_mode_marks_when_detail_section_cannot_be_marked() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input(' ').assert_backstack_eq([BackstackEntry::Mark]);

    tui.input('d');
    tui.input('l').assert_backstack_eq([
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenSplitDetailsView,
        BackstackEntry::Mark,
    ]);

    // The commit header section cannot be marked, so attempting to mark it preserves normal marks.
    tui.input(' ')
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenSplitDetailsView,
            BackstackEntry::Mark,
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/keeps_normal_mode_marks_when_detail_section_cannot_be_marked_001.svg"
        ]);
}

#[test]
fn leaving_command_mode_from_details_puts_you_back_in_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input(binds::SCROLL_UP);

    tui.input(':').assert_backstack_eq([
        BackstackEntry::LeaveCommandMode,
        BackstackEntry::Mark,
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::OpenSplitDetailsView,
    ]);
    tui.input(KeyCode::Esc)
        .assert_backstack_eq([
            BackstackEntry::Mark,
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenSplitDetailsView,
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/leaving_command_mode_from_details_puts_you_back_in_details_001.svg"
        ]);
}

#[test]
fn dims_unselectable_lines_while_in_details_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input(' ');
    tui.input('d');
    tui.input('l').assert_rendered_term_svg_eq(file![
        "snapshots/dims_unselectable_lines_while_in_details_mode_001.svg"
    ]);
}

#[test]
fn shows_synthetic_change_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(binds::SCROLL_DOWN);
    tui.input(binds::SCROLL_DOWN);
    tui.input('d')
        .assert_rendered_term_svg_eq(file!["snapshots/shows_synthetic_change_id_001.svg"]);
}

#[test]
fn marking_file_marks_all_hunks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "line\n".repeat(10));

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.env().prepend_file("file", "top");
    tui.env().append_file("file", "bottom");

    tui.reload();

    // marking the file in the status also marks both hunks in the detail view
    tui.input('g');
    tui.input('d');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/marking_file_marks_all_hunks_001.svg"]);

    // unmarking the file in the status removes all marks from detail view
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/marking_file_marks_all_hunks_002.svg"]);

    // marking a file in the detail view then clearing marks from the status also clears the detail
    // marks
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input('g')
        .assert_rendered_term_svg_eq(file!["snapshots/marking_file_marks_all_hunks_003.svg"]);
    tui.input('h');
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/marking_file_marks_all_hunks_004.svg"]);
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/marking_file_marks_all_hunks_005.svg"]);
}

#[test]
fn marking_all_hunks_marks_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "line\n".repeat(10));

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.env().prepend_file("file", "top");
    tui.env().append_file("file", "bottom");

    tui.reload();

    tui.input('g');
    tui.input('d');
    tui.input(binds::SCROLL_DOWN);
    tui.input('l')
        .assert_rendered_term_svg_eq(file!["snapshots/marking_all_hunks_marks_file_001.svg"]);

    // marking both hunks in the detail view should also mark the file in the status
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input(' ');
    tui.input('g')
        .assert_rendered_term_svg_eq(file!["snapshots/marking_all_hunks_marks_file_002.svg"]);

    // unmarking a hunk in the details view also unmarks it in the status
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input('g')
        .assert_rendered_term_svg_eq(file!["snapshots/marking_all_hunks_marks_file_003.svg"]);
}

#[test]
fn marking_all_hunks_marks_file_with_multiple_files_changed() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "line\n".repeat(10));

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.env().prepend_file("file", "top");
    tui.env().append_file("file", "bottom");

    tui.env().file("new-file", "content");

    tui.reload();

    tui.input('g');
    tui.input('d').assert_rendered_term_svg_eq(file![
        "snapshots/marking_all_hunks_marks_file_with_multiple_files_changed_001.svg"
    ]);
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input(' ');
    tui.input('g').assert_rendered_term_svg_eq(file![
        "snapshots/marking_all_hunks_marks_file_with_multiple_files_changed_002.svg"
    ]);
    tui.input((KeyModifiers::SHIFT, 'G'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_all_hunks_marks_file_with_multiple_files_changed_003.svg"
        ]);
    tui.input(binds::SCROLL_UP);
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/marking_all_hunks_marks_file_with_multiple_files_changed_004.svg"
    ]);
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/marking_all_hunks_marks_file_with_multiple_files_changed_005.svg"
    ]);
}

#[test]
fn marking_uncommitted_area_marks_all_hunks_in_detail_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "line\n".repeat(10));

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.env().prepend_file("file", "top");
    tui.env().append_file("file", "bottom");

    tui.env().file("new-file", "content");

    tui.reload();

    tui.input('g');
    tui.input('d');
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/marking_uncommitted_area_marks_all_hunks_in_detail_view_001.svg"
    ]);
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/marking_uncommitted_area_marks_all_hunks_in_detail_view_002.svg"
    ]);
}

#[test]
fn marking_file_in_pick_changes_marks_hunks_in_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "content");

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            run_options: TuiRunOptions::PickChanges,
            ..Default::default()
        },
    );

    tui.input('d');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/marking_file_in_pick_changes_marks_hunks_in_details_001.svg"
    ]);
}

#[test]
fn marking_and_discarding_all_hunks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "line\n".repeat(10));

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.env().prepend_file("file", "top");
    tui.env().append_file("file", "bottom");

    tui.env().file("new-file", "content");

    tui.reload();

    // mark both hunks in file
    tui.input('g');
    tui.input('d');
    tui.input(binds::SCROLL_DOWN);
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input(' ');
    tui.input('g')
        .assert_rendered_term_svg_eq(file!["snapshots/marking_and_discarding_all_hunks_001.svg"]);

    tui.input('x');
    tui.input('y')
        .assert_rendered_term_svg_eq(file!["snapshots/marking_and_discarding_all_hunks_002.svg"]);
}

#[test]
fn squashing_marks_from_split_details_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input(' ');
    tui.input('d');
    tui.input('l').assert_rendered_term_svg_eq(file![
        "snapshots/squashing_marks_from_split_details_view_001.svg"
    ]);
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squashing_marks_from_split_details_view_002.svg"
    ]);
    tui.input('j');
    tui.input('j');
    tui.input('u').assert_rendered_term_svg_eq(file![
        "snapshots/squashing_marks_from_split_details_view_003.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/squashing_marks_from_split_details_view_004.svg"
    ]);
}

#[test]
fn squashing_marks_from_full_screen_details_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input(' ');
    tui.input(Shift('d')).assert_rendered_term_svg_eq(file![
        "snapshots/squashing_marks_from_full_screen_details_view_001.svg"
    ]);
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squashing_marks_from_full_screen_details_view_002.svg"
    ]);
    tui.input('j');
    tui.input('j');
    tui.input('u').assert_rendered_term_svg_eq(file![
        "snapshots/squashing_marks_from_full_screen_details_view_003.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/squashing_marks_from_full_screen_details_view_004.svg"
    ]);
}

#[test]
fn squashing_selection_from_split_details_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input(binds::SCROLL_DOWN);
    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN)
        .assert_rendered_term_svg_eq(file![
            "snapshots/squashing_selection_from_split_details_view_001.svg"
        ]);
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squashing_selection_from_split_details_view_002.svg"
    ]);
    tui.input('j');
    tui.input('j');
    tui.input('u').assert_rendered_term_svg_eq(file![
        "snapshots/squashing_selection_from_split_details_view_003.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/squashing_selection_from_split_details_view_004.svg"
    ]);
}

#[test]
fn squashing_selection_from_full_screen_details_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input(binds::SCROLL_DOWN);
    tui.input(Shift('d'));
    tui.input(binds::SCROLL_DOWN)
        .assert_rendered_term_svg_eq(file![
            "snapshots/squashing_selection_from_full_screen_details_view_001.svg"
        ]);
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squashing_selection_from_full_screen_details_view_002.svg"
    ]);
    tui.input('j');
    tui.input('j');
    tui.input('u').assert_rendered_term_svg_eq(file![
        "snapshots/squashing_selection_from_full_screen_details_view_003.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/squashing_selection_from_full_screen_details_view_004.svg"
    ]);
}

#[test]
fn committing_selection_from_split_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input('l');
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/committing_selection_from_split_details_001.svg"
    ]);
    tui.input('c');
    tui.input(binds::SCROLL_DOWN)
        .assert_rendered_term_svg_eq(file![
            "snapshots/committing_selection_from_split_details_002.svg"
        ]);
    tui.input('e');
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/committing_selection_from_split_details_003.svg"
    ]);
}

#[test]
fn committing_selection_from_full_screen_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'D'));
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/committing_selection_from_full_screen_details_001.svg"
    ]);
    tui.input('c');
    tui.input(binds::SCROLL_DOWN)
        .assert_rendered_term_svg_eq(file![
            "snapshots/committing_selection_from_full_screen_details_002.svg"
        ]);
    tui.input('e');
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/committing_selection_from_full_screen_details_003.svg"
    ]);
}

#[test]
fn committing_hunks_from_split_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input('l');
    tui.input('j');
    tui.input(' ');
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/committing_hunks_from_split_details_001.svg"
    ]);
    tui.input('c').assert_rendered_term_svg_eq(file![
        "snapshots/committing_hunks_from_split_details_002.svg"
    ]);
    tui.input('j');
    tui.input('j');
    tui.input('e');
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/committing_hunks_from_split_details_003.svg"
    ]);
}

#[test]
fn committing_hunks_from_full_screen_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'D'));
    tui.input('j');
    tui.input(' ');
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/committing_hunks_from_full_screen_details_001.svg"
    ]);
    tui.input('c');
    tui.input('j');
    tui.input('j');
    tui.input('e').assert_rendered_term_svg_eq(file![
        "snapshots/committing_hunks_from_full_screen_details_002.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/committing_hunks_from_full_screen_details_003.svg"
    ]);
}

#[test]
fn commit_source_without_marks_is_selectable() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input('l');
    tui.input('j');
    tui.input('c').assert_rendered_term_svg_eq(file![
        "snapshots/commit_source_without_marks_is_selectable_001.svg"
    ]);
}

#[test]
fn commit_source_with_marks_is_selectable() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input('l');
    tui.input('j');
    tui.input(' ');
    tui.input('c').assert_rendered_term_svg_eq(file![
        "snapshots/commit_source_with_marks_is_selectable_001.svg"
    ]);
}

#[test]
fn commit_source_with_partial_marks_is_selectable() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "line\n".repeat(10));

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.env().prepend_file("file", "top");
    tui.env().append_file("file", "bottom");

    tui.env().file("new-file", "content");

    tui.reload();

    tui.input('g');
    tui.input('l');
    tui.input('j');
    tui.input(' ');
    tui.input('c')
        .assert_current_line_eq(str!["┊—  << source >> << noop >> qs M file             │                                                █"]);
}

#[test]
fn squash_source_without_marks_is_selectable() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input('l');
    tui.input('j');
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squash_source_without_marks_is_selectable_001.svg"
    ]);
}

#[test]
fn squash_source_with_marks_is_selectable() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "line");
    env.file("two", "line");

    let mut tui = test_status_tui(env);

    tui.input('l');
    tui.input('j');
    tui.input(' ');
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squash_source_with_marks_is_selectable_001.svg"
    ]);
}

#[test]
fn squash_source_with_partial_marks_is_selectable() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "line\n".repeat(10));

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.env().prepend_file("file", "top");
    tui.env().append_file("file", "bottom");

    tui.env().file("new-file", "content");

    tui.reload();

    tui.input('g');
    tui.input('l');
    tui.input('j');
    tui.input(' ');
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squash_source_with_partial_marks_is_selectable_001.svg"
    ]);
}

#[test]
fn undo_from_details_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('x');
    tui.input('y');
    tui.input('l')
        .assert_rendered_term_svg_eq(file!["snapshots/undo_from_details_mode_001.svg"]);
    tui.input('u')
        .assert_rendered_term_svg_eq(file!["snapshots/undo_from_details_mode_002.svg"]);
}

#[test]
fn discard_committed_hunk_from_commit_details() {
    let mut tui = commit_with_two_hunks_in_file();

    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(binds::SCROLL_DOWN);
    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/discard_committed_hunk_from_commit_details_001.svg"
    ]);

    // the commit is rewritten without the hunk and stays selected
    tui.input('y')
        .assert_current_line_eq(str!["┊●   [..] (no commit message)[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_committed_hunk_from_commit_details_002.svg"
        ]);
}

#[test]
fn discard_committed_hunk_from_committed_file_details() {
    let mut tui = commit_with_two_hunks_in_file();

    tui.input('f');
    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/discard_committed_hunk_from_committed_file_details_001.svg"
    ]);

    // the file in the rewritten commit stays selected
    tui.input('y')
        .assert_current_line_eq(str!["┊│     [..] M file[..]"])
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenSplitDetailsView,
            BackstackEntry::ShowFileList,
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_committed_hunk_from_committed_file_details_002.svg"
        ]);

    // discarding the last hunk of the file selects the next file in the commit
    tui.input('x');
    tui.input('y')
        .assert_current_line_eq(str!["┊│     [..] M other[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_committed_hunk_from_committed_file_details_003.svg"
        ]);
}

#[test]
fn marking_committed_hunks_marks_file() {
    let mut tui = commit_with_two_hunks_in_file();

    tui.input('f');
    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);

    // marking one hunk partially marks the file in the status
    tui.input(' ')
        .assert_marks_count_eq(1)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_committed_hunks_marks_file_001.svg"
        ]);

    // marking both hunks marks the file in the status
    tui.input(' ')
        .assert_marks_count_eq(3)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_committed_hunks_marks_file_002.svg"
        ]);

    // unmarking a hunk partially marks the file again
    tui.input(' ')
        .assert_marks_count_eq(1)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_committed_hunks_marks_file_003.svg"
        ]);
}

#[test]
fn marking_committed_file_marks_all_hunks() {
    let mut tui = commit_with_two_hunks_in_file();

    tui.input('f');

    // marking the file in the status marks both hunks in the details
    tui.input(' ');
    tui.input('k');
    tui.input('d')
        .assert_marks_count_eq(1)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_committed_file_marks_all_hunks_001.svg"
        ]);

    // unmarking a hunk in the details keeps the other hunk marked
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ')
        .assert_marks_count_eq(1)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_committed_file_marks_all_hunks_002.svg"
        ]);

    // marking the partially marked file marks all of it instead of its hunks
    tui.input('h');
    tui.input(' ')
        .assert_marks_count_eq(1)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_committed_file_marks_all_hunks_003.svg"
        ]);

    // unmarking the file also unmarks its hunks
    tui.input('k');
    tui.input(' ')
        .assert_marks_count_eq(0)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_committed_file_marks_all_hunks_004.svg"
        ]);
}

#[test]
fn marking_and_discarding_committed_hunks() {
    let mut tui = commit_with_two_hunks_in_file();

    // mark the bottom hunk of `file` and the only hunk of `other`, which also marks `other`
    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input(' ')
        .assert_marks_count_eq(3)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_and_discarding_committed_hunks_001.svg"
        ]);

    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_committed_hunks_002.svg"
    ]);

    // only the top hunk of `file` is left in the commit
    tui.input('y')
        .assert_marks_count_eq(0)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_and_discarding_committed_hunks_003.svg"
        ]);
}

#[test]
fn discarding_marked_committed_hunks_from_status() {
    let mut tui = commit_with_two_hunks_in_file();

    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');

    // the commit whose hunks are marked stays selected and shows that some of it is marked
    tui.input('h')
        .assert_current_line_eq(str!["┊—   [..] (no commit message)[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discarding_marked_committed_hunks_from_status_001.svg"
        ]);

    tui.input('x');
    tui.input('y')
        .assert_marks_count_eq(0)
        .assert_rendered_term_svg_eq(file![
            "snapshots/discarding_marked_committed_hunks_from_status_002.svg"
        ]);
}

#[test]
fn discarding_all_marked_hunks_of_renamed_file_keeps_the_rename() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let original_content = "one\ntwo\nthree\n";
    env.file("file.txt", original_content);

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.env().remove_file("file.txt");
    tui.env()
        .file("renamed_file.txt", format!("{original_content}\nnew"));
    tui.reload();

    tui.input('c');
    tui.input('e');
    tui.input('a');
    tui.input(KeyCode::Enter);

    tui.input('f')
        .assert_current_line_eq(str!["┊│     [..] R renamed_file.txt"]);
    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);

    // marking the only hunk also marks the file
    tui.input(' ')
        .assert_marks_count_eq(2)
        .assert_rendered_term_svg_eq(file![
            "snapshots/discarding_all_marked_hunks_of_renamed_file_keeps_the_rename_001.svg"
        ]);

    // like `but discard` with the hunk's ID, only the hunk is discarded and the rename stays
    tui.input('x');
    tui.input('y')
        .assert_current_line_eq(str!["┊│     [..] R renamed_file.txt[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discarding_all_marked_hunks_of_renamed_file_keeps_the_rename_002.svg"
        ]);
}

#[test]
fn discarding_marked_hunks_from_status_keeps_file_list_on_rewritten_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let original_content = "one\ntwo\nthree\n";
    env.file("file.txt", original_content);

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.env().remove_file("file.txt");
    tui.env()
        .file("renamed_file.txt", format!("{original_content}\nnew"));
    tui.reload();

    tui.input('c');
    tui.input('e');
    tui.input('a');
    tui.input(KeyCode::Enter);

    tui.input('f');
    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input('h');

    // the rename is left, so the file list moves to the rewritten commit and stays usable
    tui.input('x');
    tui.input('y')
        .assert_current_line_eq(str!["┊│     [..] R renamed_file.txt[..]"])
        .assert_backstack_eq([
            BackstackEntry::OpenSplitDetailsView,
            BackstackEntry::ShowFileList,
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discarding_marked_hunks_from_status_keeps_file_list_on_rewritten_commit_001.svg"
        ]);
    tui.input('k')
        .assert_current_line_eq(str!["┊│     [..] R renamed_file.txt[..]"]);
}

#[test]
fn marking_partially_marked_commit_marks_the_whole_commit() {
    let mut tui = commit_with_two_hunks_in_file();

    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ');
    tui.input('h')
        .assert_current_line_eq(str!["┊—   [..] (no commit message)[..]"]);

    // marking the commit replaces the marks of its hunks
    tui.input(' ')
        .assert_marks_count_eq(1)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_partially_marked_commit_marks_the_whole_commit_001.svg"
        ]);

    // unmarking the commit leaves nothing marked
    tui.input('k')
        .assert_current_line_eq(str!["┊✔︎   [..] (no commit message)[..]"]);
    tui.input(' ')
        .assert_marks_count_eq(0)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_partially_marked_commit_marks_the_whole_commit_002.svg"
        ]);
}

#[test]
fn marking_hunk_of_marked_commit_unmarks_only_that_hunk() {
    let mut tui = commit_with_two_hunks_in_file();

    // marking the commit in the status marks all of its hunks in the details
    tui.input(' ');
    tui.input('k');
    tui.input('d')
        .assert_marks_count_eq(1)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_hunk_of_marked_commit_unmarks_only_that_hunk_001.svg"
        ]);

    // unmarking a hunk keeps the other hunks of the commit marked, along with `other` since all
    // of its hunks are
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);
    tui.input(' ')
        .assert_marks_count_eq(3)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_hunk_of_marked_commit_unmarks_only_that_hunk_002.svg"
        ]);

    // the commit is partially marked
    tui.input('h')
        .assert_current_line_eq(str!["┊—   [..] (no commit message)[..]"]);
}

#[test]
fn marking_hunk_while_another_commit_is_marked_keeps_the_marks() {
    let mut tui = commit_with_two_hunks_in_file();

    tui.input('j');
    tui.input(' ');
    tui.input('k');
    tui.input('d');
    tui.input('l');
    tui.input(binds::SCROLL_DOWN);

    // hunks can't be marked together with another commit
    tui.input(' ')
        .assert_marks_count_eq(1)
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_hunk_while_another_commit_is_marked_keeps_the_marks_001.svg"
        ]);
}
