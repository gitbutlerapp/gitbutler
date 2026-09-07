//! Utilities for mutating Git configuration entries by dotted key.

use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice as _;
use gix::config::AsKey as _;

fn config_path(repo: Option<&gix::Repository>, source: gix::config::Source) -> Result<PathBuf> {
    match repo {
        Some(repo) => repo.config_path(source),
        None => gix::config_path(source, &gix::open::Options::default()),
    }
    .with_context(|| format!("failed to determine {source:?} git config location"))
}

/// Open the repository-local Git config of `repo` for reading without acquiring a write lock,
/// re-reading it from disk so changes made through other repository handles or by other
/// processes are observed.
///
/// If the config file doesn't exist yet, an empty in-memory config is returned.
pub fn open_repo_local_config_for_reading(repo: &gix::Repository) -> Result<gix::config::File> {
    let source = gix::config::Source::Local;
    let path = config_path(Some(repo), source)?;
    if !path.exists() {
        return Ok(gix::config::File::new(gix::config::file::Metadata::from(
            source,
        )));
    }
    gix::config::File::from_path_no_includes(path.clone(), source)
        .with_context(|| format!("failed to open {source:?} git config at {}", path.display()))
}

/// Open the Git config for `source` using `repo` when needed, let `edit` mutate it, and
/// write it back if the edited configuration differs from its original state.
/// Return `true` if the file changed and was written, `false` otherwise.
///
/// With `repo`, paths are resolved by [`gix::Repository::config_path()`], honoring its open options
/// and the current directory captured when it was opened. Otherwise [`gix::config_path()`] resolves
/// non-repository sources against the current directory. Local and worktree config require `repo`.
/// Editing does not refresh `repo`'s cached configuration.
pub fn edit_config(
    repo: Option<&gix::Repository>,
    source: gix::config::Source,
    edit: impl FnOnce(&mut gix::config::File) -> Result<()>,
) -> Result<bool> {
    let path = config_path(repo, source)?;
    std::fs::create_dir_all(path.parent().context("git config path has no parent")?)?;
    let mut config = match repo {
        Some(repo) => repo.config_file_mut(&path),
        None => gix::config_mut(source, &gix::open::Options::default()),
    }
    .with_context(|| format!("failed to open {source:?} git config at {}", path.display()))?;
    let previous_contents = config.to_bstring();
    edit(&mut config)?;
    let changed = config.to_bstring() != previous_contents;
    if changed {
        config
            .commit()
            .with_context(|| format!("failed to commit git config at {}", path.display()))?;
    }
    Ok(changed)
}

/// Edit the Git config for `source` using `repo`, resolving its path as described in [`edit_config()`].
/// Write it back if the edited configuration differs from its original state.
pub fn edit_repo_config(
    repo: &gix::Repository,
    source: gix::config::Source,
    edit: impl FnOnce(&mut gix::config::File) -> Result<()>,
) -> Result<bool> {
    if matches!(
        source,
        gix::config::Source::System | gix::config::Source::GitInstallation
    ) {
        bail!("editing {source:?} config through a repository is not supported");
    }
    edit_config(Some(repo), source, edit)
}

/// Set the entry in `config` identified by the dotted `key` (like `section.value` or `section.subsection.value`) to `value`.
/// This will create sections as needed, and remove all previous values under the same section with the same name.
pub fn set_config_value(config: &mut gix::config::File, key: &str, value: &str) -> Result<()> {
    remove_config_value(config, key)?;
    let key = key
        .try_as_key()
        .with_context(|| format!("invalid git config key: {key}"))?;
    config
        .section_mut_or_create_new(key.section_name, key.subsection_name)?
        .set(key.value_name, value)?;
    Ok(())
}

/// Ensure `value` is present in `config` for the multi-valued Git entry identified by `key`.
///
/// Returns `true` if the config was changed.
pub fn ensure_config_value(config: &mut gix::config::File, key: &str, value: &str) -> Result<bool> {
    let key = key
        .try_as_key()
        .with_context(|| format!("invalid git config key: {key}"))?;
    let value = value.as_bytes().as_bstr();
    let already_present =
        match config.raw_values_by(key.section_name, key.subsection_name, key.value_name) {
            Ok(values) => values.into_iter().any(|existing| existing == value),
            Err(_) => false,
        };
    if already_present {
        return Ok(false);
    }
    config
        .section_mut_or_create_new(key.section_name, key.subsection_name)?
        .push(key.value_name, Some(value))?;
    Ok(true)
}

/// Remove the Git entry in `config` identified by the dotted `key`
/// (like `section.value` or `section.subsection.value`) if it exists.
/// It's no error if it doesn't exist.
pub fn remove_config_value(config: &mut gix::config::File, key: &str) -> Result<()> {
    let key = key
        .try_as_key()
        .with_context(|| format!("invalid git config key: {key}"))?;
    config
        .section_mut(key.section_name, key.subsection_name)
        .ok()
        .and_then(|mut section| section.remove(key.value_name));
    Ok(())
}
