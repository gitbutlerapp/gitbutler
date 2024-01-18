#[allow(unused_imports)]
use crate::prelude::*;

/// The scope from/to which a configuration value is read/written.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    all(not(test), feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum ConfigScope {
    // NOTE(qix-): We disable all but `Local` when testing.
    // NOTE(qix-): This is not a standard practice, and you shouldn't
    // NOTE(qix-): do this in almost any other case. However, we do
    // NOTE(qix-): this here because most backends for Git do not have
    // NOTE(qix-): a way to override global/system/etc config locations,
    // NOTE(qix-): and we don't want to accidentally modify the user's
    // NOTE(qix-): global config when running tests or have them influence
    // NOTE(qix-): the tests in any way. Thus, we force test writers to use
    // NOTE(qix-): `Local` scope when testing. This is not ideal, but it's
    // NOTE(qix-): the best we can do for now. Sorry for the mess.
    /// Pull from the most appropriate scope.
    /// This is the default, and will fall back to a higher
    /// scope if the value is not initially found.
    #[cfg(not(test))]
    #[cfg_attr(not(test), default)]
    Auto = 0,
    /// Pull from the local scope (`.git/config`) _only_.
    #[cfg_attr(test, default)]
    Local = 1,
    /// Pull from the system-wide scope (`${prefix}/etc/gitconfig`) _only_.
    #[cfg(not(test))]
    System = 2,
    /// Pull from the global (user) scope (typically `~/.gitconfig`) _only_.
    #[cfg(not(test))]
    Global = 3,
}

/// A handle to an open Git repository.
pub trait Repository {
    /// The type of error returned by this repository.
    type Error: core::error::Error + core::fmt::Debug + Send + Sync + 'static;

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
