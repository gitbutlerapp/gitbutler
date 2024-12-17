use keyring::Result;
use std::any::Any;

/// Assure we have a mock secrets store so tests don't start writing secrets into the user's actual store,
/// as this will affect their GitButler instance.
pub fn setup_blackhole_store() {
    keyring::set_default_credential_builder(Box::new(BlackholeBuilder))
}

/// A builder that is completely mocked, able to read no secret, but allowing to write any without error.
struct BlackholeBuilder;

struct BlackholeCredential;

impl keyring::credential::CredentialApi for BlackholeCredential {
    fn set_password(&self, _password: &str) -> keyring::Result<()> {
        Ok(())
    }

    fn set_secret(&self, _password: &[u8]) -> Result<()> {
        unreachable!("unused")
    }

    fn get_password(&self) -> keyring::Result<String> {
        Err(keyring::Error::NoEntry)
    }

    fn get_secret(&self) -> Result<Vec<u8>> {
        unreachable!("unused")
    }

    fn delete_credential(&self) -> keyring::Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl keyring::credential::CredentialBuilderApi for BlackholeBuilder {
    fn build(
        &self,
        _target: Option<&str>,
        _service: &str,
        _user: &str,
    ) -> keyring::Result<Box<keyring::Credential>> {
        Ok(Box::new(BlackholeCredential))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
