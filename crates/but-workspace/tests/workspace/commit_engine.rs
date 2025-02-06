mod new_commit {
    use crate::commit_engine::utils::{
        commit_whole_files_and_all_hunks_from_workspace, stable_env, to_change_specs_all_hunks,
        to_change_specs_all_hunks_with_context_lines, to_change_specs_whole_file, visualize_tree,
        writable_scenario, writable_scenario_execute, write_sequence, CONTEXT_LINES,
    };
    use but_workspace::commit_engine;
    use but_workspace::commit_engine::{CreateCommitOutcome, Destination, DiffSpec, RefHandling};
    use gix::prelude::ObjectIdExt;
    use serial_test::serial;

    #[test]
    #[serial]
    fn from_unborn_head() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("unborn-untracked");
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(None),
            "the commit message",
        )?;
        insta::assert_debug_snapshot!(&outcome, @r#"
        CreateCommitOutcome {
            rejected_specs: [],
            new_commit: Some(
                Sha1(2eb2e90e37a7f23052db17b67b91eb5c4a7a1e81),
            ),
            ref_edit: Some(
                RefEdit {
                    change: Update {
                        log: LogChange {
                            mode: AndReference,
                            force_create_reflog: false,
                            message: "the commit message",
                        },
                        expected: Any,
                        new: Object(
                            Sha1(2eb2e90e37a7f23052db17b67b91eb5c4a7a1e81),
                        ),
                    },
                    name: FullName(
                        "refs/heads/main",
                    ),
                    deref: false,
                },
            ),
        }
        "#);

        let new_commit_id = outcome.new_commit.expect("a new commit was created");
        assert_eq!(
            repo.head_id()?,
            new_commit_id,
            "HEAD should have been updated as it's the top of the tip"
        );
        assert_eq!(
            repo.head_ref()?.expect("not detached").name().as_bstr(),
            "refs/heads/main",
            "it kept the head-ref"
        );

        let new_commit = new_commit_id.attach(&repo).object()?.peel_to_commit()?;
        assert_eq!(new_commit.message_raw()?, "the commit message");

        let tree = visualize_tree(&repo, &outcome)?;
        insta::assert_snapshot!(tree, @r#"
        861d6e2
        └── not-yet-tracked:100644:d95f3ad "content\n"
        "#);

        std::fs::write(
            repo.work_dir().expect("non-bare").join("new-untracked"),
            "new-content",
        )?;
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(Some(new_commit_id)),
            "the second commit",
        )?;

        insta::assert_debug_snapshot!(&outcome, @r#"
        CreateCommitOutcome {
            rejected_specs: [],
            new_commit: Some(
                Sha1(9fa45065e99a2f0492bca947cf462dfafd905516),
            ),
            ref_edit: Some(
                RefEdit {
                    change: Update {
                        log: LogChange {
                            mode: AndReference,
                            force_create_reflog: false,
                            message: "the second commit",
                        },
                        expected: MustExistAndMatch(
                            Object(
                                Sha1(2eb2e90e37a7f23052db17b67b91eb5c4a7a1e81),
                            ),
                        ),
                        new: Object(
                            Sha1(9fa45065e99a2f0492bca947cf462dfafd905516),
                        ),
                    },
                    name: FullName(
                        "refs/heads/main",
                    ),
                    deref: false,
                },
            ),
        }
        "#);
        let current_tip = outcome.new_commit.expect("a new commit was created");
        let head_ref = repo.head_ref()?.expect("not detached");
        assert_eq!(head_ref.id(), current_tip, "HEAD should have been updated");

        let tree = visualize_tree(&repo, &outcome)?;
        insta::assert_snapshot!(tree, @r#"
        a004469
        ├── new-untracked:100644:72278a7 "new-content"
        └── not-yet-tracked:100644:d95f3ad "content\n"
        "#);
        Ok(())
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn from_unborn_head_all_file_types() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario_execute("unborn-untracked-all-file-types");
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(None),
            "the commit message",
        )?;

        assert_eq!(
            outcome.rejected_specs,
            Vec::new(),
            "everything was committed"
        );
        let new_commit_id = outcome.new_commit.expect("a new commit was created");
        assert_eq!(
            repo.head_id()?,
            new_commit_id,
            "HEAD should have been updated as it's the top of the tip"
        );
        assert_eq!(
            repo.head_ref()?.expect("not detached").name().as_bstr(),
            "refs/heads/main",
            "it kept the head-ref"
        );

        let new_commit = new_commit_id.attach(&repo).object()?.peel_to_commit()?;
        assert_eq!(new_commit.message_raw()?, "the commit message");

        let tree = visualize_tree(&repo, &outcome)?;
        insta::assert_snapshot!(tree, @r#"
        7f802e9
        ├── link:120000:faf96c1 "untracked"
        ├── untracked:100644:d95f3ad "content\n"
        └── untracked-exe:100755:86daf54 "exe\n"
        "#);

        Ok(())
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn from_first_commit_all_file_types_changed() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario_execute("all-file-types-changed");
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(Some(repo.rev_parse_single("HEAD")?.into())),
            "the commit message",
        )?;

        let tree = visualize_tree(&repo, &outcome)?;
        insta::assert_snapshot!(tree, @r#"
        9be09ac
        ├── soon-executable:100755:d95f3ad "content\n"
        ├── soon-file-not-link:100644:72f007b "ordinary content\n"
        └── soon-not-executable:100644:86daf54 "exe\n"
        "#);
        Ok(())
    }

    #[test]
    #[serial]
    fn unborn_with_added_submodules() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("unborn-with-submodules");
        let worktree_changes = but_core::diff::worktree_changes(&repo)?;
        let outcome = but_workspace::commit_engine::create_commit(
            &repo,
            Destination::ParentForNewCommit(None),
            None,
            to_change_specs_whole_file(worktree_changes),
            "submodules have to be given as whole files but can then be handled correctly (but without Git's special handling)",
            CONTEXT_LINES,
            RefHandling::None,
        )?;

        assert_eq!(
            outcome.rejected_specs,
            vec![],
            "Everything could be added to the repository"
        );
        let tree = visualize_tree(&repo, &outcome)?;
        // The `module` is actually a repository inside the main repository, but we add it as 'embedded repository'.
        // It's a thing, it's just that Git won't know how to obtain the submodule then.
        insta::assert_snapshot!(tree, @r#"
        6260c86
        ├── .gitmodules:100644:49dc605 "[submodule \"m1\"]\n\tpath = m1\n\turl = ./module\n"
        ├── m1:160000:a047f81 
        └── module:160000:a047f81
        "#);
        Ok(())
    }

    #[test]
    #[serial]
    fn deletions() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("delete-all-file-types");
        let head_commit = repo.rev_parse_single("HEAD")?;
        insta::assert_snapshot!(gitbutler_testsupport::visualize_gix_tree(head_commit.object()?.peel_to_tree()?.id()), @r#"
        cecc2da
        ├── .gitmodules:100644:51f8807 "[submodule \"submodule\"]\n\tpath = submodule\n\turl = ./embedded-repository\n"
        ├── embedded-repository:160000:a047f81 
        ├── executable:100755:86daf54 "exe\n"
        ├── file-to-remain:100644:d95f3ad "content\n"
        ├── link:120000:b158162 "file-to-remain"
        └── submodule:160000:a047f81
        "#);
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(Some(head_commit.into())),
            "deletions maybe a bit special",
        )?;

        insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
        c15318d
        └── file-to-remain:100644:d95f3ad "content\n"
        "#);
        Ok(())
    }

    #[test]
    #[serial]
    fn renames() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario_execute("all-file-types-renamed-and-modified");
        let head_commit = repo.rev_parse_single("HEAD")?;
        insta::assert_snapshot!(gitbutler_testsupport::visualize_gix_tree(head_commit.object()?.peel_to_tree()?.id()), @r#"
        3fd29f0
        ├── executable:100755:01e79c3 "1\n2\n3\n"
        ├── file:100644:3aac70f "5\n6\n7\n8\n"
        └── link:120000:c4c364c "nonexisting-target"
        "#);
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(Some(head_commit.into())),
            "deletions maybe a bit special",
        )?;

        insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
        0236fb1
        ├── executable-renamed:100755:94ebaf9 "1\n2\n3\n4\n"
        ├── file-renamed:100644:66f816c "5\n6\n7\n8\n9\n"
        └── link-renamed:120000:94e4e07 "other-nonexisting-target"
        "#);
        Ok(())
    }

    #[test]
    #[serial]
    fn submodule_typechanges() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("submodule-typechanges");
        let worktree_changes = but_core::diff::worktree_changes(&repo)?;
        insta::assert_debug_snapshot!(worktree_changes.changes, @r#"
        [
            TreeChange {
                path: ".gitmodules",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(51f8807c330e4ae8643ca943231cc6e176038aca),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(57fc33bc66d69e4df4ab23c33ae1101e67e56079),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
            TreeChange {
                path: "file",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(a047f8183ba2bb7eb00ef89e60050c5fde740483),
                        kind: Commit,
                    },
                    flags: Some(
                        TypeChange,
                    ),
                },
            },
            TreeChange {
                path: "submodule",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(a047f8183ba2bb7eb00ef89e60050c5fde740483),
                        kind: Commit,
                    },
                    state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    flags: Some(
                        TypeChange,
                    ),
                },
            },
        ]
        "#);
        let outcome = but_workspace::commit_engine::create_commit(
            &repo,
            Destination::ParentForNewCommit(Some(repo.rev_parse_single("HEAD")?.into())),
            None,
            to_change_specs_whole_file(worktree_changes),
            "submodules have to be given as whole files but can then be handled correctly (but without Git's special handling)",
            CONTEXT_LINES,
            RefHandling::None,
        )?;

        assert_eq!(
            outcome.rejected_specs,
            vec![],
            "Everything could be added to the repository"
        );
        let tree = visualize_tree(&repo, &outcome)?;
        // The `module` is actually a repository inside the main repository, but we add it as 'embedded repository'.
        // It's a thing, it's just that Git won't know how to obtain the submodule then.
        insta::assert_snapshot!(tree, @r#"
        05b8ed2
        ├── .gitmodules:100644:57fc33b "[submodule \"submodule\"]\n\tpath = file\n\turl = ./embedded-repository\n"
        ├── embedded-repository:160000:a047f81 
        ├── file:160000:a047f81 
        └── submodule:100644:d95f3ad "content\n"
        "#);
        Ok(())
    }

    #[test]
    #[serial]
    fn commit_to_one_below_tip() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("two-commits-with-line-offset");
        write_sequence(&repo, "file", [(20, Some(40)), (80, None), (30, Some(50))])?;
        let first_commit =
            Destination::ParentForNewCommit(Some(repo.rev_parse_single("first-commit")?.into()));
        let outcome_ctx_0 = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            first_commit,
            "we apply a change with line offsets on top of the first commit, so the patch wouldn't apply without fuzzy matching.",
        )?;

        let tree = visualize_tree(&repo, &outcome_ctx_0)?;
        insta::assert_snapshot!(tree, @r#"
        754a70c
        └── file:100644:cc418b0 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
        "#);
        Ok(())
    }

    #[test]
    #[serial]
    fn commit_to_one_below_tip_with_three_context_lines() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("two-commits-with-line-offset");
        write_sequence(&repo, "file", [(20, Some(40)), (80, None), (30, Some(50))])?;
        for context_lines in [0, 3, 5] {
            let first_commit = Destination::ParentForNewCommit(Some(
                repo.rev_parse_single("first-commit")?.into(),
            ));

            let outcome = but_workspace::commit_engine::create_commit(
                &repo,
                first_commit,
                None,
                to_change_specs_all_hunks_with_context_lines(
                    &repo,
                    but_core::diff::worktree_changes(&repo)?,
                    context_lines,
                )?,
                "When using context lines, we'd still think this works just like before",
                context_lines,
                RefHandling::None,
            )?;

            assert_eq!(
                outcome.new_commit.map(|id| id.to_string()),
                Some("a33e9992196d88b09118158608acf4234b3273a9".to_string())
            );
            let tree = visualize_tree(&repo, &outcome)?;
            assert_eq!(
                tree,
                r#"754a70c
└── file:100644:cc418b0 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
"#
            );
        }
        Ok(())
    }

    #[test]
    #[serial]
    fn commit_to_branches_below_merge_commit() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");

        write_sequence(&repo, "file", [(1, 20), (40, 50)])?;
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(Some(repo.rev_parse_single("B")?.into())),
            "a new commit onto B, changing only the lines that it wrote",
        )?;

        let tree = visualize_tree(&repo, &outcome)?;
        insta::assert_snapshot!(tree, @r#"
        a38c1c3
        └── file:100644:12121fe "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
        "#);

        write_sequence(&repo, "file", [(40, 50), (10, 30)])?;
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(Some(repo.rev_parse_single("A")?.into())),
            "a new commit onto A, changing only the lines that it wrote",
        )?;

        let tree = visualize_tree(&repo, &outcome)?;
        insta::assert_snapshot!(tree, @r#"
        704f5ca
        └── file:100644:bc33e02 "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"
        "#);
        Ok(())
    }

    #[test]
    #[serial]
    fn commit_whole_file_to_conflicting_position() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");

        // rewrite all lines so changes cover both branches
        write_sequence(&repo, "file", [(40, 70)])?;
        for conflicting_parent_commit in ["A", "B", "main"] {
            let parent_commit = repo.rev_parse_single(conflicting_parent_commit)?;
            let outcome = commit_whole_files_and_all_hunks_from_workspace(
                &repo,
                Destination::ParentForNewCommit(Some(parent_commit.into())),
                "this commit can't be done as it covers multiple commits, which will conflict on cherry-picking",
            )?;
            assert_eq!(
                outcome,
                CreateCommitOutcome {
                    rejected_specs: to_change_specs_all_hunks(
                        &repo,
                        but_core::diff::worktree_changes(&repo)?
                    )?,
                    new_commit: None,
                    ref_edit: None,
                },
                "It shouldn't produce a commit and clearly mark the conflicting specs"
            );
        }

        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(Some(repo.head_id()?.into())),
            "but it can be applied directly to the tip, the merge commit itself, it always works",
        )?;
        let tree = visualize_tree(&repo, &outcome)?;
        insta::assert_snapshot!(tree, @r#"
        5bbee6d
        └── file:100644:1c9325b "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n"
        "#);
        Ok(())
    }

    #[test]
    #[serial]
    fn commit_whole_file_to_conflicting_position_one_unconflicting_file_remains(
    ) -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset-two-files");

        // rewrite all lines so changes cover both branches
        write_sequence(&repo, "file", [(40, 70)])?;
        // Change the second file to be non-conflicting, just the half the lines in the middle
        write_sequence(&repo, "other-file", [(35, 44), (80, 90), (66, 75)])?;
        for conflicting_parent_commit in ["A", "B", "main"] {
            let parent_commit = repo.rev_parse_single(conflicting_parent_commit)?;
            let outcome = commit_whole_files_and_all_hunks_from_workspace(
                &repo,
                Destination::ParentForNewCommit(Some(parent_commit.into())),
                "this commit can't be done as it covers multiple commits, which will conflict on cherry-picking",
            )?;
            assert_eq!(
                outcome.rejected_specs,
                Vec::from_iter(
                    to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?
                        .first()
                        .cloned()
                ),
                "It still produces a commit as one file was non-conflicting, keeping the base version of the non-conflicting file"
            );
            // Different bases mean different base versions for the conflicting file.
            if conflicting_parent_commit == "A" {
                insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
                0816d13
                ├── file:100644:0ff3bbb "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"
                └── other-file:100644:593469b "35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n80\n81\n82\n83\n84\n85\n86\n87\n88\n89\n90\n"
                "#);
            } else if conflicting_parent_commit == "B" {
                insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
                df6d629
                ├── file:100644:1f1542b "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
                └── other-file:100644:a935ec9 "80\n81\n82\n83\n84\n85\n86\n87\n88\n89\n90\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n"
                "#);
            } else if conflicting_parent_commit == "main" {
                insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
                d5d6e30
                ├── file:100644:e33f5e9 "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"
                └── other-file:100644:240fe08 "80\n81\n82\n83\n84\n85\n86\n87\n88\n89\n90\n"
                "#);
            }
        }

        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(Some(repo.head_id()?.into())),
            "but it can be applied directly to the tip, the merge commit itself, it always works",
        )?;
        let tree = visualize_tree(&repo, &outcome)?;
        insta::assert_snapshot!(tree, @r#"
        7d017dd
        ├── file:100644:1c9325b "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n"
        └── other-file:100644:4223e57 "35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n80\n81\n82\n83\n84\n85\n86\n87\n88\n89\n90\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n"
        "#);
        Ok(())
    }

    #[test]
    #[serial]
    fn unborn_untracked_worktree_filters_are_applied_to_whole_files() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("unborn-untracked-crlf");
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(None),
            "the commit message",
        )?;
        insta::assert_debug_snapshot!(&outcome, @r#"
        CreateCommitOutcome {
            rejected_specs: [],
            new_commit: Some(
                Sha1(f45994afa0d26558ae4bea626917b70f8863a29b),
            ),
            ref_edit: Some(
                RefEdit {
                    change: Update {
                        log: LogChange {
                            mode: AndReference,
                            force_create_reflog: false,
                            message: "the commit message",
                        },
                        expected: Any,
                        new: Object(
                            Sha1(f45994afa0d26558ae4bea626917b70f8863a29b),
                        ),
                    },
                    name: FullName(
                        "refs/heads/main",
                    ),
                    deref: false,
                },
            ),
        }
        "#);

        let new_commit_id = outcome.new_commit.expect("a new commit was created");
        assert_eq!(
            repo.head_id()?,
            new_commit_id,
            "HEAD should have been updated"
        );
        assert_eq!(
            repo.head_ref()?.expect("not detached").name().as_bstr(),
            "refs/heads/main",
            "it kept the head-ref"
        );

        let new_commit = new_commit_id.attach(&repo).object()?.peel_to_commit()?;
        assert_eq!(new_commit.message_raw()?, "the commit message");

        // What's in Git is unix style newlines
        let tree = gitbutler_testsupport::visualize_gix_tree(new_commit.tree_id()?);
        insta::assert_snapshot!(tree, @r#"
        d5949f1
        └── not-yet-tracked:100644:1191247 "1\n2\n"
        "#);

        std::fs::write(
            repo.work_dir().expect("non-bare").join("new-untracked"),
            "one\r\ntwo\r\n",
        )?;
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::ParentForNewCommit(Some(new_commit_id)),
            "the second commit",
        )?;

        insta::assert_debug_snapshot!(&outcome, @r#"
        CreateCommitOutcome {
            rejected_specs: [],
            new_commit: Some(
                Sha1(9218f64284f5b8f31c42aed238ec89ff1836a253),
            ),
            ref_edit: Some(
                RefEdit {
                    change: Update {
                        log: LogChange {
                            mode: AndReference,
                            force_create_reflog: false,
                            message: "the second commit",
                        },
                        expected: MustExistAndMatch(
                            Object(
                                Sha1(f45994afa0d26558ae4bea626917b70f8863a29b),
                            ),
                        ),
                        new: Object(
                            Sha1(9218f64284f5b8f31c42aed238ec89ff1836a253),
                        ),
                    },
                    name: FullName(
                        "refs/heads/main",
                    ),
                    deref: false,
                },
            ),
        }
        "#);
        let current_tip = outcome.new_commit.expect("a new commit was created");
        let head_ref = repo.head_ref()?.expect("not detached");
        assert_eq!(head_ref.id(), current_tip, "HEAD should have been updated");

        let tree = gitbutler_testsupport::visualize_gix_tree(
            outcome
                .new_commit
                .expect("no rejected changes")
                .attach(&repo)
                .object()?
                .peel_to_commit()?
                .tree_id()?,
        );
        insta::assert_snapshot!(tree, @r#"
        cef7412
        ├── new-untracked:100644:814f4a4 "one\ntwo\n"
        └── not-yet-tracked:100644:1191247 "1\n2\n"
        "#);

        Ok(())
    }

    #[test]
    #[ignore = "TBD"]
    fn worktree_filters_are_applied_to_whole_hunks() {}

    #[test]
    #[ignore = "TBD"]
    fn figure_out_commit_signature_test() {}

    #[test]
    #[serial]
    fn validate_no_change_on_noop() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("two-commits-with-line-offset");
        let specs = vec![DiffSpec {
            path: "file".into(),
            ..Default::default()
        }];
        let outcome = commit_engine::create_commit(
            &repo,
            Destination::ParentForNewCommit(Some(repo.head_id()?.into())),
            None,
            specs.clone(),
            "the file has no worktree changes even though we claim it - so it's rejected and no new commit is created",
            CONTEXT_LINES,
            RefHandling::UpdateHEADRefForTipCommits
        )?;
        assert_eq!(
            outcome.new_commit, None,
            "no new commit is returned as no change actually happened"
        );
        insta::assert_debug_snapshot!(&outcome, @r#"
        CreateCommitOutcome {
            rejected_specs: [
                DiffSpec {
                    previous_path: None,
                    path: "file",
                    hunk_headers: [],
                },
            ],
            new_commit: None,
            ref_edit: None,
        }
        "#);
        Ok(())
    }
}

mod utils {
    use but_core::TreeStatus;
    use but_workspace::commit_engine::{Destination, DiffSpec, RefHandling};
    use gix::prelude::ObjectIdExt;
    use gix_testtools::Creation;

    pub const CONTEXT_LINES: u32 = 0;

    /// Returns an environment that assure commits are reproducible. This needs the `testing` feature enabled in `but-core` as well to work.
    /// Note that this is racy once other tests rely on other values for these environment variables.
    pub fn stable_env() -> gix_testtools::Env<'static> {
        gix_testtools::Env::new()
            .set("GIT_AUTHOR_DATE", "2000-01-01 00:00:00 +0000")
            .set("GIT_AUTHOR_EMAIL", "author@example.com")
            .set("GIT_AUTHOR_NAME", "author")
            .set("GIT_COMMITTER_DATE", "2000-01-02 00:00:00 +0000")
            .set("GIT_COMMITTER_EMAIL", "committer@example.com")
            .set("GIT_COMMITTER_NAME", "committer")
            .set("CHANGE_ID", "committer")
    }

    fn writable_scenario_inner(
        name: &str,
        creation: Creation,
    ) -> anyhow::Result<(gix::Repository, tempfile::TempDir)> {
        let tmp = gix_testtools::scripted_fixture_writable_with_args(
            format!("scenario/{name}.sh",),
            None::<String>,
            creation,
        )
        .map_err(anyhow::Error::from_boxed)?;
        let mut options = gix::open::Options::isolated();
        options.permissions.env = gix::open::permissions::Environment::all();
        let repo = gix::open_opts(tmp.path(), options)?;
        Ok((repo, tmp))
    }

    pub fn writable_scenario(name: &str) -> (gix::Repository, tempfile::TempDir) {
        writable_scenario_inner(name, Creation::CopyFromReadOnly)
            .expect("fixtures will yield valid repositories")
    }
    pub fn writable_scenario_execute(name: &str) -> (gix::Repository, tempfile::TempDir) {
        writable_scenario_inner(name, Creation::ExecuteScript)
            .expect("fixtures will yield valid repositories")
    }
    /// Always use all the hunks.
    pub fn to_change_specs_whole_file(changes: but_core::WorktreeChanges) -> Vec<DiffSpec> {
        let out: Vec<_> = changes
            .changes
            .into_iter()
            .map(|change| DiffSpec {
                previous_path: change.previous_path().map(ToOwned::to_owned),
                path: change.path,
                hunk_headers: Vec::new(),
            })
            .collect();
        assert!(
            !out.is_empty(),
            "fixture should contain actual changes to turn into requests"
        );
        out
    }

    /// Always use all the hunks.
    pub fn to_change_specs_all_hunks(
        repo: &gix::Repository,
        changes: but_core::WorktreeChanges,
    ) -> anyhow::Result<Vec<DiffSpec>> {
        to_change_specs_all_hunks_with_context_lines(repo, changes, CONTEXT_LINES)
    }

    /// Always use all the hunks.
    pub fn to_change_specs_all_hunks_with_context_lines(
        repo: &gix::Repository,
        changes: but_core::WorktreeChanges,
        context_lines: u32,
    ) -> anyhow::Result<Vec<DiffSpec>> {
        let mut out = Vec::with_capacity(changes.changes.len());
        for change in changes.changes {
            let spec = match change.status {
                // Untracked files must always be taken from disk (they don't have a counterpart in a tree yet)
                TreeStatus::Addition { is_untracked, .. } if is_untracked => DiffSpec {
                    path: change.path,
                    ..Default::default()
                },
                _ => {
                    match change.unified_diff(repo, context_lines) {
                        Ok(but_core::UnifiedDiff::Patch { hunks }) => DiffSpec {
                            previous_path: change.previous_path().map(ToOwned::to_owned),
                            path: change.path,
                            hunk_headers: hunks.into_iter().map(Into::into).collect(),
                        },
                        Ok(_) => unreachable!("tests won't be binary or too large"),
                        Err(_err) => {
                            // Assume it's a submodule or something without content, don't do hunks then.
                            DiffSpec {
                                path: change.path,
                                ..Default::default()
                            }
                        }
                    }
                }
            };
            out.push(spec);
        }
        Ok(out)
    }

    pub fn write_sequence(
        repo: &gix::Repository,
        filename: &str,
        sequences: impl IntoIterator<Item = (impl Into<Option<usize>>, impl Into<Option<usize>>)>,
    ) -> anyhow::Result<()> {
        use std::fmt::Write;
        let mut out = String::new();
        for (start, end) in sequences {
            let (start, end) = match (start.into(), end.into()) {
                (Some(start), Some(end)) => (start, end),
                (Some(start), None) => (1, start),
                invalid => panic!("invalid sequence: {invalid:?}"),
            };
            for num in start..=end {
                writeln!(&mut out, "{}", num)?;
            }
        }
        std::fs::write(
            repo.work_dir().expect("non-bare").join(filename),
            out.as_bytes(),
        )?;
        Ok(())
    }

    pub fn visualize_tree(
        repo: &gix::Repository,
        outcome: &but_workspace::commit_engine::CreateCommitOutcome,
    ) -> anyhow::Result<String> {
        Ok(gitbutler_testsupport::visualize_gix_tree(
            outcome
                .new_commit
                .expect("no rejected changes")
                .attach(repo)
                .object()?
                .peel_to_commit()?
                .tree_id()?,
        )
        .to_string())
    }

    /// Create a commit with the entire file as change, and another time with a whole hunk.
    /// Both should be equal or it will panic.
    pub fn commit_whole_files_and_all_hunks_from_workspace(
        repo: &gix::Repository,
        destination: Destination,
        message: &str,
    ) -> anyhow::Result<but_workspace::commit_engine::CreateCommitOutcome> {
        let worktree_changes = but_core::diff::worktree_changes(repo)?;
        let whole_file_output = but_workspace::commit_engine::create_commit(
            repo,
            destination,
            None,
            to_change_specs_whole_file(worktree_changes.clone()),
            message,
            CONTEXT_LINES,
            RefHandling::None,
        )?;
        let all_hunks_output = but_workspace::commit_engine::create_commit(
            repo,
            destination,
            None,
            to_change_specs_all_hunks(repo, worktree_changes)?,
            message,
            CONTEXT_LINES,
            RefHandling::UpdateHEADRefForTipCommits,
        )?;

        if whole_file_output.new_commit.is_some() && all_hunks_output.new_commit.is_some() {
            assert_eq!(
                visualize_tree(repo, &all_hunks_output)?,
                visualize_tree(repo, &whole_file_output)?,
            );
        }
        assert_eq!(
            all_hunks_output.new_commit, whole_file_output.new_commit,
            "Adding the whole file is the same as adding all patches (but whole files are faster)"
        );
        // NOTE: cannot compare rejections as whole-file rejections don't have hunks
        assert_eq!(
            all_hunks_output
                .rejected_specs
                .iter()
                .cloned()
                .map(|mut spec| {
                    spec.hunk_headers.clear();
                    spec
                })
                .collect::<Vec<_>>(),
            whole_file_output.rejected_specs,
            "rejections are the same as well"
        );
        Ok(all_hunks_output)
    }
}
