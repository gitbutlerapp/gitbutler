use bstr::ByteSlice;
use but_testsupport::Sandbox;
use crossterm::event::KeyCode;
use snapbox::file;
use temp_env::with_var;

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
fn linked_worktree_file_can_be_committed_from_the_tui() {
    let env = active_worktree_env();
    let worktree = worktree_dir(&env, "C");
    std::fs::write(worktree.join("dirty.txt"), "dirty\n").unwrap();
    let file_id = format!("{}:dirty.txt", active_worktree_cli_id(&env, "C"));
    let editor = test_editor(&env, "linked worktree file");
    let mut tui = test_tui(env);

    jump_to(&mut tui, &file_id);
    with_var("GIT_EDITOR", Some(editor), || {
        tui.input('c');
    });

    assert_eq!(
        but_testsupport::git_status_at_dir(&worktree).unwrap(),
        "",
        "committing the selected linked-worktree file should consume it"
    );
    assert_eq!(
        tui.env().invoke_git("log -1 --format=%s feat"),
        "linked worktree file"
    );
    assert_eq!(
        tui.env()
            .open_repo()
            .rev_parse_single("feat:dirty.txt")
            .unwrap()
            .object()
            .unwrap()
            .data,
        b"dirty\n"
    );
    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/linked_worktree_file_can_be_committed_from_the_tui_final.svg"
    ]);
}

#[test]
fn marked_linked_worktree_hunks_can_be_committed_together() {
    let env = active_worktree_env();
    let fixture = three_hunk_change(&env);
    let editor = test_editor(&env, "two selected worktree hunks");
    let mut tui = test_tui(env);

    mark_first_two_hunks(&mut tui, &fixture);
    with_var("GIT_EDITOR", Some(editor), || {
        tui.input('c');
    });

    assert_eq!(
        tui.env()
            .open_repo()
            .rev_parse_single("feat:three-hunks.txt")
            .unwrap()
            .object()
            .unwrap()
            .data,
        fixture.first_two_changed.as_bytes(),
        "the new commit should contain both marked hunks and no others"
    );
    assert_eq!(
        std::fs::read_to_string(&fixture.path).unwrap(),
        fixture.all_changed,
        "the unselected third hunk should remain in the linked checkout"
    );
    tui.reload()
        .assert_rendered_contains("three-hunks.txt")
        .assert_rendered_term_svg_eq(file![
            "snapshots/marked_linked_worktree_hunks_can_be_committed_together_final.svg"
        ]);
}

#[test]
fn selected_linked_worktree_hunk_can_be_committed_from_details() {
    let env = active_worktree_env();
    let fixture = three_hunk_change(&env);
    let editor = test_editor(&env, "one selected worktree hunk");
    let mut tui = test_tui(env);

    jump_to(&mut tui, &fixture.file_id);
    tui.input('d');
    tui.input('l');
    tui.input('j')
        .assert_selected_details_cli_id(&fixture.first_hunk_id);
    with_var("GIT_EDITOR", Some(editor), || {
        tui.input('c');
    });

    assert_eq!(
        tui.env()
            .open_repo()
            .rev_parse_single("feat:three-hunks.txt")
            .unwrap()
            .object()
            .unwrap()
            .data,
        fixture.first_changed.as_bytes(),
        "the new commit should contain only the selected details hunk"
    );
    assert_eq!(
        std::fs::read_to_string(&fixture.path).unwrap(),
        fixture.all_changed,
        "the other two hunks should remain in the linked checkout"
    );
    tui.reload()
        .assert_rendered_contains("three-hunks.txt")
        .assert_rendered_term_svg_eq(file![
            "snapshots/selected_linked_worktree_hunk_can_be_committed_from_details_final.svg"
        ]);
}

#[test]
fn marked_linked_worktree_hunks_can_be_rubbed_together() {
    let env = active_worktree_env();
    let fixture = three_hunk_change(&env);
    let mut tui = test_tui(env);

    mark_first_two_hunks(&mut tui, &fixture);
    tui.input('r').assert_in_rub_mode();
    jump_to(&mut tui, &fixture.target_short);
    tui.input(KeyCode::Enter);

    assert_eq!(
        tui.env()
            .open_repo()
            .rev_parse_single("feat:three-hunks.txt")
            .unwrap()
            .object()
            .unwrap()
            .data,
        fixture.first_two_changed.as_bytes(),
        "the target commit should receive both marked hunks and no others"
    );
    assert_eq!(
        std::fs::read_to_string(&fixture.path).unwrap(),
        fixture.all_changed,
        "the unselected third hunk should remain in the linked checkout"
    );
    tui.reload()
        .assert_rendered_contains("three-hunks.txt")
        .assert_rendered_term_svg_eq(file![
            "snapshots/marked_linked_worktree_hunks_can_be_rubbed_together_final.svg"
        ]);
}

#[test]
fn selected_linked_worktree_hunk_can_be_discarded_from_details() {
    let env = active_worktree_env();
    let fixture = three_hunk_change(&env);
    let mut tui = test_tui(env);

    jump_to(&mut tui, &fixture.file_id);
    tui.input('d');
    tui.input('l');
    tui.input('j')
        .assert_selected_details_cli_id(&fixture.first_hunk_id);
    tui.input('x')
        .assert_rendered_contains("Discard hunk")
        .assert_rendered_contains(&fixture.first_hunk_id);
    tui.input('y');

    assert_eq!(
        std::fs::read_to_string(&fixture.path).unwrap(),
        fixture.last_two_changed,
        "discarding the selected hunk should leave the other two hunks untouched"
    );
    tui.reload()
        .assert_rendered_contains("three-hunks.txt")
        .assert_rendered_term_svg_eq(file![
            "snapshots/selected_linked_worktree_hunk_can_be_discarded_from_details_final.svg"
        ]);
}

#[test]
fn marked_linked_worktree_hunks_can_be_discarded_together() {
    let env = active_worktree_env();
    let fixture = three_hunk_change(&env);
    let mut tui = test_tui(env);

    mark_first_two_hunks(&mut tui, &fixture);
    tui.input('x').assert_rendered_contains("Discard hunks?");
    tui.input('y');

    assert_eq!(
        std::fs::read_to_string(&fixture.path).unwrap(),
        fixture.last_changed,
        "discarding marked hunks should leave the unmarked third hunk untouched"
    );
    tui.reload()
        .assert_rendered_contains("three-hunks.txt")
        .assert_rendered_term_svg_eq(file![
            "snapshots/marked_linked_worktree_hunks_can_be_discarded_together_final.svg"
        ]);
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
    tui.input(KeyCode::Enter);
    tui.input('r').assert_in_rub_mode();
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
fn selected_linked_worktree_hunk_can_be_rubbed_from_details() {
    let env = active_worktree_env();
    let worktree = worktree_dir(&env, "C");
    let filler = "line\n".repeat((env.app_settings().context_lines * 2 + 2) as usize);
    let baseline = format!("first\n{filler}last\n");
    let both_changed = format!("first changed\n{filler}last changed\n");
    let target_contents = format!("first changed\n{filler}last\n");

    std::fs::write(worktree.join("two-hunks.txt"), &baseline).unwrap();
    but_testsupport::invoke_bash_at_dir(
        "git add -- two-hunks.txt && git commit -qm 'worktree baseline'",
        &worktree,
    );
    std::fs::write(worktree.join("two-hunks.txt"), both_changed).unwrap();

    let file_id = format!("{}:two-hunks.txt", active_worktree_cli_id(&env, "C"));
    let target = env.open_repo().rev_parse_single("feat").unwrap().detach();
    let target_short = short_sha(target);
    let first_hunk_id = format!("{file_id}:#0");
    let second_hunk_id = format!("{file_id}:#1");
    let mut tui = test_tui(env);

    jump_to(&mut tui, &file_id);
    tui.input('d');
    tui.input('l');
    tui.input('j')
        .assert_rendered_contains(&first_hunk_id)
        .assert_rendered_contains(&second_hunk_id)
        .assert_selected_details_cli_id(&first_hunk_id)
        .assert_rendered_term_svg_eq(file![
            "snapshots/selected_linked_worktree_hunk_can_be_rubbed_from_details_001.svg"
        ]);
    tui.input('r').assert_in_rub_mode();
    jump_to(&mut tui, &target_short);
    tui.input(KeyCode::Enter);

    assert_eq!(
        tui.env()
            .open_repo()
            .rev_parse_single("feat:two-hunks.txt")
            .unwrap()
            .object()
            .unwrap()
            .data,
        target_contents.as_bytes(),
        "only the selected worktree hunk is amended into the commit"
    );
    let diff = but_testsupport::git_at_dir(&worktree)
        .args(["diff", "HEAD", "--unified=0", "--", "two-hunks.txt"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&diff.stdout).contains("-last\n+last changed"),
        "the unselected linked-worktree hunk remains dirty"
    );
    tui.reload()
        .assert_rendered_contains("two-hunks.txt")
        .assert_rendered_not_contains(&first_hunk_id)
        .assert_rendered_term_svg_eq(file![
            "snapshots/selected_linked_worktree_hunk_can_be_rubbed_from_details_final.svg"
        ]);
}

#[test]
fn selected_worktree_commit_hunk_can_be_rubbed_to_workspace_from_details() {
    let env = with_stable_commit_env(active_worktree_env);
    let worktree = worktree_dir(&env, "C");
    let filler = "line\n".repeat((env.app_settings().context_lines * 2 + 2) as usize);
    let baseline = format!("first\n{filler}last\n");
    let both_changed = format!("first changed\n{filler}last changed\n");
    let first_changed = format!("first changed\n{filler}last\n");
    let second_changed = format!("first\n{filler}last changed\n");

    env.invoke_git("config user.name 'TUI Test'");
    env.invoke_git("config user.email tui@example.com");
    env.invoke_git("config gitoxide.commit.authorDate '2000-01-01 00:00:00 +0000'");
    env.invoke_git("config gitoxide.commit.committerDate '2000-01-01 00:00:00 +0000'");
    env.file("two-hunks.txt", &baseline);
    workspace_commit(&env, "B", "workspace baseline");
    std::fs::write(worktree.join("two-hunks.txt"), &baseline).unwrap();
    but_testsupport::invoke_bash_at_dir(
        "git add -- two-hunks.txt && git commit -qm 'worktree baseline'",
        &worktree,
    );
    std::fs::write(worktree.join("two-hunks.txt"), &both_changed).unwrap();
    but_testsupport::invoke_bash_at_dir(
        "git add -- two-hunks.txt && git commit -qm 'worktree two-hunk source'",
        &worktree,
    );

    let source = env.open_repo().rev_parse_single("feat").unwrap().detach();
    let source_short = short_sha(source);
    let first_hunk_id = committed_hunk_cli_id(&env, source, 0);
    let mut tui = test_tui(env);

    jump_to(&mut tui, &source_short);
    tui.input('d');
    tui.input('l');
    tui.input('j')
        .assert_selected_details_cli_id(&first_hunk_id)
        .assert_rendered_term_svg_eq(file![
            "snapshots/selected_worktree_commit_hunk_can_be_rubbed_to_workspace_from_details_001.svg"
        ]);
    tui.input('r').assert_in_rub_mode();
    tui.input([KeyCode::Up, KeyCode::Up])
        .assert_rendered_contains("workspace baseline");
    tui.input(KeyCode::Enter);

    assert_eq!(
        tui.env()
            .open_repo()
            .rev_parse_single("feat:two-hunks.txt")
            .unwrap()
            .object()
            .unwrap()
            .data,
        second_changed.as_bytes(),
        "the source worktree commit retains only the unselected hunk"
    );
    assert_eq!(
        tui.env()
            .open_repo()
            .rev_parse_single("B:two-hunks.txt")
            .unwrap()
            .object()
            .unwrap()
            .data,
        first_changed.as_bytes(),
        "the workspace commit receives only the selected hunk"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.join("two-hunks.txt")).unwrap(),
        second_changed
    );
    assert_eq!(
        std::fs::read_to_string(tui.env().projects_root().join("two-hunks.txt")).unwrap(),
        first_changed
    );
    assert_eq!(but_testsupport::git_status_at_dir(&worktree).unwrap(), "");
    assert_eq!(tui.env().git_status(), "");
    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/selected_worktree_commit_hunk_can_be_rubbed_to_workspace_from_details_final.svg"
    ]);
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

struct ThreeHunkChange {
    path: std::path::PathBuf,
    file_id: String,
    first_hunk_id: String,
    second_hunk_id: String,
    third_hunk_id: String,
    target_short: String,
    first_changed: String,
    first_two_changed: String,
    last_changed: String,
    last_two_changed: String,
    all_changed: String,
}

fn three_hunk_change(env: &Sandbox) -> ThreeHunkChange {
    let worktree = worktree_dir(env, "C");
    let filler = "line\n".repeat((env.app_settings().context_lines * 2 + 2) as usize);
    let baseline = format!("first\n{filler}middle\n{filler}last\n");
    let first_changed = format!("first changed\n{filler}middle\n{filler}last\n");
    let first_two_changed = format!("first changed\n{filler}middle changed\n{filler}last\n");
    let last_changed = format!("first\n{filler}middle\n{filler}last changed\n");
    let last_two_changed = format!("first\n{filler}middle changed\n{filler}last changed\n");
    let all_changed = format!("first changed\n{filler}middle changed\n{filler}last changed\n");
    let path = worktree.join("three-hunks.txt");
    std::fs::write(&path, baseline).unwrap();
    with_stable_commit_env(|| {
        but_testsupport::invoke_bash_at_dir(
            "git add -- three-hunks.txt && git commit -qm 'three-hunk baseline'",
            &worktree,
        );
    });
    std::fs::write(&path, &all_changed).unwrap();

    let file_id = format!("{}:three-hunks.txt", active_worktree_cli_id(env, "C"));
    let target = env.open_repo().rev_parse_single("feat").unwrap().detach();
    ThreeHunkChange {
        path,
        first_hunk_id: format!("{file_id}:#0"),
        second_hunk_id: format!("{file_id}:#1"),
        third_hunk_id: format!("{file_id}:#2"),
        file_id,
        target_short: short_sha(target),
        first_changed,
        first_two_changed,
        last_changed,
        last_two_changed,
        all_changed,
    }
}

fn mark_first_two_hunks(tui: &mut super::utils::TestTui, fixture: &ThreeHunkChange) {
    jump_to(tui, &fixture.file_id);
    tui.input('d');
    tui.input('l');
    tui.input('j')
        .assert_selected_details_cli_id(&fixture.first_hunk_id);
    tui.input(' ')
        .assert_selected_details_cli_id(&fixture.second_hunk_id);
    tui.input(' ')
        .assert_selected_details_cli_id(&fixture.third_hunk_id);
}

fn test_editor(env: &Sandbox, message: &str) -> String {
    env.file(
        ".git/editor.sh",
        format!("printf '{message}\\n' > \"$1\"\n"),
    );
    format!(
        "sh {}",
        env.projects_root().join(".git/editor.sh").display()
    )
}

fn workspace_commit(env: &Sandbox, branch: &str, message: &str) -> gix::ObjectId {
    let mut ctx = env.context();
    let changes = but_api::diff::changes_in_worktree(&ctx, false)
        .unwrap()
        .worktree_changes
        .changes
        .into_iter()
        .map(but_core::TreeChange::from)
        .map(but_core::DiffSpec::from)
        .collect();
    but_api::commit::create::commit_create_only(
        &mut ctx,
        but_rebase::graph_rebase::mutate::RelativeTo::Reference(
            gix::refs::FullName::try_from(format!("refs/heads/{branch}")).unwrap(),
        ),
        but_rebase::graph_rebase::mutate::InsertSide::Below,
        changes,
        message.to_owned(),
        but_core::DryRun::No,
    )
    .unwrap()
    .new_commit
    .expect("the workspace change creates a commit")
}

fn committed_hunk_cli_id(env: &Sandbox, commit_id: gix::ObjectId, index: usize) -> String {
    let mut ctx = env.context();
    let id_map = crate::IdMap::legacy_new_from_context(&ctx, None).unwrap();
    let mut matches = id_map
        .parse_using_context(
            &format!("{}:two-hunks.txt:#{index}", short_sha(commit_id)),
            &mut ctx,
        )
        .unwrap();
    assert_eq!(matches.len(), 1);
    matches.remove(0).to_short_string()
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
