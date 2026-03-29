use anyhow::Result;
use but_hooks::managed_hooks::{
    install_hooks_config_key, install_managed_hooks_enabled, set_install_managed_hooks_enabled,
};
use tempfile::TempDir;

#[test]
fn enabled_defaults_to_true() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = gix::init(temp_dir.path())?;

    assert!(install_managed_hooks_enabled(&repo));
    Ok(())
}

#[test]
fn persists_false() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = gix::init(temp_dir.path())?;

    set_install_managed_hooks_enabled(&repo, false)?;

    let reopened = gix::open(temp_dir.path())?;
    assert!(!install_managed_hooks_enabled(&reopened));
    assert_eq!(
        reopened
            .config_snapshot()
            .string(install_hooks_config_key())
            .as_deref(),
        Some("false".into())
    );
    Ok(())
}

#[test]
fn persists_true() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = gix::init(temp_dir.path())?;

    set_install_managed_hooks_enabled(&repo, false)?;
    set_install_managed_hooks_enabled(&repo, true)?;

    let reopened = gix::open(temp_dir.path())?;
    assert!(install_managed_hooks_enabled(&reopened));
    assert_eq!(
        reopened
            .config_snapshot()
            .string(install_hooks_config_key())
            .as_deref(),
        Some("true".into())
    );
    Ok(())
}
