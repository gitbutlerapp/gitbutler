use anyhow::Result;
use but_api::hosted::hosted_machines;
use but_testsupport::{CommandExt, git_at_dir};

/// A hub's store for one project, as a push from `studio` leaves it: the branch under
/// `refs/heads/<machine>/<branch>` and a snapshot on top of it naming the branch.
fn hub_store_with_published_branch() -> Result<(gix::Repository, tempfile::TempDir)> {
    let tmp = tempfile::tempdir()?;
    let source = tmp.path().join("source");
    git_at_dir(tmp.path())
        .args(["init", "-q", "-b", "main", "source"])
        .run();
    std::fs::write(source.join("README.md"), "hello\n")?;
    git_at_dir(&source).args(["add", "-A"]).run();
    git_at_dir(&source)
        .args(["commit", "-q", "-m", "Initial commit"])
        .run();
    git_at_dir(&source)
        .args(["checkout", "-q", "-b", "feature/search"])
        .run();
    std::fs::write(source.join("search.txt"), "search\n")?;
    git_at_dir(&source).args(["add", "-A"]).run();
    git_at_dir(&source)
        .args(["commit", "-q", "-m", "Add search"])
        .run();

    let store = tmp.path().join("store").join("project.git");
    git_at_dir(tmp.path())
        .args(["init", "-q", "--bare"])
        .arg(&store)
        .run();
    git_at_dir(&source)
        .arg("push")
        .arg("-q")
        .arg(&store)
        .arg("feature/search:refs/heads/studio/feature/search")
        .run();
    let tip = git_at_dir(&store)
        .args(["rev-parse", "refs/heads/studio/feature/search"])
        .output()?;
    let tip = String::from_utf8(tip.stdout)?.trim().to_owned();
    let snapshot = git_at_dir(&store)
        .args(["commit-tree", &format!("{tip}^{{tree}}"), "-p", &tip, "-m"])
        .arg(r#"{"version":1,"head":"refs/heads/feature/search"}"#)
        .output()?;
    let snapshot = String::from_utf8(snapshot.stdout)?.trim().to_owned();
    git_at_dir(&store)
        .args([
            "update-ref",
            "refs/gitbutler/snapshots/studio/feature-search",
            &snapshot,
        ])
        .run();

    Ok((gix::open(&store)?, tmp))
}

#[test]
fn a_branch_on_the_hub_names_the_hub_s_own_ref() -> Result<()> {
    let (repo, _tmp) = hub_store_with_published_branch()?;
    let ctx = but_ctx::Context::from_repo_for_testing(repo.clone())?.with_memory_app_cache();

    let project = hosted_machines(&ctx)?;
    let branch = &project.machines[0].branches[0];

    assert_eq!(
        branch.branch, "feature/search",
        "the snapshot names the branch by its own name, slash included"
    );
    assert_eq!(
        branch.ref_name, "refs/heads/studio/feature/search",
        "on the hub a machine's branch is its own ref, not a fetched copy"
    );
    assert!(
        repo.try_find_reference(branch.ref_name.as_str())?.is_some(),
        "the named ref is one the hub has, so a diff can be read from it"
    );
    Ok(())
}
