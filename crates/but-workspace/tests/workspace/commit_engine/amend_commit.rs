use but_core::{DiffSpec, HunkHeader};
use but_testsupport::assure_stable_env;
use but_workspace::commit_engine::Destination;

use crate::utils::{
    CONTEXT_LINES, cat_commit, commit_from_outcome,
    commit_whole_files_and_all_hunks_from_workspace,
    read_only_in_memory_scenario_non_isolated_keep_env, visualize_commit, visualize_tree,
    writable_scenario, writable_scenario_with_ssh_key, write_local_config, write_sequence,
};

#[test]
fn all_changes_and_renames_to_topmost_commit_no_parent() -> anyhow::Result<()> {
    assure_stable_env();

    let mut repo =
        read_only_in_memory_scenario_non_isolated_keep_env("all-file-types-renamed-and-modified")?;
    // Change the committer and author dates to be able to tell what it changes.
    {
        let mut config = repo.config_snapshot_mut();
        config.set_value(
            &gix::config::tree::gitoxide::Commit::COMMITTER_DATE,
            "946771266 +0600",
        )?;
        config.set_value(
            &gix::config::tree::gitoxide::Commit::AUTHOR_DATE,
            "946684866 +0600",
        )?;
    }
    let head_commit = repo.rev_parse_single("HEAD")?;
    insta::assert_snapshot!(but_testsupport::visualize_tree(head_commit.object()?.peel_to_tree()?.id()), @r#"
    3fd29f0
    ├── executable:100755:01e79c3 "1\n2\n3\n"
    ├── file:100644:3aac70f "5\n6\n7\n8\n"
    └── link:120000:c4c364c "nonexisting-target"
    "#);
    insta::assert_snapshot!(cat_commit(head_commit)?, @r"
    tree 3fd29f0ca55ee4dc3ea6bf02a761c15fd6dc8428
    author author <author@example.com> 946684800 +0000
    committer committer <committer@example.com> 946771200 +0000
    gitbutler-headers-version 2
    gitbutler-change-id 00000000-0000-0000-0000-000000003333

    init
    ");
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::AmendCommit {
            commit_id: head_commit.into(),
            new_message: Some("init: amended".into()),
        },
    )?;
    insta::assert_debug_snapshot!(&outcome, @r"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(e69a4dfd9ef6d55fd4e49f801612fbe061677adb),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(e56fc9bacdd11ebe576b5d96d21127c423698126),
        ),
        references: [],
        rebase_output: None,
        index: None,
    }
    ");
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    e56fc9b
    ├── executable-renamed:100755:8a1218a "1\n2\n3\n4\n5\n"
    ├── file-renamed:100644:c5c4315 "5\n6\n7\n8\n9\n10\n"
    └── link-renamed:120000:94e4e07 "other-nonexisting-target"
    "#);

    // It adjusts both the author and the committer date.
    // It does, however, leave the change-id as it's considered the ID of the commit itself,
    // which thus should never be removed.
    insta::assert_snapshot!(visualize_commit(&repo, &outcome)?, @r"
    tree e56fc9bacdd11ebe576b5d96d21127c423698126
    author author <author@example.com> 946684866 +0600
    committer committer (From Env) <committer@example.com> 946771266 +0600
    gitbutler-headers-version 2
    gitbutler-change-id 00000000-0000-0000-0000-000000003333

    init: amended
    ");

    Ok(())
}

#[test]
fn all_aspects_of_amended_commit_are_copied() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");
    // Rewrite the entire file, which is fine as we rewrite/amend the base-commit itself.
    write_sequence(&repo, "file", [(40, 70)])?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::AmendCommit {
            commit_id: repo.rev_parse_single("merge")?.detach(),
            new_message: None,
        },
    )?;
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    5bbee6d
    └── file:100644:1c9325b "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n"
    "#);
    // We do not add a change-id to assure we operate correctly when doing similarity checks.
    insta::assert_snapshot!(visualize_commit(&repo, &outcome)?, @r"
    tree 5bbee6d0219923e795f7b0818dda2f33f16278b4
    parent 91ef6f6fc0a8b97fb456886c1cc3b2a3536ea2eb
    parent 7f389eda1b366f3d56ecc1300b3835727c3309b6
    author author <author@example.com> 946684800 +0000
    committer committer (From Env) <committer@example.com> 946771200 +0000

    Merge branch 'A' into merge
    ");
    Ok(())
}

#[test]
fn new_file_and_deletion_onto_merge_commit() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");
    // Rewrite the entire file, which is fine as we rewrite/amend the base-commit itself.
    write_sequence(&repo, "new-file", [(10, None)])?;
    std::fs::remove_file(repo.workdir_path("file").expect("non-bare"))?;

    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::AmendCommit {
            commit_id: repo.rev_parse_single("merge")?.detach(),
            new_message: None,
        },
    )?;
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    f8009d7
    └── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    "#);
    Ok(())
}

#[test]
fn make_a_file_empty() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");
    // Empty the file
    std::fs::write(repo.workdir_path("file").expect("non-bare"), "")?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::AmendCommit {
            commit_id: repo.rev_parse_single("merge")?.detach(),
            new_message: None,
        },
    )?;
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    df2b8fc
    └── file:100644:e69de29 ""
    "#);
    Ok(())
}

#[test]
fn new_file_and_deletion_onto_merge_commit_with_hunks() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");
    // Rewrite the entire file, which is fine as we rewrite/amend the base-commit itself.
    write_sequence(&repo, "new-file", [(10, None)])?;
    std::fs::remove_file(repo.workdir_path("file").expect("non-bare"))?;

    let outcome = but_workspace::commit_engine::create_commit(
        &repo,
        Destination::AmendCommit {
            commit_id: repo.rev_parse_single("merge")?.detach(),
            new_message: None,
        },
        vec![
            DiffSpec {
                previous_path: None,
                path: "file".into(),
                hunk_headers: vec![],
            },
            DiffSpec {
                previous_path: None,
                path: "new-file".into(),
                hunk_headers: vec![HunkHeader {
                    old_start: 1,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: 10,
                }],
            },
        ],
        CONTEXT_LINES,
    )?;
    assert_eq!(outcome.rejected_specs, vec![]);
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    f8009d7
    └── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    "#);
    Ok(())
}

#[test]
fn signatures_are_redone() -> anyhow::Result<()> {
    assure_stable_env();

    let (mut repo, _tmp) = writable_scenario_with_ssh_key("two-signed-commits-with-line-offset");

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
        Destination::AmendCommit {
            commit_id: head_id,
            new_message: None,
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

    repo.config_snapshot_mut()
        .set_raw_value(&"gitbutler.signCommits", "false")?;
    write_local_config(&repo)?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::AmendCommit {
            commit_id: head_id,
            new_message: None,
        },
    )?;
    let new_commit = commit_from_outcome(&repo, &outcome)?;
    assert!(
        new_commit.extra_headers().pgp_signature().is_none(),
        "If signing commits is disabled, \
    it will drop the signature (instead of leaving an invalid one)"
    );

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    3412b2c
    ├── .gitignore:100644:ccc87a0 "*.key*\n"
    └── file:100644:a07b65a "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    "#);
    Ok(())
}
