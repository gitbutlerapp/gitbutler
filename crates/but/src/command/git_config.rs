//! Shared helpers for editing Git configuration files.

use anyhow::{Context as _, Result};
use but_core::git_config::{edit_config, edit_repo_config};

boolean_enums::gen_boolean_enum!(pub EditGlobalConfig);

/// `edit` either the `repo`-local or user-global Git config, depending on `global`.
/// It compares the edited config to its previous value to determine whether to persist it.
/// A repository is required only for local edits.
pub(crate) fn edit_git_config(
    repo: Option<&gix::Repository>,
    global: EditGlobalConfig,
    edit: impl FnOnce(&mut gix::config::File) -> Result<()>,
) -> Result<bool> {
    if global.into() {
        edit_config(None, gix::config::Source::User, edit)
    } else {
        edit_repo_config(
            repo.context("Local Git configuration requires a git repository")?,
            gix::config::Source::Local,
            edit,
        )
    }
}
