use crate::commit_engine::refs_update::utils::{
    assure_no_worktree_changes, graph_commit_outcome, stack_with_branches,
    write_vrbranches_to_refs, write_worktree_file,
};
use crate::commit_engine::utils::{
    assure_stable_env, to_change_specs_all_hunks, to_change_specs_whole_file, visualize_index,
    visualize_tree, writable_scenario, writable_scenario_with_ssh_key, write_sequence,
    CONTEXT_LINES,
};
use but_testsupport::visualize_commit_graph;
use but_workspace::commit_engine::{Destination, ReferenceFrame};
use gitbutler_stack::VirtualBranchesState;
use gix::prelude::ObjectIdExt;
use gix::refs::transaction::PreviousValue;

#[test]
fn new_commits_to_tip_from_unborn_head() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("unborn-untracked");
    let mut vb = VirtualBranchesState::default();
    let outcome = but_workspace::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
            vb: &mut vb,
        },
        Destination::NewCommit {
            parent_commit_id: None,
            message: "initial commit".to_string(),
        },
        None,
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
    insta::assert_snapshot!(visualize_commit_graph(&repo, new_commit_id)?, @"* 4f950d2 (HEAD -> main) initial commit");
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    861d6e2
    └── not-yet-tracked:100644:d95f3ad "content\n"
    "#);
    assure_no_worktree_changes(&repo)?;

    write_worktree_file(&repo, "new-file", "other content")?;
    let outcome = but_workspace::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
            vb: &mut vb,
        },
        Destination::NewCommit {
            parent_commit_id: Some(new_commit_id),
            message: "second commit".to_string(),
        },
        None,
        to_change_specs_whole_file(but_core::diff::worktree_changes(&repo)?),
        CONTEXT_LINES,
    )?;
    // The HEAD reference was updated.
    insta::assert_snapshot!(graph_commit_outcome(&repo, &outcome)?, @r"
    * 775de8d (HEAD -> main) second commit
    * 4f950d2 initial commit
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
    let outcome = but_workspace::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
            vb: &mut vb,
        },
        Destination::NewCommit {
            parent_commit_id: Some(new_commit_id),
            message: "third commit".to_string(),
        },
        None,
        to_change_specs_whole_file(but_core::diff::worktree_changes(&repo)?),
        CONTEXT_LINES,
    )?;

    // The HEAD reference was updated, along with all other tag-references that pointed to it.
    let new_commit = outcome.new_commit.expect("a new commit was created");
    insta::assert_snapshot!(visualize_commit_graph(&repo, new_commit)?, @r"
    * 0a284ea (HEAD -> main, another-tip) third commit
    * 775de8d (tag: tag-that-should-not-move) second commit
    * 4f950d2 initial commit
    ");

    write_worktree_file(&repo, "new-file", "yet another change")?;
    let outcome = but_workspace::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
            vb: &mut vb,
        },
        Destination::AmendCommit(new_commit),
        None,
        to_change_specs_whole_file(but_core::diff::worktree_changes(&repo)?),
        CONTEXT_LINES,
    )?;

    assure_no_worktree_changes(&repo)?;
    // The top commit has a different hash now thanks to amending.
    insta::assert_snapshot!(graph_commit_outcome(&repo, &outcome)?, @r"
    * f00ac96 (HEAD -> main, another-tip) third commit
    * 775de8d (tag: tag-that-should-not-move) second commit
    * 4f950d2 initial commit
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
    );
    vb.branches.insert(stack.id, stack);

    write_worktree_file(&repo, "new-file", "the final change")?;
    let new_commit = outcome.new_commit.unwrap();
    let mut outcome = but_workspace::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: None,
            vb: &mut vb,
        },
        Destination::NewCommit {
            parent_commit_id: Some(new_commit),
            message: "fourth commit".to_string(),
        },
        None,
        to_change_specs_whole_file(but_core::diff::worktree_changes(&repo)?),
        CONTEXT_LINES,
    )?;

    // Updated references are visible (but probably nobody needs them).
    let index = outcome.index.take().unwrap();
    insta::assert_debug_snapshot!(outcome, @r#"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(0b369dfc9d27ad88b9949a51db1c2da38b40891d),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(273aeca7ca98af0f7972af6e7859a3ae7fde497a),
        ),
        references: [
            UpdatedReference {
                reference: Virtual(
                    "s1",
                ),
                old_commit_id: Sha1(f00ac96b741a8de62ebd2a0567d741e2d711b53b),
                new_commit_id: Sha1(0b369dfc9d27ad88b9949a51db1c2da38b40891d),
            },
            UpdatedReference {
                reference: Virtual(
                    "s1-b/second",
                ),
                old_commit_id: Sha1(f00ac96b741a8de62ebd2a0567d741e2d711b53b),
                new_commit_id: Sha1(0b369dfc9d27ad88b9949a51db1c2da38b40891d),
            },
            UpdatedReference {
                reference: Virtual(
                    "s2",
                ),
                old_commit_id: Sha1(f00ac96b741a8de62ebd2a0567d741e2d711b53b),
                new_commit_id: Sha1(0b369dfc9d27ad88b9949a51db1c2da38b40891d),
            },
            UpdatedReference {
                reference: Virtual(
                    "s2-b/second",
                ),
                old_commit_id: Sha1(f00ac96b741a8de62ebd2a0567d741e2d711b53b),
                new_commit_id: Sha1(0b369dfc9d27ad88b9949a51db1c2da38b40891d),
            },
            UpdatedReference {
                reference: Git(
                    FullName(
                        "refs/heads/another-tip",
                    ),
                ),
                old_commit_id: Sha1(f00ac96b741a8de62ebd2a0567d741e2d711b53b),
                new_commit_id: Sha1(0b369dfc9d27ad88b9949a51db1c2da38b40891d),
            },
            UpdatedReference {
                reference: Git(
                    FullName(
                        "refs/heads/main",
                    ),
                ),
                old_commit_id: Sha1(f00ac96b741a8de62ebd2a0567d741e2d711b53b),
                new_commit_id: Sha1(0b369dfc9d27ad88b9949a51db1c2da38b40891d),
            },
        ],
        rebase_output: None,
        index: None,
    }
    "#);
    write_vrbranches_to_refs(&vb, &repo)?;
    // It updates stack heads and stack branch heads.
    insta::assert_snapshot!(graph_commit_outcome(&repo, &outcome)?, @r"
    * 0b369df (HEAD -> main, s2-b/second, s2, s1-b/second, s1, another-tip) fourth commit
    * f00ac96 third commit
    * 775de8d (tag: tag-that-should-not-move, s2-b/first, s1-b/first) second commit
    * 4f950d2 (s2-b/init, s1-b/init) initial commit
    ");
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    273aeca
    ├── new-file:100644:aaa9a91 "the final change"
    └── not-yet-tracked:100644:d95f3ad "content\n"
    "#);

    assure_no_worktree_changes(&repo)?;
    insta::assert_snapshot!(visualize_index(&index), @r"
    100644:aaa9a91 new-file
    100644:d95f3ad not-yet-tracked
    ");
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

    let stack = stack_with_branches("s1", head_commit_id, [("s1-b/init", initial_commit_id)]);
    vb.branches.insert(stack.id, stack);
    // Add 10 lines to the end.
    write_sequence(&repo, "file", [(30, None)])?;
    let mut outcome = but_workspace::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: Some(head_commit_id),
            vb: &mut vb,
        },
        Destination::NewCommit {
            parent_commit_id: Some(initial_commit_id),
            message: "between initial and former first".to_string(),
        },
        None,
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    // it rewrites the history to the top of the stack.
    write_vrbranches_to_refs(&vb, &repo)?;
    let rewritten_head_id = repo.head_id()?.detach();
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    * b861ac3 (HEAD -> main, s1) insert 10 lines to the top
    * 9fd4d2c between initial and former first
    * ecd6722 (tag: first-commit, s1-b/init, first-commit) init
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
            Sha1(9fd4d2c10083dbac2a0b20deaf83b750174a67d5),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(5fdd31363b3f0987135feaa00a734ca31e1652d6),
        ),
        references: [
            UpdatedReference {
                reference: Virtual(
                    "s1",
                ),
                old_commit_id: Sha1(8b9db8455554fe317ea3ab86b9a042805326b493),
                new_commit_id: Sha1(b861ac3e3f8a737972034c4763825f250f3a8bf1),
            },
            UpdatedReference {
                reference: Git(
                    FullName(
                        "refs/heads/main",
                    ),
                ),
                old_commit_id: Sha1(8b9db8455554fe317ea3ab86b9a042805326b493),
                new_commit_id: Sha1(b861ac3e3f8a737972034c4763825f250f3a8bf1),
            },
        ],
        rebase_output: Some(
            RebaseOutput {
                top_commit: Sha1(b861ac3e3f8a737972034c4763825f250f3a8bf1),
                references: [],
                commit_mapping: [
                    (
                        Some(
                            Sha1(9fd4d2c10083dbac2a0b20deaf83b750174a67d5),
                        ),
                        Sha1(8b9db8455554fe317ea3ab86b9a042805326b493),
                        Sha1(b861ac3e3f8a737972034c4763825f250f3a8bf1),
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
    let mut outcome = but_workspace::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: Some(rewritten_head_id),
            vb: &mut vb,
        },
        Destination::AmendCommit(repo.rev_parse_single("@~1")?.detach()),
        None,
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;
    let index = outcome.index.take().unwrap();
    let rewritten_head_id = repo.head_id()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    * a23565d (HEAD -> main, s1) insert 10 lines to the top
    * 053486e between initial and former first
    * ecd6722 (tag: first-commit, s1-b/init, first-commit) init
    ");
    insta::assert_snapshot!(but_testsupport::visualize_tree(rewritten_head_id), @r#"
    683b451
    ├── .gitignore:100644:ccc87a0 "*.key*\n"
    └── file:100644:1c99002 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n"
    "#);

    assure_no_worktree_changes(&repo)?;
    insta::assert_snapshot!(visualize_index(&index), @r"
    100644:ccc87a0 .gitignore
    100644:1c99002 file
    ");
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
    let stack = stack_with_branches("s1", head_commit_id, [("s1-b/init", stack1_head)]);
    vb.branches.insert(stack.id, stack);

    // another 10 to the end (HEAD range is 1-30).
    write_sequence(&repo, "file", [(40, None)])?;
    let branch_b = repo.rev_parse_single("B")?.detach();
    let mut outcome = but_workspace::commit_engine::create_commit_and_update_refs(
        &repo,
        ReferenceFrame {
            workspace_tip: Some(repo.rev_parse_single("merge")?.detach()),
            branch_tip: Some(branch_b),
            vb: &mut vb,
        },
        Destination::NewCommit {
            parent_commit_id: Some(branch_b),
            message: "add 10 more lines at end".into(),
        },
        None,
        to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
        CONTEXT_LINES,
    )?;

    write_vrbranches_to_refs(&vb, &repo)?;
    assure_no_worktree_changes(&repo)?;

    let index = outcome.index.take().unwrap();
    let rewritten_head_id = repo.head_id()?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, rewritten_head_id)?, @r"
    *   e767fa6 (HEAD -> merge, s1) Merge branch 'A' into merge
    |\  
    | * 3538622 (A) add 10 to the beginning
    * | 059d194 add 10 more lines at end
    * | e81b470 (s1-b/init, B) add 10 to the end
    |/  
    * 9cf2979 (main) init
    ");
    // The new result is like we'd expect so 10 more lines are added to the full merge result.
    insta::assert_snapshot!(but_testsupport::visualize_tree(rewritten_head_id), @r#"
    401d80f
    ├── file:100644:1c99002 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n"
    └── other-file:100644:a11f0f8 "35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n"
    "#);
    insta::assert_snapshot!(visualize_index(&index), @r"
    100644:1c99002 file
    100644:a11f0f8 other-file
    ");
    Ok(())
}

#[test]
#[ignore = "TBD"]
fn amend_with_merge_and_signatures() -> anyhow::Result<()> {
    Ok(())
}

mod utils {
    use but_testsupport::visualize_commit_graph;
    use gitbutler_oxidize::{ObjectIdExt as _, OidExt};
    use gitbutler_stack::{CommitOrChangeId, VirtualBranchesState};
    use gix::refs::transaction::PreviousValue;

    pub fn assure_no_worktree_changes(repo: &gix::Repository) -> anyhow::Result<()> {
        assert_eq!(
            but_core::diff::worktree_changes(repo)?.changes.len(),
            0,
            "all changes are seemingly incorporated"
        );
        Ok(())
    }

    /// We are only interested in the head-related information, the rest is garbage.
    pub fn stack_with_branches(
        name: &str,
        tip: gix::ObjectId,
        branches: impl IntoIterator<Item = (&'static str, gix::ObjectId)>,
    ) -> gitbutler_stack::Stack {
        gitbutler_stack::Stack {
            id: Default::default(),
            name: name.into(),
            tree: tip.kind().null().to_git2(),
            head: tip.to_git2(),
            heads: branches
                .into_iter()
                .map(|(name, target_id)| new_stack_branch(name, target_id))
                .collect(),
            notes: String::new(),
            source_refname: None,
            upstream: None,
            upstream_head: None,
            created_timestamp_ms: 0,
            updated_timestamp_ms: 0,
            ownership: Default::default(),
            order: 0,
            selected_for_changes: None,
            allow_rebasing: false,
            in_workspace: false,
            not_in_workspace_wip_change_id: None,
            post_commits: false,
        }
    }

    fn new_stack_branch(name: &str, head: gix::ObjectId) -> gitbutler_stack::StackBranch {
        gitbutler_stack::StackBranch {
            head: CommitOrChangeId::CommitId(head.to_string()),
            name: name.into(),
            description: None,
            pr_number: None,
            archived: false,
            review_id: None,
        }
    }

    /// Turn all heads from `vbranches` into an aptly named standard reference.
    pub fn write_vrbranches_to_refs(
        vbranches: &VirtualBranchesState,
        repo: &gix::Repository,
    ) -> anyhow::Result<()> {
        for stack in vbranches.branches.values() {
            repo.reference(
                format!("refs/heads/{}", stack.name),
                stack.head.to_gix(),
                PreviousValue::Any,
                "create stack head for visualization",
            )?;
            for branch in &stack.heads {
                let CommitOrChangeId::CommitId(commit_id) = &branch.head else {
                    continue;
                };
                let commit_id = gix::ObjectId::from_hex(commit_id.as_bytes())?;
                repo.reference(
                    format!("refs/heads/{}", branch.name),
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
        let work_dir = repo.work_dir().expect("non-bare");
        std::fs::write(work_dir.join(rela_path), content)
    }
}
