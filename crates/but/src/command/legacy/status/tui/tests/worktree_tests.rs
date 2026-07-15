use bstr::ByteSlice;
use but_testsupport::Sandbox;
use crossterm::event::KeyCode;
use snapbox::file;

use super::utils::{test_tui, with_stable_commit_env};

#[test]
fn watches_the_main_and_active_linked_worktrees() {
    let env = active_worktree_env();

    assert_eq!(
        super::super::workdirs_to_watch(&env.context()).unwrap(),
        [
            env.projects_root().to_owned(),
            std::fs::canonicalize(worktree_dir(&env, "C")).unwrap(),
        ],
        "the TUI should reload for changes in every visible checkout"
    );
}

#[test]
fn adds_watchers_for_worktrees_activated_after_startup() {
    let env = active_worktree_env();
    let watched = super::super::workdirs_to_watch(&env.context()).unwrap();

    but_api::worktrees::worktree_set_archived(&mut env.context(), "A".to_owned(), false).unwrap();
    but_testsupport::invoke_bash_at_dir(
        "git worktree add .git/gitbutler/worktrees/D -b later main",
        env.projects_root(),
    );

    let added: std::collections::BTreeSet<_> = super::super::unwatched_workdirs(
        &env.context(),
        watched.iter().map(std::path::PathBuf::as_path),
    )
    .unwrap()
    .into_iter()
    .collect();
    let expected = [
        std::fs::canonicalize(worktree_dir(&env, "A")).unwrap(),
        std::fs::canonicalize(worktree_dir(&env, "D")).unwrap(),
    ]
    .into_iter()
    .collect();

    assert_eq!(
        added, expected,
        "the next TUI reload should start watchers for added and unarchived worktrees"
    );
}

#[test]
fn linked_worktree_shows_uncommitted_changes() {
    let env = active_worktree_env();
    std::fs::write(worktree_dir(&env, "C").join("dirty.txt"), "dirty\n").unwrap();

    let mut tui = test_tui(env);

    tui.reload()
        .assert_rendered_term_svg_eq(file![
            "snapshots/linked_worktree_shows_uncommitted_changes.svg"
        ])
        .assert_rendered_contains("dirty.txt")
        .assert_rendered_contains("feat")
        .assert_rendered_contains(".git/gitbutler/worktrees/C");
}

#[test]
fn linked_worktree_uncommitted_changes_can_be_rubbed_into_a_commit() {
    let env = active_worktree_env();
    let worktree = worktree_dir(&env, "C");
    std::fs::write(worktree.join("dirty.txt"), "dirty\n").unwrap();
    let worktree_id = active_worktree_cli_id(&env, "C");
    let target = env.open_repo().rev_parse_single("B").unwrap().detach();
    let target_short = short_sha(target);
    let mut tui = test_tui(env);

    jump_to(&mut tui, &worktree_id);
    tui.input('r').assert_rendered_contains("<< source >>");
    jump_to(&mut tui, &target_short);
    tui.input(KeyCode::Enter);

    assert_eq!(
        but_testsupport::git_status_at_dir(&worktree).unwrap(),
        "",
        "rubbing the worktree should consume its uncommitted changes"
    );
    assert_eq!(
        tui.env()
            .open_repo()
            .rev_parse_single("B:dirty.txt")
            .unwrap()
            .object()
            .unwrap()
            .data,
        b"dirty\n",
        "the selected commit should contain the linked-worktree change"
    );
    tui.reload()
        .assert_rendered_term_svg_eq(file![
            "snapshots/linked_worktree_uncommitted_changes_can_be_rubbed_into_a_commit_final.svg"
        ])
        .assert_rendered_not_contains("dirty.txt");
}

#[test]
fn linked_worktree_commits_can_be_squashed() {
    let (env, first, second) = worktree_with_two_commits();
    let first_short = short_sha(first);
    let second_short = short_sha(second);
    let mut tui = test_tui(env);

    jump_to(&mut tui, &second_short);
    tui.input('r').assert_current_line_eq(format!(
        "┊●   << source >> << noop >> {second_short} worktree two"
    ));
    jump_to(&mut tui, &first_short);
    tui.input(KeyCode::Enter);

    assert_eq!(
        tui.env().invoke_git("rev-list --count main..feat"),
        "1",
        "squashing linked-worktree commits should leave one commit"
    );
    tui.reload().assert_rendered_contains("worktree one");
}

#[test]
fn linked_worktree_commit_can_move_relative_to_another_commit() {
    let (env, first, second) = worktree_with_two_commits();
    let first_short = short_sha(first);
    let second_short = short_sha(second);
    let mut tui = test_tui(env);

    jump_to(&mut tui, &first_short);
    tui.input('m').assert_current_line_eq(format!(
        "┊●   << source >> << noop >> {first_short} worktree one"
    ));
    jump_to(&mut tui, &second_short);
    tui.input(KeyCode::Enter);

    assert_eq!(
        tui.env().invoke_git("log --format=%s -2 feat"),
        "worktree one\nworktree two",
        "moving the lower worktree commit above the tip should reorder the branch"
    );
    tui.reload().assert_rendered_contains("worktree one");
}

#[test]
fn linked_worktree_commit_can_move_to_a_branch() {
    let (env, _first, second) = worktree_with_two_commits();
    let second_short = short_sha(second);
    let mut tui = test_tui(env);

    jump_to(&mut tui, &second_short);
    tui.input('m').assert_current_line_eq(format!(
        "┊●   << source >> << noop >> {second_short} worktree two"
    ));
    tui.input('t');
    tui.input("B");
    tui.input(KeyCode::Enter)
        .assert_rendered_contains("<< move commit to branch >>");
    tui.input(KeyCode::Enter);

    assert_eq!(
        tui.env().invoke_git("log --format=%s -1 B"),
        "worktree two",
        "moving a linked-worktree commit to B should update B"
    );
    tui.reload().assert_rendered_contains("worktree two");
}

#[test]
fn linked_worktree_can_receive_a_moved_commit() {
    let env = active_worktree_env();
    let worktree_id = active_worktree_cli_id(&env, "C");
    let source = env.open_repo().rev_parse_single("B").unwrap().detach();
    let source_short = short_sha(source);
    let mut tui = test_tui(env);

    jump_to(&mut tui, &source_short);
    tui.input('m').assert_current_line_eq(format!(
        "┊●   << source >> << noop >> {source_short} B (no changes)"
    ));
    tui.input('/');
    tui.input(worktree_id)
        .assert_rendered_contains("<< move commit to worktree >>");
    tui.input(KeyCode::Enter);

    assert_eq!(
        tui.env().invoke_git("log --format=%s -1 feat"),
        "B",
        "moving a commit to the worktree should update its checked-out branch"
    );
}

#[test]
fn linked_worktree_move_rejects_branch_switch_after_render() {
    let env = active_worktree_env();
    let worktree_id = active_worktree_cli_id(&env, "C");
    let source = env.open_repo().rev_parse_single("B").unwrap().detach();
    let source_short = short_sha(source);
    let mut tui = test_tui(env);

    jump_to(&mut tui, &source_short);
    tui.input('m');
    but_testsupport::invoke_bash_at_dir("git switch -qc other main", &worktree_dir(tui.env(), "C"));
    let feat_before = tui.env().invoke_git("rev-parse feat");
    let other_before = tui.env().invoke_git("rev-parse other");
    tui.input('/');
    tui.input(worktree_id)
        .assert_rendered_contains("<< move commit to worktree >>");

    tui.input(KeyCode::Enter)
        .assert_rendered_contains("worktree C changed branches from feat to other");
    assert_eq!(tui.env().invoke_git("rev-parse feat"), feat_before);
    assert_eq!(tui.env().invoke_git("rev-parse other"), other_before);
}

#[test]
fn detached_worktree_header_is_unavailable_for_rub_and_move() {
    let env = active_worktree_env();
    but_testsupport::invoke_bash_at_dir("git checkout -q --detach", &worktree_dir(&env, "C"));
    let worktree_id = active_worktree_cli_id(&env, "C");
    let source = env.open_repo().rev_parse_single("B").unwrap().detach();
    let source_short = short_sha(source);
    let mut tui = test_tui(env);

    jump_to(&mut tui, &worktree_id);
    tui.input('r').assert_rendered_not_contains("<< source >>");

    jump_to(&mut tui, &source_short);
    tui.input('m')
        .assert_rendered_not_contains("move commit to worktree");
}

fn active_worktree_env() -> Sandbox {
    let mut env = Sandbox::init_scenario_with_target_and_default_settings_slow("two-worktrees");
    env.setup_metadata(&["A", "B"]);
    env.set_worktree_manipulation(true);
    env.context().worktrees_with_state().unwrap();
    but_testsupport::invoke_bash_at_dir(
        "git worktree add .git/gitbutler/worktrees/C -b feat main",
        env.projects_root(),
    );
    env
}

fn worktree_with_two_commits() -> (Sandbox, gix::ObjectId, gix::ObjectId) {
    let env = active_worktree_env();
    let worktree = worktree_dir(&env, "C");
    with_stable_commit_env(|| {
        but_testsupport::invoke_bash_at_dir(
            r#"printf 'one\n' > one.txt
git add one.txt
git commit -qm 'worktree one'
printf 'two\n' > two.txt
git add two.txt
git commit -qm 'worktree two'"#,
            &worktree,
        );
    });
    let repo = env.open_repo();
    let second = repo.rev_parse_single("feat").unwrap().detach();
    let first = repo.rev_parse_single("feat~1").unwrap().detach();
    drop(repo);
    (env, first, second)
}

fn worktree_dir(env: &Sandbox, name: &str) -> std::path::PathBuf {
    env.projects_root()
        .join(".git/gitbutler/worktrees")
        .join(name)
}

fn active_worktree_cli_id(env: &Sandbox, name: &str) -> String {
    let ctx = env.context();
    let guard = ctx.shared_worktree_access();
    crate::IdMap::new_from_context(&ctx, None, guard.read_permission())
        .unwrap()
        .resolve_worktree(name.as_bytes().as_bstr())
        .unwrap()
        .to_short_string()
}

fn short_sha(id: gix::ObjectId) -> String {
    id.to_string()[..7].to_owned()
}

fn jump_to(tui: &mut super::utils::TestTui, short_sha: &str) {
    tui.input('/');
    tui.input(short_sha);
}
