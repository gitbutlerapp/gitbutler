//! Utilities for mutating Git configuration entries by dotted key.

use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice as _;
use gix::config::AsKey as _;

fn config_path(repo: Option<&gix::Repository>, source: gix::config::Source) -> Result<PathBuf> {
    let path = source
        .storage_location(&mut |name| std::env::var_os(name))
        .with_context(|| format!("failed to determine {source:?} git config location"))?;
    let path = if path.is_relative() {
        let repo = repo.with_context(|| {
            format!("determining the {source:?} git config location requires a repository")
        })?;
        if source == gix::config::Source::Local {
            repo.common_dir().join(&path)
        } else {
            repo.git_dir().join(&path)
        }
    } else {
        path
    };
    Ok(path)
}

fn open_config_for_editing(
    repo: Option<&gix::Repository>,
    source: gix::config::Source,
) -> Result<(gix::config::File, gix::lock::File)> {
    let path = config_path(repo, source)?;
    std::fs::create_dir_all(path.parent().context("git config path has no parent")?)?;
    let mut lock = gix::lock::File::acquire_to_update_resource_following_symlinks(
        &path,
        gix::lock::acquire::Fail::AfterDurationWithBackoff(std::time::Duration::from_secs(1)),
        None,
    )?;
    // TODO(ST): replace this entire function with `gix::config::FileTransaction` once it can be
    //           created without repository.
    let path = lock.resource_path().to_owned();
    let config = match std::fs::metadata(&path) {
        Ok(meta) => {
            lock.with_mut(|file| file.set_permissions(meta.permissions()))?;
            gix::config::File::from_path_no_includes(path.clone(), source).with_context(|| {
                format!("failed to open {source:?} git config at {}", path.display())
            })?
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            gix::config::File::new(gix::config::file::Metadata::from(source).at(path))
        }
        Err(err) => return Err(err.into()),
    };
    Ok((config, lock))
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

fn commit_standalone_config(config: &gix::config::File, mut lock: gix::lock::File) -> Result<()> {
    let path = lock.resource_path();
    config
        .write_to(&mut lock)
        .with_context(|| format!("failed to serialize git config at {}", path.display()))?;
    std::io::Write::flush(&mut lock)
        .with_context(|| format!("failed to flush git config at {}", path.display()))?;
    lock.commit()
        .map_err(|err| err.error)
        .with_context(|| format!("failed to commit git config at {}", path.display()))?;
    Ok(())
}

/// Open the Git config for `source` using `repo` when needed, let `edit` mutate it, and
/// write it back if the edited configuration differs from its original state.
/// Return `true` if the file changed and was written, `false` otherwise.
///
/// `repo` supplies the base directory when `source` resolves to a relative path, including
/// relative paths from environment overrides. Local config is resolved against `repo.common_dir()`;
/// worktree config and other relative paths are resolved against `repo.git_dir()`.
/// Pass `None` when the configuration path is absolute, as is usual for user-global config.
/// A relative path with `repo: None` is an error. Editing does not refresh `repo`'s cached configuration.
pub fn edit_config(
    repo: Option<&gix::Repository>,
    source: gix::config::Source,
    edit: impl FnOnce(&mut gix::config::File) -> Result<()>,
) -> Result<bool> {
    let (mut config, lock) = open_config_for_editing(repo, source)?;
    let previous_contents = config.to_bstring();
    edit(&mut config)?;
    let changed = config.to_bstring() != previous_contents;
    if changed {
        commit_standalone_config(&config, lock)?;
    }
    Ok(changed)
}

/// Open the Git config for `source` relative to `repo`, let `edit` mutate it, and write it back
/// if the edited configuration differs from its original state.
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
