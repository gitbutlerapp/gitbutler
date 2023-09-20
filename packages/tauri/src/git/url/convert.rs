use bstr::ByteSlice;

use super::{Scheme, Url};

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ConvertError {
    #[error("Could not convert {from} to {to}")]
    UnsupportedPair { from: Scheme, to: Scheme },
}

pub fn to_ssh_url(url: &Url) -> Result<Url, ConvertError> {
    match url.scheme {
        Scheme::Ssh => Ok(url.clone()),
        Scheme::Http | Scheme::Https => Ok(Url {
            scheme: Scheme::Ssh,
            user: Some("git".to_string()),
            serialize_alternative_form: true,
            path: if url.path.starts_with(&[b'/']) {
                url.path.trim_start_with(|c| c == '/').into()
            } else {
                url.path.clone()
            },
            ..url.clone()
        }),
        _ => Err(ConvertError::UnsupportedPair {
            from: url.scheme.clone(),
            to: Scheme::Ssh,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_ssh_url_test() {
        vec![
            (
                "git@github.com:gitbutlerapp/gitbutler-client.git",
                "git@github.com:gitbutlerapp/gitbutler-client.git",
            ),
            (
                "https://github.com/gitbutlerapp/gitbutler-client.git",
                "git@github.com:gitbutlerapp/gitbutler-client.git",
            ),
            (
                "https://github.com/gitbutlerapp/gitbutler-client.git",
                "git@github.com:gitbutlerapp/gitbutler-client.git",
            ),
            (
                "ssh://git@github.com/gitbutlerapp/gitbutler-client.git",
                "ssh://git@github.com/gitbutlerapp/gitbutler-client.git",
            ),
        ]
        .into_iter()
        .enumerate()
        .for_each(|(i, (input, expected))| {
            let url = input.parse().unwrap();
            let ssh_url = to_ssh_url(&url).unwrap();
            assert_eq!(ssh_url.to_string(), expected, "test case {}", i);
        });
    }
}
