use crate::commit_engine::utils::{
    commit_whole_files_and_all_hunks_from_workspace, read_only_in_memory_scenario, stable_env,
    visualize_tree,
};
use but_workspace::commit_engine::Destination;
use gix::prelude::ObjectIdExt;
use serial_test::serial;

#[test]
#[serial]
fn all_changes_and_renames_to_topmost_commit_no_parent() -> anyhow::Result<()> {
    let _env = stable_env();

    let repo = read_only_in_memory_scenario("all-file-types-renamed-and-modified")?;
    let head_commit = repo.rev_parse_single("HEAD")?;
    insta::assert_snapshot!(gitbutler_testsupport::visualize_gix_tree(head_commit.object()?.peel_to_tree()?.id()), @r#"
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
    }
    ");
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    0236fb1
    ├── executable-renamed:100755:94ebaf9 "1\n2\n3\n4\n"
    ├── file-renamed:100644:66f816c "5\n6\n7\n8\n9\n"
    └── link-renamed:120000:94e4e07 "other-nonexisting-target"
    "#);
    insta::assert_debug_snapshot!(commit_from_outcome(&repo, &outcome)?, @r#"
    Commit {
        tree: Sha1(0236fb167942f3665aa348c514e8d272a6581ad5),
        parents: [],
        author: Signature {
            name: "author",
            email: "author@example.com",
            time: Time {
                seconds: 946684800,
                offset: 0,
                sign: Plus,
            },
        },
        committer: Signature {
            name: "committer",
            email: "committer@example.com",
            time: Time {
                seconds: 946771200,
                offset: 0,
                sign: Plus,
            },
        },
        encoding: None,
        message: "init\n",
        extra_headers: [],
    }
    "#);
    Ok(())
}

#[test]
#[serial]
#[ignore = "TBD"]
fn all_aspects_of_amended_commit_are_copied_including_extra_headers() -> anyhow::Result<()> {
    Ok(())
}

#[test]
#[serial]
#[ignore = "TBD"]
fn signatures_are_redone() -> anyhow::Result<()> {
    Ok(())
}

fn commit_from_outcome(
    repo: &gix::Repository,
    outcome: &but_workspace::commit_engine::CreateCommitOutcome,
) -> anyhow::Result<gix::objs::Commit> {
    Ok(outcome
        .new_commit
        .expect("the amended commit was created")
        .attach(repo)
        .object()?
        .peel_to_commit()?
        .decode()?
        .into())
}
