use std::fs;

use but_testsupport::{gix_testtools, invoke_bash_at_dir};
use gitbutler_git::tokio::TokioExecutor;

#[test]
fn cloning_without_git_preserves_object_format_refs_and_checkout_filters() -> anyhow::Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let tmp = gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        r#"
        git init -q --object-format=sha256 --initial-branch=main source
        printf 'file text eol=crlf\n' >source/.gitattributes
        printf 'content\n' >source/file
        git -C source add .
        git -C source commit -qm initial
        git -C source branch another
        git -C source tag v1
        "#,
        tmp.path(),
    );
    let source = but_testsupport::open_repo(&tmp.path().join("source"))?;
    let commit_id = source.head_id()?.detach();
    let destination = tmp.path().join("nested/clone with spaces");
    // SAFETY: this isolated process runs only this test, before starting the async runtime.
    unsafe { std::env::set_var("PATH", tmp.path().join("no-executables")) };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(gitbutler_git::clone(
        source
            .workdir()
            .expect("source has a worktree")
            .to_str()
            .expect("UTF-8 fixture path"),
        &destination,
        TokioExecutor,
        None::<fn(String) -> std::future::Ready<Option<String>>>,
    ))?;

    let cloned = but_testsupport::open_repo(&destination)?;
    assert_eq!(
        cloned.object_hash(),
        gix::hash::Kind::Sha256,
        "clone adopts the source format"
    );
    assert_eq!(
        cloned.head_id()?.detach(),
        commit_id,
        "clone preserves the advertised tip"
    );
    assert_eq!(
        cloned.head_name()?.expect("attached HEAD").as_bstr(),
        "refs/heads/main",
        "clone attaches the default branch"
    );
    for name in ["refs/remotes/origin/another", "refs/tags/v1"] {
        assert_eq!(
            cloned.find_reference(name)?.peel_to_id()?.detach(),
            commit_id,
            "clone transfers branches and tags"
        );
    }
    assert_eq!(
        fs::read(destination.join("file"))?,
        b"content\r\n",
        "checkout applies attributes"
    );
    let protected = tmp.path().join("protected");
    fs::create_dir(&protected)?;
    fs::write(protected.join("keep"), b"untouched")?;
    let error = runtime
        .block_on(gitbutler_git::clone(
            source
                .workdir()
                .expect("non-bare")
                .to_str()
                .expect("UTF-8 path"),
            &protected,
            TokioExecutor,
            None::<fn(String) -> std::future::Ready<Option<String>>>,
        ))
        .expect_err("clone refuses a nonempty destination");
    assert!(
        matches!(error, gitbutler_git::Error::RemoteExists(..)),
        "a nonempty destination retains its public error classification: {error}"
    );
    assert_eq!(
        fs::read(protected.join("keep"))?,
        b"untouched",
        "rejection preserves existing data"
    );
    assert!(
        !protected.join(".git").exists(),
        "rejection creates no repository"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn clone_runs_post_checkout_hooks_with_the_filter_environment() -> anyhow::Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let tmp = gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        r#"
        git init -q --initial-branch=main source
        printf 'file filter=prompt\n' >source/.gitattributes
        printf 'content\n' >source/file
        git -C source add .
        git -C source commit -qm initial
        git init -q empty
        mkdir -p template/hooks
        printf 'template content\n' >template/custom
        cat >template/hooks/post-checkout <<'HOOK'
#!/bin/sh
printf '%s\n' "$@" "$GIT_TERMINAL_PROMPT" >hook-output
exit 3
HOOK
        chmod +x template/hooks/post-checkout
        "#,
        tmp.path(),
    );
    let source = but_testsupport::open_repo(&tmp.path().join("source"))?;
    let commit_id = source.head_id()?.detach();
    let global = tmp.path().join("global.config");
    fs::write(
        &global,
        "[filter \"prompt\"]\n smudge = printf '%s' \\\"$GIT_TERMINAL_PROMPT\\\"\n required = true\n",
    )?;
    // SAFETY: only this test runs in this process, before any async runtime starts.
    unsafe {
        std::env::set_var("GIT_CONFIG_GLOBAL", &global);
        std::env::set_var("GIT_TEMPLATE_DIR", tmp.path().join("template"));
        std::env::set_var("PATH", tmp.path().join("no-executables"));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let parent_prompt = std::env::var_os("GIT_TERMINAL_PROMPT");
    let destination = tmp.path().join("clone");
    let error = runtime
        .block_on(gitbutler_git::clone(
            source
                .workdir()
                .expect("source has a worktree")
                .to_str()
                .expect("UTF-8 fixture path"),
            &destination,
            TokioExecutor,
            None::<fn(String) -> std::future::Ready<Option<String>>>,
        ))
        .expect_err("a failing post-checkout hook fails clone after keeping its checkout");
    assert!(
        error.to_string().contains("Post-checkout hook failed"),
        "hook failure is reported: {error}"
    );
    assert_eq!(
        fs::read(destination.join(".git/custom"))?,
        b"template content\n",
        "clone copies configured template files"
    );
    assert_eq!(
        fs::read(destination.join("file"))?,
        b"0",
        "smudge filters inherit the prompt policy"
    );
    assert_eq!(
        fs::read_to_string(destination.join("hook-output"))?,
        format!("{}\n{commit_id}\n1\n0\n", source.object_hash().null()),
        "post-checkout receives the old/new heads, checkout flag and prompt policy"
    );
    assert_eq!(
        but_testsupport::open_repo(&destination)?
            .head_id()?
            .detach(),
        commit_id,
        "hook rejection retains the completed clone, as Git does"
    );
    assert_eq!(
        std::env::var_os("GIT_TERMINAL_PROMPT"),
        parent_prompt,
        "the parent prompt policy is unchanged"
    );
    let empty_destination = tmp.path().join("empty-clone");
    runtime.block_on(gitbutler_git::clone(
        tmp.path()
            .join("empty")
            .to_str()
            .expect("UTF-8 fixture path"),
        &empty_destination,
        TokioExecutor,
        None::<fn(String) -> std::future::Ready<Option<String>>>,
    ))?;
    assert!(
        !empty_destination.join("hook-output").exists(),
        "an empty clone has no checkout and does not invoke its post-checkout hook"
    );
    Ok(())
}

#[test]
fn clone_aborts_when_the_http_remote_stalls() -> anyhow::Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    use std::{net::TcpListener, time::Duration};
    let tmp = gix_testtools::tempfile::tempdir()?;
    let global = tmp.path().join("global.config");
    fs::write(&global, "[http]\n lowSpeedTime = 1\n")?;
    // SAFETY: this test runs in its own process before spawning threads.
    unsafe {
        std::env::set_var("GIT_CONFIG_GLOBAL", &global);
        std::env::set_var("PATH", tmp.path().join("no-executables"));
    }
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}/repository", listener.local_addr()?);
    std::thread::spawn(move || {
        let _connections = listener.incoming().collect::<Vec<_>>();
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let error = runtime
        .block_on(async {
            tokio::time::timeout(
                Duration::from_secs(15),
                gitbutler_git::clone(
                    &url,
                    tmp.path().join("clone"),
                    TokioExecutor,
                    None::<fn(String) -> std::future::Ready<Option<String>>>,
                ),
            )
            .await
        })?
        .expect_err("a stalled remote must fail");
    assert!(
        format!("{error:?}").contains("Operation too slow"),
        "the configured stall policy aborts clone: {error}"
    );
    Ok(())
}

#[test]
fn clone_keeps_the_async_askpass_broker_without_git() -> anyhow::Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Duration,
    };
    let tmp = gix_testtools::tempfile::tempdir()?;
    let global = tmp.path().join("global.config");
    fs::write(&global, "[credential]\n helper =\n")?;
    // SAFETY: this isolated test has not started other threads yet.
    unsafe {
        std::env::set_var("GIT_CONFIG_GLOBAL", &global);
        std::env::set_var("PATH", tmp.path().join("no-executables"));
    }
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}/repository", listener.local_addr()?);
    std::thread::spawn(move || {
        for mut connection in listener.incoming().flatten() {
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                if connection.read(&mut byte).ok() != Some(1) {
                    break;
                }
                request.push(byte[0]);
            }
            let _ = connection.write_all(b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"test\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut prompts = Vec::new();
    let error = runtime
        .block_on(async {
            tokio::time::timeout(
                Duration::from_secs(15),
                gitbutler_git::clone(
                    &url,
                    tmp.path().join("clone"),
                    TokioExecutor,
                    Some(|prompt| {
                        prompts.push(prompt);
                        std::future::ready(None)
                    }),
                ),
            )
            .await
        })?
        .expect_err("declining credentials fails clone");
    assert_eq!(
        prompts.len(),
        1,
        "native credential prompting reaches the asynchronous broker once"
    );
    assert!(
        error.to_string().contains("none were provided"),
        "declined prompts retain their classification: {error}"
    );
    Ok(())
}

#[test]
fn clone_resolves_relative_sources_from_the_destination_parent() -> anyhow::Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let tmp = gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        "git init -q source\ngit -C source commit -qm initial --allow-empty",
        tmp.path(),
    );
    let parent = tmp.path().join("clones");
    fs::create_dir(&parent)?;
    let source = but_testsupport::open_repo(&tmp.path().join("source"))?;
    let expected_id = source.head_id()?.detach();
    let _environment = gix_testtools::Env::new().set("PATH", "");
    let runtime = tokio::runtime::Runtime::new()?;
    let destination = parent.join("client");
    runtime.block_on(gitbutler_git::clone(
        "../source",
        &destination,
        TokioExecutor,
        None::<fn(String) -> std::future::Ready<Option<String>>>,
    ))?;
    let cloned = but_testsupport::open_repo(&destination)?;
    assert_eq!(
        cloned.head_id()?,
        expected_id,
        "the relative source resolves from the operation directory"
    );
    let remote = cloned.find_remote("origin")?;
    let url = remote
        .url(gix::remote::Direction::Fetch)
        .expect("clone origin");
    assert!(
        gix::path::from_bstr(&url.path)?.is_absolute(),
        "cloning persists an absolute local origin for subsequent fetches"
    );
    Ok(())
}
