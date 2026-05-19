use anyhow::Result;
use sha2::{Digest, Sha256};

pub(crate) fn gravatar_url_from_email(email: &str) -> Result<url::Url> {
    let email_hash = Sha256::digest(email.trim().to_lowercase().as_bytes());
    let gravatar_url = format!(
        "https://www.gravatar.com/avatar/{:x}?s=100&r=g&d=retro",
        email_hash
    );
    url::Url::parse(gravatar_url.as_str()).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::gravatar_url_from_email;

    #[test]
    fn gravatar_uses_sha256_and_normalized_email() {
        assert_eq!(
            gravatar_url_from_email(" Author@example.com ")
                .expect("normalized Gravatar URLs should parse")
                .as_str(),
            "https://www.gravatar.com/avatar/b0eda69977c26118feff17875d53376006568bcbcde5ca0c916d01f05c281436?s=100&r=g&d=retro",
            "gravatar hashes should match the normalized SHA-256 email used by Gravatar"
        );
    }
}
