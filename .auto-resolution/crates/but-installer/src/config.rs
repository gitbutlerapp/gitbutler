//! Configuration and platform detection

use std::{env, path::PathBuf};

use anyhow::{Result, anyhow, bail};

/// Channel type for the installation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Release,
    Nightly,
}

impl Channel {
    pub fn display_name(&self) -> &str {
        match self {
            Channel::Release => "Release",
            Channel::Nightly => "Nightly",
        }
    }
}

/// A validated version string for GitButler installations.
///
/// This type guarantees that the version string:
/// - Is not empty
/// - Does not start with '-' (not a flag)
/// - Contains only semver-compatible characters (alphanumeric, '.', '-', '+')
/// - Contains at least one alphanumeric character
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version(String);

impl Version {
    /// Create a new Version from a string, validating the format.
    ///
    /// # Errors
    /// Returns an error if the version string is invalid.
    pub fn new(version: String) -> Result<Self> {
        Self::validate(&version)?;
        Ok(Version(version))
    }

    /// Validate a version string format
    fn validate(version: &str) -> Result<()> {
        // Reject empty strings
        if version.is_empty() {
            bail!("Invalid version: empty string. Usage: but-installer [version|nightly]");
        }

        // Reject if it looks like a flag
        if version.starts_with('-') {
            bail!("Invalid version: {version}. Usage: but-installer [version|nightly]");
        }

        // Only allow semver-compatible characters
        if !version
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '+')
        {
            bail!(
                "Invalid version format: {version}. Version must contain only alphanumeric characters, dots, hyphens, and plus signs."
            );
        }

        // Must contain at least one alphanumeric character
        if !version.chars().any(|c| c.is_alphanumeric()) {
            bail!(
                "Invalid version format: {version}. Version must contain at least one alphanumeric character."
            );
        }

        Ok(())
    }

    /// Get the version string as a &str
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Version {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Version request for the installer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionRequest {
    /// Install the latest release version
    Release,
    /// Install the latest nightly build
    Nightly,
    /// Install a specific version (e.g., "0.18.7")
    Specific(Version),
}

impl VersionRequest {
    /// Parse a version string into a VersionRequest
    pub fn from_string(version: Option<String>) -> Result<Self> {
        match version.as_deref() {
            None => Ok(VersionRequest::Release),
            Some("nightly") => Ok(VersionRequest::Nightly),
            Some(version_str) => {
                let version = Version::new(version_str.to_string())?;
                Ok(VersionRequest::Specific(version))
            }
        }
    }
}

/// Configuration for the installer
pub struct InstallerConfig {
    pub version_request: VersionRequest,
    pub home_dir: PathBuf,
    pub platform: String,
}

impl InstallerConfig {
    /// Create a new installer config, reading version from command-line arguments or environment
    pub fn new() -> Result<Self> {
        // Validate argument count - only 0 or 1 positional arguments allowed
        let args: Vec<String> = env::args().collect();
        if args.len() > 2 {
            bail!(
                "Too many arguments. Usage: but-installer [version|nightly] or GITBUTLER_VERSION=<version> but-installer"
            );
        }

        // Get version from CLI argument (takes precedence) or GITBUTLER_VERSION env var
        let version_string = args
            .get(1)
            .cloned()
            .or_else(|| env::var("GITBUTLER_VERSION").ok());

        let version_request = VersionRequest::from_string(version_string)?;
        Self::new_with_version(version_request)
    }

    /// Create a new installer config with an explicit version request
    pub(crate) fn new_with_version(version_request: VersionRequest) -> Result<Self> {
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow!("Failed to determine home directory"))?;

        // Detect platform
        let os = env::consts::OS;
        let arch = env::consts::ARCH;

        let platform = match (os, arch) {
            ("macos", "aarch64") => "darwin-aarch64",
            ("macos", "x86_64") => "darwin-x86_64",
            ("linux", "aarch64") => "linux-aarch64",
            ("linux", "x86_64") => "linux-x86_64",
            (os, arch) => bail!("unsupported OS or architecture: {os} {arch}"),
        };

        Ok(Self {
            version_request,
            home_dir,
            platform: platform.to_string(),
        })
    }

    pub fn releases_url(&self) -> String {
        match &self.version_request {
            VersionRequest::Nightly => "https://app.gitbutler.com/releases/nightly".to_string(),
            VersionRequest::Specific(version) => {
                format!(
                    "https://app.gitbutler.com/releases/version/{}",
                    version.as_str()
                )
            }
            VersionRequest::Release => "https://app.gitbutler.com/releases".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_request_from_string_valid() {
        // Valid versions
        assert_eq!(
            VersionRequest::from_string(None).unwrap(),
            VersionRequest::Release
        );
        assert_eq!(
            VersionRequest::from_string(Some("nightly".to_string())).unwrap(),
            VersionRequest::Nightly
        );
        assert_eq!(
            VersionRequest::from_string(Some("1.0.0".to_string())).unwrap(),
            VersionRequest::Specific(Version::new("1.0.0".to_string()).unwrap())
        );
        assert_eq!(
            VersionRequest::from_string(Some("1.2.3-alpha".to_string())).unwrap(),
            VersionRequest::Specific(Version::new("1.2.3-alpha".to_string()).unwrap())
        );
        assert_eq!(
            VersionRequest::from_string(Some("2.0.0+build123".to_string())).unwrap(),
            VersionRequest::Specific(Version::new("2.0.0+build123".to_string()).unwrap())
        );
        assert_eq!(
            VersionRequest::from_string(Some("0.0.1-rc.1".to_string())).unwrap(),
            VersionRequest::Specific(Version::new("0.0.1-rc.1".to_string()).unwrap())
        );
    }

    #[test]
    fn test_version_request_from_string_invalid() {
        // Invalid versions
        assert!(VersionRequest::from_string(Some("".to_string())).is_err());
        assert!(VersionRequest::from_string(Some("--help".to_string())).is_err());
        assert!(VersionRequest::from_string(Some("-v".to_string())).is_err());
        assert!(VersionRequest::from_string(Some("1.0.0/../../etc".to_string())).is_err());
        assert!(VersionRequest::from_string(Some("1.0.0?foo=bar".to_string())).is_err());
        assert!(VersionRequest::from_string(Some("1.0.0;rm -rf /".to_string())).is_err());
        assert!(VersionRequest::from_string(Some("../../../etc/passwd".to_string())).is_err());
        assert!(VersionRequest::from_string(Some("...".to_string())).is_err());
        assert!(VersionRequest::from_string(Some("---".to_string())).is_err());
        assert!(VersionRequest::from_string(Some("+++".to_string())).is_err());
    }

    #[test]
    fn test_version_new_valid() {
        // Valid versions
        assert!(Version::new("1.0.0".to_string()).is_ok());
        assert!(Version::new("1.2.3-alpha".to_string()).is_ok());
        assert!(Version::new("2.0.0+build123".to_string()).is_ok());
        assert!(Version::new("0.0.1-rc.1".to_string()).is_ok());
    }

    #[test]
    fn test_version_new_invalid() {
        // Invalid versions
        assert!(Version::new("".to_string()).is_err());
        assert!(Version::new("--help".to_string()).is_err());
        assert!(Version::new("-v".to_string()).is_err());
        assert!(Version::new("1.0.0/../../etc".to_string()).is_err());
        assert!(Version::new("1.0.0?foo=bar".to_string()).is_err());
        assert!(Version::new("1.0.0;rm -rf /".to_string()).is_err());
        assert!(Version::new("../../../etc/passwd".to_string()).is_err());
        assert!(Version::new("...".to_string()).is_err());
        assert!(Version::new("---".to_string()).is_err());
        assert!(Version::new("+++".to_string()).is_err());
    }

    #[test]
    fn test_version_as_str() {
        let version = Version::new("1.2.3".to_string()).unwrap();
        assert_eq!(version.as_str(), "1.2.3");
    }

    #[test]
    fn test_version_display() {
        let version = Version::new("1.2.3".to_string()).unwrap();
        assert_eq!(format!("{version}"), "1.2.3");
    }

    #[test]
    fn test_version_as_ref() {
        let version = Version::new("1.2.3".to_string()).unwrap();
        let s: &str = version.as_ref();
        assert_eq!(s, "1.2.3");
    }

    #[test]
    fn test_channel_display_name() {
        assert_eq!(Channel::Release.display_name(), "Release");
        assert_eq!(Channel::Nightly.display_name(), "Nightly");
    }
}
