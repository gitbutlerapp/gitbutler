//! This module contains facilities to handle the persistence of secrets.
//!
//! These are stateless and global, while discouraging storing secrets
//! in memory beyond their use.
use crate::types::Sensitive;
use anyhow::Result;
use std::sync::Mutex;

/// Persist `secret` so that it can be retrieved by the given `handle`.
pub fn persist(handle: &str, secret: &Sensitive<String>) -> Result<()> {
    Ok(entry_for(handle)?.set_password(&secret.0)?)
}

/// Obtain the previously [stored](persist()) secret known as `handle`.
pub fn retrieve(handle: &str) -> Result<Option<Sensitive<String>>> {
    match entry_for(handle)?.get_password() {
        Ok(secret) => Ok(Some(Sensitive(secret))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// Delete the secret at `handle` permanently.
pub fn delete(handle: &str) -> Result<()> {
    Ok(entry_for(handle)?.delete_password()?)
}

/// Use this `identifier` as 'namespace' for identifying secrets.
/// Each namespace has its own set of secrets, useful for different application versions.
///
/// Note that the namespace will default to `gitbutler` if empty.
pub fn set_application_namespace(identifier: impl Into<String>) {
    *NAMESPACE.lock().unwrap() = identifier.into()
}

fn entry_for(handle: &str) -> Result<keyring::Entry> {
    let ns = NAMESPACE.lock().unwrap();
    Ok(keyring::Entry::new(
        if ns.is_empty() { "gitbutler" } else { &ns },
        handle,
    )?)
}

/// How to further specialize secrets to avoid name clashes in the globally shared keystore.
static NAMESPACE: Mutex<String> = Mutex::new(String::new());
