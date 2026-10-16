//! Run explicitly inside an isolated, unlocked Secret Service session.
//! The compatibility test also requires `secret-tool`.
#![cfg(target_os = "linux")]

use but_secret::{
    Sensitive,
    secret::{self, Namespace},
};

#[test]
#[ignore = "requires an unlocked Secret Service, Linux keyutils, and secret-tool"]
fn reads_existing_secret_service_item() -> anyhow::Result<()> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    let handle = format!("zbus-existing-test-{}", std::process::id());
    let service = format!("gitbutler-{handle}");
    // These are the same lookup attributes written by the libdbus backend.
    // Seed through another client so retrieval cannot hit the keyutils cache.
    let mut child = Command::new("secret-tool")
        .args([
            "store",
            "--label=GitButler migration test",
            "service",
            &service,
            "username",
            "GitButler",
            "target",
            "default",
        ])
        .stdin(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"existing credential")?;
    assert!(child.wait()?.success(), "legacy-shaped item must be stored");
    let retrieved = secret::retrieve(&handle, Namespace::Global);
    secret::delete(&handle, Namespace::Global)?;
    assert_eq!(
        retrieved?.as_ref().map(|s| s.0.as_str()),
        Some("existing credential"),
        "existing Secret Service items must remain readable"
    );
    Ok(())
}

#[test]
#[ignore = "requires an unlocked Secret Service and Linux keyutils"]
fn persistent_keyring_inside_and_outside_tokio() -> anyhow::Result<()> {
    fn round_trip(context: &str) -> anyhow::Result<()> {
        let handle = format!("zbus-test-{}-{context}", std::process::id());
        for namespace in [Namespace::Global, Namespace::BuildKind] {
            let password = Sensitive("keyring migration test".to_owned());
            secret::persist(&handle, &password, namespace)?;
            let retrieved = secret::retrieve(&handle, namespace)?;
            assert_eq!(
                retrieved.as_ref().map(|s| s.0.as_str()),
                Some(password.0.as_str()),
                "stored credentials must remain readable"
            );
            secret::delete(&handle, namespace)?;
            assert!(
                secret::retrieve(&handle, namespace)?.is_none(),
                "deleted credentials must be absent"
            );
            secret::delete(&handle, namespace)?;
        }
        Ok(())
    }

    round_trip("sync")?;
    tokio::runtime::Builder::new_current_thread()
        .build()?
        .block_on(async { round_trip("current-thread") })?;
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .build()?
        .block_on(async { round_trip("multi-thread") })?;
    Ok(())
}
