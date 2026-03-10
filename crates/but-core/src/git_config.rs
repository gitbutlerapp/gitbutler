//! Utilities for mutating Git configuration entries by dotted key.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use gix::config::AsKey as _;

/// Open the user-global Git config for editing, creating it first if needed.
pub fn open_user_global_config_for_editing() -> Result<(gix::config::File<'static>, PathBuf)> {
    let path = gix::config::Source::User
        .storage_location(&mut |name| std::env::var_os(name))
        .context("failed to determine global git config location")?
        .into_owned();
    if !path.exists() {
        std::fs::create_dir_all(
            path.parent()
                .context("global git config path has no parent")?,
        )?;
        std::fs::File::create(&path)?;
    }
    let config = gix::config::File::from_path_no_includes(path.clone(), gix::config::Source::User)
        .with_context(|| format!("failed to open global git config at {}", path.display()))?;
    Ok((config, path))
}

/// Serialize a Git `config` file back to disk at `path`.
pub fn write_config(path: &Path, config: &gix::config::File<'_>) -> Result<()> {
    let mut file = std::io::BufWriter::new(
        std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(path)
            .with_context(|| {
                format!(
                    "failed to open git config for writing at {}",
                    path.display()
                )
            })?,
    );
    config
        .write_to(&mut file)
        .with_context(|| format!("failed to serialize git config at {}", path.display()))?;
    std::io::Write::flush(&mut file)
        .with_context(|| format!("failed to flush git config at {}", path.display()))?;
    Ok(())
}

/// Set the entry in `config` identified by the dotted `key` (like `section.value` or `section.subsection.value`) to `value`.
/// This will create sections as needed.
pub fn set_config_value(
    config: &mut gix::config::File<'static>,
    key: &str,
    value: &str,
) -> Result<()> {
    remove_config_value(config, key)?;
    let key = key
        .try_as_key()
        .with_context(|| format!("invalid git config key: {key}"))?;
    config
        .section_mut_or_create_new(key.section_name, key.subsection_name)?
        .set(
            gix::config::parse::section::ValueName::try_from(key.value_name.to_owned())?,
            value.into(),
        );
    Ok(())
}

/// Remove the Git entry in `config` identified by the dotted `key`
/// (like `section.value` or `section.subsection.value`) if it exists.
/// It's no error if it doesn't exist.
pub fn remove_config_value(config: &mut gix::config::File<'static>, key: &str) -> Result<()> {
    let key = key
        .try_as_key()
        .with_context(|| format!("invalid git config key: {key}"))?;
    config
        .section_mut(key.section_name, key.subsection_name)
        .ok()
        .and_then(|mut section| section.remove(key.value_name));
    Ok(())
}
