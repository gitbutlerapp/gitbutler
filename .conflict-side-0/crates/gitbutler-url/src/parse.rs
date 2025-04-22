use std::borrow::Cow;

use bstr::{BStr, BString, ByteSlice};

use super::{Scheme, Url};

/// The Error returned by [`parse()`]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not decode URL as UTF8")]
    Utf8(#[from] std::str::Utf8Error),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error("URLs need to specify the path to the repository")]
    MissingResourceLocation,
    #[error("file URLs require an absolute or relative path to the repository")]
    MissingRepositoryPath,
    #[error("\"{url}\" is not a valid local path")]
    NotALocalFile { url: BString },
    #[error("Relative URLs are not permitted: {url:?}")]
    RelativeUrl { url: String },
}

fn str_to_protocol(s: &str) -> Scheme {
    Scheme::from(s)
}

fn guess_protocol(url: &[u8]) -> Option<&str> {
    match url.find_byte(b':') {
        Some(colon_pos) => {
            if url[..colon_pos].find_byteset(b"@.").is_some() {
                "ssh"
            } else {
                url.get(colon_pos + 1..).and_then(|from_colon| {
                    (from_colon.contains(&b'/') || from_colon.contains(&b'\\')).then_some("file")
                })?
            }
        }
        None => "file",
    }
    .into()
}

/// Extract the path part from an SCP-like URL `[user@]host.xz:path/to/repo.git/`
fn extract_scp_path(url: &str) -> Option<&str> {
    url.splitn(2, ':').last()
}

fn sanitize_for_protocol<'a>(protocol: &str, url: &'a str) -> Cow<'a, str> {
    match protocol {
        "ssh" => url.replacen(':', "/", 1).into(),
        _ => url.into(),
    }
}

fn has_no_explicit_protocol(url: &[u8]) -> bool {
    url.find(b"://").is_none()
}

fn to_owned_url(url: &url::Url) -> Url {
    let password = url.password();
    Url {
        serialize_alternative_form: false,
        scheme: str_to_protocol(url.scheme()),
        password: password.map(ToOwned::to_owned),
        user: if url.username().is_empty() && password.is_none() {
            None
        } else {
            Some(url.username().into())
        },
        host: url.host_str().map(Into::into),
        port: url.port(),
        path: url.path().into(),
    }
}

/// Parse the given `bytes` as git url.
///
/// # Note
///
/// We cannot and should never have to deal with UTF-16 encoded windows strings, so bytes input is acceptable.
/// For file-paths, we don't expect UTF8 encoding either.
pub fn parse(input: &BStr) -> Result<Url, Error> {
    let guessed_protocol =
        guess_protocol(input).ok_or_else(|| Error::NotALocalFile { url: input.into() })?;
    let path_without_file_protocol = input.strip_prefix(b"file://");
    if path_without_file_protocol.is_some()
        || (has_no_explicit_protocol(input) && guessed_protocol == "file")
    {
        let path =
            path_without_file_protocol.map_or_else(|| input.into(), |stripped_path| stripped_path);
        if path.is_empty() {
            return Err(Error::MissingRepositoryPath);
        }
        let input_starts_with_file_protocol = input.starts_with(b"file://");
        if input_starts_with_file_protocol {
            let wanted = b"/";
            if !wanted.iter().any(|w| path.contains(w)) {
                return Err(Error::MissingRepositoryPath);
            }
        }
        return Ok(Url {
            scheme: Scheme::File,
            path: path.into(),
            serialize_alternative_form: !input_starts_with_file_protocol,
            ..Default::default()
        });
    }

    let url_str = std::str::from_utf8(input)?;
    let (mut url, mut scp_path) = match url::Url::parse(url_str) {
        Ok(url) => (url, None),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            // happens with bare paths as well as scp like paths. The latter contain a ':' past the host portion,
            // which we are trying to detect.
            (
                url::Url::parse(&format!(
                    "{}://{}",
                    guessed_protocol,
                    sanitize_for_protocol(guessed_protocol, url_str)
                ))?,
                extract_scp_path(url_str),
            )
        }
        Err(err) => return Err(err.into()),
    };
    // SCP like URLs without user parse as 'something' with the scheme being the 'host'. Hosts always have dots.
    if url.scheme().find('.').is_some() {
        // try again with prefixed protocol
        url = url::Url::parse(&format!("ssh://{}", sanitize_for_protocol("ssh", url_str)))?;
        scp_path = extract_scp_path(url_str);
    }
    if url.path().is_empty() && ["ssh", "git"].contains(&url.scheme()) {
        return Err(Error::MissingResourceLocation);
    }
    if url.cannot_be_a_base() {
        return Err(Error::RelativeUrl { url: url.into() });
    }

    let mut url = to_owned_url(&url);
    if let Some(path) = scp_path {
        url.path = path.into();
        url.serialize_alternative_form = true;
    }
    Ok(url)
}
