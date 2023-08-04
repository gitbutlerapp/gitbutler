use std::env;

use crate::keys;

pub type CredentialsCallback<'a> = Box<
    dyn FnMut(&str, Option<&str>, git2::CredentialType) -> Result<git2::Cred, git2::Error> + 'a,
>;

pub fn for_key(key: &keys::PrivateKey) -> Vec<CredentialsCallback<'_>> {
    let mut credentials = vec![];
    if let Ok(home_path) = env::var("HOME") {
        let home_path = std::path::Path::new(&home_path);

        let id_rsa_path = home_path.join(".ssh").join("id_rsa");
        if id_rsa_path.exists() {
            credentials.push(from_keypath(id_rsa_path.to_path_buf()));
        }

        let id_ed25519_path = home_path.join(".ssh").join("id_ed25519");
        if id_ed25519_path.exists() {
            credentials.push(from_keypath(id_ed25519_path.to_path_buf()));
        }

        let id_ecdsa_path = home_path.join(".ssh").join("id_ecdsa");
        if id_ecdsa_path.exists() {
            credentials.push(from_keypath(id_ecdsa_path.to_path_buf()));
        }
    }
    credentials.push(from_key(key));
    credentials
}

fn from_keypath<'a>(key_path: std::path::PathBuf) -> CredentialsCallback<'a> {
    Box::new(move |_url, _username_from_url, _allowed_types| {
        git2::Cred::ssh_key("git", None, &key_path, None)
    })
}

fn from_key(key: &keys::PrivateKey) -> CredentialsCallback<'_> {
    Box::new(|_url, _username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_memory("git", None, &key.to_string(), None)
    })
}
