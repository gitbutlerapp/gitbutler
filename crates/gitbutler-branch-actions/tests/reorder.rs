use std::collections::HashMap;

use anyhow::Result;
use gitbutler_branch_actions::{reorder_stack, SeriesOrder, StackOrder};
use gitbutler_command_context::CommandContext;
use gitbutler_stack::VirtualBranchesHandle;
use tempfile::TempDir;

#[test]
fn noop_reorder_errors() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let order = StackOrder {
        series: vec![
            SeriesOrder {
                name: "top-series".to_string(),
                commit_ids: vec![
                    test_ctx.top_commits["commit 6"],
                    test_ctx.top_commits["commit 5"],
                    test_ctx.top_commits["commit 4"],
                ],
            },
            SeriesOrder {
                name: "a-branch-2".to_string(),
                commit_ids: vec![
                    test_ctx.bottom_commits["commit 3"],
                    test_ctx.bottom_commits["commit 2"],
                    test_ctx.bottom_commits["commit 1"],
                ],
            },
        ],
    };
    let result = reorder_stack(ctx.project(), test_ctx.stack.id, order);
    assert_eq!(
        result.unwrap_err().to_string(),
        "The new order is the same as the current order"
    );
    Ok(())
}

fn command_ctx(name: &str) -> Result<(CommandContext, TempDir)> {
    gitbutler_testsupport::writable::fixture("reorder.sh", name)
}

fn test_ctx(ctx: &CommandContext) -> Result<TestContext> {
    let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let branches = handle.list_all_branches()?;
    let stack = branches.iter().find(|b| b.name == "my_stack").unwrap();

    let all_series = stack.list_series(&ctx)?;

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
