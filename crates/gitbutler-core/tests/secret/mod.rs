use gitbutler_core::secret;
use gitbutler_core::types::Sensitive;
use serial_test::serial;

#[test]
#[serial]
fn retrieve_unknown_is_none() {
    setup();
    assert!(secret::retrieve("does not exist for sure")
        .expect("no error to ask for non-existing")
        .is_none());
}

#[test]
#[serial]
fn store_and_retrieve() -> anyhow::Result<()> {
    setup();
    secret::persist("new", &Sensitive("secret".into()))?;
    let secret = secret::retrieve("new")?.expect("it was just stored");
    assert_eq!(
        secret.0, "secret",
        "note that this works only if the engine supports actual persistence, \
               which should be the default outside of tests"
    );
    Ok(())
}

fn setup() {
    keyring::set_default_credential_builder(Box::<credentials::Builder>::default());
}

mod credentials {
    use keyring::credential::{CredentialApi, CredentialBuilderApi, CredentialPersistence};
    use keyring::Credential;
    use std::any::Any;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Store {
        store: BTreeMap<String, String>,
    }

    type SharedStore = Arc<Mutex<Store>>;

    struct Entry {
        handle: String,
        inner: SharedStore,
    }

    impl CredentialApi for Entry {
        fn set_password(&self, password: &str) -> keyring::Result<()> {
            self.inner
                .lock()
                .unwrap()
                .store
                .insert(self.handle.clone(), password.into());
            Ok(())
        }

        fn get_password(&self) -> keyring::Result<String> {
            match self.inner.lock().unwrap().store.get(&self.handle) {
                Some(secret) => Ok(secret.clone()),
                None => Err(keyring::Error::NoEntry),
            }
        }

        fn delete_password(&self) -> keyring::Result<()> {
            todo!()
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[derive(Default)]
    pub(super) struct Builder {
        store: SharedStore,
    }

    impl CredentialBuilderApi for Builder {
        fn build(
            &self,
            _target: Option<&str>,
            _service: &str,
            user: &str,
        ) -> keyring::Result<Box<Credential>> {
            let credential = Entry {
                handle: user.to_string(),
                inner: self.store.clone(),
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
}
