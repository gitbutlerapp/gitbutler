//! This module contains facilities to handle the persistence of secrets.
//!
//! These are stateless and global, while discouraging storing secrets
//! in memory beyond their use.

use crate::types::Sensitive;
use anyhow::Result;
use std::sync::Mutex;

/// Determines how a secret's name should be modified to produce a namespace.
///
/// Namespaces can be used to partition secrets, depending on some criteria.
#[derive(Debug, Clone, Copy)]
pub enum Namespace {
    /// Each application build, like `dev`, `production` and `nightly` have their
    /// own set of secrets. They do not overlap, which reflects how data-files
    /// are stored as well.
    BuildKind,
    /// All secrets are in a single namespace. There is no partitioning.
    /// This can be useful for secrets to be shared across all build kinds.
    Global,
}

/// Persist `secret` in `namespace` so that it can be retrieved by the given `handle`.
pub fn persist(handle: &str, secret: &Sensitive<String>, namespace: Namespace) -> Result<()> {
    let entry = entry_for(handle, namespace)?;
    if secret.0.is_empty() {
        entry.delete_password()?;
    } else {
        entry.set_password(&secret.0)?;
    }
    Ok(())
}

/// Obtain the previously [stored](persist()) secret known as `handle` from `namespace`.
pub fn retrieve(handle: &str, namespace: Namespace) -> Result<Option<Sensitive<String>>> {
    match entry_for(handle, namespace)?.get_password() {
        Ok(secret) => Ok(Some(Sensitive(secret))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// Delete the secret at `handle` permanently from `namespace`.
pub fn delete(handle: &str, namespace: Namespace) -> Result<()> {
    Ok(entry_for(handle, namespace)?.delete_password()?)
}

/// Use this `identifier` as 'namespace' for identifying secrets.
/// Each namespace has its own set of secrets, useful for different application versions.
///
/// Note that the namespace will be `development` if `identifier` is empty (or wasn't set).
pub fn set_application_namespace(identifier: impl Into<String>) {
    *NAMESPACE.lock().unwrap() = identifier.into()
}

fn entry_for(handle: &str, namespace: Namespace) -> Result<keyring::Entry> {
    let ns = match namespace {
        Namespace::BuildKind => NAMESPACE.lock().unwrap().clone(),
        Namespace::Global => "gitbutler".into(),
    };
    Ok(keyring::Entry::new(
        &format!(
            "{prefix}-{handle}",
            prefix = if ns.is_empty() { "development" } else { &ns }
        ),
        "GitButler",
    )?)
}

/// How to further specialize secrets to avoid name clashes in the globally shared keystore.
static NAMESPACE: Mutex<String> = Mutex::new(String::new());

/// A keystore that uses git-credentials under to hood. It's useful on Systems that nag the user
/// with popups if the underlying binary changes, and is available if `git` can be found and executed.
pub mod git_credentials {
    use anyhow::Result;
    use keyring::credential::{CredentialApi, CredentialBuilderApi, CredentialPersistence};
    use keyring::Credential;
    use std::any::Any;
    use std::sync::Arc;
    use tracing::instrument;

    pub(super) struct Store(gix::config::File<'static>);

    impl Store {
        /// Create an instance by resolving the global environment just well enough.
        ///
        /// # Limitation
        ///
        /// This does not fully resolve includes, so it's not truly production ready but should be
        /// fine for developer setups.
        fn from_globals() -> Result<Self> {
            Ok(Store(gix::config::File::from_globals()?))
        }

        /// Provide credentials preconfigured for the given secrets `handle`.
        /// They can then be queried.
        fn credentials(
            &self,
            handle: &str,
            password: Option<&str>,
        ) -> Result<(
            gix::credentials::helper::Cascade,
            gix::credentials::helper::Action,
            gix::prompt::Options<'static>,
        )> {
            let url = gix::Url::from_parts(
                gix::url::Scheme::Https,
                Some("gitbutler-secrets".into()),
                password.map(ToOwned::to_owned),
                Some("gitbutler.com".into()),
                None,
                format!("/{handle}").into(),
                false,
            )?;
            gix::config::credential_helpers(
                url,
                &self.0,
                true,
                &mut gix::config::section::is_trusted,
                gix::open::permissions::Environment::isolated(),
                true, /* use http path by default */
            )
            .map(|mut t| {
                let ctx = t.1.context_mut().expect("get always has context");
                // Assure the context has fields for all parts in the URL, even
                // if later we choose to use store or erase actions.
                // Usually these are naturally populated,
                // but not if we do everything by hand.
                // This is not a shortcoming in `gitoxide` - it simply doesn't touch
                // the output of previous invocations to not unintentionally affect them.
                ctx.destructure_url_in_place(true /* use http path */)
                    .expect("input URL is valid");
                t.2.mode = gix::prompt::Mode::Disable;
                t
            })
            .map_err(Into::into)
        }
    }

    pub(super) type SharedStore = Arc<Store>;

    struct Entry {
        handle: String,
        store: SharedStore,
    }

    impl CredentialApi for Entry {
        #[instrument(skip(self, password), err(Debug))]
        fn set_password(&self, password: &str) -> keyring::Result<()> {
            // credential helper on macos can't overwrite existing values apparently, workaround that.
            #[cfg(target_os = "macos")]
            self.delete_password().ok();
            let (mut cascade, action, prompt) = self
                .store
                .credentials(&self.handle, Some(password))
                .map_err(|err| keyring::Error::PlatformFailure(err.into()))?;
            let ctx = action.context().expect("available for get").to_owned();
            let action = gix::credentials::helper::NextAction::from(ctx).store();
            cascade
                .invoke(action, prompt)
                .map_err(|err| keyring::Error::PlatformFailure(err.into()))?;
            Ok(())
        }

        #[instrument(skip(self), err(Debug))]
        fn get_password(&self) -> keyring::Result<String> {
            let (mut cascade, get_action, prompt) = self
                .store
                .credentials(&self.handle, None)
                .map_err(|err| keyring::Error::PlatformFailure(err.into()))?;
            match cascade.invoke(get_action, prompt) {
                Ok(Some(out)) => Ok(out.identity.password),
                Ok(None) => Err(keyring::Error::NoEntry),
                Err(err) => {
                    tracing::debug!(err = ?err, "credential-helper invoke failed - usually this means it wanted to prompt which is disabled");
                    Err(keyring::Error::NoEntry)
                }
            }
        }

        #[instrument(skip(self), err(Debug))]
        fn delete_password(&self) -> keyring::Result<()> {
            let (mut cascade, action, prompt) = self
                .store
                .credentials(&self.handle, None)
                .map_err(|err| keyring::Error::PlatformFailure(err.into()))?;
            let ctx = action.context().expect("available for get").to_owned();
            let action = gix::credentials::helper::NextAction::from(ctx).erase();
            cascade
                .invoke(action, prompt)
                .map_err(|err| keyring::Error::PlatformFailure(err.into()))?;
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
            CredentialPersistence::UntilReboot
        }
    }

    /// Initialize the credentials store so that secrets are using `git credential`.
    #[instrument(err(Debug))]
    pub fn setup() -> Result<()> {
        let store = Arc::new(Store::from_globals()?);
        keyring::set_default_credential_builder(Box::new(Builder { store }));
        Ok(())
    }
}
