//! Shared helpers for editing Git configuration files.

use anyhow::Result;
use but_core::git_config::edit_repo_config;

boolean_enums::gen_boolean_enum!(pub EditGlobalConfig);

/// `edit` either the `repo`-local or user-global Git config, depending on `global`.
/// It compares the edited config to its previous value to determine whether to persist it.
pub(crate) fn edit_git_config(
    repo: &gix::Repository,
    global: EditGlobalConfig,
    edit: impl FnOnce(&mut gix::config::File<'static>) -> Result<()>,
) -> Result<bool> {
    let source = if global.into() {
        gix::config::Source::User
    } else {
        gix::config::Source::Local
    };
    edit_repo_config(repo, source, edit)
}
