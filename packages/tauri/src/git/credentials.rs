use std::env;

use resolve_path::PathResolveExt;

use crate::keys;

pub type CredentialsCallback<'a> = Box<
    dyn FnMut(&str, Option<&str>, git2::CredentialType) -> Result<git2::Cred, git2::Error> + 'a,
>;

pub fn for_key(key: &keys::Key) -> Vec<CredentialsCallback<'_>> {
    let mut credentials = vec![];
    match key {
        keys::Key::Local {
            private_key_path,
            passphrase,
        } => {
            credentials.push(from_keypath(
                private_key_path.to_path_buf(),
                passphrase.as_deref(),
            ));
        }
        keys::Key::Generated(private_key) => {
            if let Ok(home_path) = env::var("HOME") {
                let home_path = std::path::Path::new(&home_path);

                let id_rsa_path = home_path.join(".ssh").join("id_rsa");
                if id_rsa_path.exists() {
                    credentials.push(from_keypath(id_rsa_path.to_path_buf(), None));
                }

                let id_ed25519_path = home_path.join(".ssh").join("id_ed25519");
                if id_ed25519_path.exists() {
                    credentials.push(from_keypath(id_ed25519_path.to_path_buf(), None));
                }

                let id_ecdsa_path = home_path.join(".ssh").join("id_ecdsa");
                if id_ecdsa_path.exists() {
                    credentials.push(from_keypath(id_ecdsa_path.to_path_buf(), None));
                }
            }
            credentials.push(from_key(private_key));
        }
    }
    credentials
}

fn from_keypath(key_path: std::path::PathBuf, passphrase: Option<&str>) -> CredentialsCallback {
    Box::new(move |url, _username_from_url, _allowed_types| {
        let key_path = key_path.resolve();
        tracing::debug!("authenticating with {} using {}", url, key_path.display());
        git2::Cred::ssh_key("git", None, &key_path, passphrase)
    })
}

fn from_key(key: &keys::PrivateKey) -> CredentialsCallback {
    Box::new(|url, _username_from_url, _allowed_types| {
        tracing::debug!("authenticating with {} using gitbutler's key", url);
        git2::Cred::ssh_key_from_memory("git", None, &key.to_string(), None)
    })
}
