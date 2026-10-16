#![cfg(target_os = "linux")]

use std::{fs, os::unix::fs::PermissionsExt};

use but_secret::{
    Sensitive,
    secret::{self, Namespace, file_credentials},
};

// Tests run serially because each installs a process-global keyring backend.

fn credentials_directory() -> std::io::Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
}

#[test]
#[serial_test::serial]
fn stores_and_gets_password() -> anyhow::Result<()> {
    let directory = credentials_directory()?;
    file_credentials::setup(directory.path())?;

    secret::persist("account", &Sensitive("password".into()), Namespace::Global)?;

    assert_eq!(
        secret::retrieve("account", Namespace::Global)?.map(|value| value.0),
        Some("password".into()),
        "stored password must be readable"
    );
    Ok(())
}

#[test]
#[serial_test::serial]
fn stored_credentials_file_has_mode_600() -> anyhow::Result<()> {
    let directory = credentials_directory()?;
    file_credentials::setup(directory.path())?;

    secret::persist("account", &Sensitive("password".into()), Namespace::Global)?;

    let entries = fs::read_dir(directory.path())?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(entries.len(), 1, "persist must create one credentials file");
    let metadata = entries[0].metadata()?;
    assert!(
        metadata.is_file(),
        "stored credential must be a regular file"
    );
    assert_eq!(
        metadata.permissions().mode() & 0o7777,
        0o600,
        "credentials file must be readable and writable only by its owner"
    );
    Ok(())
}

#[test]
#[serial_test::serial]
fn created_credentials_directory_has_mode_700() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = directory.path().join("missing/parents/secrets");
    assert!(!store.exists(), "nested store must not exist before setup");

    file_credentials::setup(&store)?;

    // A restrictive umask such as 0077 can hide missing permission enforcement.
    // Relying on the usual 0022 is good enough here; leave process-global umask alone.
    assert!(store.is_dir(), "setup must create the final subdirectory");
    assert_eq!(
        fs::metadata(&store)?.permissions().mode() & 0o7777,
        0o700,
        "final credentials subdirectory must be accessible only by its owner"
    );
    Ok(())
}

#[test]
#[serial_test::serial]
fn existing_credentials_directory_rejects_modes_other_than_700() -> anyhow::Result<()> {
    let directory = credentials_directory()?;
    let store = directory.path();
    file_credentials::setup(store)?;

    // Cover missing owner permissions, each group/other bit, common shared modes,
    // and special bits: the contract requires exactly 0700, not just no sharing.
    for mode in [
        0o000, 0o100, 0o200, 0o300, 0o400, 0o500, 0o600, 0o701, 0o702, 0o704, 0o710, 0o720, 0o740,
        0o750, 0o755, 0o770, 0o777,
    ] {
        fs::set_permissions(store, fs::Permissions::from_mode(mode))?;
        assert_eq!(
            fs::metadata(store)?.permissions().mode() & 0o7777,
            mode,
            "test directory must have requested mode {mode:04o}"
        );
        let result = file_credentials::setup(store);
        // Restore access before asserting, including when setup unexpectedly succeeds.
        fs::set_permissions(store, fs::Permissions::from_mode(0o700))?;
        assert!(
            result.is_err(),
            "setup must reject directory mode {mode:04o}"
        );
    }
    Ok(())
}

#[test]
#[serial_test::serial]
fn overwrites_existing_password() -> anyhow::Result<()> {
    let directory = credentials_directory()?;
    file_credentials::setup(directory.path())?;

    secret::persist(
        "account",
        &Sensitive("old password".into()),
        Namespace::Global,
    )?;
    secret::persist("account", &Sensitive("new".into()), Namespace::Global)?;

    assert_eq!(
        secret::retrieve("account", Namespace::Global)?.map(|value| value.0),
        Some("new".into()),
        "replacement must contain only the new password, without trailing old bytes"
    );
    Ok(())
}

#[test]
#[serial_test::serial]
fn missing_password_returns_none() -> anyhow::Result<()> {
    let directory = credentials_directory()?;
    file_credentials::setup(directory.path())?;

    assert!(
        secret::retrieve("missing", Namespace::Global)?.is_none(),
        "a credential that was never stored must be absent"
    );
    Ok(())
}

#[test]
#[serial_test::serial]
fn deleting_missing_password_succeeds() -> anyhow::Result<()> {
    let directory = credentials_directory()?;
    file_credentials::setup(directory.path())?;

    assert!(
        secret::delete("missing", Namespace::Global).is_ok(),
        "deleting a credential that was never stored must succeed"
    );
    assert!(
        secret::retrieve("missing", Namespace::Global)?.is_none(),
        "deleting a missing credential must leave it absent"
    );
    Ok(())
}

#[test]
#[serial_test::serial]
fn deleting_password_repeatedly_succeeds() -> anyhow::Result<()> {
    let directory = credentials_directory()?;
    file_credentials::setup(directory.path())?;
    secret::persist("account", &Sensitive("password".into()), Namespace::Global)?;

    assert!(
        secret::delete("account", Namespace::Global).is_ok(),
        "deleting a stored credential must succeed"
    );
    assert!(
        secret::retrieve("account", Namespace::Global)?.is_none(),
        "deleting a stored credential must remove it"
    );
    assert!(
        secret::delete("account", Namespace::Global).is_ok(),
        "deleting an already-deleted credential must succeed"
    );
    Ok(())
}

#[test]
#[serial_test::serial]
fn setup_rejects_file_at_intermediate_or_final_path() -> anyhow::Result<()> {
    for store_path in ["blocked/secrets", "blocked"] {
        let directory = credentials_directory()?;
        let blocker = directory.path().join("blocked");
        fs::write(&blocker, "existing file")?;
        // Match valid directory permissions so rejection must be based on file type.
        fs::set_permissions(&blocker, fs::Permissions::from_mode(0o700))?;
        let store = directory.path().join(store_path);

        let result = file_credentials::setup(&store);

        assert!(
            result.is_err(),
            "setup must reject a file blocking credentials path {store_path}"
        );
        assert!(
            !store.is_dir(),
            "setup must not create the credentials directory"
        );
        assert_eq!(
            fs::read_to_string(&blocker)?,
            "existing file",
            "setup must leave the blocking file intact"
        );
    }
    Ok(())
}

#[test]
#[serial_test::serial]
fn setup_creates_missing_parent_directories() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = directory.path().join("missing/parents/secrets");
    assert!(!store.exists(), "nested store must not exist before setup");

    file_credentials::setup(&store)?;

    assert!(
        store.is_dir(),
        "setup must create the nested store directory"
    );
    secret::persist("account", &Sensitive("password".into()), Namespace::Global)?;
    assert_eq!(
        secret::retrieve("account", Namespace::Global)?.map(|value| value.0),
        Some("password".into()),
        "newly created nested store must support password storage"
    );
    Ok(())
}
