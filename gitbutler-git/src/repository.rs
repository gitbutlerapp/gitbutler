#[allow(unused_imports)]
use crate::prelude::*;

/// The scope from/to which a configuration value is read/written.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConfigScope {
    /// Pull from the most appropriate scope.
    /// This is the default, and will fall back to a higher
    /// scope if the value is not initially found.
    #[default]
    Auto = 0,
    /// Pull from the local scope (`.git/config`) _only_.
    Local = 1,
    /// Pull from the system-wide scope (`${prefix}/etc/gitconfig`) _only_.
    System = 2,
    /// Pull from the global (user) scope (typically `~/.gitconfig`) _only_.
    Global = 3,
}

/// A handle to an open Git repository.
pub trait Repository {
    /// The type of error returned by this repository.
    type Error: core::error::Error + Send + Sync + 'static;

    /// Reads a configuration value.
    ///
    /// Errors if the value is not valid UTF-8.
    async fn config_get(
        &self,
        key: &str,
        scope: ConfigScope,
    ) -> Result<Option<String>, Self::Error>;

    /// Writes a configuration value.
    ///
    /// Errors if the new value is not valid UTF-8.
    async fn config_set(
        &self,
        key: &str,
        value: &str,
        scope: ConfigScope,
    ) -> Result<(), Self::Error>;
}
