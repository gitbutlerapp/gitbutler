use crate::utils::{
    CONTEXT_LINES, commit_from_outcome, commit_whole_files_and_all_hunks_from_workspace,
    read_only_in_memory_scenario, to_change_specs_all_hunks,
    to_change_specs_all_hunks_with_context_lines, to_change_specs_whole_file, visualize_tree,
    writable_scenario, writable_scenario_with_ssh_key, write_sequence,
};
use but_testsupport::assure_stable_env;
use but_workspace::commit_engine;
use commit_engine::{Destination, DiffSpec};
use gix::prelude::ObjectIdExt;

mod with_refs_update {}

#[test]
fn from_unborn_head() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("unborn-untracked");
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: None,
            message: "the commit message".into(),
            stack_segment: None,
        },
    )?;
    insta::assert_debug_snapshot!(&outcome, @r"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(209c17a41b38f51f76d9912e2c62f008969774f3),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(861d6e23ee6a2d7276618bb78700354a3506bd71),
        ),
        references: [],
        rebase_output: None,
        index: None,
    }
    ");

    let new_commit_id = outcome.new_commit.expect("a new commit was created");
    assert!(
        repo.try_find_reference(repo.head_name()?.expect("not detached").as_ref())?
            .is_none(),
        "the HEAD reference isn't altered, so the repository stays unborn",
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
        Destination::NewCommit {
            parent_commit_id: Some(new_commit_id),
            message: "the second commit".into(),
            stack_segment: None,
        },
    )?;

    insta::assert_debug_snapshot!(&outcome, @r"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(40051c4ef1ec3214b32312b5a7db410c13fc35a1),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(a0044697412bfa8432298d6bd6a2ad0dbd655c9f),
        ),
        references: [],
        rebase_output: None,
        index: None,
    }
    ");
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    a004469
    ├── new-untracked:100644:72278a7 "new-content"
    └── not-yet-tracked:100644:d95f3ad "content\n"
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn from_unborn_head_all_file_types() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("unborn-untracked-all-file-types")?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: None,
            message: "the commit message".into(),
            stack_segment: None,
        },
    )?;

    assert_eq!(
        outcome.rejected_specs,
        Vec::new(),
        "everything was committed"
    );
    let new_commit_id = outcome.new_commit.expect("a new commit was created");

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
#[cfg(unix)]
fn from_first_commit_all_file_types_changed() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("all-file-types-changed")?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.rev_parse_single("HEAD")?.into()),
            message: "the commit message".into(),
            stack_segment: None,
        },
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
fn unborn_with_added_submodules() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("unborn-with-submodules");
    let worktree_changes = but_core::diff::worktree_changes(&repo)?;
    let outcome = commit_engine::create_commit(
        &repo,
        Destination::NewCommit {
            parent_commit_id: None,
            message:
                "submodules have to be given as whole files but can then be handled correctly \
            (but without Git's special handling)"
                    .into(),
            stack_segment: None,
        },
        None,
        to_change_specs_whole_file(worktree_changes),
        CONTEXT_LINES,
    )?;

    assert_eq!(
        outcome.rejected_specs,
        vec![],
        "Everything could be added to the repository"
    );
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    6260c86
    ├── .gitmodules:100644:49dc605 "[submodule \"m1\"]\n\tpath = m1\n\turl = ./module\n"
    ├── m1:160000:a047f81 
    └── module:160000:a047f81
    "#);
    Ok(())
}

#[test]
fn deletions() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("delete-all-file-types")?;
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
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(head_commit.into()),
            message: "deletions maybe a bit special".into(),
            stack_segment: None,
        },
    )?;

    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    c15318d
    └── file-to-remain:100644:d95f3ad "content\n"
    "#);
    assert_eq!(
        but_core::diff::worktree_changes(&repo)?.changes.len(),
        5,
        "we don't actually change the index to match, nor is the HEAD changed, worktree changes seem to remain"
    );
    Ok(())
}

#[test]
fn renames() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("all-file-types-renamed-and-modified")?;
    let head_commit = repo.rev_parse_single("HEAD")?;
    insta::assert_snapshot!(but_testsupport::visualize_tree(head_commit.object()?.peel_to_tree()?.id()), @r#"
    3fd29f0
    ├── executable:100755:01e79c3 "1\n2\n3\n"
    ├── file:100644:3aac70f "5\n6\n7\n8\n"
    └── link:120000:c4c364c "nonexisting-target"
    "#);
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(head_commit.into()),
            message: "renames need special care to delete the source".into(),
            stack_segment: None,
        },
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
fn submodule_typechanges() -> anyhow::Result<()> {
    assure_stable_env();

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
    let outcome = commit_engine::create_commit(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.rev_parse_single("HEAD")?.into()),
            message:
                "submodules have to be given as whole files but can then be handled correctly \
            (but without Git's special handling)"
                    .into(),
            stack_segment: None,
        },
        None,
        to_change_specs_whole_file(worktree_changes),
        CONTEXT_LINES,
    )?;

    assert_eq!(
        outcome.rejected_specs,
        vec![],
        "Everything could be added to the repository"
    );
    let tree = visualize_tree(&repo, &outcome)?;
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
fn commit_to_one_below_tip() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("two-commits-with-line-offset");
    write_sequence(&repo, "file", [(20, Some(40)), (80, None), (30, Some(50))])?;
    let first_commit = Destination::NewCommit {
        parent_commit_id: Some(repo.rev_parse_single("first-commit")?.into()),
        message: "we apply a change with line offsets on top of the first commit, so the patch wouldn't apply cleanly.".into(),
        stack_segment: None,
    };

    let outcome = commit_whole_files_and_all_hunks_from_workspace(&repo, first_commit)?;
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    754a70c
    └── file:100644:cc418b0 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
    "#);
    Ok(())
}

#[test]
fn commit_to_one_below_tip_with_three_context_lines() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("two-commits-with-line-offset");
    write_sequence(&repo, "file", [(20, Some(40)), (80, None), (30, Some(50))])?;
    for context_lines in [0, 3, 5] {
        let first_commit = Destination::NewCommit {
            parent_commit_id: Some(repo.rev_parse_single("first-commit")?.into()),
            message: "When using context lines, we'd still think this works just like before"
                .into(),
            stack_segment: None,
        };

        let outcome = commit_engine::create_commit(
            &repo,
            first_commit,
            None,
            to_change_specs_all_hunks_with_context_lines(
                &repo,
                but_core::diff::worktree_changes(&repo)?,
                context_lines,
            )?,
            context_lines,
        )?;

        assert_eq!(
            outcome.new_commit.map(|id| id.to_string()),
            Some("d5e787a63a63186f5a65403c5814148b8bbb54f7".to_string())
        );
        let tree = visualize_tree(&repo, &outcome)?;
        assert_eq!(
            tree,
            r#"754a70c
└── file:100644:cc418b0 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
"#
        );

        assert_eq!(
            but_testsupport::visualize_tree(
                outcome
                    .changed_tree_pre_cherry_pick
                    .expect("present if new commit is present")
                    .attach(&repo),
            )
            .to_string(),
            r#"2f19efb
└── file:100644:33e9beb "20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
"#
        );
    }
    Ok(())
}

#[test]
fn commit_to_branches_below_merge_commit() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");

    write_sequence(&repo, "file", [(1, 20), (40, 50)])?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.rev_parse_single("B")?.into()),
            message: "a new commit onto B, changing only the lines that it wrote".into(),
            stack_segment: None,
        },
    )?;

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    a38c1c3
    └── file:100644:12121fe "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
    "#);

    write_sequence(&repo, "file", [(40, 50), (10, 30)])?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.rev_parse_single("A")?.into()),
            message: "a new commit onto A, changing only the lines that it wrote".into(),
            stack_segment: None,
        },
    )?;

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    704f5ca
    └── file:100644:bc33e02 "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"
    "#);

    insta::assert_snapshot!(but_testsupport::visualize_tree(outcome.changed_tree_pre_cherry_pick.unwrap().attach(&repo)), @r#"
    3cca5b3
    └── file:100644:144ccb0 "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    "#);
    Ok(())
}

#[test]
fn commit_whole_file_to_conflicting_position() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");

    // rewrite all lines so changes cover both branches
    write_sequence(&repo, "file", [(40, 70)])?;
    for conflicting_parent_commit in ["A", "B", "main"] {
        let parent_commit = repo.rev_parse_single(conflicting_parent_commit)?;
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::NewCommit {
                parent_commit_id: Some(parent_commit.into()),
                message: "this commit can't be done as it covers multiple commits, \
            which will conflict on cherry-picking"
                    .into(),
                stack_segment: None,
            },
        )?;
        assert_eq!(
            outcome
                .rejected_specs
                .into_iter()
                .map(|t| t.1)
                .collect::<Vec<_>>(),
            to_change_specs_all_hunks(&repo, but_core::diff::worktree_changes(&repo)?)?,
            "It shouldn't produce a commit and clearly mark the conflicting specs"
        );
    }

    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.head_id()?.into()),
            message: "but it can be applied directly to the tip, the merge commit itself, it always works".into(),
            stack_segment: None,
        },
    )?;
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    5bbee6d
    └── file:100644:1c9325b "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n"
    "#);
    Ok(())
}

#[test]
fn commit_whole_file_to_conflicting_position_one_unconflicting_file_remains() -> anyhow::Result<()>
{
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset-two-files");

    // rewrite all lines so changes cover both branches
    write_sequence(&repo, "file", [(40, 70)])?;
    // Change the second file to be non-conflicting, just the half the lines in the middle
    write_sequence(&repo, "other-file", [(35, 44), (80, 90), (66, 75)])?;
    for conflicting_parent_commit in ["A", "B", "main"] {
        let parent_commit = repo.rev_parse_single(conflicting_parent_commit)?;
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::NewCommit {
                parent_commit_id: Some(parent_commit.into()),
                message: "this commit can't be done as it covers multiple commits, \
            which will conflict on cherry-picking"
                    .into(),
                stack_segment: None,
            },
        )?;
        assert_eq!(
            outcome
                .rejected_specs
                .iter()
                .map(|t| t.1.clone())
                .collect::<Vec<_>>(),
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
        Destination::NewCommit {
            parent_commit_id: Some(repo.head_id()?.into()),
            message: "but it can be applied directly to the tip, \
            the merge commit itself, it always works"
                .into(),
            stack_segment: None,
        },
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
fn unborn_untracked_worktree_filters_are_applied_to_whole_files() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("unborn-untracked-crlf");
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: None,
            message: "the commit message".into(),
            stack_segment: None,
        },
    )?;
    insta::assert_debug_snapshot!(&outcome, @r"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(e0f2b2cd094c6b390abf9859cdc250e10510fd1a),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(d5949f12727c8e89e1351b89e8e510dfa1e2adc9),
        ),
        references: [],
        rebase_output: None,
        index: None,
    }
    ");

    let new_commit_id = outcome.new_commit.expect("a new commit was created");
    let new_commit = new_commit_id.attach(&repo).object()?.peel_to_commit()?;
    assert_eq!(new_commit.message_raw()?, "the commit message");

    // What's in Git is unix style newlines
    let tree = but_testsupport::visualize_tree(new_commit.tree_id()?);
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
        Destination::NewCommit {
            parent_commit_id: Some(new_commit_id),
            message: "the second commit".into(),
            stack_segment: None,
        },
    )?;

    insta::assert_debug_snapshot!(&outcome, @r"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(ccc603e141a363a98221cb365fb9695cc3348088),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(cef74127e0e9f4c46b5ff360d6208ee0cc839eba),
        ),
        references: [],
        rebase_output: None,
        index: None,
    }
    ");

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    cef7412
    ├── new-untracked:100644:814f4a4 "one\ntwo\n"
    └── not-yet-tracked:100644:1191247 "1\n2\n"
    "#);

    Ok(())
}

#[test]
fn signatures_are_redone() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario_with_ssh_key("two-signed-commits-with-line-offset");

    let head_id = repo.head_id()?;
    let head_commit = head_id.object()?.into_commit().decode()?.to_owned();
    let head_id = head_id.detach();
    let previous_signature = head_commit
        .extra_headers()
        .pgp_signature()
        .expect("it's signed by default");

    // Rewrite everything for amending on top.
    write_sequence(&repo, "file", [(40, 60)])?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(head_id),
            message: "a commit with signature".into(),
            stack_segment: None,
        },
    )?;

    let new_commit = commit_from_outcome(&repo, &outcome)?;
    let new_signature = new_commit
        .extra_headers()
        .pgp_signature()
        .expect("signing config is respected");
    assert_ne!(
        previous_signature, new_signature,
        "signatures are recreated as the commit is changed"
    );
    assert_eq!(
        new_commit
            .extra_headers()
            .find_all(gix::objs::commit::SIGNATURE_FIELD_NAME)
            .count(),
        1,
        "it doesn't leave outdated signatures on top of the updated one"
    );
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    3412b2c
    ├── .gitignore:100644:ccc87a0 "*.key*\n"
    └── file:100644:a07b65a "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    "#);

    Ok(())
}

#[test]
fn validate_no_change_on_noop() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("two-commits-with-line-offset")?;
    let specs = vec![DiffSpec {
        path: "file".into(),
        ..Default::default()
    }];
    let outcome = commit_engine::create_commit(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.head_id()?.into()),
            message: "the file has no worktree changes even though we claim it - \
        so it's rejected and no new commit is created"
                .into(),
            stack_segment: None,
        },
        None,
        specs.clone(),
        CONTEXT_LINES,
    )?;
    assert_eq!(
        outcome.new_commit, None,
        "no new commit is returned as no change actually happened"
    );
    insta::assert_debug_snapshot!(&outcome, @r#"
    CreateCommitOutcome {
        rejected_specs: [
            (
                NoEffectiveChanges,
                DiffSpec {
                    previous_path: None,
                    path: "file",
                    hunk_headers: [],
                },
            ),
        ],
        new_commit: None,
        changed_tree_pre_cherry_pick: None,
        references: [],
        rebase_output: None,
        index: None,
    }
    "#);
    Ok(())
}
