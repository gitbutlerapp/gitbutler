use std::collections::HashMap;

use anyhow::Result;
use but_ctx::Context;
use gitbutler_branch_actions::{StackOrder, reorder::SeriesOrder, reorder_stack};
use gitbutler_commit::commit_ext::CommitMessageBstr as _;
use gitbutler_stack::VirtualBranchesHandle;
use itertools::Itertools;
use tempfile::TempDir;

use crate::{driverless, support};

#[test]
fn noop_reorder_errors() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![
            test_ctx.top_commits["commit 6"],
            test_ctx.top_commits["commit 5"],
            test_ctx.top_commits["commit 4"],
        ],
        vec![
            test_ctx.bottom_commits["commit 3"],
            test_ctx.bottom_commits["commit 2"],
            test_ctx.bottom_commits["commit 1"],
        ],
    ]);
    let result = reorder_stack(&mut ctx, test_ctx.stack.id, order);
    assert_eq!(
        result.unwrap_err().to_string(),
        "The new order is the same as the current order"
    );
    Ok(())
}

#[test]
fn reorder_in_top_series() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![
            test_ctx.top_commits["commit 6"],
            test_ctx.top_commits["commit 4"], // currently 5
            test_ctx.top_commits["commit 5"], // currently 4
        ],
        vec![
            test_ctx.bottom_commits["commit 3"],
            test_ctx.bottom_commits["commit 2"],
            test_ctx.bottom_commits["commit 1"],
        ],
    ]);
    reorder_stack(&mut ctx, test_ctx.stack.id, order.clone())?;
    let commits = vb_commits(&ctx);

    // Verify the commit messages and ids in the second (top) series - top-series
    assert_eq!(commits[0].msgs(), vec!["commit 6", "commit 4", "commit 5"]);
    assert_ne!(commits[0].ids()[0], order.series[0].commit_ids[0]);
    assert_ne!(commits[0].ids()[1], order.series[0].commit_ids[1]);
    assert_ne!(commits[0].ids()[2], order.series[0].commit_ids[2]);

    // Verify the commit messages and ids in the first (bottom) series
    assert_eq!(commits[1].msgs(), vec!["commit 3", "commit 2", "commit 1"]);
    assert_eq!(commits[1].ids(), order.series[1].commit_ids);
    Ok(())
}

#[test]
fn reorder_in_top_series_head() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![
            test_ctx.top_commits["commit 5"], // currently 6
            test_ctx.top_commits["commit 6"], // currently 5
            test_ctx.top_commits["commit 4"],
        ],
        vec![
            test_ctx.bottom_commits["commit 3"],
            test_ctx.bottom_commits["commit 2"],
            test_ctx.bottom_commits["commit 1"],
        ],
    ]);
    reorder_stack(&mut ctx, test_ctx.stack.id, order.clone())?;
    let commits = vb_commits(&ctx);

    // Verify the commit messages and ids in the second (top) series - top-series
    assert_eq!(commits[0].msgs(), vec!["commit 5", "commit 6", "commit 4"]);
    assert_ne!(commits[0].ids()[0], order.series[0].commit_ids[0]);
    assert_ne!(commits[0].ids()[1], order.series[0].commit_ids[1]);
    assert_eq!(commits[0].ids()[2], order.series[0].commit_ids[2]); // not rebased from here down

    // Verify the commit messages and ids in the first (bottom) series
    assert_eq!(commits[1].msgs(), vec!["commit 3", "commit 2", "commit 1"]);
    assert_eq!(commits[1].ids(), order.series[1].commit_ids);
    Ok(())
}

#[test]
fn reorder_between_series() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![
            test_ctx.top_commits["commit 6"],
            test_ctx.top_commits["commit 5"],
            test_ctx.bottom_commits["commit 2"], // from the bottom series
            test_ctx.top_commits["commit 4"],
        ],
        vec![
            test_ctx.bottom_commits["commit 3"],
            test_ctx.bottom_commits["commit 1"],
        ],
    ]);
    reorder_stack(&mut ctx, test_ctx.stack.id, order.clone())?;
    let commits = vb_commits(&ctx);

    // Verify the commit messages and ids in the second (top) series - top-series
    assert_eq!(
        commits[0].msgs(),
        vec!["commit 6", "commit 5", "commit 2", "commit 4"]
    );
    for i in 0..3 {
        assert_ne!(commits[0].ids()[i], order.series[0].commit_ids[i]); // all in the top series are rebased
    }

    // Verify the commit messages and ids in the first (bottom) series
    assert_eq!(commits[1].msgs(), vec!["commit 3", "commit 1"]);
    assert_ne!(commits[1].ids()[0], order.series[1].commit_ids[0]);
    assert_eq!(commits[1].ids()[1], order.series[1].commit_ids[1]); // the bottom most commit is the same
    Ok(())
}

#[test]
fn reorder_series_head_to_another_series() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![
            test_ctx.top_commits["commit 6"],
            test_ctx.top_commits["commit 5"],
            test_ctx.bottom_commits["commit 3"],
            test_ctx.top_commits["commit 4"],
        ],
        vec![
            test_ctx.bottom_commits["commit 2"],
            test_ctx.bottom_commits["commit 1"],
        ],
    ]);
    reorder_stack(&mut ctx, test_ctx.stack.id, order.clone())?;
    let commits = vb_commits(&ctx);

    // Verify the commit messages and ids in the second (top) series - top-series
    assert_eq!(
        commits[0].msgs(),
        vec!["commit 6", "commit 5", "commit 3", "commit 4"]
    );
    for i in 0..3 {
        assert_ne!(commits[0].ids()[i], order.series[0].commit_ids[i]); // all in the top series are rebased
    }

    // Verify the commit messages and ids in the first (bottom) series
    assert_eq!(commits[1].msgs(), vec!["commit 2", "commit 1"]);
    assert_eq!(commits[1].ids()[0], order.series[1].commit_ids[0]);
    assert_eq!(commits[1].ids()[1], order.series[1].commit_ids[1]);
    Ok(())
}

#[test]
fn reorder_stack_head_to_another_series() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![
            test_ctx.top_commits["commit 5"],
            test_ctx.top_commits["commit 4"],
        ],
        vec![
            test_ctx.top_commits["commit 6"], // from the top series
            test_ctx.bottom_commits["commit 3"],
            test_ctx.bottom_commits["commit 2"],
            test_ctx.bottom_commits["commit 1"],
        ],
    ]);
    reorder_stack(&mut ctx, test_ctx.stack.id, order.clone())?;
    let commits = vb_commits(&ctx);

    // Verify the commit messages and ids in the second (top) series - top-series
    assert_eq!(commits[0].msgs(), vec!["commit 5", "commit 4"]);
    for i in 0..2 {
        assert_ne!(commits[0].ids()[i], order.series[0].commit_ids[i]); // all in the top series are rebased
    }

    // Verify the commit messages and ids in the first (bottom) series
    assert_eq!(
        commits[1].msgs(),
        vec!["commit 6", "commit 3", "commit 2", "commit 1"]
    );
    assert_ne!(commits[1].ids()[0], order.series[1].commit_ids[0]);
    assert_eq!(commits[1].ids()[1], order.series[1].commit_ids[1]);
    assert_eq!(commits[1].ids()[2], order.series[1].commit_ids[2]);
    assert_eq!(commits[1].ids()[3], order.series[1].commit_ids[3]);
    Ok(())
}

#[test]
fn reorder_shift_last_in_series_to_previous() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![
            test_ctx.top_commits["commit 6"],
            test_ctx.top_commits["commit 5"],
        ],
        vec![
            test_ctx.top_commits["commit 4"], // from the top series
            test_ctx.bottom_commits["commit 3"],
            test_ctx.bottom_commits["commit 2"],
            test_ctx.bottom_commits["commit 1"],
        ],
    ]);
    reorder_stack(&mut ctx, test_ctx.stack.id, order.clone())?;
    let commits = vb_commits(&ctx);

    // Verify the commit messages and ids in the second (top) series - top-series
    assert_eq!(commits[0].msgs(), vec!["commit 6", "commit 5"]);
    assert_eq!(commits[0].ids(), order.series[0].commit_ids); // nothing was rebased

    // Verify the commit messages and ids in the first (bottom) series
    assert_eq!(
        commits[1].msgs(),
        vec!["commit 4", "commit 3", "commit 2", "commit 1"]
    );
    assert_eq!(commits[1].ids(), order.series[1].commit_ids); // nothing was rebased
    Ok(())
}

#[test]
fn reorder_stack_making_top_empty_series() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("multiple-commits-small")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![],
        vec![
            test_ctx.top_commits["commit 2"], // from the top series
            test_ctx.bottom_commits["commit 1"],
        ],
    ]);
    reorder_stack(&mut ctx, test_ctx.stack.id, order.clone())?;
    let commits = vb_commits(&ctx);

    // Verify the commit messages and ids in the second (top) series - top-series
    assert!(commits[0].msgs().is_empty());
    assert!(commits[0].ids().is_empty());

    // Verify the commit messages and ids in the first (bottom) series
    assert_eq!(commits[1].msgs(), vec!["commit 2", "commit 1"]);
    assert_eq!(commits[1].ids(), order.series[1].commit_ids); // nothing was rebased
    Ok(())
}

#[test]
fn reorder_stack_making_bottom_empty_series() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("multiple-commits-small")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![
            test_ctx.top_commits["commit 2"],
            test_ctx.bottom_commits["commit 1"], // from the bottom series
        ],
        vec![],
    ]);
    reorder_stack(&mut ctx, test_ctx.stack.id, order.clone())?;
    let commits = vb_commits(&ctx);

    // Verify the commit messages and ids in the second (top) series - top-series
    assert_eq!(commits[0].msgs(), vec!["commit 2", "commit 1"]);
    assert_eq!(commits[0].ids(), order.series[0].commit_ids); // nothing was rebased

    Ok(())
}

#[test]
fn reorder_stack_into_empty_top() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("multiple-commits-empty-top")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![
            test_ctx.bottom_commits["commit 1"], // from the bottom series
        ],
        vec![],
    ]);
    reorder_stack(&mut ctx, test_ctx.stack.id, order.clone())?;
    let commits = vb_commits(&ctx);

    // Verify the commit messages and ids in the second (top) series - top-series
    assert_eq!(commits[0].msgs(), vec!["commit 1"]);
    assert_eq!(commits[0].ids(), order.series[0].commit_ids); // nothing was rebased

    Ok(())
}

#[test]
fn conflicting_reorder_stack() -> Result<()> {
    // Before:        : After:          :
    // commit 2: y    : commit 1': x    :
    // |              :                 :
    // commit 1: x    : commit 2': a    : <- commit 2' is the auto-resolved tree (conflicted)
    // |              :                 :
    // MB:       a    : MB:        a    :

    let (mut ctx, _temp_dir) = command_ctx("overlapping-commits")?;
    let test = test_ctx(&ctx)?;

    // There is a stack of 2:
    // [] <- top-series
    // [  <- my_stack
    //   commit 2,
    //   commit 1
    // ]
    let commits = vb_commits(&ctx);

    // Verify the initial order
    assert_eq!(commits[1].msgs(), vec!["commit 2", "commit 1"]);
    assert_eq!(commits[1].conflicted(), vec![false, false]); // no conflicts
    assert_eq!(file(&ctx, test.stack.head_oid(&ctx)?), "y\n"); // y is the last version
    assert!(commits[1].timestamps().windows(2).all(|w| w[0] >= w[1])); // commit timestamps in descending order

    // Reorder the stack in a way that will cause a conflict
    let new_order = order(vec![
        vec![],
        vec![
            test.bottom_commits["commit 1"], // swapping 1 and 2
            test.bottom_commits["commit 2"],
        ],
    ]);
    reorder_stack(&mut ctx, test.stack.id, new_order.clone())?;
    let test = test_ctx(&ctx)?;
    let commits = vb_commits(&ctx);

    // Verify that the commits are now in the updated order
    assert_eq!(commits[1].msgs(), vec!["commit 1", "commit 2"]); // swapped
    assert_eq!(commits[1].conflicted(), vec![false, true]); // bottom commit is now conflicted
    assert_eq!(file(&ctx, test.stack.head_oid(&ctx)?), "x\n"); // x is the last version
    // assert!(commits[1].timestamps().windows(2).all(|w| w[0] >= w[1])); // commit timestamps in descending order NB: This assertion started failing after switching to ws3

    {
        let repo = ctx.repo.get()?;
        assert_commit_tree_matches(&repo, commits[1].ids()[0], &[("file", b"x\n")])?;
        assert_commit_tree_matches(
            &repo,
            commits[1].ids()[1],
            &[
                (".auto-resolution/file", b"a\n"),
                (".conflict-base-0/file", b"x\n"),
                (".conflict-side-0/file", b"a\n"),
                (".conflict-side-1/file", b"y\n"),
            ],
        )?;
    }

    // Reordered the commits back to the original order
    let new_order = order(vec![
        vec![],
        vec![
            test.bottom_commits["commit 2"],
            test.bottom_commits["commit 1"],
        ],
    ]);

    reorder_stack(&mut ctx, test.stack.id, new_order.clone())?;
    let test = test_ctx(&ctx)?;
    let commits = vb_commits(&ctx);

    // Verify that the commits are now in the updated order
    assert_eq!(commits[1].msgs(), vec!["commit 2", "commit 1"]); // swapped
    assert_eq!(commits[1].conflicted(), vec![false, false]); // conflicts are gone
    assert_eq!(file(&ctx, test.stack.head_oid(&ctx)?), "y\n"); // y is the last version again
    assert!(commits[1].timestamps().windows(2).all(|w| w[0] >= w[1])); // commit timestamps in descending order

    {
        let repo = ctx.repo.get()?;
        assert_commit_tree_matches(&repo, commits[1].ids()[0], &[("file", b"y\n")])?;
        assert_commit_tree_matches(&repo, commits[1].ids()[1], &[("file", b"x\n")])?;
    }

    Ok(())
}

fn order(series: Vec<Vec<gix::ObjectId>>) -> StackOrder {
    StackOrder {
        series: vec![
            SeriesOrder {
                name: "top-series".to_string(),
                commit_ids: series[0].clone(),
            },
            SeriesOrder {
                name: "my_stack".to_string(),
                commit_ids: series[1].clone(),
            },
        ],
    }
}

trait CommitHelpers {
    fn msgs(&self) -> Vec<String>;
    fn ids(&self) -> Vec<gix::ObjectId>;
    fn conflicted(&self) -> Vec<bool>;
    fn timestamps(&self) -> Vec<u128>;
}

impl CommitHelpers for Vec<(gix::ObjectId, String, bool, u128)> {
    fn msgs(&self) -> Vec<String> {
        self.iter().map(|(_, msg, _, _)| msg.clone()).collect_vec()
    }
    fn ids(&self) -> Vec<gix::ObjectId> {
        self.iter().map(|(id, _, _, _)| *id).collect_vec()
    }
    fn conflicted(&self) -> Vec<bool> {
        self.iter()
            .map(|(_, _, conflicted, _)| *conflicted)
            .collect_vec()
    }
    fn timestamps(&self) -> Vec<u128> {
        self.iter().map(|(_, _, _, ts)| *ts).collect_vec()
    }
}

/// Commits from list_virtual_branches
fn vb_commits(ctx: &Context) -> Vec<Vec<(gix::ObjectId, String, bool, u128)>> {
    let details = support::stack_details(ctx);
    let (_, my_stack) = details
        .iter()
        .find(|(_, d)| d.derived_name == "top-series")
        .expect("top-series should exist");

    let mut out = vec![];
    for b in my_stack.branch_details.iter() {
        let mut commits = vec![];
        for c in b.commits.iter() {
            commits.push((
                c.id,
                c.message.to_string(),
                c.has_conflicts,
                c.created_at as u128,
            ));
        }
        out.push(commits);
    }
    out
}

fn file(ctx: &Context, commit_id: gix::ObjectId) -> String {
    let repo = ctx.repo.get().unwrap();
    let tree = repo.find_commit(commit_id).unwrap().tree().unwrap();
    let entry = tree.lookup_entry_by_path("file").unwrap().unwrap();
    let blob = entry.object().unwrap().into_blob();
    String::from_utf8(blob.data.to_vec()).unwrap()
}

fn assert_commit_tree_matches(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
    files: &[(&str, &[u8])],
) -> Result<()> {
    let tree = repo.find_commit(commit_id)?.tree()?;
    for (path, expected) in files {
        let entry = tree
            .lookup_entry_by_path(path)?
            .unwrap_or_else(|| panic!("expected {path} in commit {commit_id}"));
        let blob = entry.object()?.into_blob();
        assert_eq!(
            blob.data.as_slice(),
            *expected,
            "unexpected blob contents for {path} in commit {commit_id}"
        );
    }
    Ok(())
}

fn command_ctx(name: &str) -> Result<(Context, TempDir)> {
    driverless::writable_context("reorder.sh", name)
}

fn test_ctx(ctx: &Context) -> Result<TestContext> {
    let handle = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks = handle.list_all_stacks()?;
    let stack = stacks.iter().find(|b| b.name() == "my_stack").unwrap();

    let branches = stack.branches();
    let repo = ctx.repo.get()?;
    let top_commits: HashMap<String, gix::ObjectId> = branches[1]
        .commit_ids(&repo, ctx, stack)?
        .local_commits
        .iter()
        .map(|id| {
            let commit = repo.find_commit(*id).unwrap();
            (commit.message_bstr().to_string(), *id)
        })
        .collect();
    let bottom_commits: HashMap<String, gix::ObjectId> = branches[0]
        .commit_ids(&repo, ctx, stack)?
        .local_commits
        .iter()
        .map(|id| {
            let commit = repo.find_commit(*id).unwrap();
            (commit.message_bstr().to_string(), *id)
        })
        .collect();

    Ok(TestContext {
        stack: stack.clone(),
        top_commits,
        bottom_commits,
    })
}
struct TestContext {
    stack: gitbutler_stack::Stack,
    top_commits: HashMap<String, gix::ObjectId>,
    bottom_commits: HashMap<String, gix::ObjectId>,
}
