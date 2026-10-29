use std::fs;

use but_testsupport::{gix_testtools, invoke_bash_at_dir};
use gitbutler_git::tokio::TokioExecutor;

#[test]
fn fetch_without_git_preserves_refspec_fallback_tags_pruning_and_fetch_head() -> anyhow::Result<()>
{
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let tmp = gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        r#"
        git init -q --object-format=sha256 --initial-branch=main source
        git -C source commit -qm initial --allow-empty
        git -C source branch removed
        git -C source tag old-tag
        git clone -q source client
        git -C client remote set-url origin ../source
        git -C client config --unset-all remote.origin.fetch
        git -C client config fetch.prune true
        git -C client config fetch.pruneTags true
        git -C source branch -D removed
        git -C source tag -d old-tag
        git -C source commit -qm updated --allow-empty
        git -C source branch feature
        git -C source tag new-tag
        cp -R client expected
        git -C expected fetch -q origin '+refs/heads/*:refs/remotes/origin/*'
        "#,
        tmp.path(),
    );
    let expected = but_testsupport::open_repo(&tmp.path().join("expected"))?;
    let expected_fetch_head = fs::read(expected.git_dir().join("FETCH_HEAD"))?;
    let expected_refs = refs(&expected)?;
    // SAFETY: this test runs in its own process, before starting the async runtime.
    unsafe { std::env::set_var("PATH", tmp.path().join("no-executables")) };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(gitbutler_git::fetch(
        tmp.path().join("client"),
        TokioExecutor,
        "origin",
        None::<fn(String) -> std::future::Ready<Option<String>>>,
    ))?;
    let actual = but_testsupport::open_repo(&tmp.path().join("client"))?;
    assert_eq!(
        refs(&actual)?,
        expected_refs,
        "native fetch matches Git's refs, including pruning and tags"
    );
    assert_eq!(
        fs::read(actual.git_dir().join("FETCH_HEAD"))?,
        expected_fetch_head,
        "FETCH_HEAD retains Git's fetched-ref and merge-selection information"
    );
    Ok(())
}

#[test]
fn fetch_without_git_honors_tag_policy_and_reports_missing_refs() -> anyhow::Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let tmp = gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        r#"
        git init -q --initial-branch=main source
        git -C source commit -qm initial --allow-empty
        git -C source tag v1
        git init -q --initial-branch=main client
        git -C client remote add origin "$PWD/source"
        git -C client config remote.origin.fetch '+refs/heads/main:refs/remotes/origin/main'
        git -C client config remote.origin.tagOpt --no-tags
        git init -q missing
        git -C missing remote add origin "$PWD/source"
        git -C missing config remote.origin.fetch '+refs/heads/main:refs/remotes/origin/main'
        git -C missing config --add remote.origin.fetch '+refs/heads/absent:refs/remotes/origin/absent'
        "#,
        tmp.path(),
    );
    // SAFETY: this isolated test has not started any worker threads.
    unsafe { std::env::set_var("PATH", tmp.path().join("no-executables")) };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(gitbutler_git::fetch(
        tmp.path().join("client"),
        TokioExecutor,
        "origin",
        None::<fn(String) -> std::future::Ready<Option<String>>>,
    ))?;
    let actual = but_testsupport::open_repo(&tmp.path().join("client"))?;
    assert!(
        actual.try_find_reference("refs/tags/v1")?.is_none(),
        "--no-tags disables automatic tag following"
    );
    let error = runtime
        .block_on(gitbutler_git::fetch(
            tmp.path().join("missing"),
            TokioExecutor,
            "origin",
            None::<fn(String) -> std::future::Ready<Option<String>>>,
        ))
        .expect_err("one missing exact ref rejects the whole fetch before ref updates");
    assert!(
        matches!(error, gitbutler_git::Error::RefNotFound(ref name) if name == "refs/heads/absent"),
        "missing refs retain their name and public error variant: {error}"
    );
    let missing = but_testsupport::open_repo(&tmp.path().join("missing"))?;
    assert!(
        missing
            .try_find_reference("refs/remotes/origin/main")?
            .is_none(),
        "a missing exact ref prevents partial fetch updates"
    );
    Ok(())
}

#[test]
fn fetch_without_git_reports_rejected_ref_updates() -> anyhow::Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let tmp = gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        r#"
        git init -q --initial-branch=main source
        git -C source commit -qm initial --allow-empty
        git -C source tag v1
        git clone -q source client
        git -C client commit -qm local --allow-empty
        git -C client update-ref refs/remotes/origin/main HEAD
        git -C client config remote.origin.fetch 'refs/heads/main:refs/remotes/origin/main'
        git -C client remote add checkedout "$PWD/source"
        git -C client config remote.checkedout.fetch 'refs/heads/main:refs/heads/main'
        git -C client remote add tags "$PWD/source"
        git -C client config remote.tags.fetch 'refs/tags/v1:refs/tags/v1'
        git -C source commit -qm remote --allow-empty
        git -C source tag -f v1
        "#,
        tmp.path(),
    );
    let client = tmp.path().join("client");
    let before = refs(&but_testsupport::open_repo(&client)?)?;
    // SAFETY: this test runs in its own process before starting the async runtime.
    unsafe { std::env::set_var("PATH", tmp.path().join("no-executables")) };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    for (remote, reason) in [
        ("origin", "non-fast-forward"),
        ("checkedout", "checked-out branch"),
        ("tags", "overwrite existing tag"),
    ] {
        let error = runtime
            .block_on(gitbutler_git::fetch(
                &client,
                TokioExecutor,
                remote,
                None::<fn(String) -> std::future::Ready<Option<String>>>,
            ))
            .expect_err("rejected updates make the fetch fail even though objects were received");
        assert!(
            matches!(error, gitbutler_git::Error::Backend(_)) && error.to_string().contains(reason),
            "the native update rejection is reported to the caller: {error}"
        );
        assert_eq!(
            refs(&but_testsupport::open_repo(&client)?)?,
            before,
            "rejected updates leave existing reference targets intact"
        );
    }
    Ok(())
}

fn refs(repo: &gix::Repository) -> anyhow::Result<Vec<(gix::bstr::BString, gix::ObjectId)>> {
    let mut refs = repo
        .references()?
        .all()?
        .map(|reference| {
            let mut reference = reference?;
            let name = reference.name().as_bstr().to_owned();
            Ok((name, reference.peel_to_id()?.detach()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    refs.sort();
    Ok(refs)
}
