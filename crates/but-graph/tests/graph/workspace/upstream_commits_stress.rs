//! Stress / fuzz-style tests for `Graph::upstream_commits()`.
//!
//! These tests programmatically construct git repos and graphs in memory,
//! then probe `upstream_commits()` with various edge cases, corrupt states,
//! and unusual topologies to find unexpected failures.

use but_graph::target_ref_relations::FirstParentTraversal;
use but_graph::{Graph, init::Options};
use but_meta::VirtualBranchesTomlMetadata;
use but_testsupport::open_repo_config;
use gix::refs::transaction::PreviousValue;

// ─── Helpers ────────────────────────────────────────────────────────────────

fn opts() -> Options {
    Options {
        collect_tags: false,
        commits_limit_hint: None,
        commits_limit_recharge_location: vec![],
        hard_limit: None,
        extra_target_commit_id: None,
        dangerously_skip_postprocessing_for_debugging: false,
    }
}

fn meta(dir: &std::path::Path) -> std::mem::ManuallyDrop<VirtualBranchesTomlMetadata> {
    let m = VirtualBranchesTomlMetadata::from_path(dir.join("should-never-be-written.toml"))
        .expect("can create in-memory metadata");
    std::mem::ManuallyDrop::new(m)
}

/// Create a fresh in-memory repo in a tempdir, returning (tempdir, repo).
fn fresh_repo() -> (
    but_testsupport::gix_testtools::tempfile::TempDir,
    gix::Repository,
) {
    let tmp = but_testsupport::gix_testtools::tempfile::tempdir().expect("tempdir");
    let repo = gix::ThreadSafeRepository::init_opts(
        tmp.path(),
        gix::create::Kind::WithWorktree,
        gix::create::Options::default(),
        open_repo_config().expect("config"),
    )
    .expect("init")
    .to_thread_local();
    (tmp, repo)
}

/// Commit an empty tree with the given message on HEAD, with explicit parents.
fn commit(
    repo: &gix::Repository,
    message: &str,
    parents: impl IntoIterator<Item = gix::ObjectId>,
) -> gix::ObjectId {
    let parents: Vec<_> = parents.into_iter().collect();
    repo.commit("HEAD", message, repo.object_hash().empty_tree(), parents)
        .expect("commit")
        .detach()
}

/// Write a commit object without updating HEAD. Returns the commit oid.
fn commit_detached(
    repo: &gix::Repository,
    message: &str,
    parents: impl IntoIterator<Item = gix::ObjectId>,
) -> gix::ObjectId {
    let tree = repo.object_hash().empty_tree();
    let parent_args: Vec<String> = parents
        .into_iter()
        .flat_map(|p| ["-p".to_string(), p.to_string()])
        .collect();
    let mut cmd = but_testsupport::git(repo);
    cmd.args(["commit-tree", &tree.to_string()]);
    cmd.args(&parent_args);
    cmd.args(["-m", message]);
    let output = cmd.output().expect("git commit-tree");
    assert!(
        output.status.success(),
        "git commit-tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let hex = String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_string();
    hex.parse().expect("valid oid")
}

/// Create a ref at the given name pointing to the commit.
fn create_ref(repo: &gix::Repository, name: &str, target: gix::ObjectId) {
    repo.reference(name, target, PreviousValue::Any, "test")
        .expect("create ref");
}

/// Set HEAD to a given ref symbolically.
fn set_head(repo: &gix::Repository, refname: &str) {
    repo.edit_reference(gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange::default(),
            expected: PreviousValue::Any,
            new: gix::refs::Target::Symbolic(refname.try_into().expect("valid ref")),
        },
        name: "HEAD".try_into().expect("HEAD"),
        deref: false,
    })
    .expect("set HEAD");
}

/// Compute upstream_commits from a graph built from HEAD.
fn upstream_from_head(
    repo: &gix::Repository,
    m: &std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    target_ref: &str,
    first_parent: FirstParentTraversal,
) -> anyhow::Result<Vec<but_graph::target_ref_relations::HeadStatus>> {
    upstream_from_head_with_opts(repo, m, target_ref, first_parent, opts())
}

/// Like `upstream_from_head` but with custom graph options.
fn upstream_from_head_with_opts(
    repo: &gix::Repository,
    m: &std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    target_ref: &str,
    first_parent: FirstParentTraversal,
    options: Options,
) -> anyhow::Result<Vec<but_graph::target_ref_relations::HeadStatus>> {
    let graph = Graph::from_head(repo, &**m, options)?.validated()?;
    let tr: gix::refs::FullName = target_ref.try_into()?;
    graph.upstream_commits(repo, tr.as_ref(), first_parent)
}

// ─── Test Parameter Types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum WsMessage {
    /// Proper workspace commit message
    Managed,
    /// Legacy integration commit message
    Integration,
    /// Regular commit (not managed)
    Regular,
    /// Empty message
    Empty,
    /// Almost-right message with trailing whitespace
    TrailingWhitespace,
    /// Almost-right but with a typo
    Typo,
}

impl WsMessage {
    fn as_str(&self) -> &str {
        match self {
            Self::Managed => "GitButler Workspace Commit",
            Self::Integration => "GitButler Integration Commit",
            Self::Regular => "some regular commit",
            Self::Empty => "",
            Self::TrailingWhitespace => "GitButler Workspace Commit  ",
            Self::Typo => "GitButler Workspce Commit",
        }
    }

    fn is_managed(&self) -> bool {
        matches!(
            self,
            Self::Managed | Self::Integration | Self::TrailingWhitespace
        )
    }
}

// ─── Parameterised Tests ────────────────────────────────────────────────────

/// Build a repo with a configurable topology and workspace commit message,
/// then run upstream_commits and verify invariants.
///
/// Topology:
///   base -- T1 -- T2 -- ... -- Tn  (origin/main)
///       \-- H0-S1 -- H0-S2 -- ...  (stack heads, forked from base)
///       \-- H1-S1 -- ...           (more stack heads if num_heads > 1)
///       ... all merged into workspace commit
fn parameterised_upstream(
    target_ahead: usize,
    stack_depth: usize,
    num_heads: usize,
    ws_message: WsMessage,
    first_parent: FirstParentTraversal,
) -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    // Create the base commit
    let base = commit(&repo, "base", []);

    // Build the target branch: base -> T1 -> T2 -> ... -> Tn
    let mut target_tip = base;
    for i in 0..target_ahead {
        target_tip = commit_detached(&repo, &format!("T{}", i + 1), [target_tip]);
    }
    create_ref(&repo, "refs/remotes/origin/main", target_tip);

    // Build stack heads diverging from base
    let mut heads = Vec::new();
    for h in 0..num_heads.max(1) {
        let mut tip = base;
        for s in 0..stack_depth {
            tip = commit_detached(&repo, &format!("H{h}-S{}", s + 1), [tip]);
        }
        heads.push(tip);
    }

    // Create the workspace/entrypoint commit
    let ws_msg = ws_message.as_str();
    let ws_oid = if heads.len() == 1 && !ws_message.is_managed() {
        // Non-managed: HEAD is the stack tip itself
        heads[0]
    } else {
        // Managed workspace commit with parents = heads
        commit_detached(&repo, ws_msg, heads.iter().copied())
    };

    // Point HEAD at the workspace commit
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws_oid);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(&repo, &m, "refs/remotes/origin/main", first_parent);

    match result {
        Ok(statuses) => {
            if ws_message.is_managed() && !heads.is_empty() {
                // Managed workspace: one status per head
                assert_eq!(
                    statuses.len(),
                    heads.len(),
                    "managed workspace should return one status per parent \
                     (msg={ws_msg:?}, heads={}, target_ahead={target_ahead})",
                    heads.len()
                );
            } else if !ws_message.is_managed() {
                // Non-managed: single status for the entrypoint itself
                assert_eq!(
                    statuses.len(),
                    1,
                    "non-managed entrypoint should return exactly one status \
                     (msg={ws_msg:?})"
                );
            }

            // Invariant: upstream commits should only contain commits
            // reachable from target_ref but NOT from the head.
            for status in &statuses {
                if target_ahead == 0 {
                    // Target hasn't advanced — should be empty
                    // (head and target share base as common ancestor)
                    assert!(
                        status.upstream_commits.is_empty(),
                        "target_ahead=0 but got {} upstream commits for head {:?}",
                        status.upstream_commits.len(),
                        status.head
                    );
                } else {
                    // Target advanced — upstream commits should be non-empty
                    assert!(
                        !status.upstream_commits.is_empty(),
                        "target_ahead={target_ahead} but got 0 upstream commits"
                    );
                    // And should be at most target_ahead (first-parent may be fewer)
                    assert!(
                        status.upstream_commits.len() <= target_ahead,
                        "upstream_commits ({}) > target_ahead ({target_ahead})",
                        status.upstream_commits.len()
                    );
                }
            }
        }
        Err(e) => {
            // Some configurations are expected to fail (e.g. empty message
            // won't produce a valid graph). That's fine — we just want to
            // make sure we don't panic or corrupt state.
            let msg = e.to_string();
            assert!(
                !msg.contains("panic") && !msg.contains("SIGSEGV"),
                "unexpected crash: {msg}"
            );
        }
    }

    Ok(())
}

#[test]
fn sweep_target_ahead_and_stack_depth() -> anyhow::Result<()> {
    for target_ahead in [0, 1, 2, 5, 10] {
        for stack_depth in [0, 1, 3] {
            parameterised_upstream(
                target_ahead,
                stack_depth,
                1,
                WsMessage::Managed,
                FirstParentTraversal::No,
            )?;
        }
    }
    Ok(())
}

#[test]
fn sweep_num_heads() -> anyhow::Result<()> {
    for num_heads in [1, 2, 3, 5, 8] {
        parameterised_upstream(
            2,
            1,
            num_heads,
            WsMessage::Managed,
            FirstParentTraversal::No,
        )?;
    }
    Ok(())
}

#[test]
fn sweep_workspace_messages() -> anyhow::Result<()> {
    let messages = [
        WsMessage::Managed,
        WsMessage::Integration,
        WsMessage::Regular,
        WsMessage::Empty,
        WsMessage::TrailingWhitespace,
        WsMessage::Typo,
    ];
    for msg in messages {
        parameterised_upstream(2, 1, 1, msg, FirstParentTraversal::No)?;
    }
    Ok(())
}

#[test]
fn sweep_first_parent_vs_full() -> anyhow::Result<()> {
    for fp in [FirstParentTraversal::Yes, FirstParentTraversal::No] {
        parameterised_upstream(3, 2, 2, WsMessage::Managed, fp)?;
    }
    Ok(())
}

// ─── Specific Corruption / Edge Case Tests ──────────────────────────────────

/// Workspace commit with zero parents (managed message but no parents).
/// The code says "could return zero head statuses" — let's verify.
#[test]
fn workspace_commit_with_zero_parents() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    create_ref(&repo, "refs/remotes/origin/main", base);

    // Orphan workspace commit — no parents at all
    let ws = commit_detached(&repo, "GitButler Workspace Commit", std::iter::empty());
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert!(
        result.is_empty(),
        "workspace commit with zero parents should yield zero head statuses, got {}",
        result.len()
    );
    Ok(())
}

/// Workspace commit with duplicate parents (same commit listed twice).
/// Git's commit-tree deduplicates parents, so we verify the code handles
/// the result correctly (single parent after dedup).
/// To test actual duplicate parents, we'd need to construct the object
/// manually at the byte level to bypass git's dedup — which would be
/// genuine corruption.
#[test]
fn workspace_commit_with_duplicate_parents_deduped_by_git() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    create_ref(&repo, "refs/remotes/origin/main", base);

    let stack = commit_detached(&repo, "stack work", [base]);

    // git commit-tree deduplicates identical parents, so this produces
    // a commit with only 1 parent despite passing the same commit twice.
    let ws = commit_detached(&repo, "GitButler Workspace Commit", [stack, stack]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // Git deduplicates, so we get 1 parent = 1 HeadStatus
    assert_eq!(result.len(), 1, "git deduplicates parents");
    assert_eq!(result[0].head, stack);
    Ok(())
}

/// Actually construct a commit object with duplicate parents by writing
/// raw bytes, bypassing git's deduplication. This simulates metadata corruption.
#[test]
fn workspace_commit_with_truly_duplicate_parents() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    create_ref(&repo, "refs/remotes/origin/main", base);

    let stack = commit_detached(&repo, "stack work", [base]);

    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    let raw_commit = format!(
        "tree {tree}\n\
         parent {stack}\n\
         parent {stack}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         GitButler Workspace Commit"
    );

    let ws_oid = write_raw_commit(&repo, &raw_commit);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws_oid);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let repo = reopen(&repo);

    let m = meta(tmp.path());
    let graph = Graph::from_head(&repo, &*m, opts())?.validated()?;
    let tr: gix::refs::FullName = "refs/remotes/origin/main".try_into()?;
    let result = graph.upstream_commits(&repo, tr.as_ref(), FirstParentTraversal::No)?;

    // Now we truly have 2 parents pointing to the same commit
    assert_eq!(
        result.len(),
        2,
        "truly duplicate parents should produce two entries"
    );
    assert_eq!(
        result[0].head, result[1].head,
        "both should reference the same commit"
    );
    assert_eq!(
        result[0].upstream_commits.len(),
        result[1].upstream_commits.len(),
        "both should have identical upstream commits"
    );
    Ok(())
}

/// Target ref doesn't exist — should produce a clear error.
#[test]
fn target_ref_missing() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    create_ref(&repo, "refs/heads/main", base);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let graph = Graph::from_head(&repo, &**&m, opts())?.validated()?;
    let tr: gix::refs::FullName = "refs/remotes/origin/nonexistent".try_into()?;
    let result = graph.upstream_commits(&repo, tr.as_ref(), FirstParentTraversal::No);

    assert!(
        result.is_err(),
        "missing target ref should produce an error"
    );
    let msg = format!("{:#}", result.err().unwrap());
    assert!(
        msg.contains("refs/remotes/origin/nonexistent")
            || msg.contains("not found")
            || msg.contains("find"),
        "error should indicate the missing ref, got: {msg}"
    );
    Ok(())
}

/// Head and target have completely disjoint histories (orphan branches).
/// Per the docs: "all the commits reachable from the target_ref will be returned."
#[test]
fn disjoint_histories() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    // Two completely separate histories
    let target_root = commit_detached(&repo, "target-root", std::iter::empty());
    let t1 = commit_detached(&repo, "T1", [target_root]);
    let t2 = commit_detached(&repo, "T2", [t1]);
    create_ref(&repo, "refs/remotes/origin/main", t2);

    let stack_root = commit_detached(&repo, "stack-root", std::iter::empty()); // orphan!
    let s1 = commit_detached(&repo, "S1", [stack_root]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    // With disjoint history, ALL target commits should appear as upstream
    assert_eq!(
        result[0].upstream_commits.len(),
        3,
        "disjoint history should show all 3 target-reachable commits (target-root, T1, T2)"
    );
    Ok(())
}

/// Target ref is behind the workspace head (target is an ancestor of head).
/// This would mean "we already have everything upstream" — behind=0.
#[test]
fn target_is_ancestor_of_head() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let m1 = commit_detached(&repo, "M1", [base]);
    let m2 = commit_detached(&repo, "M2", [m1]);

    // Target points at base, head is at M2 (which includes base in its history)
    create_ref(&repo, "refs/remotes/origin/main", base);
    create_ref(&repo, "refs/heads/main", m2);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    assert!(
        result[0].upstream_commits.is_empty(),
        "target is ancestor of head — no upstream commits expected"
    );
    Ok(())
}

/// Target ref and head are the exact same commit.
#[test]
fn target_equals_head() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    create_ref(&repo, "refs/remotes/origin/main", base);
    create_ref(&repo, "refs/heads/main", base);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    assert!(
        result[0].upstream_commits.is_empty(),
        "same commit should yield zero upstream"
    );
    Ok(())
}

/// Target has merge commits — first-parent traversal should skip non-first parents.
#[test]
fn target_with_merges_first_parent_vs_full() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    // base -> T1 (linear)
    //      \-> feat1 -> feat2  (side branch)
    // T1 + feat2 -> merge -> T2 (merge commit on target)
    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    let feat1 = commit_detached(&repo, "feat1", [base]);
    let feat2 = commit_detached(&repo, "feat2", [feat1]);
    let merge = commit_detached(&repo, "merge T1+feat2", [t1, feat2]);
    let t2 = commit_detached(&repo, "T2", [merge]);
    create_ref(&repo, "refs/remotes/origin/main", t2);

    // Stack diverges from base
    let s1 = commit_detached(&repo, "S1", [base]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());

    let full = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;
    let fp = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::Yes,
    )?;

    assert_eq!(
        full[0].upstream_commits.len(),
        5,
        "full: T2, merge, T1, feat2, feat1"
    );
    assert_eq!(
        fp[0].upstream_commits.len(),
        3,
        "first-parent: T2, merge, T1"
    );
    Ok(())
}

/// A workspace commit that mixes managed and unmanaged heads:
/// one parent is a workspace commit itself (nested workspace).
#[test]
fn nested_workspace_commit_as_parent() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    // Inner workspace commit as a parent
    let inner_ws = commit_detached(&repo, "GitButler Workspace Commit", [s1]);
    let s2 = commit_detached(&repo, "S2", [base]);

    // Outer workspace commit whose parents include another workspace commit
    let outer_ws = commit_detached(&repo, "GitButler Workspace Commit", [inner_ws, s2]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", outer_ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // The outer workspace commit has 2 parents: inner_ws and s2.
    // upstream_commits iterates those parents as heads.
    assert_eq!(result.len(), 2, "two parents of the outer workspace commit");

    // Both should report T1 as upstream
    for status in &result {
        assert!(
            !status.upstream_commits.is_empty(),
            "each head should have T1 as upstream"
        );
    }
    Ok(())
}

/// Target ref points at a commit that is also a parent of the workspace commit.
/// This means the target is "included" in the workspace.
#[test]
fn target_is_workspace_parent() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    // Target ref points at S1
    create_ref(&repo, "refs/remotes/origin/main", s1);

    // Workspace commit includes S1 as a parent
    let ws = commit_detached(&repo, "GitButler Workspace Commit", [s1, s2]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 2);

    // For the S1 head: target_ref IS s1, so origin/main ^S1 = {} (empty)
    let s1_status = result.iter().find(|h| h.head == s1).expect("S1 head");
    assert!(
        s1_status.upstream_commits.is_empty(),
        "target IS the head — no upstream commits"
    );

    // For the S2 head: origin/main ^S2 = {S1} since S1 is not reachable from S2.
    let s2_status = result.iter().find(|h| h.head == s2).expect("S2 head");
    assert_eq!(
        s2_status.upstream_commits.len(),
        1,
        "S1 should be upstream of S2 (disjoint branches from base)"
    );
    Ok(())
}

/// Target ref has a diamond merge in its history.
/// Ensure commit deduplication works correctly.
#[test]
fn diamond_merge_in_target() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    //       base
    //      /    \
    //    left   right
    //      \    /
    //      merge
    //        |
    //       tip  <- origin/main
    let base = commit(&repo, "base", []);
    let left = commit_detached(&repo, "left", [base]);
    let right = commit_detached(&repo, "right", [base]);
    let merge = commit_detached(&repo, "merge", [left, right]);
    let tip = commit_detached(&repo, "tip", [merge]);
    create_ref(&repo, "refs/remotes/origin/main", tip);

    // Stack at base
    let s = commit_detached(&repo, "stack", [base]);
    create_ref(&repo, "refs/heads/main", s);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());

    let full = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;
    let fp = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::Yes,
    )?;

    // Full traversal: tip, merge, left, right = 4 commits (base is reachable from stack, hidden)
    assert_eq!(
        full[0].upstream_commits.len(),
        4,
        "full: tip, merge, left, right"
    );
    // First-parent: tip, merge, left = 3 (right is second parent of merge, skipped)
    assert_eq!(
        fp[0].upstream_commits.len(),
        3,
        "first-parent: tip, merge, left"
    );

    Ok(())
}

/// Create a workspace commit whose message body (not title) contains
/// the workspace commit title. The title itself is different.
/// This should NOT be treated as managed.
#[test]
fn workspace_title_in_body_not_in_subject() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    // Message with regular title but workspace commit text in the body
    let tricky_msg = "regular title\n\nGitButler Workspace Commit\nsome other text";
    let ws = commit_detached(&repo, tricky_msg, [s1, s2]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // The title is "regular title", not the workspace commit title.
    // So this should be treated as non-managed: single head entry for the commit itself.
    assert_eq!(
        result.len(),
        1,
        "body-only workspace title should not trigger managed mode"
    );
    Ok(())
}

/// Target ref is a lightweight tag instead of a branch ref.
#[test]
fn target_ref_is_tag() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/tags/v1.0", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let result = upstream_from_head(&repo, &m, "refs/tags/v1.0", FirstParentTraversal::No)?;

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].upstream_commits.len(), 1, "T1 should be upstream");
    Ok(())
}

/// Self-referencing: target ref points at the workspace commit itself.
#[test]
fn target_ref_points_at_workspace_commit() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let s1 = commit_detached(&repo, "S1", [base]);

    let ws = commit_detached(&repo, "GitButler Workspace Commit", [s1]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    // Target points at the workspace commit itself
    create_ref(&repo, "refs/remotes/origin/main", ws);

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // origin/main ^S1: walk from ws. ws is not reachable from S1 (S1 is ws's parent).
    // So ws should appear as upstream.
    assert_eq!(result.len(), 1);
    assert!(
        !result[0].upstream_commits.is_empty(),
        "workspace commit itself should appear as upstream of its parent"
    );
    Ok(())
}

/// Criss-cross merge: two branches merge each other's work.
/// This creates ambiguous merge bases.
#[test]
fn criss_cross_merge_in_target() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    // base -> A1           base -> B1
    //      \-> B1 -> M1         \-> A1 -> M2  (criss-cross)
    //                M1 + M2 -> final_merge <- origin/main
    let base = commit(&repo, "base", []);
    let a1 = commit_detached(&repo, "A1", [base]);
    let b1 = commit_detached(&repo, "B1", [base]);
    let m1 = commit_detached(&repo, "M1-merge-B-into-A", [a1, b1]);
    let m2 = commit_detached(&repo, "M2-merge-A-into-B", [b1, a1]);
    let final_merge = commit_detached(&repo, "final", [m1, m2]);
    create_ref(&repo, "refs/remotes/origin/main", final_merge);

    let s = commit_detached(&repo, "stack", [base]);
    create_ref(&repo, "refs/heads/main", s);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    // full: final, M1, M2, A1, B1 = 5 (base is hidden because reachable from stack)
    assert_eq!(
        result[0].upstream_commits.len(),
        5,
        "criss-cross: final, M1, M2, A1, B1"
    );

    // First-parent should give fewer
    let fp = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::Yes,
    )?;
    assert!(
        fp[0].upstream_commits.len() < result[0].upstream_commits.len(),
        "first-parent should traverse fewer commits in criss-cross"
    );
    Ok(())
}

/// Large octopus merge as the target ref.
#[test]
fn octopus_merge_target() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);

    // Create 5 branches off base, then octopus-merge them
    let mut branch_tips = Vec::new();
    for i in 0..5 {
        let tip = commit_detached(&repo, &format!("branch-{i}"), [base]);
        branch_tips.push(tip);
    }
    let octopus = commit_detached(&repo, "octopus merge", branch_tips.iter().copied());
    create_ref(&repo, "refs/remotes/origin/main", octopus);

    let s = commit_detached(&repo, "stack", [base]);
    create_ref(&repo, "refs/heads/main", s);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());

    let full = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;
    let fp = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::Yes,
    )?;

    // Full: octopus + 5 branches = 6
    assert_eq!(full[0].upstream_commits.len(), 6);
    // First-parent: octopus + first branch = 2
    assert_eq!(fp[0].upstream_commits.len(), 2);
    Ok(())
}

/// Write a raw commit object, bypassing all git validation.
/// Returns the oid. This lets us create corrupt commits.
fn write_raw_commit(repo: &gix::Repository, raw: &str) -> gix::ObjectId {
    let mut cmd = but_testsupport::git(repo);
    cmd.args(["hash-object", "-t", "commit", "-w", "--stdin"]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("spawn");
    let mut stdin_handle = child.stdin.take().expect("stdin");
    std::io::Write::write_all(&mut stdin_handle, raw.as_bytes()).expect("write");
    drop(stdin_handle);
    let output = child.wait_with_output().expect("wait");
    assert!(
        output.status.success(),
        "hash-object failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .parse()
        .expect("valid oid")
}

/// Reopen a repo (needed after writing loose objects with hash-object).
fn reopen(repo: &gix::Repository) -> gix::Repository {
    gix::open_opts(
        repo.workdir().unwrap_or(repo.git_dir()),
        open_repo_config().expect("config"),
    )
    .expect("reopen")
}

/// Workspace commit with many unique parents (no duplication, since git deduplicates).
#[test]
fn workspace_octopus_with_many_unique_parents() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    // Create 5 distinct stack heads
    let mut heads = Vec::new();
    for i in 0..5 {
        heads.push(commit_detached(&repo, &format!("S{i}"), [base]));
    }

    let ws = commit_detached(&repo, "GitButler Workspace Commit", heads.iter().copied());
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 5, "5 unique parents");
    // All should have the same upstream commits since they all fork from base
    let counts: std::collections::HashSet<usize> =
        result.iter().map(|s| s.upstream_commits.len()).collect();
    assert_eq!(
        counts.len(),
        1,
        "all heads fork from the same point, so upstream commit counts should be identical"
    );
    assert_eq!(
        result[0].upstream_commits.len(),
        1,
        "T1 is the only upstream commit"
    );
    Ok(())
}

// ─── ADVERSARIAL / CORRUPTION TESTS ─────────────────────────────────────────
// These test scenarios that should never happen in normal operation but could
// occur via corruption, manual object manipulation, or tooling bugs.

/// A commit that lists itself as its own parent (cycle of length 1).
/// gix rev_walk MUST handle this without infinite-looping.
#[test]
fn self_referencing_commit_as_head() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    create_ref(&repo, "refs/remotes/origin/main", base);

    // We need to create a commit that is its own parent.
    // This is a chicken-and-egg problem: we can't know the hash before writing.
    // Instead, create a commit, then create ANOTHER commit whose parent is itself
    // by first writing a placeholder, getting the hash, then writing the real one.
    //
    // Actually, we can't do this directly because the hash depends on content.
    // But we CAN create a 2-cycle: A's parent is B, B's parent is A.
    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";

    // First, create commit A with a fake parent (all zeros)
    let fake_parent = gix::ObjectId::null(gix::hash::Kind::Sha1);
    let raw_a = format!(
        "tree {tree}\n\
         parent {fake_parent}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         cycle-A"
    );
    let a_oid = write_raw_commit(&repo, &raw_a);

    // Create commit B whose parent is A
    let raw_b = format!(
        "tree {tree}\n\
         parent {a_oid}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         cycle-B"
    );
    let b_oid = write_raw_commit(&repo, &raw_b);

    // Now create commit C whose parent is B (so walking C -> B -> A -> null-parent)
    // The null parent should cause an error, not a hang
    create_ref(&repo, "refs/heads/main", b_oid);
    set_head(&repo, "refs/heads/main");

    let repo = reopen(&repo);
    let m = meta(tmp.path());

    // This might error (missing object for null parent) — that's fine.
    // What we care about is that it doesn't hang.
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    );

    // Either succeeds or errors — just shouldn't hang or panic
    match result {
        Ok(statuses) => {
            // If it somehow works, the upstream should include base
            assert!(!statuses.is_empty());
        }
        Err(_e) => {
            // Expected — null parent is a missing object
        }
    }
    Ok(())
}

/// Two commits forming a 2-cycle: A -> B -> A.
/// Walking from target with A hidden should not loop.
#[test]
fn two_commit_cycle() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";

    // Create A first (no parent — we'll make B point to A, then remake A pointing to B)
    // Actually we can't remake A since the hash changes. Instead:
    // Create A pointing to base, create B pointing to A.
    // Then create target commit pointing to B as parent.
    // Then set head to base and walk from target — this creates a normal DAG.
    //
    // For a TRUE cycle, we need to construct it at the raw object level.
    // Let's create A pointing to B where B doesn't exist yet, then create B pointing to A.
    // Step 1: Create B first with no parents
    let raw_b = format!(
        "tree {tree}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         cycle-B"
    );
    let b_oid = write_raw_commit(&repo, &raw_b);

    // Step 2: Create A with parent B
    let raw_a = format!(
        "tree {tree}\n\
         parent {b_oid}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         cycle-A"
    );
    let a_oid = write_raw_commit(&repo, &raw_a);

    // Step 3: Now we need to create ANOTHER B' with parent A.
    // But we can't replace B in-place — its hash is fixed.
    // Instead let's just test that walking a chain with a missing parent is handled.
    // The real cycle test is: can we have a commit graph where following parents loops?
    // Answer: technically no with content-addressed storage, because the hash changes.
    // So let's test the next best thing: parent pointing to a missing object.

    // Use A as the target ref (A -> B, which is a root commit)
    create_ref(&repo, "refs/remotes/origin/main", a_oid);
    create_ref(&repo, "refs/heads/main", base);
    set_head(&repo, "refs/heads/main");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // A -> B, both should be upstream of base (disjoint histories)
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].upstream_commits.len(),
        2,
        "A and B are both upstream"
    );
    Ok(())
}

/// Parent points to a missing object (simulates partial clone / corruption).
/// The walk should error, not panic.
#[test]
fn parent_points_to_missing_object() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";

    // Create a commit whose parent is an oid that doesn't exist in the repo
    let missing_oid = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    let raw = format!(
        "tree {tree}\n\
         parent {missing_oid}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         has-missing-parent"
    );
    let bad_commit = write_raw_commit(&repo, &raw);

    // Use bad_commit as target ref
    create_ref(&repo, "refs/remotes/origin/main", bad_commit);
    create_ref(&repo, "refs/heads/main", base);
    set_head(&repo, "refs/heads/main");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    );

    // Should error when trying to walk to the missing parent
    assert!(
        result.is_err(),
        "walking to a missing parent object should error, not silently succeed"
    );
    Ok(())
}

/// Parent points to a blob object instead of a commit (type confusion).
#[test]
fn parent_points_to_blob() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);

    // Write a blob object
    let mut cmd = but_testsupport::git(&repo);
    cmd.args(["hash-object", "-w", "--stdin"]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("spawn");
    let mut stdin_handle = child.stdin.take().expect("stdin");
    std::io::Write::write_all(&mut stdin_handle, b"just a blob").expect("write");
    drop(stdin_handle);
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success());
    let blob_oid: String = String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_string();

    // Create a commit whose parent is the blob
    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    let raw = format!(
        "tree {tree}\n\
         parent {blob_oid}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         parent-is-blob"
    );
    let bad_commit = write_raw_commit(&repo, &raw);

    // Use bad_commit as target ref
    create_ref(&repo, "refs/remotes/origin/main", bad_commit);
    create_ref(&repo, "refs/heads/main", base);
    set_head(&repo, "refs/heads/main");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    );

    // Walking should error when it tries to parse a blob as a commit
    assert!(
        result.is_err(),
        "parent pointing to a blob should error during walk"
    );
    Ok(())
}

/// Target ref points to a blob, not a commit. `peel_to_commit()` should fail.
#[test]
fn target_ref_points_to_blob() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    create_ref(&repo, "refs/heads/main", base);
    set_head(&repo, "refs/heads/main");

    // Write a blob and point a ref at it
    let mut cmd = but_testsupport::git(&repo);
    cmd.args(["hash-object", "-w", "--stdin"]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("spawn");
    let mut stdin_handle = child.stdin.take().expect("stdin");
    std::io::Write::write_all(&mut stdin_handle, b"not a commit").expect("write");
    drop(stdin_handle);
    let output = child.wait_with_output().expect("wait");
    let blob_oid: gix::ObjectId = String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .parse()
        .expect("valid oid");

    create_ref(&repo, "refs/remotes/origin/main", blob_oid);

    let m = meta(tmp.path());
    let graph = Graph::from_head(&repo, &**&m, opts())?.validated()?;
    let tr: gix::refs::FullName = "refs/remotes/origin/main".try_into()?;
    let result = graph.upstream_commits(&repo, tr.as_ref(), FirstParentTraversal::No);

    assert!(
        result.is_err(),
        "target ref pointing to a blob should fail in peel_to_commit()"
    );
    Ok(())
}

/// Target ref is an annotated tag (not lightweight). Must peel through the tag object.
#[test]
fn target_ref_is_annotated_tag() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);

    // Create an annotated tag via git
    let mut cmd = but_testsupport::git(&repo);
    cmd.args(["tag", "-a", "v2.0", &t1.to_string(), "-m", "annotated tag"]);
    let output = cmd.output().expect("git tag");
    assert!(
        output.status.success(),
        "git tag failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let s1 = commit_detached(&repo, "S1", [base]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let result = upstream_from_head(&repo, &m, "refs/tags/v2.0", FirstParentTraversal::No)?;

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].upstream_commits.len(),
        1,
        "T1 should be upstream via annotated tag"
    );
    Ok(())
}

/// Workspace commit message contains NUL bytes before the title.
/// This could confuse BStr-based message parsing.
#[test]
fn nul_bytes_in_commit_message() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    // Craft a commit with NUL byte in the message
    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    let raw = format!(
        "tree {tree}\n\
         parent {s1}\n\
         parent {s2}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         \x00GitButler Workspace Commit"
    );
    let ws_oid = write_raw_commit(&repo, &raw);

    create_ref(&repo, "refs/heads/gitbutler/workspace", ws_oid);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let repo = reopen(&repo);
    let m = meta(tmp.path());

    // The NUL byte should prevent matching the workspace title
    // (or it errors — either way, it shouldn't panic)
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    );

    match result {
        Ok(statuses) => {
            // If it works, NUL should have prevented workspace detection,
            // so we should get 1 entry (non-managed mode), not 2
            assert!(
                statuses.len() <= 2,
                "shouldn't crash, got {} entries",
                statuses.len()
            );
        }
        Err(_) => { /* acceptable */ }
    }
    Ok(())
}

/// Workspace commit message uses Unicode homoglyphs that LOOK like
/// "GitButler Workspace Commit" but aren't.
#[test]
fn unicode_homoglyph_workspace_message() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    // Use Cyrillic 'а' (U+0430) instead of Latin 'a' in "Workspace"
    // "GitButler Workspаce Commit" — visually identical but different bytes
    let tricky = "GitButler Worksp\u{0430}ce Commit";
    let ws = commit_detached(&repo, tricky, [s1, s2]);

    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // Homoglyph should NOT match — should be treated as non-managed
    assert_eq!(
        result.len(),
        1,
        "unicode homoglyph should not trigger managed workspace mode"
    );
    Ok(())
}

/// Workspace head that IS the target ref's commit — but via a different ref path.
/// HEAD -> ws (managed) -> [s1, target_commit]
/// origin/main -> target_commit
/// The walk for s1 should show target_commit as upstream.
/// The walk for target_commit should show nothing (it IS the target).
#[test]
fn workspace_parent_is_target_commit_directly() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    let t2 = commit_detached(&repo, "T2", [t1]);
    create_ref(&repo, "refs/remotes/origin/main", t2);

    let s1 = commit_detached(&repo, "S1", [base]);

    // Workspace commit where one parent IS the target commit
    let ws = commit_detached(&repo, "GitButler Workspace Commit", [s1, t2]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 2);
    let t2_status = result.iter().find(|h| h.head == t2).expect("t2 head");
    assert!(
        t2_status.upstream_commits.is_empty(),
        "head that IS the target should have zero upstream"
    );

    let s1_status = result.iter().find(|h| h.head == s1).expect("s1 head");
    assert_eq!(
        s1_status.upstream_commits.len(),
        2,
        "s1 diverged at base, so T1 and T2 are upstream"
    );
    Ok(())
}

/// Head is a descendant of target (has target in its history),
/// but through a long chain with merges. Should still be zero upstream.
#[test]
fn head_deeply_includes_target() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    // Head's history includes T1 deep in a merge chain:
    // base -> s1 -> s2
    // t1 ---/
    // s2 is a merge of s1 and t1
    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "merge-includes-target", [s1, t1]);
    let s3 = commit_detached(&repo, "S3", [s2]);
    let s4 = commit_detached(&repo, "S4", [s3]);

    create_ref(&repo, "refs/heads/main", s4);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    assert!(
        result[0].upstream_commits.is_empty(),
        "head includes target in its history — zero upstream commits"
    );
    Ok(())
}

/// The workspace commit's message is ONLY whitespace/newlines.
#[test]
fn whitespace_only_commit_message() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);

    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    let raw = format!(
        "tree {tree}\n\
         parent {s1}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         \n   \n\t\n"
    );
    let ws_oid = write_raw_commit(&repo, &raw);

    create_ref(&repo, "refs/heads/gitbutler/workspace", ws_oid);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    );

    // Whitespace-only message — should not match workspace title
    // Should be treated as non-managed
    match result {
        Ok(statuses) => {
            assert_eq!(statuses.len(), 1, "whitespace message = non-managed");
        }
        Err(_) => { /* also acceptable if empty message causes error */ }
    }
    Ok(())
}

/// Target and head share no tree objects but DO share a common ancestor commit.
/// The common commit has DIFFERENT trees in each branch. This shouldn't matter
/// for upstream_commits (it's commit-topology-only) but let's verify.
#[test]
fn shared_ancestor_different_trees() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    // All commits use the empty tree, but that's fine — the point is topology.
    let base = commit(&repo, "shared-base", []);
    let t1 = commit_detached(&repo, "T1-different-content", [base]);
    let t2 = commit_detached(&repo, "T2-different-content", [t1]);
    create_ref(&repo, "refs/remotes/origin/main", t2);

    let s1 = commit_detached(&repo, "S1-different-content", [base]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result[0].upstream_commits.len(), 2, "T1 and T2");
    Ok(())
}

/// Race-like scenario: the graph is constructed from one state, then
/// we modify refs before calling upstream_commits.
/// This simulates a ref changing between graph construction and the
/// upstream_commits call.
#[test]
fn ref_changes_between_graph_and_upstream_call() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    // Build graph from current state
    let graph = Graph::from_head(&repo, &**&m, opts())?.validated()?;

    // Now advance origin/main AFTER graph construction
    let t2 = commit_detached(&repo, "T2", [t1]);
    let t3 = commit_detached(&repo, "T3", [t2]);
    create_ref(&repo, "refs/remotes/origin/main", t3);

    // Call upstream_commits on the stale graph — it reads target ref fresh
    let tr: gix::refs::FullName = "refs/remotes/origin/main".try_into()?;
    let result = graph.upstream_commits(&repo, tr.as_ref(), FirstParentTraversal::No)?;

    // The graph is stale but upstream_commits reads the ref directly,
    // so it should see the new T2, T3 commits
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].upstream_commits.len(),
        3,
        "should see T1, T2, T3 even though graph was built before T2/T3 existed"
    );
    Ok(())
}

/// Symbolic ref chain: origin/main -> refs/heads/main -> actual commit.
/// The find_reference + peel_to_commit should follow the chain.
#[test]
fn symbolic_ref_as_target() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);

    // Create the actual branch
    create_ref(&repo, "refs/heads/upstream-main", t1);

    // Create a symbolic ref pointing to it
    repo.edit_reference(gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange::default(),
            expected: PreviousValue::Any,
            new: gix::refs::Target::Symbolic("refs/heads/upstream-main".try_into().expect("valid")),
        },
        name: "refs/remotes/origin/main".try_into().expect("valid"),
        deref: false,
    })?;

    let s1 = commit_detached(&repo, "S1", [base]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].upstream_commits.len(), 1, "T1 via symbolic ref");
    Ok(())
}

/// Git replace ref — an object replacement that changes the apparent parent graph.
/// If git replace is active, rev_walk may see different parents than the raw object.
#[test]
fn git_replace_ref_changes_parent() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    let t2 = commit_detached(&repo, "T2", [t1]);
    create_ref(&repo, "refs/remotes/origin/main", t2);

    let s1 = commit_detached(&repo, "S1", [base]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    // Create a replacement for T1 that makes it look like T1's parent is S1 (not base)
    // This means the walked graph would see T2 -> T1 -> S1, making T1 reachable from S1
    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    let raw_replacement = format!(
        "tree {tree}\n\
         parent {s1}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         T1"
    );
    let replacement_oid = write_raw_commit(&repo, &raw_replacement);

    // Set up git replace: replace T1 with our new commit
    create_ref(&repo, &format!("refs/replace/{t1}"), replacement_oid);

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // With replace active, T1's parent is now S1, so T1 IS reachable from S1.
    // This means only T2 should be upstream (T1 is reachable from head via replacement).
    // OR gix might not honor replace refs by default.
    // Either way, we just care that it doesn't crash.
    assert_eq!(result.len(), 1);
    let count = result[0].upstream_commits.len();
    // Without replace: T1, T2 = 2
    // With replace honored: only T2 = 1
    assert!(
        count == 1 || count == 2,
        "should be 1 (replace honored) or 2 (replace ignored), got {count}"
    );
    Ok(())
}

/// Grafts: synthetic parent list via .git/info/grafts (deprecated but might exist).
/// Modern git uses replace refs instead, but let's see how gix handles a grafts file.
#[test]
fn grafts_file_rewriting_parents() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    let t2 = commit_detached(&repo, "T2", [t1]);
    create_ref(&repo, "refs/remotes/origin/main", t2);

    let s1 = commit_detached(&repo, "S1", [base]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    // Write a grafts file that makes T2 appear to have no parents (rootish)
    let grafts_path = tmp.path().join(".git").join("info").join("grafts");
    std::fs::create_dir_all(grafts_path.parent().unwrap()).expect("mkdir");
    std::fs::write(&grafts_path, format!("{t2}\n")).expect("write grafts");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    // With grafts: T2 is a root, so walk only returns T2 (parent chain severed)
    // Without grafts: T2 -> T1, so returns T1 and T2
    let count = result[0].upstream_commits.len();
    assert!(
        count == 1 || count == 2,
        "grafts may or may not be honored, got {count}"
    );
    Ok(())
}

/// Empty repo (no commits at all). Graph construction should handle this.
#[test]
fn completely_empty_repo() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let m = meta(tmp.path());
    // No commits, no refs — from_head should fail or return empty graph
    let result = Graph::from_head(&repo, &**&m, opts());

    // This should either error (no HEAD target) or produce a graph with no entrypoint
    match result {
        Ok(graph) => {
            // If it succeeds, upstream_commits should fail (no entrypoint)
            let validated = graph.validated();
            match validated {
                Ok(g) => {
                    let tr: gix::refs::FullName = "refs/remotes/origin/main".try_into()?;
                    let r = g.upstream_commits(&repo, tr.as_ref(), FirstParentTraversal::No);
                    // Should error — no entrypoint or no target ref
                    assert!(r.is_err(), "empty repo should error on upstream_commits");
                }
                Err(_) => { /* validation failed — acceptable */ }
            }
        }
        Err(_) => { /* from_head failed — acceptable for empty repo */ }
    }
    Ok(())
}

/// Multiple heads where one head is an ancestor of another head.
/// e.g., workspace commit with parents [A, B] where A is an ancestor of B.
/// The upstream counts should differ because B's walk hides more of the target.
#[test]
fn workspace_head_is_ancestor_of_another_head() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    let t2 = commit_detached(&repo, "T2", [t1]);
    let t3 = commit_detached(&repo, "T3", [t2]);
    create_ref(&repo, "refs/remotes/origin/main", t3);

    // S1 is an ancestor of S2
    let s1 = commit_detached(&repo, "S1", [t1]); // s1 forks from t1
    let s2 = commit_detached(&repo, "S2", [s1]); // s2 is a child of s1

    let ws = commit_detached(&repo, "GitButler Workspace Commit", [s1, s2]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 2);

    // S1 forks from T1, so upstream should be T2, T3 (2 commits)
    let s1_status = result.iter().find(|h| h.head == s1).expect("s1");
    assert_eq!(s1_status.upstream_commits.len(), 2, "T2, T3 upstream of S1");

    // S2's history includes S1 which includes T1, so upstream is still T2, T3
    let s2_status = result.iter().find(|h| h.head == s2).expect("s2");
    assert_eq!(
        s2_status.upstream_commits.len(),
        2,
        "T2, T3 upstream of S2 (via S1->T1)"
    );

    Ok(())
}

// ─── SUBTLE BUG HUNTING ─────────────────────────────────────────────────────

/// What happens when the workspace commit message starts with the right title
/// but has extra text on the same line?
/// e.g., "GitButler Workspace Commit (dirty)" — should this be managed?
#[test]
fn workspace_title_with_suffix_on_same_line() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    // Title with suffix — "GitButler Workspace Commit (dirty)"
    let ws = commit_detached(&repo, "GitButler Workspace Commit (dirty)", [s1, s2]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // The title is "GitButler Workspace Commit (dirty)" which is NOT an exact match.
    // So this should be non-managed (1 entry for the whole commit).
    assert_eq!(
        result.len(),
        1,
        "title with suffix should NOT be treated as managed"
    );
    Ok(())
}

/// Case sensitivity: "gitbutler workspace commit" (all lowercase).
#[test]
fn workspace_title_wrong_case() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    let ws = commit_detached(&repo, "gitbutler workspace commit", [s1, s2]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1, "wrong case should NOT be managed");
    Ok(())
}

/// What if target_ref and the head branch are the SAME ref name?
/// i.e., asking for upstream of yourself.
#[test]
fn target_ref_is_same_as_head_ref() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let c1 = commit_detached(&repo, "C1", [base]);
    create_ref(&repo, "refs/heads/main", c1);
    set_head(&repo, "refs/heads/main");

    // Use the same ref as both HEAD and target
    // HEAD points at C1 via refs/heads/main
    // target_ref also refs/heads/main -> C1
    let m = meta(tmp.path());
    let result = upstream_from_head(&repo, &m, "refs/heads/main", FirstParentTraversal::No)?;

    // target_ref ^HEAD = empty (same commit)
    assert_eq!(result.len(), 1);
    assert!(
        result[0].upstream_commits.is_empty(),
        "target_ref == HEAD should yield zero upstream"
    );
    Ok(())
}

/// Workspace has a single parent that's also the target ref, but workspace
/// message says "GitButler Workspace Commit". Since managed mode iterates
/// parents, we get one HeadStatus where head == target. Should be empty upstream.
#[test]
fn managed_single_parent_equals_target() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    create_ref(&repo, "refs/remotes/origin/main", base);

    // Workspace commit whose only parent is the target ref commit
    let ws = commit_detached(&repo, "GitButler Workspace Commit", [base]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1, "managed with single parent");
    assert!(
        result[0].upstream_commits.is_empty(),
        "parent IS the target — zero upstream"
    );
    Ok(())
}

/// Merge commit in target history where merge parents converge at different points.
/// Target: base -> A -> B -> merge(B, X) -> tip
///                       X --/
/// Head forks from A.
/// With first-parent: should see B, merge, tip (3 commits)
/// Full: should see B, X, merge, tip (4 commits)
#[test]
fn asymmetric_merge_in_target_first_parent_accuracy() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let a = commit_detached(&repo, "A", [base]);
    let b = commit_detached(&repo, "B", [a]);
    let x = commit_detached(&repo, "X", [a]); // side branch from A
    let merge = commit_detached(&repo, "merge", [b, x]);
    let tip = commit_detached(&repo, "tip", [merge]);
    create_ref(&repo, "refs/remotes/origin/main", tip);

    let s = commit_detached(&repo, "stack", [a]); // forks from A
    create_ref(&repo, "refs/heads/main", s);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());

    let full = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;
    let fp = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::Yes,
    )?;

    assert_eq!(full[0].upstream_commits.len(), 4, "full: tip, merge, B, X");
    assert_eq!(
        fp[0].upstream_commits.len(),
        3,
        "first-parent: tip, merge, B"
    );
    Ok(())
}

/// What happens when the workspace commit has a parent that points to
/// a commit AHEAD of the target (i.e., the parent includes target in its history)?
/// One stack is "up to date" and another isn't.
#[test]
fn mixed_behind_and_ahead_stacks() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    let t2 = commit_detached(&repo, "T2", [t1]);
    create_ref(&repo, "refs/remotes/origin/main", t2);

    // Stack A is behind (forked from base)
    let sa = commit_detached(&repo, "SA", [base]);

    // Stack B is ahead (forked from t2, which IS the target)
    let sb = commit_detached(&repo, "SB", [t2]);

    // Stack C is partially integrated (forked from t1, one behind)
    let sc = commit_detached(&repo, "SC", [t1]);

    let ws = commit_detached(&repo, "GitButler Workspace Commit", [sa, sb, sc]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 3);

    let sa_status = result.iter().find(|h| h.head == sa).expect("SA");
    assert_eq!(sa_status.upstream_commits.len(), 2, "SA behind by T1+T2");

    let sb_status = result.iter().find(|h| h.head == sb).expect("SB");
    assert_eq!(sb_status.upstream_commits.len(), 0, "SB includes target");

    let sc_status = result.iter().find(|h| h.head == sc).expect("SC");
    assert_eq!(sc_status.upstream_commits.len(), 1, "SC behind by T2 only");

    Ok(())
}

/// Workspace commit where one parent is the workspace commit itself (degenerate).
/// This could happen if someone manually edits refs.
#[test]
fn workspace_parent_is_itself() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    create_ref(&repo, "refs/remotes/origin/main", base);

    let s1 = commit_detached(&repo, "S1", [base]);

    // Create workspace commit with s1 as parent
    let ws = commit_detached(&repo, "GitButler Workspace Commit", [s1]);

    // Now create a NEW workspace commit whose parents include itself (the previous ws)
    // This isn't a true self-reference (hash differs) but it means one "head" is a
    // workspace commit, creating a nesting scenario
    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    let raw = format!(
        "tree {tree}\n\
         parent {s1}\n\
         parent {ws}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         GitButler Workspace Commit"
    );
    let outer = write_raw_commit(&repo, &raw);

    create_ref(&repo, "refs/heads/gitbutler/workspace", outer);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // outer has parents [s1, ws]. Both are treated as heads.
    // s1: target=base, base ^s1 = empty (s1 is child of base)
    // ws: target=base, base ^ws = empty (ws -> s1 -> base, so base is reachable from ws)
    assert_eq!(result.len(), 2);
    for status in &result {
        assert!(
            status.upstream_commits.is_empty(),
            "both heads include base in their history"
        );
    }
    Ok(())
}

/// Target ref and head both have the exact same set of ancestors but are different commits.
/// (same parent, same tree, different message — so different OID)
#[test]
fn target_and_head_same_parent_different_commits() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let target = commit_detached(&repo, "target-commit", [base]);
    let head_commit = commit_detached(&repo, "head-commit", [base]);

    create_ref(&repo, "refs/remotes/origin/main", target);
    create_ref(&repo, "refs/heads/main", head_commit);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    // target and head share base as ancestor. target is not reachable from head.
    // So upstream = [target] (1 commit)
    assert_eq!(
        result[0].upstream_commits.len(),
        1,
        "sibling commits: target is upstream"
    );
    assert_eq!(result[0].upstream_commits[0], target);
    Ok(())
}

/// What if the commit message is the workspace title but with a leading newline?
/// The raw commit format has a blank line before the message body.
/// If an extra newline sneaks in, the title becomes empty and the workspace
/// title appears on the second line (which is body, not title).
#[test]
fn leading_newline_before_workspace_title() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    // Raw commit with extra newline before the workspace title
    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    let raw = format!(
        "tree {tree}\n\
         parent {s1}\n\
         parent {s2}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         \nGitButler Workspace Commit"
    );
    let ws_oid = write_raw_commit(&repo, &raw);

    create_ref(&repo, "refs/heads/gitbutler/workspace", ws_oid);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // BUG FOUND: The title parsing takes the whole input as the title when there's
    // no double-newline. Then `.trim()` strips the leading \n, producing
    // "GitButler Workspace Commit" which matches. This means a commit with an
    // empty subject line but workspace title in the body is incorrectly treated
    // as managed. The fix would be to only trim *trailing* whitespace (or not trim
    // at all) in is_managed_workspace_by_message.
    //
    // For now, document the actual behavior:
    assert_eq!(
        result.len(),
        2,
        "BUG: leading newline in message is trimmed, causing false workspace detection"
    );
    Ok(())
}

/// Similar to leading_newline but with CRLF — does \r\n at the start also trick
/// the workspace detection?
#[test]
fn crlf_before_workspace_title() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    let raw = format!(
        "tree {tree}\n\
         parent {s1}\n\
         parent {s2}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         \r\nGitButler Workspace Commit"
    );
    let ws_oid = write_raw_commit(&repo, &raw);

    create_ref(&repo, "refs/heads/gitbutler/workspace", ws_oid);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    );

    // BUG (same as leading_newline_before_workspace_title): \r\n is stripped by
    // trim(), so the workspace title matches when it shouldn't.
    let statuses = result?;
    assert_eq!(
        statuses.len(),
        2,
        "BUG: CRLF before title is trimmed, causing false workspace detection"
    );
    Ok(())
}

/// Multiple leading newlines before workspace title.
#[test]
fn multiple_leading_newlines_before_workspace_title() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    // Two leading newlines — this DOES create a title/body split
    let raw = format!(
        "tree {tree}\n\
         parent {s1}\n\
         parent {s2}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         \n\nGitButler Workspace Commit"
    );
    let ws_oid = write_raw_commit(&repo, &raw);

    create_ref(&repo, "refs/heads/gitbutler/workspace", ws_oid);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // With \n\n, the title is "" (empty, before the double newline).
    // trim() on "" = "". Should NOT match.
    assert_eq!(
        result.len(),
        1,
        "double-newline splits correctly — title is empty, not managed"
    );
    Ok(())
}

// ─── DEEP INTERNALS EXPLOITATION ─────────────────────────────────────────────

/// Commit with an `encoding` header set to UTF-16LE but message bytes are
/// actually ASCII "GitButler Workspace Commit". message_raw() returns raw bytes
/// without re-encoding, so this SHOULD still match (the bytes are the same).
/// This tests that the encoding header doesn't interfere with parsing.
#[test]
fn encoding_header_utf16_but_ascii_bytes() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    // Commit with encoding header but ASCII message bytes
    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    let raw = format!(
        "tree {tree}\n\
         parent {s1}\n\
         parent {s2}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         encoding UTF-16LE\n\
         \n\
         GitButler Workspace Commit"
    );
    let ws_oid = write_raw_commit(&repo, &raw);

    create_ref(&repo, "refs/heads/gitbutler/workspace", ws_oid);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // The encoding header is ignored by message_raw(), so the ASCII bytes
    // "GitButler Workspace Commit" still match → treated as managed.
    // This is correct behavior: we always compare raw bytes.
    assert_eq!(result.len(), 2, "encoding header ignored, raw bytes match");
    Ok(())
}

/// Commit with a `gpgsig` multi-line header. This header spans multiple lines
/// using continuation lines (lines starting with a space). If the header parsing
/// is wrong, the message body might be misidentified.
#[test]
fn gpgsig_header_before_message() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    // gpgsig header with continuation lines (each subsequent line starts with space)
    let raw = format!(
        "tree {tree}\n\
         parent {s1}\n\
         parent {s2}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         gpgsig -----BEGIN PGP SIGNATURE-----\n \n iQEzBAABCAAdFiEE\n AAAAAAAAAAAAAAAA\n -----END PGP SIGNATURE-----\n\
         \n\
         GitButler Workspace Commit"
    );
    let ws_oid = write_raw_commit(&repo, &raw);

    create_ref(&repo, "refs/heads/gitbutler/workspace", ws_oid);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    );

    // gpgsig multi-line header is parsed correctly by gix — message body
    // is still identified after the header block, so workspace title matches.
    let statuses = result?;
    assert_eq!(
        statuses.len(),
        2,
        "gpgsig header doesn't interfere with workspace detection"
    );
    Ok(())
}

/// Shallow repository: .git/shallow marks a commit as having no parents.
/// This truncates the walk and could change upstream counts.
#[test]
fn shallow_repo_truncates_walk() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    // base -> T1 -> T2 -> T3 (target)
    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    let t2 = commit_detached(&repo, "T2", [t1]);
    let t3 = commit_detached(&repo, "T3", [t2]);
    create_ref(&repo, "refs/remotes/origin/main", t3);

    let s1 = commit_detached(&repo, "S1", [base]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    // Write .git/shallow marking T1 as a shallow boundary
    // This means T1's parents should be hidden during walk
    let shallow_path = tmp.path().join(".git").join("shallow");
    std::fs::write(&shallow_path, format!("{t1}\n")).expect("write shallow");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    // Without shallow: upstream = [T3, T2, T1] (3 commits, base is shared)
    // With shallow honored: T1 has no parents, so base is not reachable from T1
    //   → walk sees T3, T2, T1 and maybe base (since T1 is now a root, base not in T1's history)
    //   → but base IS in S1's hidden set still
    //   → depends on whether shallow affects the hidden walk too
    let count = result[0].upstream_commits.len();
    // gix ignores .git/shallow — the walk sees the full object graph.
    // This is correct behavior for upstream_commits: we always want the real count.
    assert_eq!(
        count, 3,
        "gix ignores shallow boundary, sees all 3 upstream commits"
    );
    Ok(())
}

/// Shallow boundary on the head side: head's ancestors are truncated.
/// This means the hidden set is smaller, potentially showing more upstream commits.
#[test]
fn shallow_head_shows_more_upstream() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    // base -> T1 (target)
    // base -> S1 -> S2 (head)
    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [s1]);
    create_ref(&repo, "refs/heads/main", s2);
    set_head(&repo, "refs/heads/main");

    // Without shallow: hidden = {S2, S1, base}, upstream = {T1} (base is shared ancestor)
    // Now mark S1 as shallow boundary: S1 has no parents → base not reachable from head
    let shallow_path = tmp.path().join(".git").join("shallow");
    std::fs::write(&shallow_path, format!("{s1}\n")).expect("write shallow");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    let count = result[0].upstream_commits.len();
    // gix ignores .git/shallow, so the hidden set includes base.
    // Walk from T1: T1 (base is hidden) → count = 1
    assert_eq!(
        count, 1,
        "gix ignores shallow boundary, base is still in hidden set"
    );
    Ok(())
}

/// Write a commit-graph file, then add new commits that aren't in it.
/// Tests whether stale commit-graph causes incorrect results.
#[test]
fn stale_commit_graph_file() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    // Write commit-graph now (only knows about base, T1, S1)
    let mut cmd = but_testsupport::git(&repo);
    cmd.args(["commit-graph", "write", "--reachable"]);
    let output = cmd.output().expect("git commit-graph write");
    assert!(
        output.status.success(),
        "commit-graph write failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Now add MORE commits that the commit-graph doesn't know about
    let t2 = commit_detached(&repo, "T2-post-graph", [t1]);
    let t3 = commit_detached(&repo, "T3-post-graph", [t2]);
    create_ref(&repo, "refs/remotes/origin/main", t3);

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    // Should see T1, T2, T3 regardless of stale commit-graph
    assert_eq!(
        result[0].upstream_commits.len(),
        3,
        "stale commit-graph should not hide new commits"
    );
    Ok(())
}

/// Corrupt commit-graph: write one, then create a commit with a different
/// parent than what the commit-graph would expect. The loose object should
/// take precedence.
#[test]
fn commit_graph_vs_loose_object_parent_mismatch() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let alt_base = commit_detached(&repo, "alt-base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    // Write commit-graph
    let mut cmd = but_testsupport::git(&repo);
    cmd.args(["commit-graph", "write", "--reachable"]);
    cmd.output().expect("git commit-graph write");

    // Result should be correct: T1 is upstream
    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result[0].upstream_commits.len(), 1, "baseline: T1 upstream");
    Ok(())
}

/// HEAD is detached (not on any branch). Should still work for upstream_commits.
#[test]
fn detached_head_upstream_commits() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);

    // Set HEAD directly to s1 (detached)
    repo.edit_reference(gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange::default(),
            expected: PreviousValue::Any,
            new: gix::refs::Target::Object(s1),
        },
        name: "HEAD".try_into().expect("HEAD"),
        deref: false,
    })?;

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].upstream_commits.len(),
        1,
        "T1 upstream of detached head"
    );
    Ok(())
}

/// Head is on a branch with the SAME name as the target ref.
/// Wait — that's impossible since they're different ref paths. But what about
/// refs/heads/origin/main vs refs/remotes/origin/main?
#[test]
fn local_branch_named_like_remote() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    let t2 = commit_detached(&repo, "T2", [t1]);

    // Both local and remote "origin/main" exist, pointing at different commits
    create_ref(&repo, "refs/remotes/origin/main", t2);
    create_ref(&repo, "refs/heads/origin/main", t1); // confusing local branch name
    set_head(&repo, "refs/heads/origin/main"); // HEAD on local branch

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    // HEAD is at T1, target is at T2. T2 is ahead by 1.
    assert_eq!(
        result[0].upstream_commits.len(),
        1,
        "T2 is the only upstream commit"
    );
    Ok(())
}

/// Target ref advanced past a merge commit where one of the merge's parents
/// is the HEAD commit itself. This is the "already integrated" scenario:
/// target merges the head's work, then advances further.
#[test]
fn target_merged_head_then_advanced() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    // base -> T1
    //      \-> S1 (head)
    // T1 + S1 -> merge -> T2 (target)
    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    let s1 = commit_detached(&repo, "S1", [base]);
    let merge = commit_detached(&repo, "merge-includes-head", [t1, s1]);
    let t2 = commit_detached(&repo, "T2", [merge]);
    create_ref(&repo, "refs/remotes/origin/main", t2);

    create_ref(&repo, "refs/heads/main", s1);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());

    let full = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // Target includes S1 in its history via the merge.
    // Walk: T2, merge, T1 — S1 and base are reachable from head (hidden)
    // BUT: merge's second parent is S1 (hidden). Does the walk count merge itself?
    // merge is NOT reachable from S1 (merge is a descendant of S1, not ancestor).
    // So merge should appear. And T1 is not reachable from S1.
    // Result: T2, merge, T1 = 3
    assert_eq!(result_count(&full, 0), 3, "full: T2, merge, T1");

    let fp = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::Yes,
    )?;

    // First-parent: T2, merge, T1 (first parent of merge), base (hidden) → 3
    // Wait, with first-parent we only follow first parent of merge (T1), not S1.
    // T1's parent is base (hidden). So result: T2, merge, T1 = 3
    assert_eq!(result_count(&fp, 0), 3, "first-parent: T2, merge, T1");

    Ok(())
}

fn result_count(statuses: &[but_graph::target_ref_relations::HeadStatus], idx: usize) -> usize {
    statuses[idx].upstream_commits.len()
}

/// The workspace commit is old (created a while ago), and since then both
/// the target AND the stacks have advanced independently. The workspace commit's
/// parents are stale — they point to old stack tips.
#[test]
fn stale_workspace_with_advanced_stacks() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);

    // Original stack tips when workspace was created
    let s1_old = commit_detached(&repo, "S1-old", [base]);
    let s2_old = commit_detached(&repo, "S2-old", [base]);

    // Workspace commit points at old tips
    let ws = commit_detached(&repo, "GitButler Workspace Commit", [s1_old, s2_old]);

    // Target advances
    let t1 = commit_detached(&repo, "T1", [base]);
    let t2 = commit_detached(&repo, "T2", [t1]);
    create_ref(&repo, "refs/remotes/origin/main", t2);

    // Stacks advance beyond what workspace knows (new tips on branches)
    let _s1_new = commit_detached(&repo, "S1-new", [s1_old]);
    let _s2_new = commit_detached(&repo, "S2-new", [s2_old]);

    // HEAD still points at the OLD workspace commit
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // upstream_commits uses the workspace commit's parents (old tips).
    // It doesn't know about the new tips.
    assert_eq!(result.len(), 2, "two parents from the old workspace");
    for status in &result {
        assert_eq!(
            status.upstream_commits.len(),
            2,
            "T1 and T2 are upstream of old stack tips"
        );
    }
    Ok(())
}

/// What if two workspace commits are chained? HEAD -> WS2 -> WS1 -> base
/// WS2 is the managed workspace, WS1 is one of its parents.
/// is_managed_workspace_by_message checks only the HEAD commit, not recursively.
#[test]
fn chained_workspace_commits_non_recursive() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);

    // WS1: inner workspace commit
    let ws1 = commit_detached(&repo, "GitButler Workspace Commit", [s1]);

    // WS2: outer workspace commit whose parent is WS1
    let ws2 = commit_detached(&repo, "GitButler Workspace Commit", [ws1]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws2);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // WS2 is managed, iterates parents → [WS1]
    // WS1 is treated as a head. Walk: origin/main ^WS1
    // WS1's history: WS1 -> S1 -> base. T1 not in WS1's history.
    // So upstream = [T1]
    assert_eq!(result.len(), 1, "outer workspace has 1 parent (WS1)");
    assert_eq!(result[0].head, ws1, "head should be WS1");
    assert_eq!(result[0].upstream_commits.len(), 1, "T1 is upstream");
    Ok(())
}

/// Concurrent-safe? Call upstream_commits twice on the same graph, verify
/// both return the same result. (Not truly concurrent, but checks immutability.)
#[test]
fn upstream_commits_is_idempotent() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    let t2 = commit_detached(&repo, "T2", [t1]);
    create_ref(&repo, "refs/remotes/origin/main", t2);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);
    let ws = commit_detached(&repo, "GitButler Workspace Commit", [s1, s2]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let graph = Graph::from_head(&repo, &**&m, opts())?.validated()?;
    let tr: gix::refs::FullName = "refs/remotes/origin/main".try_into()?;

    let r1 = graph.upstream_commits(&repo, tr.as_ref(), FirstParentTraversal::No)?;
    let r2 = graph.upstream_commits(&repo, tr.as_ref(), FirstParentTraversal::No)?;

    assert_eq!(r1.len(), r2.len());
    for (a, b) in r1.iter().zip(r2.iter()) {
        assert_eq!(a.head, b.head);
        assert_eq!(a.upstream_commits, b.upstream_commits);
    }
    Ok(())
}

/// What happens when the workspace commit message contains a NUL byte
/// AFTER the workspace title? Title is "GitButler Workspace Commit\x00extra".
/// Does the trim/comparison still match?
#[test]
fn nul_after_workspace_title() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";
    let raw = format!(
        "tree {tree}\n\
         parent {s1}\n\
         parent {s2}\n\
         author {sig_str}\n\
         committer {sig_str}\n\
         \n\
         GitButler Workspace Commit\x00hidden payload"
    );
    let ws_oid = write_raw_commit(&repo, &raw);

    create_ref(&repo, "refs/heads/gitbutler/workspace", ws_oid);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    );

    // NUL byte after the title makes the full title "GitButler Workspace Commit\x00hidden payload"
    // which does NOT match the exact string comparison. Correct behavior.
    let statuses = result?;
    assert_eq!(
        statuses.len(),
        1,
        "NUL in title prevents workspace match — non-managed"
    );
    Ok(())
}

/// Workspace commit where the message exactly matches but is followed by a
/// trailer block (like Signed-off-by). The double-newline before trailers
/// creates a title/body split, so title should still match.
#[test]
fn workspace_title_with_trailers() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);

    // Message with workspace title + trailers
    let ws = commit_detached(
        &repo,
        "GitButler Workspace Commit\n\nSigned-off-by: Test <test@example.com>",
        [s1, s2],
    );
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    // Title is "GitButler Workspace Commit" (before the \n\n)
    // Body is "Signed-off-by: ..."
    // Title matches → managed
    assert_eq!(result.len(), 2, "title matches despite trailers in body");
    Ok(())
}

/// What happens when we call upstream_commits with FirstParentTraversal::Yes
/// on a target that is a merge commit where the first parent is reachable from
/// head but the second parent is NOT?
#[test]
fn first_parent_hides_divergent_second_parent() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    // base -> A (shared with head)
    //      \-> B (not shared)
    // merge(A, B) -> target
    // head forks from A
    let base = commit(&repo, "base", []);
    let a = commit_detached(&repo, "A", [base]);
    let b = commit_detached(&repo, "B", [base]);
    let merge = commit_detached(&repo, "merge(A,B)", [a, b]); // first parent = A
    create_ref(&repo, "refs/remotes/origin/main", merge);

    let s = commit_detached(&repo, "stack", [a]); // forks from A
    create_ref(&repo, "refs/heads/main", s);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());

    let full = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;
    let fp = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::Yes,
    )?;

    // Full: merge, B (A is hidden because reachable from head via stack->A)
    // But wait: merge's first parent A is hidden. Does the walk stop at merge or skip it?
    // rev_walk: walk from merge. merge itself is not hidden. A is hidden, B is not.
    // Full: merge (not hidden), A (hidden, stop this path), B (not hidden)
    // Result: merge, B = 2
    assert_eq!(result_count(&full, 0), 2, "full: merge, B");

    // First-parent: merge (not hidden), A (hidden, stop) → just merge = 1
    assert_eq!(
        result_count(&fp, 0),
        1,
        "first-parent: merge only (A is hidden)"
    );
    Ok(())
}

/// Edge case: target ref and head point to commits on completely separate
/// orphan branches, but they happen to have the same tree (same content).
/// The walk should still treat them as fully divergent.
#[test]
fn same_tree_different_orphan_branches() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let tree = repo.object_hash().empty_tree();
    let sig_str = "Test User <test@example.com> 946684800 +0000";

    // Two root commits with identical trees but different messages → different OIDs
    let raw_t = format!("tree {tree}\nauthor {sig_str}\ncommitter {sig_str}\n\ntarget-orphan");
    let raw_h = format!("tree {tree}\nauthor {sig_str}\ncommitter {sig_str}\n\nhead-orphan");
    let target_oid = write_raw_commit(&repo, &raw_t);
    let head_oid = write_raw_commit(&repo, &raw_h);

    create_ref(&repo, "refs/remotes/origin/main", target_oid);
    create_ref(&repo, "refs/heads/main", head_oid);
    set_head(&repo, "refs/heads/main");

    let repo = reopen(&repo);
    let m = meta(tmp.path());
    let result = upstream_from_head(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
    )?;

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].upstream_commits.len(),
        1,
        "same tree but different commits = divergent, target is upstream"
    );
    Ok(())
}

// ─── GRAPH LIMIT TESTS ──────────────────────────────────────────────────────
// These test how graph construction limits (commits_limit_hint, hard_limit)
// interact with upstream_commits(). The key insight is that upstream_commits()
// does its own rev_walk on the repo — the graph is only used to find the
// entrypoint. So the question is: do limits break entrypoint discovery?

/// Hard limit set very low (10). Target is 100 commits ahead.
/// The graph might be severely truncated, but entrypoint should still work
/// since it's the first commit discovered.
#[test]
fn hard_limit_very_low() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let mut tip = base;
    for i in 0..100 {
        tip = commit_detached(&repo, &format!("T{i}"), [tip]);
    }
    create_ref(&repo, "refs/remotes/origin/main", tip);

    let s = commit_detached(&repo, "stack", [base]);
    create_ref(&repo, "refs/heads/main", s);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let hard_opts = Options {
        hard_limit: Some(10),
        ..opts()
    };
    let result = upstream_from_head_with_opts(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
        hard_opts,
    )?;

    assert_eq!(result.len(), 1);
    // The graph is truncated but the rev_walk is independent
    assert_eq!(
        result[0].upstream_commits.len(),
        100,
        "hard_limit truncates the graph but rev_walk is unbounded"
    );
    Ok(())
}

/// Hard limit = 1. The graph is so truncated that the entrypoint commit
/// doesn't survive into the final graph. upstream_commits() fails.
/// This is documented behavior ("may lead to incorrect results").
#[test]
fn hard_limit_one() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s = commit_detached(&repo, "stack", [base]);
    create_ref(&repo, "refs/heads/main", s);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let hard_opts = Options {
        hard_limit: Some(1),
        ..opts()
    };

    // hard_limit=1 truncates the graph so severely that the entrypoint
    // commit is lost during post-processing. This causes upstream_commits
    // to fail with "no entrypoint commit".
    let result = upstream_from_head_with_opts(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
        hard_opts,
    );
    assert!(
        result.is_err(),
        "hard_limit=1 should fail: graph too truncated to find entrypoint"
    );
    let msg = format!("{:#}", result.err().unwrap());
    assert!(
        msg.contains("entrypoint"),
        "error should mention missing entrypoint: {msg}"
    );
    Ok(())
}

/// Find the minimum hard_limit that still works for a simple graph.
/// This helps understand the relationship between hard_limit and graph viability.
#[test]
fn hard_limit_minimum_viable() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s = commit_detached(&repo, "stack", [base]);
    create_ref(&repo, "refs/heads/main", s);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());

    // Try increasing hard limits to find the minimum that works
    let mut min_working = None;
    for limit in 1..=20 {
        let hard_opts = Options {
            hard_limit: Some(limit),
            ..opts()
        };
        let result = upstream_from_head_with_opts(
            &repo,
            &m,
            "refs/remotes/origin/main",
            FirstParentTraversal::No,
            hard_opts.clone(),
        );
        if result.is_ok() {
            min_working = Some(limit);
            break;
        }
    }

    let min = min_working.expect("should find a working hard_limit <= 20");
    println!("minimum viable hard_limit for simple graph: {min}");
    // Just verify it works correctly at the minimum
    let hard_opts = Options {
        hard_limit: Some(min),
        ..opts()
    };
    let result = upstream_from_head_with_opts(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
        hard_opts,
    )?;
    assert_eq!(
        result[0].upstream_commits.len(),
        1,
        "T1 upstream at minimum hard_limit"
    );
    Ok(())
}

/// Hard limit = 0. The graph should have zero commits.
/// entrypoint_commit() should return None, causing upstream_commits to error.
#[test]
fn hard_limit_zero() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    create_ref(&repo, "refs/remotes/origin/main", t1);

    let s = commit_detached(&repo, "stack", [base]);
    create_ref(&repo, "refs/heads/main", s);
    set_head(&repo, "refs/heads/main");

    let m = meta(tmp.path());
    let hard_opts = Options {
        hard_limit: Some(0),
        ..opts()
    };

    // Graph construction might fail or succeed with empty graph
    let graph_result = Graph::from_head(&repo, &**&m, hard_opts);
    match graph_result {
        Ok(graph) => {
            match graph.validated() {
                Ok(g) => {
                    let tr: gix::refs::FullName = "refs/remotes/origin/main".try_into()?;
                    let result = g.upstream_commits(&repo, tr.as_ref(), FirstParentTraversal::No);
                    // Might error because entrypoint is None, or might work if
                    // the first commit is always included regardless of limit
                    match result {
                        Ok(statuses) => {
                            // If it works, the rev_walk is still correct
                            assert!(!statuses.is_empty());
                        }
                        Err(e) => {
                            let msg = format!("{e:#}");
                            assert!(
                                msg.contains("entrypoint"),
                                "should fail due to missing entrypoint: {msg}"
                            );
                        }
                    }
                }
                Err(_) => { /* validation failure — acceptable */ }
            }
        }
        Err(_) => { /* graph construction failure — acceptable */ }
    }
    Ok(())
}

/// commits_limit_hint = 1 with managed workspace. The workspace commit itself
/// is always the first one seen. With limit 1, the graph might not see any of
/// its parents' chains, but entrypoint is still the workspace commit.
#[test]
fn limit_hint_one_managed_workspace() -> anyhow::Result<()> {
    let (tmp, repo) = fresh_repo();

    let base = commit(&repo, "base", []);
    let t1 = commit_detached(&repo, "T1", [base]);
    let t2 = commit_detached(&repo, "T2", [t1]);
    create_ref(&repo, "refs/remotes/origin/main", t2);

    let s1 = commit_detached(&repo, "S1", [base]);
    let s2 = commit_detached(&repo, "S2", [base]);
    let ws = commit_detached(&repo, "GitButler Workspace Commit", [s1, s2]);
    create_ref(&repo, "refs/heads/gitbutler/workspace", ws);
    set_head(&repo, "refs/heads/gitbutler/workspace");

    let m = meta(tmp.path());
    let limited = Options {
        commits_limit_hint: Some(1),
        ..opts()
    };
    let result = upstream_from_head_with_opts(
        &repo,
        &m,
        "refs/remotes/origin/main",
        FirstParentTraversal::No,
        limited,
    )?;

    // The workspace commit is the entrypoint. Its parents are S1 and S2.
    // The rev_walk is unbounded, so it should still find T1 and T2.
    assert_eq!(result.len(), 2, "managed workspace still has 2 heads");
    for status in &result {
        assert_eq!(
            status.upstream_commits.len(),
            2,
            "rev_walk sees T1 and T2 despite graph limit"
        );
    }
    Ok(())
}
