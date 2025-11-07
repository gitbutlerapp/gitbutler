use anyhow::Result;
use serde_json_lenient::json;
use std::path::Path;

use crate::permissions::{Permission, SerializationContext};

#[derive(Clone, Debug)]
pub enum SettingsKind {
    Allow,
    Deny,
}

pub fn add_permission_to_settings(
    kind: &SettingsKind,
    perm: &Permission,
    ctx: &SerializationContext,
    path: &Path,
) -> Result<()> {
    let mut settings = if path.try_exists()? {
        let file = std::fs::read_to_string(path)?;
        serde_json_lenient::from_str_lenient(&file)?
    } else {
        json!({})
    };

    let permissions_key = match kind {
        SettingsKind::Allow => "allow",
        SettingsKind::Deny => "deny",
    };

    let mut existing_perms = settings["permissions"][permissions_key]
        .as_array()
        .map(ToOwned::to_owned)
        .unwrap_or(vec![]);

    existing_perms.push(json!(perm.serialize(ctx)?));

    settings["permissions"][permissions_key] = json!(existing_perms);

    std::fs::write(path, serde_json_lenient::to_string_pretty(&settings)?)?;

    Ok(())
}
