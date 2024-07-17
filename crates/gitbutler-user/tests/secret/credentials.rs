use keyring::credential::{CredentialApi, CredentialBuilderApi, CredentialPersistence};
use keyring::Credential;
use std::any::Any;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(super) struct Store(BTreeMap<String, String>);

pub(super) type SharedStore = Arc<Mutex<Store>>;

struct Entry {
    handle: String,
    store: SharedStore,
}

impl CredentialApi for Entry {
    fn set_password(&self, password: &str) -> keyring::Result<()> {
        self.store
            .lock()
            .unwrap()
            .0
            .insert(self.handle.clone(), password.into());
        Ok(())
    }

    fn get_password(&self) -> keyring::Result<String> {
        match self.store.lock().unwrap().0.get(&self.handle) {
            Some(secret) => Ok(secret.clone()),
            None => Err(keyring::Error::NoEntry),
        }
    }

    fn delete_password(&self) -> keyring::Result<()> {
        self.store.lock().unwrap().0.remove(&self.handle);
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(super) struct Builder {
    pub(super) store: SharedStore,
}

impl CredentialBuilderApi for Builder {
    fn build(
        &self,
        _target: Option<&str>,
        service: &str,
        _user: &str,
    ) -> keyring::Result<Box<Credential>> {
        let credential = Entry {
            handle: service.to_string(),
            store: self.store.clone(),
        };
        Ok(Box::new(credential))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// We keep information in memory
    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::ProcessOnly
    }
}

static CURRENT_STORE: Mutex<Option<SharedStore>> = Mutex::new(None);

/// Initialize the credentials store to be isolated and usable for testing.
///
/// Note that this is a resource shared in the process, and deterministic tests must
/// use the `[serial]` annotation.
pub fn setup() {
    let store = SharedStore::default();
    *CURRENT_STORE.lock().unwrap() = Some(store.clone());

    keyring::set_default_credential_builder(Box::new(Builder { store }));
}

/// Return the amount of stored secrets
pub fn count() -> usize {
    CURRENT_STORE
        .lock()
        .unwrap()
        .as_ref()
        .expect("BUG: call setup")
        .lock()
        .unwrap()
        .0
        .len()
}
