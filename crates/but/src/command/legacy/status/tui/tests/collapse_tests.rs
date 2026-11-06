use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::str;

use crate::command::legacy::status::{
    TuiLaunchOptions,
    tui::tests::utils::{TestTuiOptions, test_status_tui, test_status_tui_with_options},
};

#[test]
fn collapsing_from_a_commit_hides_the_branch_commits_behind_a_summary() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   ywx add second"]);

    let collapsed = tui.input('z');
    // The branch line carries the summary and its commits are gone.
    snapbox::assert_data_eq!(
        collapsed.rendered_output(),
        str![[r#"
"╭┄ @ [uncommitted] (no changes)                                                                     "
"┊                                                                                                   "
"┊╭┄ g0 [A] (2 commits, 2 files)                                                                     "
"├╯                                                                                                  "
"┊                                                                                                   "
"┴ 1bbc04b (common base, main, origin/main) 2000-01-02 add Base                                      "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"  normal   ↑/k up • ↓/j down • c commit • r squash • m move • b branch • s stack • ? help • q quit  "

"#]]
    );
    collapsed.assert_current_line_eq(str!["┊╭┄ g0 [A] (2 commits, 2 files)"]);

    let expanded = tui.input('z');
    // Expanding restores the commits and drops the summary.
    snapbox::assert_data_eq!(
        expanded.rendered_output(),
        str![[r#"
"╭┄ @ [uncommitted] (no changes)                                                                     "
"┊                                                                                                   "
"┊╭┄ g0 [A]                                                                                          "
"┊●   ywx add second                                                                                 "
"┊●   zll add first                                                                                  "
"├╯                                                                                                  "
"┊                                                                                                   "
"┴ 1bbc04b (common base, main, origin/main) 2000-01-02 add Base                                      "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"  normal   ↑/k up • ↓/j down • c commit • r squash • m move • b branch • s stack • ? help • q quit  "

"#]]
    );
}

#[test]
fn collapsing_one_stacked_branch_keeps_the_branch_below_expanded() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-dependent-branches");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [B]"]);

    let collapsed = tui.input('z');
    // Only the selected branch hides its commits.
    snapbox::assert_data_eq!(
        collapsed.rendered_output(),
        str![[r#"
"╭┄ @ [uncommitted] (no changes)                                                                     "
"┊                                                                                                   "
"┊╭┄ g0 [B] (1 commit, 1 file)                                                                       "
"┊│                                                                                                  "
"┊├┄ h0 [A]                                                                                          "
"┊●   tpm add A                                                                                      "
"├╯                                                                                                  "
"┊                                                                                                   "
"┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M                                         "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"  normal   ↑/k up • ↓/j down • c commit • r squash • m move • b branch • s stack • ? help • q quit  "

"#]]
    );
}

#[test]
fn shift_z_collapses_every_branch_then_expands_them_all() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    let collapsed = tui.input((KeyModifiers::SHIFT, 'Z'));
    // Both stacks collapse and the cursor stays on the uncommitted area.
    snapbox::assert_data_eq!(
        collapsed.rendered_output(),
        str![[r#"
"╭┄ @ [uncommitted] (no changes)                                                                     "
"┊                                                                                                   "
"┊╭┄ g0 [A] (1 commit, 1 file)                                                                       "
"├╯                                                                                                  "
"┊                                                                                                   "
"┊╭┄ h0 [B] (1 commit, 1 file)                                                                       "
"├╯                                                                                                  "
"┊                                                                                                   "
"┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M                                         "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"  normal   ↑/k up • ↓/j down • c commit • r squash • m move • b branch • s stack • ? help • q quit  "

"#]]
    );
    collapsed.assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    let expanded = tui.input((KeyModifiers::SHIFT, 'Z'));
    // A collapsed branch exists, so the second press expands everything.
    snapbox::assert_data_eq!(
        expanded.rendered_output(),
        str![[r#"
"╭┄ @ [uncommitted] (no changes)                                                                     "
"┊                                                                                                   "
"┊╭┄ g0 [A]                                                                                          "
"┊●   tpm add A                                                                                      "
"├╯                                                                                                  "
"┊                                                                                                   "
"┊╭┄ h0 [B]                                                                                          "
"┊●   lrm add B                                                                                      "
"├╯                                                                                                  "
"┊                                                                                                   "
"┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M                                         "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"                                                                                                    "
"  normal   ↑/k up • ↓/j down • c commit • r squash • m move • b branch • s stack • ? help • q quit  "

"#]]
    );
}

#[test]
fn collapsing_from_a_commit_file_list_closes_it_and_selects_the_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Char('f')])
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input('z')
        .assert_current_line_eq(str!["┊╭┄ g0 [A] (1 commit, 1 file)"]);

    // The file list is closed, so the cursor can leave the collapsed branch.
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ h0 [B]"]);
}

#[test]
fn shift_z_ignores_collapsed_branches_that_are_no_longer_shown() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    // A branch collapsed before its stack was unapplied stays in the set.
    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            launch_options: TuiLaunchOptions {
                collapsed_branches: ["unapplied".to_owned()].into(),
                ..Default::default()
            },
            ..Default::default()
        },
    );

    // Nothing shown is collapsed, so one press collapses every shown branch.
    tui.input((KeyModifiers::SHIFT, 'Z'))
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A] (1 commit, 1 file)"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ h0 [B] (1 commit, 1 file)"]);
}

#[test]
fn collapsing_on_the_merge_base_does_nothing() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'G'))
        .assert_current_line_eq(str!["┴ 0dc3733 (common base, [..]) 2000-01-02 add M"]);

    // The merge base belongs to no branch, so neither the branch above it nor the cursor moves.
    tui.input('z')
        .assert_current_line_eq(str!["┴ 0dc3733 (common base, [..]) 2000-01-02 add M"])
        .assert_rendered_not_contains("(1 commit, 1 file)");
}

#[test]
fn collapsing_a_branch_with_only_upstream_commits_counts_them() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    // A has no commits of its own, and its remote has one that was never fetched into it.
    let upstream = env.invoke_git("commit-tree main^{tree} -p main -m upstream-only");
    env.invoke_git(&format!("update-ref refs/remotes/origin/A {upstream}"));
    env.invoke_git("config branch.A.remote origin");
    env.invoke_git("config branch.A.merge refs/heads/A");
    env.invoke_git("update-ref refs/heads/A refs/heads/main");

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A] (no commits)"]);
    // The hidden upstream commit is counted rather than reported as no commits at all.
    tui.input('z')
        .assert_current_line_eq(str!["┊╭┄ g0 [A] (0 commits, 1 upstream, 0 files)"])
        .assert_rendered_not_contains("upstream-only");
}

#[test]
fn committing_into_a_collapsed_branch_expands_it_for_the_inline_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Char('z')])
        .assert_current_line_eq(str!["┊╭┄ g0 [A] (1 commit, 1 file)"]);

    tui.env().file("test.txt", "content");
    tui.reload();

    // Pick the inline composer, as `commit_with_inline_reword` does.
    tui.input([KeyCode::Char('c'), KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄ g0 [A] (1 commit, 1 file)"]);
    tui.input(['e', 'i', 'i', 'i'].map(KeyCode::Char));

    // The new commit would be hidden, so its branch expands and the composer opens on it.
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   nsl"]);
    tui.input("commit message here")
        .assert_current_line_eq(str!["┊●   nsl commit message here"]);
}

#[test]
fn collapsing_from_an_upstream_commit_file_folds_its_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    // A's only commit, with its file, now exists just on its remote.
    let upstream = env.invoke_git("commit-tree A^{tree} -p main -m upstream-only");
    env.invoke_git(&format!("update-ref refs/remotes/origin/A {upstream}"));
    env.invoke_git("config branch.A.remote origin");
    env.invoke_git("config branch.A.merge refs/heads/A");
    env.invoke_git("update-ref refs/heads/A refs/heads/main");

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'F'));
    tui.input([KeyCode::Down; 2])
        .assert_current_line_eq(str!["┊│     A A"]);

    // The file row carries its upstream commit's ID, which still leads to the branch.
    tui.input('z')
        .assert_current_line_eq(str!["┊╭┄ g0 [A] (0 commits, 1 upstream, 0 files)"]);
}
