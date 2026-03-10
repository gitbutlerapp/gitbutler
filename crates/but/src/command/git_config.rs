//! Shared helpers for editing Git configuration files.

use anyhow::Result;
use but_core::{
    RepositoryExt as _,
    git_config::{open_user_global_config_for_editing, write_config},
};

boolean_enums::gen_boolean_enum!(pub EditGlobalConfig);

/// `edit` either the `repo`-local or user-global Git config, depending on `global`.
/// It calls `edit(config) -> changed` to find out if the passed configuration was indeed changed,
/// to persist the file if that was the case.
pub(crate) fn edit_git_config(
    repo: &gix::Repository,
    global: EditGlobalConfig,
    edit: impl FnOnce(&mut gix::config::File<'static>) -> Result<bool>,
) -> Result<bool> {
    if global.into() {
        let (mut config, path) = open_user_global_config_for_editing()?;
        let changed = edit(&mut config)?;
        if changed {
            write_config(&path, &config)?;
        }
        Ok(changed)
    } else {
        let mut config = repo.local_common_config_for_editing()?;
        let changed = edit(&mut config)?;
        if changed {
            repo.write_local_common_config(&config)?;
        }
        Ok(changed)
    }
}
