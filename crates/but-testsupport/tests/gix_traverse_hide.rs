//! Tests for the gix-traverse hide commits functionality.
//!
//! This module tests a known bug in gix-traverse where commits can be returned
//! by the traversal before the hidden tips have had a chance to mark them as hidden.
//!
//! The bug occurs because the implementation doesn't use proper "graph painting"
//! via gix-revwalk - hidden commits are added to the traversal queue, but the
//! traversal can return commits before they've been painted as hidden.

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

/// Test that demonstrates the bug in gix-traverse hide functionality.
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
/// - But with multiple interesting tips, the traversal might return fork
///   before h3's traversal has a chance to mark it as hidden
#[test]
fn hide_commits_shared_ancestry_bug() -> Result<()> {
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

    // Test with BreadthFirst sorting with MULTIPLE tips - this is where the bug manifests
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
    // However, due to the bug with multiple tips, the traversal might:
    // 1. Process i3 and i4 (the tips)
    // 2. Add i1 and i2 to queue (parents)
    // 3. Process i1, add fork to queue
    // 4. Process i2, fork is already seen (interesting)
    // 5. Process fork, add base to queue
    // 6. Meanwhile h3's traversal is processing h2, h1, then reaches fork
    // 7. If fork or base is returned before being marked hidden -> BUG!

    let result_set: HashSet<_> = result.iter().collect();

    // Expected: only i3, i1, i4, i2 (commits unique to feature branches)
    let expected_set: HashSet<_> = [i3, i1, i4, i2].into_iter().collect();

    if result_set == expected_set {
        eprintln!("\n✓ Test passed: Only i3, i1, i4, i2 were returned (correct behavior)");
    } else {
        // Document what the bug looks like
        if result_set.contains(&fork) {
            eprintln!("\n✗ BUG DETECTED: fork was incorrectly returned!");
            eprintln!("  fork is an ancestor of the hidden tip h3, so it should be hidden.");
            eprintln!("  This demonstrates the graph painting bug in gix-traverse.");
        }
        if result_set.contains(&base) {
            eprintln!("\n✗ BUG DETECTED: base was incorrectly returned!");
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
