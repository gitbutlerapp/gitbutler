use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::tests::utils::{test_status_tui, unarchive_worktrees};

#[test]
fn unapply_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('b');
    tui.input('n');

    tui.input((KeyModifiers::SHIFT, 'G'));
    tui.input('b');
    tui.input('n');

    tui.input('g')
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_001.svg"]);

    tui.input('s')
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_002.svg"]);

    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_003.svg"]);

    tui.input('k');
    tui.input('u')
        .assert_current_line_eq(str!["┊├┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_004.svg"]);
}

#[test]
fn unapply_stack_selects_base_branch_when_next_stack_has_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('b');
    tui.input('n');
    tui.input('n');

    tui.input((KeyModifiers::SHIFT, 'G'));
    tui.input('b');
    tui.input('n');

    tui.input('g');
    tui.input('s');
    tui.input('u').assert_current_line_eq(str!["┊├┄ g0 [A]"]);
}

#[test]
fn enter_stack_mode_from_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('s')
        .assert_rendered_term_svg_eq(file!["snapshots/enter_stack_mode_from_commits_001.svg"]);
}

#[test]
fn moving_stacks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_status_tui(env);

    for name in ["one", "two", "three"] {
        tui.input('g');
        tui.input('b');
        tui.input('n');
        tui.input(KeyCode::Enter);
        for _ in 0..100 {
            tui.input(KeyCode::Backspace);
        }
        tui.input(name);
        tui.input(KeyCode::Enter);
        tui.input('g');
    }

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_001.svg"]);

    tui.input('j');
    tui.input('s');
    tui.input('m')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_002.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_003.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_004.svg"]);

    tui.input('s');
    tui.input('m');
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_005.svg"]);

    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_006.svg"]);
}

#[test]
fn applying_stacks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_status_tui(env);

    for name in ["one", "two"] {
        tui.input('g');
        tui.input('b');
        tui.input('n');
        tui.input(KeyCode::Enter);
        for _ in 0..100 {
            tui.input(KeyCode::Backspace);
        }
        tui.input(name);
        tui.input(KeyCode::Enter);
        tui.input('g');
    }

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/applying_stacks_001.svg"]);

    for _ in 0..2 {
        tui.input('s');
        tui.input('u');
        tui.input('g');
    }

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/applying_stacks_002.svg"]);

    tui.input('s');
    tui.input('a')
        .assert_rendered_term_svg_eq(file!["snapshots/applying_stacks_003.svg"]);
    tui.input("two")
        .assert_rendered_term_svg_eq(file!["snapshots/applying_stacks_004.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/applying_stacks_005.svg"]);
}

#[test]
fn escape_moves_cursor_back_to_valid_position() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j').assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    tui.input('s');
    tui.input('m');
    tui.input('j');

    // cancelling should put the cursor at a valid position
    tui.input(KeyCode::Esc).assert_current_line_eq(str![
        "┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M"
    ]);
}

#[test]
fn maintains_cursor_position_if_on_source() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j').assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    tui.input('s');
    tui.input('m');

    // cancelling should put the cursor at a valid position
    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
}

#[test]
fn moving_stacks_skips_worktree_lanes() {
    // Worktree creation resolves the user's home through but_path, outside the sandbox repo.
    but_testsupport::isolated_app_data_dir(|| {
        let env =
            Sandbox::init_scenario_with_target_and_default_settings_slow("one-stack-with-worktree");
        env.setup_metadata(&["A"]);

        unarchive_worktrees(&env);
        let mut tui = test_status_tui(env);

        // A new stack above `A`, whose commit has a worktree lane nested on it.
        tui.input('g');
        tui.input('b');
        tui.input('n');
        tui.input(KeyCode::Enter);
        for _ in 0..100 {
            tui.input(KeyCode::Backspace);
        }
        tui.input("new");
        tui.input(KeyCode::Enter);

        // A worktree resting on the base, which renders below all stacks.
        tui.input('w');
        tui.input(['j'; 20]);
        tui.input('n');

        tui.input('t');
        tui.input("new");
        tui.input(KeyCode::Enter)
            .assert_current_line_eq(str!["┊╭┄ ne [new] (no commits)"]);
        tui.input('s');
        tui.input('m').assert_rendered_term_svg_eq(file![
            "snapshots/moving_stacks_skips_worktree_lanes_001.svg"
        ]);

        // The worktree lane nested inside `A` isn't between stacks, so the first target is below `A`.
        tui.input('j')
            .assert_current_line_eq(str!["┊<< move stack >>"])
            .assert_rendered_term_svg_eq(file![
                "snapshots/moving_stacks_skips_worktree_lanes_002.svg"
            ]);

        // Stacks always render above worktrees resting on the base, so there is no target below one.
        tui.input('j').assert_rendered_term_svg_eq(file![
            "snapshots/moving_stacks_skips_worktree_lanes_003.svg"
        ]);

        tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
            "snapshots/moving_stacks_skips_worktree_lanes_004.svg"
        ]);
    });
}
