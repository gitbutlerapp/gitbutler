//! Tests for the gix-traverse hide commits functionality.
//!
//! This module tests the `hide()` functionality in gix-traverse which allows
//! hiding commits and their ancestry during traversal.
//!
//! ## Background
//!
//! There was a concern that the implementation of `hide()` in gix-traverse wasn't
//! based on proper "graph painting" via gix-revwalk, which could potentially cause
//! commits to be returned before hidden tips have marked them as hidden.
//!
//! ## Implementation Analysis
//!
//! The current implementation handles this by:
//! 1. Adding hidden tips to the FRONT of the traversal queue (for BreadthFirst)
//!    or with appropriate priority (for time-based sorting)
//! 2. Using a `candidates` buffer that holds interesting commits that might later
//!    be discovered to be hidden
//! 3. When a commit is discovered via both an interesting and hidden path,
//!    it's removed from the candidates buffer
//! 4. Only returning candidates after the main traversal is complete
//!
//! ## These Tests
//!
//! These tests serve as regression tests to ensure the hiding functionality
//! continues to work correctly. They test various graph topologies with:
//! - Multiple interesting tips converging on a common ancestor
//! - Hidden tips that share ancestry with interesting tips
//! - Different sorting modes (BreadthFirst, ByCommitTime)
//!
//! All tests currently pass, indicating the implementation handles these
//! scenarios correctly.

use gix::traverse::commit::{simple::Sorting, Parents, Simple};
use std::collections::HashSet;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Helper to convert a hex string to an ObjectId
fn hex_to_id(hex: &str) -> gix::ObjectId {
    gix::ObjectId::from_hex(hex.as_bytes()).expect("valid hex")
}

/// Creates the test repository and returns the path
fn setup_repo() -> Result<std::path::PathBuf> {
    let repo_path = gix_testtools::scripted_fixture_read_only_standalone("hide_commits_bug.sh")?;
    Ok(repo_path)
}

/// Get commit ids by message from the repository
fn get_commit_by_message(
    _store: &gix::odb::Handle,
    repo_path: &std::path::Path,
) -> Result<std::collections::HashMap<String, gix::ObjectId>> {
    use std::process::Command;

    let mut map = std::collections::HashMap::new();

    // Get all commits
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["log", "--all", "--format=%H %s"])
        .output()?;

    if !output.status.success() {
        return Err("git log failed".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() == 2 {
            let id = hex_to_id(parts[0]);
            let message = parts[1].to_string();
            map.insert(message, id);
        }
    }

    Ok(map)
}

/// Execute a git log and return the output as a string
fn git_graph(repo_dir: &std::path::Path) -> Result<String> {
    let out = std::process::Command::new(gix::path::env::exe_invocation())
        .current_dir(repo_dir)
        .args([
            "log",
            "--oneline",
            "--graph",
            "--decorate",
            "--all",
            "--pretty=format:%H %d %s",
        ])
        .output()?;
    if !out.status.success() {
        return Err("git log failed".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Regression test for gix-traverse hide functionality with multiple tips.
///
/// This test verifies that when traversing from multiple interesting tips
/// while hiding another tip, commits that are ancestors of the hidden tip
/// are correctly excluded from the results.
///
/// Graph structure:
/// ```text
///    base -- fork
///            / | \
///          i1  h1 i2
///          |   |   |
///          i3  h2 i4
///              |
///              h3 (hidden tip)
/// ```
///
/// When traversing from [i3, i4] while hiding h3:
/// - fork and base are the common ancestors
/// - fork and base should be hidden because they're reachable from h3
/// - The test verifies that only i3, i1, i4, i2 are returned
#[test]
fn hide_commits_shared_ancestry() -> Result<()> {
    let repo_path = setup_repo()?;
    let store = gix::odb::at(repo_path.join(".git").join("objects"))?;

    // Get commit IDs by message
    let commits = get_commit_by_message(&store, &repo_path)?;

    let base = commits.get("base").expect("base exists");
    let fork = commits.get("fork").expect("fork exists");
    let i1 = commits.get("i1").expect("i1 exists");
    let i2 = commits.get("i2").expect("i2 exists");
    let i3 = commits.get("i3").expect("i3 exists");
    let i4 = commits.get("i4").expect("i4 exists");
    let h1 = commits.get("h1").expect("h1 exists");
    let h2 = commits.get("h2").expect("h2 exists");
    let h3 = commits.get("h3").expect("h3 exists");

    // Print the graph for debugging
    eprintln!("Repository graph:\n{}", git_graph(&repo_path)?);
    eprintln!("\nCommit IDs:");
    eprintln!("  base: {}", base);
    eprintln!("  fork: {}", fork);
    eprintln!("  i1: {}", i1);
    eprintln!("  i2: {}", i2);
    eprintln!("  i3: {}", i3);
    eprintln!("  i4: {}", i4);
    eprintln!("  h1: {}", h1);
    eprintln!("  h2: {}", h2);
    eprintln!("  h3: {}", h3);

    // Test with BreadthFirst sorting with MULTIPLE tips
    let result: Vec<gix::ObjectId> = Simple::new([*i3, *i4], &store)
        .sorting(Sorting::BreadthFirst)?
        .parents(Parents::All)
        .hide([*h3])?
        .map(|res| res.map(|info| info.id))
        .collect::<std::result::Result<Vec<_>, _>>()?;

    eprintln!("\nTraversal result (BreadthFirst with multiple tips):");
    for (i, id) in result.iter().enumerate() {
        let msg = commits
            .iter()
            .find(|(_, v)| *v == id)
            .map(|(k, _)| k.as_str())
            .unwrap_or("unknown");
        eprintln!("  {}: {} ({})", i, id, msg);
    }

    // The expected behavior: Only i3, i1, i4, i2 should be returned (both branches).
    // fork and base should NOT be returned because they're ancestors of h3.
    //
    // The traversal works as follows:
    // 1. h3 is added to front of queue (via hide())
    // 2. i3 and i4 are at back of queue (via new())
    // 3. h3 is processed first, h2 is added as hidden
    // 4. Eventually fork is reached via h1 and marked hidden
    // 5. When fork is reached via i1 or i2, it's already marked hidden
    // 6. Hidden commits are never returned

    let result_set: HashSet<_> = result.iter().collect();

    // Expected: only i3, i1, i4, i2 (commits unique to feature branches)
    let expected_set: HashSet<_> = [i3, i1, i4, i2].into_iter().collect();

    if result_set == expected_set {
        eprintln!("\n✓ Test passed: Only i3, i1, i4, i2 were returned (correct behavior)");
    } else {
        // Document what would be wrong
        if result_set.contains(&fork) {
            eprintln!("\n✗ FAILURE: fork was incorrectly returned!");
            eprintln!("  fork is an ancestor of the hidden tip h3, so it should be hidden.");
        }
        if result_set.contains(&base) {
            eprintln!("\n✗ FAILURE: base was incorrectly returned!");
        }
    }

    assert_eq!(
        result_set, expected_set,
        "Only i3, i1, i4, i2 should be returned when hiding h3 and its ancestry.\n\
         Result contained: {:?}\n\
         Expected: {:?}",
        result.iter().map(|id| commits.iter().find(|(_, v)| *v == id).map(|(k, _)| k.as_str()).unwrap_or("?")).collect::<Vec<_>>(),
        ["i3", "i1", "i4", "i2"]
    );

    Ok(())
}

/// Test the same scenario with commit-time sorting (NewestFirst)
#[test]
fn hide_commits_shared_ancestry_commit_time_sorting() -> Result<()> {
    use gix::traverse::commit::simple::CommitTimeOrder;

    let repo_path = setup_repo()?;
    let store = gix::odb::at(repo_path.join(".git").join("objects"))?;

    let commits = get_commit_by_message(&store, &repo_path)?;

    let i1 = commits.get("i1").expect("i1 exists");
    let i2 = commits.get("i2").expect("i2 exists");
    let i3 = commits.get("i3").expect("i3 exists");
    let i4 = commits.get("i4").expect("i4 exists");
    let h3 = commits.get("h3").expect("h3 exists");

    // Test with ByCommitTime sorting - NewestFirst with multiple tips
    let result: Vec<gix::ObjectId> = Simple::new([*i3, *i4], &store)
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))?
        .parents(Parents::All)
        .hide([*h3])?
        .map(|res| res.map(|info| info.id))
        .collect::<std::result::Result<Vec<_>, _>>()?;

    eprintln!("\nTraversal result (ByCommitTime NewestFirst):");
    for (i, id) in result.iter().enumerate() {
        let msg = commits
            .iter()
            .find(|(_, v)| *v == id)
            .map(|(k, _)| k.as_str())
            .unwrap_or("unknown");
        eprintln!("  {}: {} ({})", i, id, msg);
    }

    let result_set: HashSet<_> = result.iter().collect();
    let expected_set: HashSet<_> = [i3, i1, i4, i2].into_iter().collect();

    assert_eq!(
        result_set, expected_set,
        "Only i3, i1, i4, i2 should be returned when hiding h3 and its ancestry (ByCommitTime NewestFirst)."
    );

    Ok(())
}

/// Test with OldestFirst sorting
#[test]
fn hide_commits_shared_ancestry_oldest_first() -> Result<()> {
    use gix::traverse::commit::simple::CommitTimeOrder;

    let repo_path = setup_repo()?;
    let store = gix::odb::at(repo_path.join(".git").join("objects"))?;

    let commits = get_commit_by_message(&store, &repo_path)?;

    let i1 = commits.get("i1").expect("i1 exists");
    let i2 = commits.get("i2").expect("i2 exists");
    let i3 = commits.get("i3").expect("i3 exists");
    let i4 = commits.get("i4").expect("i4 exists");
    let h3 = commits.get("h3").expect("h3 exists");

    let result: Vec<gix::ObjectId> = Simple::new([*i3, *i4], &store)
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::OldestFirst))?
        .parents(Parents::All)
        .hide([*h3])?
        .map(|res| res.map(|info| info.id))
        .collect::<std::result::Result<Vec<_>, _>>()?;

    eprintln!("\nTraversal result (ByCommitTime OldestFirst):");
    for (i, id) in result.iter().enumerate() {
        let msg = commits
            .iter()
            .find(|(_, v)| *v == id)
            .map(|(k, _)| k.as_str())
            .unwrap_or("unknown");
        eprintln!("  {}: {} ({})", i, id, msg);
    }

    let result_set: HashSet<_> = result.iter().collect();
    let expected_set: HashSet<_> = [i3, i1, i4, i2].into_iter().collect();

    assert_eq!(
        result_set, expected_set,
        "Only i3, i1, i4, i2 should be returned when hiding h3 and its ancestry (ByCommitTime OldestFirst)."
    );

    Ok(())
}
