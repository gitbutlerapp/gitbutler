use std::collections::HashMap;

use anyhow::Result;
use git2::Oid;
use gitbutler_branch_actions::{list_virtual_branches, reorder_stack, SeriesOrder, StackOrder};
use gitbutler_command_context::CommandContext;
use gitbutler_stack::VirtualBranchesHandle;
use itertools::Itertools;
use tempfile::TempDir;

#[test]
fn noop_reorder_errors() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
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
    let result = reorder_stack(ctx.project(), test_ctx.stack.id, order);
    assert_eq!(
        result.unwrap_err().to_string(),
        "The new order is the same as the current order"
    );
    Ok(())
}

#[test]
fn reorder_in_top_series() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
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
    reorder_stack(ctx.project(), test_ctx.stack.id, order.clone())?;
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
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
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
    reorder_stack(ctx.project(), test_ctx.stack.id, order.clone())?;
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
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
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
    reorder_stack(ctx.project(), test_ctx.stack.id, order.clone())?;
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
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
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
    reorder_stack(ctx.project(), test_ctx.stack.id, order.clone())?;
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
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
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
    reorder_stack(ctx.project(), test_ctx.stack.id, order.clone())?;
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
fn reorder_stack_making_top_empty_series() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits-small")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![],
        vec![
            test_ctx.top_commits["commit 2"], // from the top series
            test_ctx.bottom_commits["commit 1"],
        ],
    ]);
    reorder_stack(ctx.project(), test_ctx.stack.id, order.clone())?;
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
    let (ctx, _temp_dir) = command_ctx("multiple-commits-small")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![
            test_ctx.top_commits["commit 2"],
            test_ctx.bottom_commits["commit 1"], // from the bottom series
        ],
        vec![],
    ]);
    reorder_stack(ctx.project(), test_ctx.stack.id, order.clone())?;
    let commits = vb_commits(&ctx);

    // Verify the commit messages and ids in the second (top) series - top-series
    assert_eq!(commits[0].msgs(), vec!["commit 2", "commit 1"]);
    assert_eq!(commits[0].ids(), order.series[0].commit_ids); // nothing was rebased

    // Verify the commit messages and ids in the first (bottom) series
    assert!(commits[1].msgs().is_empty());
    assert!(commits[1].ids().is_empty());

    Ok(())
}

#[test]
fn reorder_stack_into_empty_top() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits-empty-top")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = order(vec![
        vec![
            test_ctx.bottom_commits["commit 1"], // from the bottom series
        ],
        vec![],
    ]);
    reorder_stack(ctx.project(), test_ctx.stack.id, order.clone())?;
    let commits = vb_commits(&ctx);

    // Verify the commit messages and ids in the second (top) series - top-series
    assert_eq!(commits[0].msgs(), vec!["commit 1"]);
    assert_eq!(commits[0].ids(), order.series[0].commit_ids); // nothing was rebased

    // Verify the commit messages and ids in the first (bottom) series
    assert!(commits[1].msgs().is_empty());
    assert!(commits[1].ids().is_empty());

    Ok(())
}

fn order(series: Vec<Vec<Oid>>) -> StackOrder {
    StackOrder {
        series: vec![
            SeriesOrder {
                name: "top-series".to_string(),
                commit_ids: series[0].clone(),
            },
            SeriesOrder {
                name: "a-branch-2".to_string(),
                commit_ids: series[1].clone(),
            },
        ],
    }
}

trait CommitHelpers {
    fn msgs(&self) -> Vec<String>;
    fn ids(&self) -> Vec<Oid>;
}

impl CommitHelpers for Vec<(Oid, String)> {
    fn msgs(&self) -> Vec<String> {
        self.iter().map(|(_, msg)| msg.clone()).collect_vec()
    }
    fn ids(&self) -> Vec<Oid> {
        self.iter().map(|(id, _)| *id).collect_vec()
    }
}

/// Commits from list_virtual_branches
fn vb_commits(ctx: &CommandContext) -> Vec<Vec<(git2::Oid, String)>> {
    let (vbranches, _) = list_virtual_branches(ctx.project()).unwrap();
    let vbranch = vbranches.iter().find(|vb| vb.name == "my_stack").unwrap();
    let mut out = vec![];
    for series in vbranch.series.clone() {
        let messages = series
            .patches
            .iter()
            .map(|p| (p.id, p.description.to_string()))
            .collect_vec();
        out.push(messages)
    }
    out
}

fn command_ctx(name: &str) -> Result<(CommandContext, TempDir)> {
    gitbutler_testsupport::writable::fixture("reorder.sh", name)
}

fn test_ctx(ctx: &CommandContext) -> Result<TestContext> {
    let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let branches = handle.list_all_branches()?;
    let stack = branches.iter().find(|b| b.name == "my_stack").unwrap();

    let all_series = stack.list_series(ctx)?;

    let top_commits: HashMap<String, git2::Oid> = all_series[1]
        .local_commits
        .iter()
        .map(|c| (c.message().unwrap().to_string(), c.id()))
        .collect();

    let bottom_commits: HashMap<String, git2::Oid> = all_series[0]
        .local_commits
        .iter()
        .map(|c| (c.message().unwrap().to_string(), c.id()))
        .collect();

    Ok(TestContext {
        stack: stack.clone(),
        top_commits,
        bottom_commits,
    })
}
struct TestContext {
    stack: gitbutler_stack::Stack,
    top_commits: HashMap<String, git2::Oid>,
    bottom_commits: HashMap<String, git2::Oid>,
}
