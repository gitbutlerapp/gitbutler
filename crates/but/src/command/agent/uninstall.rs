//! `but agent uninstall`: remove what `but agent setup` and `but skill install`
//! wrote for coding agents.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};

use super::{
    Scope, discover_repo, display_path,
    files::remove_managed_block_file,
    plan::{AgentTarget, collect_instruction_writes},
};
use crate::{
    command::skill,
    theme::{self, Paint},
    utils::OutputChannel,
};

/// Remove every GitButler skill that `but skill check` finds and the managed
/// block from every instruction file `but agent setup` writes, in the home
/// directory and the current repository.
pub(super) fn uninstall(current_dir: &Path, out: &mut OutputChannel) -> Result<()> {
    let repo = discover_repo(current_dir);
    let home = but_path::home_dir().context("Could not determine home directory")?;

    let mut skills = Vec::new();
    let bases =
        std::iter::once((home, true)).chain(repo.as_ref().map(|repo| (repo.root.clone(), false)));
    for (base, global) in bases {
        for (path, _) in skill::find_installations(&base, global) {
            match std::fs::remove_dir_all(&path) {
                Ok(()) => skills.push(path),
                // Already gone when two agents' skill folders link to the same place.
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(err)
                        .with_context(|| format!("Failed to remove {}", path.display()));
                }
            }
        }
    }

    let scope = if repo.is_some() {
        Scope::Both
    } else {
        Scope::Global
    };
    let (writes, _) = collect_instruction_writes(&AgentTarget::ALL, scope, repo.as_ref())?;
    let mut instruction_files = Vec::new();
    for write in writes {
        if remove_managed_block_file(&write.path)? {
            instruction_files.push(write.path);
        }
    }

    if let Some(json_out) = out.for_json() {
        json_out.write_value(serde_json::json!({
            "skills": skills,
            "instructionFiles": instruction_files,
        }))?;
    } else if let Some(writer) = out.for_human() {
        if skills.is_empty() && instruction_files.is_empty() {
            writeln!(writer, "No GitButler skills or agent instructions found.")?;
        } else {
            writeln!(writer, "Removed GitButler from your coding agents:")?;
            write_section(writer, "Skills", &skills)?;
            write_section(writer, "GitButler section in", &instruction_files)?;
        }
    }
    Ok(())
}

fn write_section(
    writer: &mut (impl fmt::Write + ?Sized),
    title: &str,
    paths: &[PathBuf],
) -> fmt::Result {
    if paths.is_empty() {
        return Ok(());
    }
    let t = theme::get();
    writeln!(writer)?;
    writeln!(writer, "{}", t.important.paint(title))?;
    for path in paths {
        writeln!(
            writer,
            "  {} {}",
            t.info.paint("•"),
            t.config_value.paint(display_path(path))
        )?;
    }
    Ok(())
}
