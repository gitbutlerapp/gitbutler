use but_core::DiffSpec;
use but_testsupport::{assure_stable_env, hunk_header, visualize_commit_graph};
use but_workspace::{
    commit_engine::{Destination, StackSegmentId},
    legacy::commit_engine::ReferenceFrame,
};
use gitbutler_stack::VirtualBranchesState;
use gix::{prelude::ObjectIdExt, refs::transaction::PreviousValue};

use crate::{
    commit_engine::{
        refs_update::utils::{
            graph_commit_outcome, has_signature, stack_with_branches, write_vrbranches_to_refs,
            write_worktree_file,
        },
        utils::assure_no_worktree_changes,
    },
    utils::{
        CONTEXT_LINES, read_only_in_memory_scenario, to_change_specs_all_hunks,
        to_change_specs_whole_file, visualize_index, visualize_index_with_content, visualize_tree,
        worktree_changes_with_diffs, writable_scenario, writable_scenario_with_ssh_key,
        write_sequence,
    },
};

#[test]
fn new_commits_to_tip_from_unborn_head() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("unborn-untracked");
    let mut vb = VirtualBranchesState::default();
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: None,
            message: "initial commit".to_string(),
            stack_segment: None,
        },
        to_change_specs_whole_file(but_core::diff::worktree_changes(&repo)?),
        CONTEXT_LINES,
    )?;

    let new_commit_id = outcome.new_commit.expect("a new commit was created");
    assert_eq!(
        repo.head_id()?,
        new_commit_id,
        "the HEAD reference was updated, HEAD is now born",
    );

    // The head was updated, along with the ref that it points to.
    insta::assert_snapshot!(visualize_commit_graph(&repo, new_commit_id)?, @"* 3dd3955 (HEAD -> main) initial commit");
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    861d6e2
    └── not-yet-tracked:100644:d95f3ad "content\n"
    "#);
    assure_no_worktree_changes(&repo)?;

    write_worktree_file(&repo, "new-file", "other content")?;
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(new_commit_id),
            message: "second commit".to_string(),
            stack_segment: None,
        },
        to_change_specs_whole_file(but_core::diff::worktree_changes(&repo)?),
        CONTEXT_LINES,
    )?;
    // The HEAD reference was updated.
    insta::assert_snapshot!(graph_commit_outcome(&repo, &outcome)?, @r"
    * 64c4463 (HEAD -> main) second commit
    * 3dd3955 initial commit
    ");

    // Create another tip at the same location as head to see if it gets updated as well.
    // Tags are never updated.
    let new_commit_id = outcome.new_commit.expect("a new commit was created");
    repo.reference(
        "refs/heads/another-tip",
        new_commit_id,
        PreviousValue::Any,
        "the log message",
    )?;
    repo.tag_reference(
        "tag-that-should-not-move",
        new_commit_id,
        PreviousValue::Any,
    )?;

    write_worktree_file(&repo, "new-file", "change")?;
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(new_commit_id),
            message: "third commit".to_string(),
            stack_segment: None,
        },
        to_change_specs_whole_file(but_core::diff::worktree_changes(&repo)?),
        CONTEXT_LINES,
    )?;

    // The HEAD reference was updated, along with all other tag-references that pointed to it.
    let new_commit = outcome.new_commit.expect("a new commit was created");
    insta::assert_snapshot!(visualize_commit_graph(&repo, new_commit)?, @r"
    * b780e49 (HEAD -> main) third commit
    * 64c4463 (tag: tag-that-should-not-move, another-tip) second commit
    * 3dd3955 initial commit
    ");

    write_worktree_file(&repo, "new-file", "yet another change")?;
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
        },
        &mut vb,
        Destination::AmendCommit {
            commit_id: new_commit,
            new_message: None,
        },
        to_change_specs_whole_file(but_core::diff::worktree_changes(&repo)?),
        CONTEXT_LINES,
    )?;

    assure_no_worktree_changes(&repo)?;
    // The top commit has a different hash now thanks to amending.
    insta::assert_snapshot!(graph_commit_outcome(&repo, &outcome)?, @r"
    * 6073a81 (HEAD -> main) third commit
    * 64c4463 (tag: tag-that-should-not-move, another-tip) second commit
    * 3dd3955 initial commit
    ");

    assert_eq!(vb, VirtualBranchesState::default(), "Nothing changed yet");
    let head_id = repo.head_id()?.detach();
    let stack = stack_with_branches(
        "s1",
        head_id,
        [
            ("s1-b/second", repo.rev_parse_single("@")?.detach()),
            ("s1-b/first", repo.rev_parse_single("@~1")?.detach()),
            ("s1-b/init", repo.rev_parse_single("@~2")?.detach()),
        ],
        &repo,
    );
    vb.branches.insert(stack.id, stack);

    let stack = stack_with_branches(
        "s2",
        head_id,
        [
            ("s2-b/second", repo.rev_parse_single("@")?.detach()),
            ("s2-b/first", repo.rev_parse_single("@~1")?.detach()),
            ("s2-b/init", repo.rev_parse_single("@~2")?.detach()),
        ],
        &repo,
    );
    vb.branches.insert(stack.id, stack);

    write_worktree_file(&repo, "new-file", "the final change")?;
    let new_commit = outcome.new_commit.unwrap();
    let mut outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(new_commit),
            message: "fourth commit".to_string(),
            stack_segment: None,
        },
        to_change_specs_whole_file(but_core::diff::worktree_changes(&repo)?),
        CONTEXT_LINES,
    )?;

    // Updated references are visible (but probably nobody needs them).
    let index = outcome.index.take().unwrap();
    insta::assert_debug_snapshot!(outcome, @r#"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(28868dd070be350f335ad8869c728343fa2929f8),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(273aeca7ca98af0f7972af6e7859a3ae7fde497a),
        ),
        references: [
            UpdatedReference {
                reference: Git(
                    FullName(
                        "refs/heads/main",
                    ),
                ),
                old_commit_id: Sha1(6073a81d14db7169b56ac39bcf59f906df532302),
                new_commit_id: Sha1(28868dd070be350f335ad8869c728343fa2929f8),
            },
            UpdatedReference {
                reference: Virtual(
                    "s1-b/second",
                ),
                old_commit_id: Sha1(6073a81d14db7169b56ac39bcf59f906df532302),
                new_commit_id: Sha1(28868dd070be350f335ad8869c728343fa2929f8),
            },
            UpdatedReference {
                reference: Virtual(
                    "s2-b/second",
                ),
                old_commit_id: Sha1(6073a81d14db7169b56ac39bcf59f906df532302),
                new_commit_id: Sha1(28868dd070be350f335ad8869c728343fa2929f8),
            },
        ],
        rebase_output: None,
        index: None,
    }
    "#);
    write_vrbranches_to_refs(&vb, &repo)?;
    // It updates stack heads and stack branch heads.
    insta::assert_snapshot!(graph_commit_outcome(&repo, &outcome)?, @r"
    * 28868dd (HEAD -> main, s2-b/second, s1-b/second) fourth commit
    * 6073a81 third commit
    * 64c4463 (tag: tag-that-should-not-move, s2-b/first, s1-b/first, another-tip) second commit
    * 3dd3955 (s2-b/init, s1-b/init) initial commit
    ");
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    273aeca
    ├── new-file:100644:aaa9a91 "the final change"
    └── not-yet-tracked:100644:d95f3ad "content\n"
    "#);

    insta::assert_snapshot!(visualize_index(&index), @r"
    100644:aaa9a91 new-file
    100644:d95f3ad not-yet-tracked
    ");
    assure_no_worktree_changes(&repo)?;
    Ok(())
}

/// A special case for now where a branch stack-branch was added to the workspace, but isn't yet
/// in the workspace commit. See https://github.com/gitbutlerapp/gitbutler/pull/7976 for details.
/// https://github.com/gitbutlerapp/gitbutler/pull/7596 may affect the solution here, but
/// it's not yet ready.
#[test]
fn new_stack_receives_commit_and_adds_it_to_workspace_commit() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("three-commits-with-line-offset-and-workspace-commit");

    let mut vb = VirtualBranchesState::default();
    let uninitialized_stack_id = repo.rev_parse_single("@~2")?.detach();
    let initial_stack_id = repo.rev_parse_single("@~1")?.detach();
    let workspace_commit_id = repo.rev_parse_single("@")?.detach();
    insta::assert_snapshot!(visualize_commit_graph(&repo, workspace_commit_id)?, @r"
    * 47c9e16 (HEAD -> main) GitButler Workspace Commit
    * b451685 (feat1) insert 5 lines to the top
    * d15b5ae (tag: first-commit) init
    ");

    let stack = stack_with_branches(
        "s1",
        workspace_commit_id,
        [("s1/top", initial_stack_id)],
        &repo,
    );
    vb.branches.insert(stack.id, stack);
    let stack = stack_with_branches(
        "s2",
        uninitialized_stack_id,
        [("s2/top", uninitialized_stack_id)],
        &repo,
    );
    vb.branches.insert(stack.id, stack);

    write_sequence(&repo, "new-file", [(15, None)])?;
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: Some(workspace_commit_id),
            branch_tip: Some(uninitialized_stack_id),
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(uninitialized_stack_id),
            message: "new file with 15 lines".into(),
            stack_segment: None,
        },
        to_change_specs_whole_file(but_core::diff::worktree_changes(&repo)?),
        CONTEXT_LINES,
    )?;

    write_vrbranches_to_refs(&vb, &repo)?;
    // head was updated to point to the new workspace commit.
    insta::assert_snapshot!(visualize_commit_graph(&repo, repo.head_id()?)?, @r"
    *   ed11351 (HEAD -> main) GitButler Workspace Commit
    |\  
    | * 2ed9fca (s2/top) new file with 15 lines
    * | b451685 (s1/top, feat1) insert 5 lines to the top
    |/  
    * d15b5ae (tag: first-commit) init
    ");

    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    6e57057
    ├── file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    └── new-file:100644:97b3d1a "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n"
    "#);
    assert_eq!(
        but_core::diff::worktree_changes(&repo)?.changes.len(),
        0,
        "There seems to be no change as the new commit contains the new file, which is part of the workspace now"
    );
    Ok(())
}

/// There is an untracked file with multiple lines, and we commit only a couple of them.
#[test]
fn first_partial_commit_to_tip_from_unborn_head() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("unborn-untracked");
    write_sequence(&repo, "not-yet-tracked", [(4, None)])?;
    let mut vb = VirtualBranchesState::default();

    insta::assert_debug_snapshot!(worktree_changes_with_diffs(&repo)?, @r#"
    [
        (
            TreeChange {
                path: "not-yet-tracked",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    is_untracked: true,
                },
            },
            [
                DiffHunk(""@@ -1,0 +1,4 @@\n+1\n+2\n+3\n+4\n""),
            ],
        ),
    ]
    "#);

    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: None,
            message: "initial commit with two lines".to_string(),
            stack_segment: None,
        },
        vec![DiffSpec {
            previous_path: None,
            path: "not-yet-tracked".into(),
            // Add the first two lines
            hunk_headers: vec![hunk_header("-0,0", "+1,2")],
        }],
        CONTEXT_LINES,
    )?;

    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    d5949f1
    └── not-yet-tracked:100644:1191247 "1\n2\n"
    "#);
    insta::assert_snapshot!(visualize_index_with_content(&repo, &outcome.index.unwrap()), @r#"100644:1191247 not-yet-tracked "1\n2\n""#);

    // There are still untracked changes.
    insta::assert_debug_snapshot!(worktree_changes_with_diffs(&repo)?, @r#"
    [
        (
            TreeChange {
                path: "not-yet-tracked",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(1191247b6d9a206f6ba3d8ac79e26d041dd86941),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
            [
                DiffHunk(""@@ -3,0 +3,2 @@\n+3\n+4\n""),
            ],
        ),
    ]
    "#);

    let head_commit = outcome.new_commit.unwrap();
    insta::assert_snapshot!(visualize_commit_graph(&repo, head_commit)?, @"* 5284afd (HEAD -> main) initial commit with two lines");

    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(head_commit),
            message: "Add yet another line".to_string(),
            stack_segment: None,
        },
        vec![DiffSpec {
            previous_path: None,
            path: "not-yet-tracked".into(),
            // Add the last line
            hunk_headers: vec![hunk_header("-0,0", "+4,1")],
        }],
        CONTEXT_LINES,
    )?;
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    f9cc7d6
    └── not-yet-tracked:100644:e8a01cd "1\n2\n4\n"
    "#);

    // One line left as changed
    insta::assert_debug_snapshot!(worktree_changes_with_diffs(&repo)?, @r#"
    [
        (
            TreeChange {
                path: "not-yet-tracked",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(e8a01cd985125a824409ef6b2f264c76b54ffe6a),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
            [
                DiffHunk(""@@ -3,0 +3,1 @@\n+3\n""),
            ],
        ),
    ]
    "#);

    let head_commit = outcome.new_commit.unwrap();
    insta::assert_snapshot!(visualize_commit_graph(&repo, head_commit)?, @r"
    * b43af70 (HEAD -> main) Add yet another line
    * 5284afd initial commit with two lines
    ");

    write_sequence(&repo, "other-untracked-non-racy", [(4, None)])?;
    std::fs::write(
        repo.workdir_path("other-untracked-added-as-whole").unwrap(),
        b"just one line",
    )?;
    // With a racy index, we will see changes even though otherwise it might not detect them as it trusts mtimes that it shouldn't.
    // Git only has second-level precision, so need to wait that long at least.
    // See #8213.
    let delay_to_assure_non_racy_git_index = std::time::Duration::from_secs(1);
    std::thread::sleep(delay_to_assure_non_racy_git_index);
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(head_commit),
            message: "add a part of an untracked file, again".to_string(),
            stack_segment: None,
        },
        vec![
            DiffSpec {
                previous_path: None,
                path: "not-yet-tracked".into(),
                // Add the remainder
                hunk_headers: vec![hunk_header("-3,0", "+3,1")],
            },
            DiffSpec {
                previous_path: None,
                path: "other-untracked-non-racy".into(),
                // Add the first line
                hunk_headers: vec![hunk_header("-0,0", "+1,1")],
            },
            DiffSpec {
                previous_path: None,
                path: "other-untracked-added-as-whole".into(),
                // Add the only line
                hunk_headers: vec![hunk_header("-0,0", "+1,1")],
            },
        ],
        CONTEXT_LINES,
    )?;

    let head_commit = outcome.new_commit.unwrap();
    insta::assert_snapshot!(visualize_commit_graph(&repo, head_commit)?, @r"
    * fde2fa5 (HEAD -> main) add a part of an untracked file, again
    * b43af70 Add yet another line
    * 5284afd initial commit with two lines
    ");

    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    6e86388
    ├── not-yet-tracked:100644:94ebaf9 "1\n2\n3\n4\n"
    ├── other-untracked-added-as-whole:100644:19f7fbc "just one line"
    └── other-untracked-non-racy:100644:d00491f "1\n"
    "#);

    // The index represents the tree, so far so good.
    insta::assert_snapshot!(visualize_index_with_content(&repo, &outcome.index.unwrap()), @r#"
    100644:94ebaf9 not-yet-tracked "1\n2\n3\n4\n"
    100644:19f7fbc other-untracked-added-as-whole "just one line"
    100644:d00491f other-untracked-non-racy "1\n"
    "#);

    // three lines left in the worktree, nothing else should show up.
    insta::assert_debug_snapshot!(worktree_changes_with_diffs(&repo)?, @r#"
    [
        (
            TreeChange {
                path: "other-untracked-non-racy",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(d00491fd7e5bb6fa28c517a0bb32b8b506539d4d),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
            [
                DiffHunk(""@@ -2,0 +2,3 @@\n+2\n+3\n+4\n""),
            ],
        ),
    ]
    "#);

    Ok(())
}

#[test]
fn insert_commit_into_single_stack_with_signatures() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario_with_ssh_key("two-signed-commits-with-line-offset");
    let mut vb = VirtualBranchesState::default();
    let initial_commit_id = repo.rev_parse_single("@~1")?.detach();
    let head_commit_id = repo.rev_parse_single("@")?.detach();
    insta::assert_snapshot!(visualize_commit_graph(&repo, head_commit_id)?, @r"
    * 8b9db84 (HEAD -> main) insert 10 lines to the top
    * ecd6722 (tag: first-commit, first-commit) init
    ");

    let stack = stack_with_branches(
        "s1",
        head_commit_id,
        [("s1-b/init", initial_commit_id)],
        &repo,
    );
    vb.branches.insert(stack.id, stack);
    // Add 10 lines to the end.
    write_sequence(&repo, "file", [(30, None)])?;
    let mut outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: Some(head_commit_id),
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(initial_commit_id),
            message: "between initial and former first".to_string(),
            stack_segment: None,
        },
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    // it rewrites the history to the top of the stack.
    write_vrbranches_to_refs(&vb, &repo)?;
    let rewritten_head_id = repo.head_id()?.detach();
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    * a8fbed8 (HEAD -> main) insert 10 lines to the top
    * 170d5fe (s1-b/init) between initial and former first
    * ecd6722 (tag: first-commit, first-commit) init
    ");
    insta::assert_snapshot!(but_testsupport::visualize_tree(rewritten_head_id.attach(&repo)), @r#"
    5fdd313
    ├── .gitignore:100644:ccc87a0 "*.key*\n"
    └── file:100644:e8823e1 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    "#
    );

    outcome.index.take();
    insta::assert_debug_snapshot!(&outcome, @r#"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(170d5fe258ee28dd6de85bfd6d566231c446d8ec),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(5fdd31363b3f0987135feaa00a734ca31e1652d6),
        ),
        references: [
            UpdatedReference {
                reference: Virtual(
                    "",
                ),
                old_commit_id: Sha1(ecd67221705b069c4f46365a46c8f2cd8a97ec19),
                new_commit_id: Sha1(170d5fe258ee28dd6de85bfd6d566231c446d8ec),
            },
            UpdatedReference {
                reference: Git(
                    FullName(
                        "refs/heads/main",
                    ),
                ),
                old_commit_id: Sha1(8b9db8455554fe317ea3ab86b9a042805326b493),
                new_commit_id: Sha1(a8fbed8ea304d850e168033468be9d9f128e17c3),
            },
            UpdatedReference {
                reference: Virtual(
                    "s1-b/init",
                ),
                old_commit_id: Sha1(ecd67221705b069c4f46365a46c8f2cd8a97ec19),
                new_commit_id: Sha1(170d5fe258ee28dd6de85bfd6d566231c446d8ec),
            },
        ],
        rebase_output: Some(
            RebaseOutput {
                top_commit: Sha1(a8fbed8ea304d850e168033468be9d9f128e17c3),
                references: [],
                commit_mapping: [
                    (
                        Some(
                            Sha1(170d5fe258ee28dd6de85bfd6d566231c446d8ec),
                        ),
                        Sha1(8b9db8455554fe317ea3ab86b9a042805326b493),
                        Sha1(a8fbed8ea304d850e168033468be9d9f128e17c3),
                    ),
                ],
            },
        ),
        index: None,
    }
    "#);
    let head_commit = but_core::Commit::from_id(rewritten_head_id.attach(&repo))?;
    assert!(
        head_commit.inner.extra_headers().pgp_signature().is_some(),
        "Rewritten commits are signed if settings permit"
    );
    assert!(
        head_commit.headers().is_some(),
        "rewritten commits always get a special header"
    );
    assert!(!head_commit.is_conflicted());

    write_sequence(&repo, "file", [(40, None)])?;
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: Some(rewritten_head_id),
        },
        &mut vb,
        Destination::AmendCommit {
            commit_id: repo.rev_parse_single("@~1")?.detach(),
            new_message: None,
        },
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;
    let rewritten_head_id = repo.head_id()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    * 07a0229 (HEAD -> main) insert 10 lines to the top
    * d9d87b9 (s1-b/init) between initial and former first
    * ecd6722 (tag: first-commit, first-commit) init
    ");
    insta::assert_snapshot!(but_testsupport::visualize_tree(rewritten_head_id), @r#"
    683b451
    ├── .gitignore:100644:ccc87a0 "*.key*\n"
    └── file:100644:1c99002 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n"
    "#);

    insta::assert_snapshot!(visualize_index(&outcome.index.unwrap()), @r"
    100644:ccc87a0 .gitignore
    100644:1c99002 file
    ");

    assure_no_worktree_changes(&repo)?;
    Ok(())
}

#[test]
fn branch_tip_below_non_merge_workspace_commit() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("two-commits-with-line-offset");

    let mut vb = VirtualBranchesState::default();
    let initial_commit_id = repo.rev_parse_single("@~1")?.detach();
    let head_commit_id = repo.rev_parse_single("@")?.detach();
    insta::assert_snapshot!(visualize_commit_graph(&repo, head_commit_id)?, @r"
    * 40ceac2 (HEAD -> main) insert 20 lines to the top
    * 4342edf (tag: first-commit) init
    ");

    let stack = stack_with_branches(
        "s1",
        head_commit_id,
        [("s1-b/init", initial_commit_id)],
        &repo,
    );
    vb.branches.insert(stack.id, stack);

    write_sequence(&repo, "file", [(110, None)])?;
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: Some(head_commit_id),
            branch_tip: Some(initial_commit_id),
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(initial_commit_id),
            message: "extend lines to 110".into(),
            stack_segment: None,
        },
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    write_vrbranches_to_refs(&vb, &repo)?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, repo.head_id()?)?, @r"
    * 5f32f5c (HEAD -> main) insert 20 lines to the top
    * e798e62 (s1-b/init) extend lines to 110
    * 4342edf (tag: first-commit) init
    ");

    insta::assert_snapshot!(but_testsupport::visualize_tree(outcome.new_commit.unwrap().attach(&repo)), @r#"
    35d7a5e
    └── file:100644:c6fc2ee "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n81\n82\n83\n84\n85\n86\n87\n88\n89\n90\n91\n92\n93\n94\n95\n96\n97\n98\n99\n100\n101\n102\n103\n104\n105\n106\n107\n108\n109\n110\n"
    "#);
    assert_eq!(
        but_core::diff::worktree_changes(&repo)?.changes.len(),
        1,
        "Even though the cherry-pick works, the cherry-pick doesn't produce the worktree\
        lines 20-40 are present in head-commit, but not in worktree"
    );
    Ok(())
}

#[test]
fn deletions() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("delete-all-file-types");
    let head_commit = repo.rev_parse_single("HEAD")?;
    insta::assert_snapshot!(but_testsupport::visualize_tree(head_commit.object()?.peel_to_tree()?.id()), @r#"
    cecc2da
    ├── .gitmodules:100644:51f8807 "[submodule \"submodule\"]\n\tpath = submodule\n\turl = ./embedded-repository\n"
    ├── embedded-repository:160000:a047f81 
    ├── executable:100755:86daf54 "exe\n"
    ├── file-to-remain:100644:d95f3ad "content\n"
    ├── link:120000:b158162 "file-to-remain"
    └── submodule:160000:a047f81
    "#);

    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: Some(head_commit.detach()),
        },
        &mut Default::default(),
        Destination::NewCommit {
            parent_commit_id: Some(head_commit.into()),
            message: "deletions maybe a bit special".into(),
            stack_segment: None,
        },
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    c15318d
    └── file-to-remain:100644:d95f3ad "content\n"
    "#);
    insta::assert_snapshot!(visualize_index(&outcome.index.unwrap()), @"100644:d95f3ad file-to-remain");
    assure_no_worktree_changes(&repo)?;
    Ok(())
}

#[test]
fn insert_commits_into_workspace() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset-two-files");

    let head_commit_id = repo.head_id()?.detach();
    insta::assert_snapshot!(visualize_commit_graph(&repo, head_commit_id)?, @r"
    *   77dbf51 (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 3538622 (A) add 10 to the beginning
    * | e81b470 (B) add 10 to the end
    |/  
    * 9cf2979 (main) init
    ");

    let mut vb = VirtualBranchesState::default();
    let stack1_head = repo.rev_parse_single("merge^1")?.detach();
    let stack = stack_with_branches("s1", head_commit_id, [("s1-b/init", stack1_head)], &repo);
    vb.branches.insert(stack.id, stack);

    // another 10 to the end (HEAD range is 1-30).
    write_sequence(&repo, "file", [(40, None)])?;
    let branch_b = repo.rev_parse_single("B")?.detach();
    let mut outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: Some(repo.rev_parse_single("merge")?.detach()),
            branch_tip: Some(branch_b),
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(branch_b),
            message: "add 10 more lines at end".into(),
            stack_segment: None,
        },
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    write_vrbranches_to_refs(&vb, &repo)?;
    assure_no_worktree_changes(&repo)?;

    let rewritten_head_id = repo.head_id()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    *   7f680d5 (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 3538622 (A) add 10 to the beginning
    * | 9762353 (s1-b/init) add 10 more lines at end
    * | e81b470 (B) add 10 to the end
    |/  
    * 9cf2979 (main) init
    ");
    // The new result is like we'd expect so 10 more lines are added to the full merge result.
    insta::assert_snapshot!(but_testsupport::visualize_tree(rewritten_head_id), @r#"
    401d80f
    ├── file:100644:1c99002 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n"
    └── other-file:100644:a11f0f8 "35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n"
    "#);

    let index = outcome.index.take().unwrap();
    insta::assert_snapshot!(visualize_index(&index), @r"
    100644:1c99002 file
    100644:a11f0f8 other-file
    ");
    Ok(())
}

#[test]
fn insert_commits_into_workspace_with_conflict() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset-two-files");

    let head_commit_id = repo.head_id()?.detach();
    insta::assert_snapshot!(visualize_commit_graph(&repo, head_commit_id)?, @r"
    *   77dbf51 (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 3538622 (A) add 10 to the beginning
    * | e81b470 (B) add 10 to the end
    |/  
    * 9cf2979 (main) init
    ");

    let mut vb = VirtualBranchesState::default();
    let stack1_head = repo.rev_parse_single("main")?.detach();
    let stack = stack_with_branches("s1", head_commit_id, [("s1-b/init", stack1_head)], &repo);
    vb.branches.insert(stack.id, stack);

    // 10 to the beginning, but conflicts with branch A
    write_sequence(&repo, "file", [(1, 9), (11, 19), (21, 30)])?;
    // std::fs::remove_file(repo.workdir_path("file").unwrap())?;
    // 10 to the end, without a conflict.
    write_sequence(&repo, "other-file", [(35, 85)])?;
    let branch_b = repo.rev_parse_single("B")?.detach();
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: Some(repo.rev_parse_single("merge")?.detach()),
            branch_tip: Some(branch_b),
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(branch_b),
            message: "with 'file' conflict, but 'other-file' is fine".into(),
            stack_segment: None,
        },
        to_change_specs_whole_file(but_core::diff::worktree_changes(&repo)?),
        CONTEXT_LINES,
    )
    .expect("the rebase engine should communicate the merge-conflict failure");
    // The failing path is clearly communicated.
    insta::assert_debug_snapshot!(outcome.rejected_specs, @r#"
    [
        (
            WorkspaceMergeConflict,
            DiffSpec {
                previous_path: None,
                path: "file",
                hunk_headers: [],
            },
        ),
    ]
    "#);
    assert_eq!(
        outcome.new_commit, None,
        "No commit could be created as the workspace commit merge failed"
    );

    write_vrbranches_to_refs(&vb, &repo)?;
    // Both files are still changed
    insta::assert_debug_snapshot!(worktree_changes_with_diffs(&repo), @r#"
    Ok(
        [
            (
                TreeChange {
                    path: "file",
                    status: Modification {
                        previous_state: ChangeState {
                            id: Sha1(e8823e1766638e70fd9e260913a383f8fe68a237),
                            kind: Blob,
                        },
                        state: ChangeState {
                            id: Sha1(0000000000000000000000000000000000000000),
                            kind: Blob,
                        },
                        flags: None,
                    },
                },
                [
                    DiffHunk(""@@ -10,1 +10,0 @@\n-10\n""),
                    DiffHunk(""@@ -20,1 +19,0 @@\n-20\n""),
                ],
            ),
            (
                TreeChange {
                    path: "other-file",
                    status: Modification {
                        previous_state: ChangeState {
                            id: Sha1(a11f0f89f5421624d9b9b69db837d91eee93bf43),
                            kind: Blob,
                        },
                        state: ChangeState {
                            id: Sha1(0000000000000000000000000000000000000000),
                            kind: Blob,
                        },
                        flags: None,
                    },
                },
                [
                    DiffHunk(""@@ -42,0 +42,10 @@\n+76\n+77\n+78\n+79\n+80\n+81\n+82\n+83\n+84\n+85\n""),
                ],
            ),
        ],
    )
    "#);

    let unchanged_head_id = repo.head_id()?;
    // there was no change to HEAD.
    insta::assert_snapshot!(visualize_commit_graph(&repo, unchanged_head_id)?, @r"
    *   77dbf51 (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 3538622 (A) add 10 to the beginning
    * | e81b470 (B) add 10 to the end
    |/  
    * 9cf2979 (s1-b/init, main) init
    ");

    Ok(())
}

#[test]
fn workspace_commit_with_merge_conflict() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("merge-with-two-branches-auto-resolved-merge")?;

    let head_commit_id = repo.head_id()?;
    let initial_state = visualize_commit_graph(&repo, head_commit_id)?;
    insta::assert_snapshot!(initial_state, @r"
    *   076bc28 (HEAD -> merge) merge A and B with forced resolution
    |\  
    | * 88d7acc (A) 10 to 20
    * | 47334c6 (B) 20 to 30
    |/  
    * 15bcd1b (main) init
    ");

    let branch_b = repo.rev_parse_single("B")?.detach();
    for destination in [
        Destination::NewCommit {
            parent_commit_id: Some(branch_b),
            message: "rewrite with 30 - 40".into(),
            stack_segment: None,
        },
        Destination::AmendCommit {
            commit_id: branch_b,
            new_message: None,
        },
    ] {
        let out = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
            &repo,
            ReferenceFrame {
                workspace_tip: Some(repo.rev_parse_single("merge")?.detach()),
                branch_tip: Some(branch_b),
            },
            &mut Default::default(),
            destination,
            to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
            CONTEXT_LINES,
        )
        .expect("merge fails but we make it observable");
        insta::allow_duplicates! {
        insta::assert_debug_snapshot!(out, @r#"
        CreateCommitOutcome {
            rejected_specs: [
                (
                    WorkspaceMergeConflict,
                    DiffSpec {
                        previous_path: None,
                        path: "file",
                        hunk_headers: [
                            HunkHeader("-1,10", "+1,0"),
                            HunkHeader("-12,0", "+2,10"),
                        ],
                    },
                ),
            ],
            new_commit: None,
            changed_tree_pre_cherry_pick: None,
            references: [],
            rebase_output: None,
            index: None,
        }
        "#)
        }
    }

    assert_eq!(
        visualize_commit_graph(&repo, repo.head_id()?)?,
        initial_state,
        "nothing actually changed"
    );
    Ok(())
}

#[test]
fn merge_commit_remains_unsigned_in_remerge() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario_with_ssh_key("merge-signed-with-two-branches-line-offset");

    let head_commit_id = repo.head_id()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, head_commit_id)?, @r"
    *   bc9cc8a (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * eede47d (A) add 10 to the beginning
    * | 16fe86e (B) add 10 to the end
    |/  
    * 6074509 (main) init
    ");
    assert!(
        !has_signature(head_commit_id)?,
        "merge commit isn't initially signed, like would be the case in a workspace commit"
    );
    assert!(
        has_signature(repo.rev_parse_single("@~1")?)?,
        "everything else is signed though"
    );

    let branch_a = repo.rev_parse_single("A")?.detach();
    let mut vb = VirtualBranchesState::default();
    let stack = stack_with_branches("s1", branch_a, [("s1-b/top", branch_a)], &repo);
    vb.branches.insert(stack.id, stack);

    // initial is 1-30, remove first 5
    write_sequence(&repo, "file", [(5, 30)])?;
    let mut outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: Some(head_commit_id.detach()),
            branch_tip: Some(branch_a),
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(branch_a),
            message: "remove 5 lines from beginning".into(),
            stack_segment: None,
        },
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    write_vrbranches_to_refs(&vb, &repo)?;

    let rewritten_head_id = repo.head_id()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    *   7044c9b (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 12d8f47 (s1-b/top) remove 5 lines from beginning
    | * eede47d (A) add 10 to the beginning
    * | 16fe86e (B) add 10 to the end
    |/  
    * 6074509 (main) init
    ");
    assert!(
        has_signature(repo.rev_parse_single("A")?)?,
        "The new commit is signed because the settings permit it"
    );
    assert!(
        !has_signature(rewritten_head_id)?,
        "It detects this case and doesn't resign the merge - it wasn't signed before"
    );

    // The tree at HEAD has the right state.
    insta::assert_snapshot!(but_testsupport::visualize_tree(rewritten_head_id), @r#"
    4b16750
    ├── .gitignore:100644:ccc87a0 "*.key*\n"
    └── file:100644:c8ebab8 "5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    "#);
    assert_eq!(
        outcome.rejected_specs,
        vec![],
        "no patch gets rejected here"
    );

    let index = outcome.index.take().unwrap();
    insta::assert_snapshot!(visualize_index(&index), @r"
    100644:ccc87a0 .gitignore
    100644:c8ebab8 file
    ");

    assure_no_worktree_changes(&repo)?;
    Ok(())
}

#[test]
fn two_commits_three_buckets_disambiguate_insertion_position_to_one_below_top() -> anyhow::Result<()>
{
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("two-commits-three-buckets");
    // duplicate the existing branches into VB
    let branch_b = repo.rev_parse_single("B")?.detach();
    let mut vb = VirtualBranchesState::default();
    let stack = stack_with_branches(
        "C",
        branch_b,
        [
            ("A", repo.rev_parse_single("A")?.detach()),
            ("B", branch_b),
            ("C", branch_b),
        ],
        &repo,
    );
    vb.branches.insert(stack.id, stack.clone());

    insta::assert_snapshot!(visualize_commit_graph(&repo, branch_b)?, @r"
    * e399378 (HEAD -> main, C, B) 2
    * 2db94ad (A) 1
    ");

    write_sequence(&repo, "file", [(5, None)])?;
    let mut outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame::default(),
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(branch_b),
            message: "replace 'file' with 5 lines".into(),
            // This needs us to update B & C
            stack_segment: Some(StackSegmentId {
                segment_ref: "refs/heads/B".try_into()?,
                stack_id: stack.id,
            }),
        },
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    write_vrbranches_to_refs(&vb, &repo)?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, outcome.new_commit.unwrap())?, @r"
    * 2185d68 (HEAD -> main, C, B) replace 'file' with 5 lines
    * e399378 2
    * 2db94ad (A) 1
    ");

    let index = outcome.index.take().unwrap();
    insta::assert_snapshot!(visualize_index(&index), @"100644:8a1218a file");

    assure_no_worktree_changes(&repo)?;
    Ok(())
}

#[test]
fn two_commits_three_buckets_disambiguate_insertion_position_to_top() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("two-commits-three-buckets");
    // duplicate the existing branches into VB
    let branch_b = repo.rev_parse_single("B")?.detach();
    let mut vb = VirtualBranchesState::default();
    let stack = stack_with_branches(
        "C",
        branch_b,
        [
            ("A", repo.rev_parse_single("A")?.detach()),
            ("B", branch_b),
            ("C", branch_b),
        ],
        &repo,
    );
    vb.branches.insert(stack.id, stack.clone());

    insta::assert_snapshot!(visualize_commit_graph(&repo, branch_b)?, @r"
    * e399378 (HEAD -> main, C, B) 2
    * 2db94ad (A) 1
    ");

    write_sequence(&repo, "file", [(5, None)])?;
    let mut outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame::default(),
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(branch_b),
            message: "replace 'file' with 5 lines".into(),
            // This needs us to update C only, leaving B in place, also the default which is tested elsewhere.
            stack_segment: Some(StackSegmentId {
                segment_ref: "refs/heads/C".try_into()?,
                stack_id: stack.id,
            }),
        },
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    write_vrbranches_to_refs(&vb, &repo)?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, outcome.new_commit.unwrap())?, @r"
    * 2185d68 (HEAD -> main, C) replace 'file' with 5 lines
    * e399378 (B) 2
    * 2db94ad (A) 1
    ");

    let index = outcome.index.take().unwrap();
    insta::assert_snapshot!(visualize_index(&index), @"100644:8a1218a file");

    assure_no_worktree_changes(&repo)?;
    Ok(())
}

#[test]
fn commit_on_top_of_branch_in_workspace() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");

    let head_commit_id = repo.head_id()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, head_commit_id)?, @r"
    *   2a6d103 (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 7f389ed (A) add 10 to the beginning
    * | 91ef6f6 (B) add 10 to the end
    |/  
    * ff045ef (main) init
    ");

    let branch_a = repo.rev_parse_single("A")?.detach();
    let branch_b = repo.rev_parse_single("B")?.detach();
    let mut vb = VirtualBranchesState::default();
    let stack_a = stack_with_branches(
        "s1",
        branch_a,
        // The order indicates which one actually is on top, even though they both point to the
        // same commit.
        [("s1-b/below-top", branch_a), ("s1-b/top", branch_a)],
        &repo,
    );
    vb.branches.insert(stack_a.id, stack_a.clone());

    let stack_b = stack_with_branches(
        "s2",
        branch_b,
        [("s2-b/below-top", branch_b), ("s2-b/top", branch_b)],
        &repo,
    );
    vb.branches.insert(stack_b.id, stack_b.clone());

    // initial is 1-30, make a change that transfers correctly to A where it is 5-20.
    // This forces us to handle the index update (more) correctly, but also shows
    // that the removal of 10 lines is just going away during the cherry-pick.
    write_sequence(&repo, "file", [(5, 20)])?;
    let mut outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: Some(head_commit_id.detach()),
            branch_tip: Some(branch_a),
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(branch_a),
            message: "remove 5 lines from beginning".into(),
            stack_segment: Some(StackSegmentId {
                segment_ref: "refs/heads/s1-b/top".try_into()?,
                stack_id: stack_a.id,
            }),
        },
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    write_vrbranches_to_refs(&vb, &repo)?;

    let rewritten_head_id = repo.head_id()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    *   f00525a (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 608f07b (s1-b/top) remove 5 lines from beginning
    | * 7f389ed (s1-b/below-top, A) add 10 to the beginning
    * | 91ef6f6 (s2-b/top, s2-b/below-top, B) add 10 to the end
    |/  
    * ff045ef (main) init
    ");

    // The tree at HEAD has the right state, despite it feeling a bit strange as the result
    // isn't absolute if the changes don't go into the right commits.
    // This makes it important to one day be able to properly absorb, to have the correct final result
    // without rejecting patches.
    // To make this work properly, one would have to split the change and put each part into the right commit in A and B.
    insta::assert_snapshot!(but_testsupport::visualize_tree(rewritten_head_id), @r#"
    6420707
    └── file:100644:c8ebab8 "5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    "#);
    assert_eq!(
        outcome.rejected_specs,
        vec![],
        "no patch gets rejected here, even though the line removal isn't present (-10 lines at the end)"
    );

    let index = outcome.index.take().unwrap();
    insta::assert_snapshot!(visualize_index_with_content(&repo, &index), @r#"100644:c8ebab8 file "5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n""#);

    // As the 10 removed lines were ignored and aren't in the commit, the worktree still seems to have changes
    insta::assert_debug_snapshot!(worktree_changes_with_diffs(&repo)?, @r#"
    [
        (
            TreeChange {
                path: "file",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(c8ebab81e52366c1c670281a408a27fad79ee257),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
            [
                DiffHunk(""@@ -17,10 +17,0 @@\n-21\n-22\n-23\n-24\n-25\n-26\n-27\n-28\n-29\n-30\n""),
            ],
        ),
    ]
    "#);

    // Put 5 of these ten lines as removals onto branch B
    let mut outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: Some(rewritten_head_id.detach()),
            branch_tip: Some(branch_b),
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(branch_b),
            message: "remove 5 lines from the end".into(),
            stack_segment: Some(StackSegmentId {
                segment_ref: "refs/heads/s2-b/top".try_into()?,
                stack_id: stack_b.id,
            }),
        },
        vec![DiffSpec {
            previous_path: None,
            path: "file".into(),
            // Remove 5 lines from the end.
            hunk_headers: vec![hunk_header("-22,5", "+0,0")],
        }],
        CONTEXT_LINES,
    )?;
    assert_eq!(outcome.rejected_specs, vec![]);

    // The last 5 lines were removed
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    e6575c0
    └── file:100644:8452dbe "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n"
    "#);

    let rewritten_head_id = repo.head_id()?;
    // The B-segment refs moved
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    *   376bcdb (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 608f07b (s1-b/top) remove 5 lines from beginning
    | * 7f389ed (s1-b/below-top, A) add 10 to the beginning
    * | b5ec010 (s2-b/top) remove 5 lines from the end
    * | 91ef6f6 (s2-b/below-top, B) add 10 to the end
    |/  
    * ff045ef (main) init
    ");

    let index = outcome.index.take().unwrap();
    insta::assert_snapshot!(visualize_index_with_content(&repo, &index), @r#"100644:1cc1fac file "5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n""#);

    // The worktree only shows 5 lines missing, as the 5 deletions were applied in branch B.
    insta::assert_debug_snapshot!(worktree_changes_with_diffs(&repo)?, @r#"
    [
        (
            TreeChange {
                path: "file",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(1cc1fac76944ae8361a73461d54e85a50ccc7bd1),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
            [
                DiffHunk(""@@ -17,5 +17,0 @@\n-21\n-22\n-23\n-24\n-25\n""),
            ],
        ),
    ]
    "#);

    let rewritten_head_id = repo.head_id()?;
    let top_of_branch = outcome.new_commit.expect("created above");
    let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: Some(rewritten_head_id.detach()),
            branch_tip: Some(top_of_branch),
        },
        &mut vb,
        Destination::NewCommit {
            parent_commit_id: Some(top_of_branch),
            message: "empty commit".into(),
            stack_segment: Some(StackSegmentId {
                segment_ref: "refs/heads/s2-b/top".try_into()?,
                stack_id: stack_b.id,
            }),
        },
        vec![],
        CONTEXT_LINES,
    )?;
    assert_eq!(outcome.rejected_specs, vec![], "nothing to reject");

    // Nothing changed
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    e6575c0
    └── file:100644:8452dbe "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n"
    "#);

    let rewritten_head_id = repo.head_id()?;
    // The empty commit was inserted.
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    *   f3de308 (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 608f07b (s1-b/top) remove 5 lines from beginning
    | * 7f389ed (s1-b/below-top, A) add 10 to the beginning
    * | e43241c (s2-b/top) empty commit
    * | b5ec010 remove 5 lines from the end
    * | 91ef6f6 (s2-b/below-top, B) add 10 to the end
    |/  
    * ff045ef (main) init
    ");
    Ok(())
}

#[test]
fn amend_on_top_of_branch_in_workspace() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");

    let head_commit_id = repo.head_id()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, head_commit_id)?, @r"
    *   2a6d103 (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 7f389ed (A) add 10 to the beginning
    * | 91ef6f6 (B) add 10 to the end
    |/  
    * ff045ef (main) init
    ");

    let branch_a = repo.rev_parse_single("A")?.detach();
    let mut vb = VirtualBranchesState::default();
    let stack = stack_with_branches("s1", branch_a, [("s1-b/top", branch_a)], &repo);
    vb.branches.insert(stack.id, stack);

    // initial is 1-30, make a change that transfers correctly to A where it is 5-20.
    // This forces us to handle the index update (more) correctly, but also shows
    // that the removal of 10 lines is just going away during the cherry-pick.
    write_sequence(&repo, "file", [(5, 20)])?;
    let mut outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: Some(head_commit_id.detach()),
            branch_tip: Some(branch_a),
        },
        &mut vb,
        Destination::AmendCommit {
            commit_id: branch_a,
            new_message: None,
        },
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    write_vrbranches_to_refs(&vb, &repo)?;

    let rewritten_head_id = repo.head_id()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    *   0bb4efb (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 3edfe68 (s1-b/top, A) add 10 to the beginning
    * | 91ef6f6 (B) add 10 to the end
    |/  
    * ff045ef (main) init
    ");

    insta::assert_snapshot!(but_testsupport::visualize_tree(rewritten_head_id), @r#"
    6420707
    └── file:100644:c8ebab8 "5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    "#);
    assert_eq!(
        outcome.rejected_specs,
        vec![],
        "no patch gets rejected here, even though the line removal isn't present (-10 lines at the end)"
    );

    let index = outcome.index.take().unwrap();
    insta::assert_snapshot!(visualize_index(&index), @"100644:c8ebab8 file");

    assert_eq!(
        but_core::diff::worktree_changes(&repo)?.changes.len(),
        1,
        "As the 10 removed lines were ignored and aren't in the commit, \
           the worktree still seems to have changes"
    );
    Ok(())
}

#[test]
fn amend_edit_message_only() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");

    let head_commit_id = repo.head_id()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, head_commit_id)?, @r"
    *   2a6d103 (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * 7f389ed (A) add 10 to the beginning
    * | 91ef6f6 (B) add 10 to the end
    |/  
    * ff045ef (main) init
    ");

    let branch_a = repo.rev_parse_single("A")?.detach();
    let mut vb = VirtualBranchesState::default();
    let stack = stack_with_branches("s1", branch_a, [("s1-b/top", branch_a)], &repo);
    vb.branches.insert(stack.id, stack);

    assure_no_worktree_changes(&repo)?;
    let mut outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: Some(head_commit_id.detach()),
            branch_tip: Some(branch_a),
        },
        &mut vb,
        Destination::AmendCommit {
            commit_id: branch_a,
            new_message: Some("add 10 to the beginning (amended)".into()),
        },
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    assert!(
        outcome.new_commit.is_some(),
        "a new commit was created, despite changes"
    );
    write_vrbranches_to_refs(&vb, &repo)?;

    let rewritten_head_id = repo.head_id()?;
    // TODO: make some change observable.
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    *   42690f2 (HEAD -> merge) Merge branch 'A' into merge
    |\  
    | * bc22104 (s1-b/top, A) add 10 to the beginning (amended)
    * | 91ef6f6 (B) add 10 to the end
    |/  
    * ff045ef (main) init
    ");

    insta::assert_snapshot!(but_testsupport::visualize_tree(rewritten_head_id), @r#"
    9573cc7
    └── file:100644:e8823e1 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    "#);
    assert_eq!(outcome.rejected_specs, vec![]);

    // We still produce an index as the re-merge might change things.
    // In theory, there should be no difference though.
    let index = outcome.index.take().unwrap();
    insta::assert_snapshot!(visualize_index(&index), @"100644:e8823e1 file");

    Ok(())
}

mod utils {
    use but_testsupport::visualize_commit_graph;
    use gitbutler_stack::VirtualBranchesState;
    use gix::refs::transaction::PreviousValue;

    pub fn has_signature(commit: gix::Id<'_>) -> anyhow::Result<bool> {
        Ok(commit
            .object()?
            .into_commit()
            .decode()?
            .extra_headers()
            .pgp_signature()
            .is_some())
    }

    /// We are only interested in the head-related information, the rest is garbage.
    pub fn stack_with_branches(
        _name: &str,
        _tip: gix::ObjectId,
        branches: impl IntoIterator<Item = (&'static str, gix::ObjectId)>,
        repo: &gix::Repository,
    ) -> gitbutler_stack::Stack {
        let heads = branches
            .into_iter()
            .map(|(name, target_id)| new_stack_branch(name, target_id, repo))
            .filter_map(Result::ok)
            .collect();
        gitbutler_stack::Stack::new_with_just_heads(heads, 0, 0, true)
    }

    fn new_stack_branch(
        name: &str,
        head: gix::ObjectId,
        repo: &gix::Repository,
    ) -> anyhow::Result<gitbutler_stack::StackBranch> {
        gitbutler_stack::StackBranch::new(head, name.into(), None, repo)
    }

    /// Turn all heads from `vbranches` into an aptly named standard reference.
    pub fn write_vrbranches_to_refs(
        vbranches: &VirtualBranchesState,
        repo: &gix::Repository,
    ) -> anyhow::Result<()> {
        for stack in vbranches.branches.values() {
            // makes no sense to crate this?
            // repo.reference(
            //     format!("refs/heads/{}", stack.name),
            //     stack.head(repo)?.to_gix(),
            //     PreviousValue::Any,
            //     "create stack head for visualization",
            // )?;
            for branch in &stack.heads {
                let commit_id = branch.head_oid(repo)?;
                repo.reference(
                    format!("refs/heads/{}", branch.name()),
                    commit_id,
                    PreviousValue::Any,
                    "create branch head for visualization",
                )
                .unwrap();
            }
        }
        Ok(())
    }

    pub fn graph_commit_outcome(
        repo: &gix::Repository,
        outcome: &but_workspace::commit_engine::CreateCommitOutcome,
    ) -> std::io::Result<String> {
        let new_commit_id = outcome.new_commit.expect("a new commit was created");
        // The HEAD reference was updated.
        visualize_commit_graph(repo, new_commit_id)
    }

    /// Write a file at `rela_path` with `content` directly into the worktree of `repo`.
    pub fn write_worktree_file(
        repo: &gix::Repository,
        rela_path: &str,
        content: &str,
    ) -> std::io::Result<()> {
        let work_path = repo.workdir_path(rela_path).expect("non-bare");
        std::fs::write(work_path, content)
    }
}
