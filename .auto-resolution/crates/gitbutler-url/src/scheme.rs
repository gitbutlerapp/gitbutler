/// A scheme or protocol for use in a [`Url`][super::Url].
///
/// It defines how to talk to a given repository.
#[derive(Default, PartialEq, Eq, Debug, Hash, Ord, PartialOrd, Clone)]
pub enum Scheme {
    /// A local resource that is accessible on the current host.
    File,
    /// A git daemon, like `File` over TCP/IP.
    Git,
    /// Launch `git-upload-pack` through an `ssh` tunnel.
    #[default]
    Ssh,
    /// Use the HTTP protocol to talk to git servers.
    Http,
    /// Use the HTTPS protocol to talk to git servers.
    Https,
    /// Any other protocol or transport that isn't known at compile time.
    ///
    /// It's used to support plug-in transports.
    Ext(String),
}

impl<'a> From<&'a str> for Scheme {
    fn from(value: &'a str) -> Self {
        match value {
            "ssh" => Scheme::Ssh,
            "file" => Scheme::File,
            "git" => Scheme::Git,
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            unknown => Scheme::Ext(unknown.into()),
        }
    }
}

impl Scheme {
    /// Return ourselves parseable name.
    pub fn as_str(&self) -> &str {
        match self {
            Self::File => "file",
            Self::Git => "git",
            Self::Ssh => "ssh",
            Self::Http => "http",
            Self::Https => "https",
            Self::Ext(name) => name.as_str(),
        }
    }
}

impl std::fmt::Display for Scheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
