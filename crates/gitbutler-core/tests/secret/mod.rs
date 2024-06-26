//! Note that these tests *must* be run in their own process, as they rely on having a deterministic
//! credential store. Due to its global nature, tests cannot run in parallel
//! (or mixed with parallel tests that set their own credential store)
use gitbutler_core::secret;
use gitbutler_core::types::Sensitive;
use serial_test::serial;

#[test]
#[serial]
fn retrieve_unknown_is_none() {
    credentials::setup();
    assert!(secret::retrieve("does not exist for sure")
        .expect("no error to ask for non-existing")
        .is_none());
}

#[test]
#[serial]
fn store_and_retrieve() -> anyhow::Result<()> {
    credentials::setup();
    secret::persist("new", &Sensitive("secret".into()))?;
    let secret = secret::retrieve("new")?.expect("it was just stored");
    assert_eq!(
        secret.0, "secret",
        "note that this works only if the engine supports actual persistence, \
               which should be the default outside of tests"
    );
    Ok(())
}

#[test]
#[serial]
fn store_empty_equals_deletion() -> anyhow::Result<()> {
    credentials::setup();
    secret::persist("new", &Sensitive("secret".into()))?;
    assert_eq!(credentials::count(), 1);

    secret::persist("new", &Sensitive("".into()))?;
    assert_eq!(
        secret::retrieve("new")?.map(|s| s.0),
        None,
        "empty passwords are automatically deleted"
    );
    assert_eq!(credentials::count(), 0);
    Ok(())
}

pub(crate) mod credentials;
mod users;
