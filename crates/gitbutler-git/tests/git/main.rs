mod refspec;

/// Uses a shell script as `remote.origin.uploadpack`, so unix only.
#[cfg(unix)]
mod slow_fetch {
    use std::time::{Duration, Instant};

    use but_testsupport::{gix_testtools::tempfile::TempDir, invoke_bash_at_dir};
    use gitbutler_git::tokio::TokioExecutor;

    /// A fetch whose Git operation outlives any askpass accept deadline must run to
    /// completion; the askpass socket only matters if Git actually asks for credentials.
    /// Regression test for slow fetches dying after 120s as
    /// "failed to create askpass server: timed out".
    ///
    /// The paused clock auto-advances whenever the runtime is only waiting on the real
    /// Git child, so a reintroduced accept deadline of any length (the bug had 120s)
    /// fires immediately and fails this test without real waiting.
    #[tokio::test(start_paused = true)]
    async fn slow_fetch_without_prompt_completes() {
        let tmp = TempDir::new().unwrap();
        invoke_bash_at_dir(
            r#"
            set -eu
            git init -q source
            git -C source commit -q --allow-empty -m init
            git clone -q --bare source origin.git
            git clone -q origin.git client
            cat > slow-upload-pack.sh <<'EOF'
#!/bin/sh
sleep 3
exec git upload-pack "$@"
EOF
            chmod +x slow-upload-pack.sh
            git -C client config remote.origin.uploadpack "$PWD/slow-upload-pack.sh"
            "#,
            tmp.path(),
        );

        let start = Instant::now();
        // Answering `None` turns any unexpected credential prompt into a fetch
        // error, proving the no-prompt path completes on its own.
        gitbutler_git::fetch(
            tmp.path().join("client"),
            TokioExecutor,
            "origin",
            Some(|_prompt: String| async { None::<String> }),
        )
        .await
        .expect("a slow fetch that never prompts for credentials completes");
        assert!(
            start.elapsed() >= Duration::from_secs(3),
            "the delayed upload-pack should have been exercised"
        );
    }
}

#[cfg(test)]
mod askpass {
    use std::time::Duration;

    use gitbutler_git::{
        executor::{AskpassServer, GitExecutor, Socket},
        tokio::{TokioAskpassServer, TokioExecutor},
    };

    // cargo test --package gitbutler-git --lib test_askpass
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn git_askpass() {
        let secret = "super-secret-secret";
        let executor = TokioExecutor;
        #[expect(unsafe_code)]
        let sock_server: TokioAskpassServer = unsafe { executor.create_askpass_server() }
            .await
            .expect("create_askpass_server():");
        let sock_server_string = sock_server.to_string();
        let handle = tokio::spawn(async move {
            snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("gitbutler-git-askpass"))
                .env("GITBUTLER_ASKPASS_PIPE", sock_server_string)
                .env("GITBUTLER_ASKPASS_SECRET", secret)
                .arg("Please enter your password:")
                .assert()
                .success()
                .stdout_eq("super_secret_password\n");
        });

        let mut sock = tokio::time::timeout(Duration::from_secs(10), sock_server.accept())
            .await
            .expect("timed out waiting for askpass connection")
            .expect("accept():");

        let peer_secret = sock.read_line().await.expect("read_line() peer_secret:");

        assert_eq!(peer_secret, secret);

        let prompt = sock.read_line().await.expect("read_line() prompt:");
        assert_eq!(prompt.trim(), "Please enter your password:");

        sock.write_line("super_secret_password")
            .await
            .expect("write_line() password:");
        handle.await.expect("Askpass command failed");
    }
}
