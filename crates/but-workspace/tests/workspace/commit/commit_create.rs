use anyhow::Result;
use but_core::DiffSpec;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _,
    mutate::{InsertSide, RelativeToRef},
};
use but_workspace::commit::{ChangeSource, commit_create};
use but_workspace::commit_engine::{Destination, create_commit};

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

fn worktree_changes_as_specs(repo: &gix::Repository) -> Result<Vec<DiffSpec>> {
    Ok(but_core::diff::worktree_changes(repo)?
        .changes
        .into_iter()
        .map(DiffSpec::from)
        .collect())
}

#[test]
fn new_commit_uses_configured_user_as_committer() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let repo = gix::init(tmp.path())?;
    but_core::git_config::edit_repo_config(&repo, gix::config::Source::Local, |config| {
        but_core::git_config::set_config_value(config, "user.name", "Configured User")?;
        but_core::git_config::set_config_value(config, "user.email", "configured@example.com")
    })?;
    std::fs::write(tmp.path().join("file"), "content")?;

    but_testsupport::isolated_app_data_dir(|| -> Result<()> {
        let ctx = but_ctx::Context::open_with_repo_open_mode(
            tmp.path(),
            but_ctx::RepoOpenMode::Isolated,
        )?;
        let repo = ctx.repo.get()?;
        let outcome = create_commit(
            &repo,
            Destination::NewCommit {
                parent_commit_id: None,
                stack_segment: None,
                message: "new commit".into(),
            },
            worktree_changes_as_specs(&repo)?,
            0,
        )?;
        let commit = repo.find_commit(outcome.new_commit.expect("a commit was created"))?;
        let committer = commit.committer()?;
        assert_eq!(
            (committer.name.to_owned(), committer.email.to_owned()),
            ("Configured User".into(), "configured@example.com".into()),
            "a fallback committer must not override the configured user identity"
        );
        Ok(())
    })
}

#[test]
fn new_commit_whose_every_change_is_rejected_is_not_made() -> Result<()> {
    use but_testsupport::{CommandExt as _, git_at_dir};
    let tmp = tempfile::tempdir()?;
    let git = || {
        let mut cmd = git_at_dir(tmp.path());
        cmd.args(["-c", "user.name=T", "-c", "user.email=t@example.com"]);
        cmd.args(["-c", "commit.gpgsign=false"]);
        cmd
    };
    git().args(["init", "-q"]).run();
    std::fs::write(tmp.path().join("file"), "one\n")?;
    git().args(["add", "file"]).run();
    git().args(["commit", "-q", "-m", "base"]).run();
    std::fs::write(tmp.path().join("file"), "first's\n")?;
    git().args(["commit", "-q", "-am", "first"]).run();
    // An edit of the line only `first` has, so it can't go on `base`.
    std::fs::write(tmp.path().join("file"), "first's, edited\n")?;

    let repo = gix::open(tmp.path())?;
    let base = repo.rev_parse_single("HEAD~1")?.detach();
    let outcome = create_commit(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(base),
            stack_segment: None,
            message: "edit".into(),
        },
        worktree_changes_as_specs(&repo)?,
        0,
    )?;
    assert_eq!(
        outcome.rejected_specs.len(),
        1,
        "picking the edit onto `base` conflicts"
    );
    assert!(
        outcome.new_commit.is_none(),
        "with every change rejected, no empty commit is made on `base`"
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn symlink_targets_are_committed_with_forward_slashes() -> Result<()> {
    use but_testsupport::{CommandExt as _, git_at_dir};
    use std::os::windows::fs::symlink_dir;

    fn commit_and_read_link(
        repo: &gix::Repository,
        specs: Vec<DiffSpec>,
    ) -> Result<gix::bstr::BString> {
        let outcome = create_commit(
            repo,
            Destination::NewCommit {
                parent_commit_id: repo.head_id().ok().map(|id| id.detach()),
                stack_segment: None,
                message: "link".into(),
            },
            specs,
            0,
        )?;
        let tree = repo
            .find_commit(outcome.new_commit.expect("a commit was created"))?
            .tree()?;
        let link = tree.find_entry("link").expect("the link was committed");
        Ok(link.object()?.detach().data.into())
    }

    let tmp = tempfile::tempdir()?;
    let git = || git_at_dir(tmp.path());
    git().args(["init", "-q"]).run();
    git().args(["config", "core.symlinks", "true"]).run();
    let link = tmp.path().join("link");
    // Windows tools such as Node's `fs.symlinkSync` write `\` into link targets.
    symlink_dir(r"a\b", &link)?;

    let repo = but_testsupport::open_repo(tmp.path())?;
    assert_eq!(
        commit_and_read_link(&repo, worktree_changes_as_specs(&repo)?)?,
        "a/b",
        "a new link is committed whole, with `/` as `git add` writes it"
    );

    git().args(["add", "link"]).run();
    git().args(["commit", "-q", "-m", "add link"]).run();
    std::fs::remove_dir(&link)?;
    symlink_dir(r"c\d", &link)?;
    let repo = but_testsupport::open_repo(tmp.path())?;
    let specs =
        crate::utils::to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?;
    assert_eq!(
        commit_and_read_link(&repo, specs)?,
        "c/d",
        "a changed link is committed by hunk, also with `/`"
    );
    Ok(())
}

#[test]
fn amend_whose_every_change_is_rejected_is_not_made() -> Result<()> {
    use but_testsupport::{CommandExt as _, git_at_dir};
    let tmp = tempfile::tempdir()?;
    let git = || {
        let mut cmd = git_at_dir(tmp.path());
        cmd.args(["-c", "user.name=T", "-c", "user.email=t@example.com"]);
        cmd.args(["-c", "commit.gpgsign=false"]);
        cmd
    };
    git().args(["init", "-q"]).run();
    git().args(["config", "user.name", "T"]).run();
    git().args(["config", "user.email", "t@example.com"]).run();
    std::fs::write(tmp.path().join("file"), "one\n")?;
    git().args(["add", "file"]).run();
    git().args(["commit", "-q", "-m", "base"]).run();
    std::fs::write(tmp.path().join("file"), "first's\n")?;
    git().args(["commit", "-q", "-am", "first"]).run();
    // An edit of the line only `first` has, so it can't be amended into `base`.
    std::fs::write(tmp.path().join("file"), "first's, edited\n")?;

    let repo = gix::open(tmp.path())?;
    let base = repo.rev_parse_single("HEAD~1")?.detach();
    let amend = |new_message: Option<&str>| {
        create_commit(
            &repo,
            Destination::AmendCommit {
                commit_id: base,
                new_message: new_message.map(Into::into),
            },
            worktree_changes_as_specs(&repo)?,
            0,
        )
    };

    let outcome = amend(None)?;
    assert_eq!(
        outcome.rejected_specs.len(),
        1,
        "the edit conflicts on `base`"
    );
    assert!(
        outcome.new_commit.is_none(),
        "with every change rejected, `base` must not be rewritten"
    );

    let outcome = amend(Some("reworded"))?;
    assert_eq!(outcome.rejected_specs.len(), 1);
    assert!(
        outcome.new_commit.is_some(),
        "a requested message change is still applied"
    );
    Ok(())
}

#[test]
fn commit_above_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    std::fs::write(
        repo.workdir_path("inserted-above-commit.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeToRef::Commit(two_id),
        InsertSide::Above,
        "insert above commit",
        0,
        ChangeSource::Head,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let selector = outcome
        .commit_selector
        .expect("a selector for the new commit");
    let materialized = outcome.rebase.materialize(Default::default())?;
    let new_commit_id = materialized.lookup_pick(selector)?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert above commit");
    assert_eq!(
        new_commit.parent_ids().next().expect("one parent"),
        two_id,
        "new commit should be based on the target commit when inserted above"
    );
    let mut two_ref = repo.find_reference("two")?;
    assert_eq!(
        two_ref.peel_to_id()?,
        new_commit_id,
        "the two reference should now point to the inserted commit"
    );

    Ok(())
}

#[test]
fn commit_below_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    let one_id = repo.rev_parse_single("one")?.detach();
    let two_id = repo.rev_parse_single("two")?.detach();
    std::fs::write(
        repo.workdir_path("inserted-below-commit.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeToRef::Commit(two_id),
        InsertSide::Below,
        "insert below commit",
        0,
        ChangeSource::Head,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let selector = outcome
        .commit_selector
        .expect("a selector for the new commit");
    let materialized = outcome.rebase.materialize(Default::default())?;
    let new_commit_id = materialized.lookup_pick(selector)?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert below commit");
    assert_eq!(
        new_commit.parent_ids().next().expect("one parent"),
        one_id,
        "new commit should be based on the target's first parent when inserted below"
    );

    Ok(())
}

#[test]
fn commit_above_reference() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    let reference = repo.find_reference("two")?;
    std::fs::write(
        repo.workdir_path("inserted-above-reference.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeToRef::Reference(reference.name()),
        InsertSide::Above,
        "insert above reference",
        0,
        ChangeSource::Head,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let selector = outcome
        .commit_selector
        .expect("a selector for the new commit");
    let materialized = outcome.rebase.materialize(Default::default())?;
    let new_commit_id = materialized.lookup_pick(selector)?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert above reference");
    assert_eq!(
        new_commit.parent_ids().next().expect("one parent"),
        two_id,
        "new commit should be based on the referenced commit"
    );
    let mut two_ref = repo.find_reference("two")?;
    assert_eq!(
        two_ref.peel_to_id()?,
        two_id,
        "when inserting above a reference, the reference keeps pointing to the original commit"
    );

    Ok(())
}

#[test]
fn commit_below_merge_commit_uses_first_parent() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
        writable_scenario("merge-with-two-branches-line-offset", |_| {})?;
    let merge_id = repo.rev_parse_single("HEAD")?.detach();
    let merge_commit = repo.find_commit(merge_id)?;
    let first_parent_id = merge_commit
        .parent_ids()
        .next()
        .expect("merge commit has parent")
        .detach();
    std::fs::write(
        repo.workdir_path("inserted-below-merge.txt")
            .expect("non-bare"),
        "inserted\n",
    )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    let outcome = commit_create(
        editor,
        worktree_changes_as_specs(&repo)?,
        RelativeToRef::Commit(merge_id),
        InsertSide::Below,
        "insert below merge",
        0,
        ChangeSource::Head,
    )?;

    assert!(outcome.rejected_specs.is_empty());
    let selector = outcome
        .commit_selector
        .expect("a selector for the new commit");
    let materialized = outcome.rebase.materialize(Default::default())?;
    let new_commit_id = materialized.lookup_pick(selector)?;

    let new_commit = repo.find_commit(new_commit_id)?;
    assert_eq!(new_commit.message_raw()?, "insert below merge");
    assert_eq!(
        new_commit.parent_ids().next().expect("has a parent"),
        first_parent_id,
        "for below merge commits, we base creation on first parent"
    );

    Ok(())
}

#[test]
fn commit_all_rejected_is_noop() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;

    let outcome = commit_create(
        editor,
        vec![DiffSpec {
            previous_path: None,
            path: "does-not-exist".into(),
            hunk_headers: vec![],
        }],
        RelativeToRef::Commit(two_id),
        InsertSide::Above,
        "no-op commit",
        0,
        ChangeSource::Head,
    )?;

    assert!(
        outcome.commit_selector.is_none(),
        "no selector if there is no new commit"
    );
    assert_eq!(
        outcome.rejected_specs.len(),
        1,
        "the invalid spec should be rejected"
    );
    assert_eq!(outcome.rejected_specs[0].1.path, "does-not-exist");

    Ok(())
}
