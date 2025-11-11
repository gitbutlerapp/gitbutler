mod refspec;

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

        let mut sock = sock_server
            .accept(Some(Duration::from_secs(10)))
            .await
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
