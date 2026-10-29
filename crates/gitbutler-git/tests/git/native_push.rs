use std::fs;

use but_testsupport::{gix_testtools, invoke_bash_at_dir};
use gitbutler_git::{RefSpec, tokio::TokioExecutor};

fn push(
    runtime: &tokio::runtime::Runtime,
    path: &std::path::Path,
    spec: &str,
    force: bool,
    protected: bool,
    options: Vec<String>,
) -> Result<String, gitbutler_git::Error<impl std::error::Error + Send + Sync + 'static>> {
    runtime.block_on(gitbutler_git::push(
        path,
        TokioExecutor,
        "origin",
        RefSpec::parse(spec).expect("test refspec"),
        force,
        protected,
        None::<fn(String) -> std::future::Ready<Option<String>>>,
        options,
    ))
}

#[test]
fn native_push_updates_every_push_url_tracking_refs_and_remote_output() -> anyhow::Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let tmp = gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        r#"
        git init -q --object-format=sha256 --initial-branch=main client
        git -C client commit -qm initial --allow-empty
        git clone -q --bare client one.git
        git clone -q --bare client two.git
        git -C client remote add origin "$PWD/one.git"
        git -C client remote set-url --add --push origin ../one.git
        git -C client remote set-url --add --push origin ../two.git
        git -C client fetch -q origin
        git -C client commit -qm next --allow-empty
        for receiver in one.git two.git; do
            git -C "$receiver" config receive.advertisePushOptions true
            cat >"$receiver/hooks/pre-receive" <<'HOOK'
#!/bin/sh
printf 'option=%s prompt=%s\n' "$GIT_PUSH_OPTION_0" "$GIT_TERMINAL_PROMPT" >&2
HOOK
            chmod +x "$receiver/hooks/pre-receive"
        done
        printf '#!/bin/sh\nexit 1\n' >client/.git/hooks/pre-push
        chmod +x client/.git/hooks/pre-push
        "#,
        tmp.path(),
    );
    let client = but_testsupport::open_repo(&tmp.path().join("client"))?;
    let tip_id = client.head_id()?.detach();
    let _environment = gix_testtools::Env::new().set("PATH", "");
    let runtime = tokio::runtime::Runtime::new()?;
    let output = push(
        &runtime,
        client.workdir().expect("client checkout"),
        "HEAD:refs/heads/main",
        false,
        false,
        vec!["review=yes".into()],
    )?;
    assert_eq!(
        output.matches("option=review=yes prompt=0").count(),
        2,
        "each receiver gets push options and the operation environment; output is retained"
    );
    for receiver in ["one.git", "two.git"] {
        assert_eq!(
            but_testsupport::open_repo(&tmp.path().join(receiver))?.head_id()?,
            tip_id,
            "every configured push URL is updated"
        );
    }
    assert_eq!(
        client.find_reference("refs/remotes/origin/main")?.id(),
        tip_id,
        "successful push updates the local tracking ref"
    );
    assert!(
        fs::read_to_string(client.git_dir().join("logs/refs/remotes/origin/main"))?
            .contains("update by push"),
        "tracking updates leave a reflog entry"
    );
    fs::write(
        client.git_dir().join("refs/remotes/origin/main.lock"),
        "held\n",
    )?;
    let old_id = client.rev_parse_single("HEAD^")?.detach();
    let error = push(
        &runtime,
        client.workdir().expect("client checkout"),
        "HEAD^:refs/heads/main",
        true,
        false,
        vec!["review=yes".into()],
    )
    .expect_err("a local tracking lock must be reported after the push succeeds");
    assert!(
        error.to_string().contains("push succeeded"),
        "the error identifies the completed remote mutation: {error}"
    );
    for receiver in ["one.git", "two.git"] {
        assert_eq!(
            but_testsupport::open_repo(&tmp.path().join(receiver))?.head_id()?,
            old_id,
            "local tracking errors do not prevent later push URLs"
        );
    }
    assert_eq!(
        client.find_reference("refs/remotes/origin/main")?.id(),
        tip_id,
        "locked tracking refs stay unchanged"
    );
    let first_receiver = but_testsupport::open_repo(&tmp.path().join("one.git"))?;
    first_receiver.reference(
        "refs/heads/main",
        tip_id,
        gix::refs::transaction::PreviousValue::Any,
        "restore the receiver before testing a later remote failure",
    )?;
    fs::rename(
        tmp.path().join("two.git"),
        tmp.path().join("unavailable.git"),
    )?;
    let error = push(
        &runtime,
        client.workdir().expect("client checkout"),
        "HEAD^:refs/heads/main",
        true,
        false,
        vec!["review=yes".into()],
    )
    .expect_err("the second push URL is unavailable");
    assert!(
        error.to_string().contains("two.git"),
        "the later remote failure takes precedence over a local tracking error: {error}"
    );
    assert_eq!(
        first_receiver.head_id()?,
        old_id,
        "the first push URL succeeds before the second one fails"
    );
    assert_eq!(
        client.find_reference("refs/remotes/origin/main")?.id(),
        tip_id,
        "the first receiver's successful push still cannot update its locked tracking ref"
    );
    Ok(())
}

#[test]
fn native_push_preserves_force_protection_and_deletion_without_git() -> anyhow::Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let tmp = gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        r#"
        git init -q --initial-branch=main client
        git -C client commit -qm base --allow-empty
        git -C client commit -qm original --allow-empty
        git clone -q --bare client remote.git
        git -C client remote add origin "$PWD/remote.git"
        git -C client fetch -q origin
        git -C remote.git config receive.denyDeleteCurrent ignore
        git -C client commit -q --amend -m rewritten --allow-empty
        "#,
        tmp.path(),
    );
    let client = but_testsupport::open_repo(&tmp.path().join("client"))?;
    let receiver = but_testsupport::open_repo(&tmp.path().join("remote.git"))?;
    let original_id = receiver.head_id()?.detach();
    let rewritten_id = client.head_id()?.detach();
    let _environment = gix_testtools::Env::new().set("PATH", "");
    let runtime = tokio::runtime::Runtime::new()?;
    let workdir = client.workdir().expect("client checkout");
    let error = push(
        &runtime,
        workdir,
        "HEAD:refs/heads/main",
        false,
        false,
        vec![],
    )
    .expect_err("a rewrite needs force");
    assert!(
        matches!(error, gitbutler_git::Error::NonFastForward(_)),
        "the typed rejection remains recoverable: {error}"
    );
    push(
        &runtime,
        workdir,
        "HEAD:refs/heads/main",
        true,
        true,
        vec![],
    )?;
    assert_eq!(
        receiver.head_id()?,
        rewritten_id,
        "the reflog includes the original remote tip"
    );
    receiver.reference(
        "refs/heads/main",
        original_id,
        gix::refs::transaction::PreviousValue::Any,
        "concurrent update",
    )?;
    let error = push(
        &runtime,
        workdir,
        "HEAD:refs/heads/main",
        true,
        true,
        vec![],
    )
    .expect_err("a stale tracking lease protects remote changes");
    assert!(
        matches!(error, gitbutler_git::Error::ForcePushProtection(_)),
        "lease failures retain the force-protection category: {error}"
    );
    assert_eq!(
        receiver.head_id()?,
        original_id,
        "rejection preserves the remote"
    );
    client.reference(
        "refs/remotes/origin/main",
        original_id,
        gix::refs::transaction::PreviousValue::Any,
        "fetch",
    )?;
    push(&runtime, workdir, ":refs/heads/main", true, true, vec![])?;
    assert!(
        receiver.try_find_reference("refs/heads/main")?.is_none(),
        "a matching lease allows deletion"
    );
    assert!(
        client
            .try_find_reference("refs/remotes/origin/main")?
            .is_none(),
        "deletion removes the tracking ref"
    );
    let error = push(
        &runtime,
        workdir,
        "absent:refs/heads/main",
        false,
        false,
        vec![],
    )
    .expect_err("missing push sources are rejected before connecting");
    assert!(
        matches!(&error, gitbutler_git::Error::RefNotFound(name) if name == "absent"),
        "missing source retains its error category: {error}"
    );
    Ok(())
}

#[test]
fn native_push_keeps_the_async_askpass_broker_without_git() -> anyhow::Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Duration,
    };
    let tmp = gix_testtools::tempfile::tempdir()?;
    invoke_bash_at_dir(
        "git init -q client\ngit -C client commit -qm initial --allow-empty",
        tmp.path(),
    );
    let repo = but_testsupport::open_repo(&tmp.path().join("client"))?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let mut config = repo.config_file_mut(repo.git_dir().join("config"))?;
    config.set_raw_value(
        "remote.origin.url",
        format!("http://{}/repository", listener.local_addr()?),
    )?;
    config.set_raw_value("credential.helper", "")?;
    config.commit()?;
    let _environment = gix_testtools::Env::new().set("PATH", "");
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
    let runtime = tokio::runtime::Runtime::new()?;
    let mut prompts = Vec::new();
    let error = runtime
        .block_on(async {
            tokio::time::timeout(
                Duration::from_secs(15),
                gitbutler_git::push(
                    repo.workdir().expect("client checkout"),
                    TokioExecutor,
                    "origin",
                    RefSpec::parse("HEAD:refs/heads/main")?,
                    false,
                    false,
                    Some(|prompt| {
                        prompts.push(prompt);
                        std::future::ready(None)
                    }),
                    vec![],
                ),
            )
            .await
            .map_err(anyhow::Error::from)
        })?
        .expect_err("declining credentials fails push");
    assert_eq!(
        prompts.len(),
        1,
        "native pushing reaches the async askpass broker"
    );
    assert!(
        error.to_string().contains("none were provided"),
        "declined prompts retain their classification: {error}"
    );
    Ok(())
}
