//! `but agent uninstall`: remove what `but agent setup` and `but skill install`
//! wrote for coding agents.

use std::{
    fmt::{self, Write as _},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};

use super::{
    Scope, discover_repo, display_path,
    files::{has_managed_block, remove_managed_block_file},
    plan::{AgentTarget, collect_instruction_writes},
};
use crate::{
    command::skill,
    theme::{self, Paint},
    utils::{Confirm, ConfirmDefault, OutputChannel},
};

/// Remove every GitButler skill that `but skill check` finds and the managed
/// block from every instruction file `but agent setup` writes, in the home
/// directory and the current repository.
pub(super) fn uninstall(current_dir: &Path, out: &mut OutputChannel) -> Result<()> {
    let repo = discover_repo(current_dir);
    let home = but_path::home_dir().context("Could not determine home directory")?;

    let bases =
        std::iter::once((home, true)).chain(repo.as_ref().map(|repo| (repo.root.clone(), false)));
    let skills: Vec<PathBuf> = bases
        .flat_map(|(base, global)| skill::find_installations(&base, global))
        .map(|(path, _)| path)
        .collect();

    let scope = if repo.is_some() {
        Scope::Both
    } else {
        Scope::Global
    };
    let (writes, _) = collect_instruction_writes(&AgentTarget::ALL, scope, repo.as_ref())?;
    let mut instruction_files = Vec::new();
    for write in writes {
        if has_managed_block(&write.path)? {
            instruction_files.push(write.path);
        }
    }

    let found = !skills.is_empty() || !instruction_files.is_empty();
    // The list goes through the prompt's channel in a terminal, so a pager
    // can't hold it back until after the question.
    if found && let Some(mut input) = out.prepare_for_terminal_input() {
        write_found(&mut input, &skills, &instruction_files)?;
        if input.confirm("Remove these?", ConfirmDefault::Yes)? == Confirm::No {
            writeln!(input, "Cancelled. Nothing was removed.")?;
            return Ok(());
        }
    } else if let Some(writer) = out.for_human() {
        write_found(writer, &skills, &instruction_files)?;
    }

    for path in &skills {
        match std::fs::remove_dir_all(path) {
            // Already gone when two agents' skill folders link to the same place.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            result => result.with_context(|| format!("Failed to remove {}", path.display()))?,
        }
    }
    for path in &instruction_files {
        remove_managed_block_file(path)?;
    }

    if let Some(json_out) = out.for_json() {
        json_out.write_value(serde_json::json!({
            "skills": skills,
            "instructionFiles": instruction_files,
        }))?;
    } else if let Some(writer) = out.for_human()
        && found
    {
        writeln!(writer, "{} Removed.", theme::get().sym().success)?;
    }
    Ok(())
}

fn write_found(
    writer: &mut (impl fmt::Write + ?Sized),
    skills: &[PathBuf],
    instruction_files: &[PathBuf],
) -> fmt::Result {
    if skills.is_empty() && instruction_files.is_empty() {
        return writeln!(writer, "No GitButler skills or agent instructions found.");
    }
    writeln!(writer, "Found GitButler in your coding agents:")?;
    write_section(writer, "Skills", skills)?;
    write_section(writer, "GitButler section in", instruction_files)?;
    writeln!(writer)
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
