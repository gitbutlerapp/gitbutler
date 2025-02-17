use crate::commit_engine::utils::{
    assure_stable_env, commit_from_outcome, commit_whole_files_and_all_hunks_from_workspace,
    read_only_in_memory_scenario, visualize_commit, visualize_tree, writable_scenario,
    writable_scenario_with_ssh_key, write_local_config, write_sequence,
};
use but_workspace::commit_engine::Destination;

#[test]
fn all_changes_and_renames_to_topmost_commit_no_parent() -> anyhow::Result<()> {
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
        Destination::AmendCommit(head_commit.into()),
    )?;
    insta::assert_debug_snapshot!(&outcome, @r"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(aacf6391c96a59461df0a241caad4ad24368f542),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(0236fb167942f3665aa348c514e8d272a6581ad5),
        ),
        references: [],
        rebase_output: None,
        index: None,
    }
    ");
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    0236fb1
    ├── executable-renamed:100755:94ebaf9 "1\n2\n3\n4\n"
    ├── file-renamed:100644:66f816c "5\n6\n7\n8\n9\n"
    └── link-renamed:120000:94e4e07 "other-nonexisting-target"
    "#);
    insta::assert_snapshot!(visualize_commit(&repo, &outcome)?, @r"
    tree 0236fb167942f3665aa348c514e8d272a6581ad5
    author author <author@example.com> 946684800 +0000
    committer committer <committer@example.com> 946771200 +0000

    init
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
        Destination::AmendCommit(repo.rev_parse_single("merge")?.detach()),
    )?;
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    5bbee6d
    └── file:100644:1c9325b "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n"
    "#);
    insta::assert_snapshot!(visualize_commit(&repo, &outcome)?, @r"
    tree 5bbee6d0219923e795f7b0818dda2f33f16278b4
    parent 91ef6f6fc0a8b97fb456886c1cc3b2a3536ea2eb
    parent 7f389eda1b366f3d56ecc1300b3835727c3309b6
    author author <author@example.com> 946684800 +0000
    committer committer <committer@example.com> 946771200 +0000

    Merge branch 'A' into merge
    ");
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
    let outcome =
        commit_whole_files_and_all_hunks_from_workspace(&repo, Destination::AmendCommit(head_id))?;

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
    let outcome =
        commit_whole_files_and_all_hunks_from_workspace(&repo, Destination::AmendCommit(head_id))?;
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
