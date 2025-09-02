mod convert;
mod parse;
mod scheme;

use std::str::FromStr;

use bstr::ByteSlice;
pub use convert::ConvertError;
// pub use parse::Error as ParseError;
pub use scheme::Scheme;

#[derive(Default, Clone, Hash, PartialEq, Eq, Debug, thiserror::Error)]
pub struct Url {
    /// The URL scheme.
    pub scheme: Scheme,
    /// The user to impersonate on the remote.
    user: Option<String>,
    /// The password associated with a user.
    password: Option<String>,
    /// The host to which to connect. Localhost is implied if `None`.
    pub host: Option<String>,
    /// When serializing, use the alternative forms as it was parsed as such.
    serialize_alternative_form: bool,
    /// The port to use when connecting to a host. If `None`, standard ports depending on `scheme` will be used.
    pub port: Option<u16>,
    /// The path portion of the URL, usually the location of the git repository.
    pub path: bstr::BString,
}

impl Url {
    pub fn is_github(&self) -> bool {
        self.host
            .as_ref()
            .is_some_and(|host| host.contains("github.com"))
    }
}

impl std::fmt::Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !(self.serialize_alternative_form
            && (self.scheme == Scheme::File || self.scheme == Scheme::Ssh))
        {
            f.write_str(self.scheme.as_str())?;
            f.write_str("://")?;
        }
        match (&self.user, &self.host) {
            (Some(user), Some(host)) => {
                f.write_str(user)?;
                if let Some(password) = &self.password {
                    f.write_str(":")?;
                    f.write_str(password)?;
                }
                f.write_str("@")?;
                f.write_str(host)?;
            }
            (None, Some(host)) => {
                f.write_str(host)?;
            }
            (None, None) => {}
            (Some(_user), None) => {
                unreachable!("BUG: should not be possible to have a user but no host")
            }
        };
        if let Some(port) = &self.port {
            f.write_str(&format!(":{port}"))?;
        }
        if self.serialize_alternative_form && self.scheme == Scheme::Ssh {
            f.write_str(":")?;
        }
        f.write_str(self.path.to_str().unwrap())?;
        Ok(())
    }
}

impl Url {
    pub fn as_ssh(&self) -> Result<Self, ConvertError> {
        convert::to_ssh_url(self)
    }

    pub fn as_https(&self) -> Result<Self, ConvertError> {
        convert::to_https_url(self)
    }
}

impl FromStr for Url {
    type Err = parse::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse::parse(s.as_bytes().into())
    }
}
