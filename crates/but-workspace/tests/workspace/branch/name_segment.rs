use bstr::BString;
use but_core::RefMetadata;
use but_graph::{WorktreeKind, init::Options};
use but_rebase::graph_rebase::Editor;
use but_testsupport::{CommandExt as _, git, git_at_dir, visualize_commit_graph_all};

use crate::{
    ref_info::with_workspace_commit::utils::{named_writable_scenario, project_meta},
    utils::r,
};

fn name_segment(
    repo: &gix::Repository,
    meta: &mut impl RefMetadata,
    db: &mut but_db::DbHandle,
    checkout: WorktreeKind,
    top: &str,
    ref_name: &str,
) -> anyhow::Result<()> {
    db.worktree_meta_mut().mark_adopted()?;
    let options = Options {
        worktrees: true,
        ..Options::limited()
    };
    let graph = but_graph::Graph::from_head(repo, &*meta, project_meta(repo)?, db, options)?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, meta, repo, db)?;
    let top = repo.rev_parse_single(top)?.detach();
    but_workspace::branch::name_segment(editor, &checkout, top, r(ref_name))?
        .materialize(Default::default())?;
    Ok(())
}

fn linked(name: &str) -> WorktreeKind {
    WorktreeKind::LinkedId(BString::from(name))
}

fn worktree_head(repo: &gix::Repository, name: &str) -> anyhow::Result<String> {
    let worktree = gix::open(repo.workdir().expect("non-bare").join(name))?;
    Ok(match worktree.head_name()? {
        Some(name) => name.shorten().to_string(),
        None => format!("detached at {}", worktree.head_id()?.shorten()?),
    })
}

#[test]
fn workspace_reference_without_workspace_commit() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-branch-no-ws-commit")?;
    git(&repo)
        .args(["commit", "--allow-empty", "-m", "X"])
        .run();

    name_segment(
        &repo,
        &mut meta,
        &mut db,
        WorktreeKind::Main,
        "HEAD",
        "refs/heads/named",
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 455e76a (HEAD -> gitbutler/workspace, named) X
* 3183e43 (origin/main, main) M1

"#]]
    );
    Ok(())
}

#[test]
fn stack_tip_beneath_workspace_commit() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;
    let top = repo.rev_parse_single("B")?.detach().to_string();
    git(&repo).args(["branch", "-D", "B"]).run();

    name_segment(
        &repo,
        &mut meta,
        &mut db,
        WorktreeKind::Main,
        &top,
        "refs/heads/named",
    )?;

    // `named` took the place of `B` beneath the workspace commit, which `HEAD` stays on.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* d69fe94 (named) B
* 09d8e52 (A) A
* 85efbe4 (origin/main, main) M

"#]]
    );
    Ok(())
}

#[test]
fn detached_head() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;
    git(&repo).args(["checkout", "--detach", "B"]).run();
    git(&repo)
        .args(["commit", "--allow-empty", "-m", "D"])
        .run();

    name_segment(
        &repo,
        &mut meta,
        &mut db,
        WorktreeKind::Main,
        "HEAD",
        "refs/heads/named",
    )?;

    // `HEAD` is attached to `named`, which points where it was detached.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2076060 (gitbutler/workspace) GitButler Workspace Commit
| * 95355bd (HEAD -> named) D
|/  
* d69fe94 (B) B
* 09d8e52 (A) A
* 85efbe4 (origin/main, main) M

"#]]
    );
    Ok(())
}

#[test]
fn detached_linked_worktree() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;
    git(&repo)
        .args(["worktree", "add", "--detach", "wt", "B"])
        .run();
    git_at_dir(repo.workdir().expect("non-bare").join("wt"))
        .args(["commit", "--allow-empty", "-m", "W"])
        .run();
    let top = gix::open(repo.workdir().expect("non-bare").join("wt"))?
        .head_id()?
        .to_string();

    name_segment(
        &repo,
        &mut meta,
        &mut db,
        linked("wt"),
        &top,
        "refs/heads/named",
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 5bd4ca8 (named) W
|/  
* d69fe94 (B) B
* 09d8e52 (A) A
* 85efbe4 (origin/main, main) M

"#]]
    );
    assert_eq!(
        worktree_head(&repo, "wt")?,
        "named",
        "the worktree is attached to the branch naming its segment"
    );
    Ok(())
}

#[test]
fn only_the_given_checkout_is_attached() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;
    git(&repo).args(["checkout", "--detach", "B"]).run();
    git(&repo)
        .args(["commit", "--allow-empty", "-m", "D"])
        .run();
    git(&repo)
        .args(["worktree", "add", "--detach", "wt", "HEAD"])
        .run();

    name_segment(
        &repo,
        &mut meta,
        &mut db,
        linked("wt"),
        "HEAD",
        "refs/heads/named",
    )?;

    // `HEAD` stays detached at `D`, as git can't check out `named` in two worktrees at once.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2076060 (gitbutler/workspace) GitButler Workspace Commit
| * 95355bd (HEAD, named) D
|/  
* d69fe94 (B) B
* 09d8e52 (A) A
* 85efbe4 (origin/main, main) M

"#]]
    );
    assert_eq!(
        worktree_head(&repo, "wt")?,
        "named",
        "the worktree that was asked for is attached"
    );
    Ok(())
}

#[test]
fn taken_name_is_rejected() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;
    git(&repo).args(["checkout", "--detach", "B"]).run();
    git(&repo)
        .args(["commit", "--allow-empty", "-m", "D"])
        .run();

    let err = name_segment(
        &repo,
        &mut meta,
        &mut db,
        WorktreeKind::Main,
        "HEAD",
        "refs/heads/A",
    )
    .unwrap_err();

    assert_eq!(err.to_string(), "Reference 'A' already exists");
    Ok(())
}
