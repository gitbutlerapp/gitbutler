//! The user's keymap, `keymap.json` in the settings folder, which `crate::keymap` binds.

use std::path::PathBuf;

use anyhow::{Context as _, Result};
use notify::Watcher as _;
use notify::event::{EventKind, ModifyKind};

const FILE: &str = "keymap.json";

fn path() -> Result<PathBuf> {
    Ok(super::settings::config_dir()?.join(FILE))
}

/// What the keymap holds, or `None` when there's no such file.
pub fn read() -> Result<Option<String>> {
    match std::fs::read_to_string(path()?) {
        Ok(json) => Ok(Some(json)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).context("couldn't read keymap.json"),
    }
}

/// The keymap's path, written as `template` first when there's none yet.
pub fn create(template: &str) -> Result<PathBuf> {
    let path = path()?;
    if !path.try_exists()? {
        std::fs::write(&path, template).context("couldn't create keymap.json")?;
    }
    Ok(path)
}

/// Replace the keymap with `contents`, as Settings › Keyboard changes it.
pub fn write(contents: &str) -> Result<()> {
    std::fs::write(path()?, contents).context("couldn't write keymap.json")
}

/// `contents` in the system's temporary folder as `name`, to open and read.
pub fn temporary(name: &str, contents: &str) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, contents).with_context(|| format!("couldn't write {name}"))?;
    Ok(path)
}

/// Calls `changed` each time something writes the keymap, while the returned watcher lives.
pub fn watch(changed: impl Fn() + Send + 'static) -> Result<notify::RecommendedWatcher> {
    let path = path()?;
    let dir = path.parent().context("the keymap has no folder")?;
    std::fs::create_dir_all(dir)?;
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let ours = |event: &notify::Event| {
            (event.paths.iter()).any(|path| path.file_name() == Some(FILE.as_ref()))
        };
        // Linux reports opening the file and its access time too, which reading it would set
        // off again and again: only a change to what it holds counts.
        let written = |event: &notify::Event| {
            matches!(
                event.kind,
                EventKind::Create(_)
                    | EventKind::Remove(_)
                    | EventKind::Modify(
                        ModifyKind::Data(_) | ModifyKind::Name(_) | ModifyKind::Any
                    )
            )
        };
        if event.is_ok_and(|event| ours(&event) && written(&event)) {
            changed();
        }
    })?;
    watcher.watch(dir, notify::RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
