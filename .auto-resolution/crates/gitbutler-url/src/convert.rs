use bstr::ByteSlice;

use super::{Scheme, Url};

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum ConvertError {
    #[error("Could not convert {from} to {to}")]
    UnsupportedPair { from: Scheme, to: Scheme },
}

pub(crate) fn to_https_url(url: &Url) -> Result<Url, ConvertError> {
    match url.scheme {
        Scheme::Https => Ok(url.clone()),
        Scheme::Http => Ok(Url {
            scheme: Scheme::Https,
            ..url.clone()
        }),
        Scheme::Ssh => Ok(Url {
            scheme: Scheme::Https,
            user: None,
            serialize_alternative_form: true,
            path: if url.path.starts_with(b"/") {
                url.path.clone()
            } else {
                format!("/{}", url.path.to_str().unwrap()).into()
            },
            ..url.clone()
        }),
        _ => Err(ConvertError::UnsupportedPair {
            from: url.scheme.clone(),
            to: Scheme::Ssh,
        }),
    }
}

pub(crate) fn to_ssh_url(url: &Url) -> Result<Url, ConvertError> {
    match url.scheme {
        Scheme::Ssh => Ok(url.clone()),
        Scheme::Http | Scheme::Https => Ok(Url {
            scheme: Scheme::Ssh,
            user: Some("git".to_string()),
            serialize_alternative_form: true,
            path: if url.path.starts_with(b"/") {
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
    fn to_https_url_test() {
        for (input, expected) in [
            (
                "https://github.com/gitbutlerapp/gitbutler.git",
                "https://github.com/gitbutlerapp/gitbutler.git",
            ),
            (
                "http://github.com/gitbutlerapp/gitbutler.git",
                "https://github.com/gitbutlerapp/gitbutler.git",
            ),
            (
                "git@github.com:gitbutlerapp/gitbutler.git",
                "https://github.com/gitbutlerapp/gitbutler.git",
            ),
            (
                "ssh://git@github.com/gitbutlerapp/gitbutler.git",
                "https://github.com/gitbutlerapp/gitbutler.git",
            ),
            (
                "git@bitbucket.org:gitbutler-nikita/test.git",
                "https://bitbucket.org/gitbutler-nikita/test.git",
            ),
            (
                "https://bitbucket.org/gitbutler-nikita/test.git",
                "https://bitbucket.org/gitbutler-nikita/test.git",
            ),
        ] {
            let url = input.parse().unwrap();
            let https_url = to_https_url(&url).unwrap();
            assert_eq!(https_url.to_string(), expected, "test case {url}");
        }
    }

    #[test]
    fn to_ssh_url_test() {
        for (input, expected) in [
            (
                "git@github.com:gitbutlerapp/gitbutler.git",
                "git@github.com:gitbutlerapp/gitbutler.git",
            ),
            (
                "https://github.com/gitbutlerapp/gitbutler.git",
                "git@github.com:gitbutlerapp/gitbutler.git",
            ),
            (
                "https://github.com/gitbutlerapp/gitbutler.git",
                "git@github.com:gitbutlerapp/gitbutler.git",
            ),
            (
                "ssh://git@github.com/gitbutlerapp/gitbutler.git",
                "ssh://git@github.com/gitbutlerapp/gitbutler.git",
            ),
            (
                "https://bitbucket.org/gitbutler-nikita/test.git",
                "git@bitbucket.org:gitbutler-nikita/test.git",
            ),
            (
                "git@bitbucket.org:gitbutler-nikita/test.git",
                "git@bitbucket.org:gitbutler-nikita/test.git",
            ),
        ] {
            let url = input.parse().unwrap();
            let ssh_url = to_ssh_url(&url).unwrap();
            assert_eq!(ssh_url.to_string(), expected, "test case {url}");
        }
    }
}
