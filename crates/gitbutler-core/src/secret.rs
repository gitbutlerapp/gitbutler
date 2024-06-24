//! This module contains facilities to handle the persistence of secrets.
//!
//! These are stateless and global, while discouraging storing secrets
//! in memory beyond their use.
use crate::types::Sensitive;
use anyhow::Result;

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

fn entry_for(handle: &str) -> Result<keyring::Entry> {
    Ok(keyring::Entry::new("gitbutler", handle)?)
}
