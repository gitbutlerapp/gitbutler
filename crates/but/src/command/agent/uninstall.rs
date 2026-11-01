//! `but agent uninstall`: remove what `but agent setup` and `but skill install`
//! wrote for coding agents.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Write as _},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};

use super::{
    RepoInfo, discover_repo, display_path,
    files::remove_managed_blocks,
    plan::{AgentTarget, join_components},
};
use crate::{
    command::skill,
    theme::{self, Paint},
    utils::{Confirm, ConfirmDefault, OutputChannel},
};

#[derive(Debug)]
struct Removal {
    /// GitButler skill folders.
    skills: BTreeSet<PathBuf>,
    /// Instruction files with a managed block, mapped to their content without it.
    instruction_files: BTreeMap<PathBuf, String>,
}

impl Removal {
    fn is_empty(&self) -> bool {
        self.skills.is_empty() && self.instruction_files.is_empty()
    }

    fn apply(&self) -> Result<()> {
        for path in &self.skills {
            match std::fs::remove_dir_all(path) {
                // Already gone when two agents' skill folders link to the same place.
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                result => result.with_context(|| format!("Failed to remove {}", path.display()))?,
            }
        }
        for (path, updated) in &self.instruction_files {
            // Deleting a symlink would leave the GitButler section in its
            // target, so an emptied symlink gets an empty target instead.
            if updated.is_empty() && !path.is_symlink() {
                std::fs::remove_file(path)
            } else {
                std::fs::write(path, updated)
            }
            .with_context(|| format!("Failed to update {}", path.display()))?;
        }
        Ok(())
    }
}

pub(super) fn uninstall(current_dir: &Path, out: &mut OutputChannel) -> Result<()> {
    let home = but_path::home_dir().context("Could not determine home directory")?;
    let removal = find_removal(&home, discover_repo(current_dir).as_ref())?;

    let confirmed_in_terminal = !removal.is_empty() && out.can_prompt();
    if confirmed_in_terminal {
        let mut input = out
            .prepare_for_terminal_input()
            .context("Confirming the removal requires a terminal.")?;
        writeln!(input, "This removes GitButler from your coding agents:")?;
        write_removal(&mut input, &removal)?;
        writeln!(input)?;
        if input.confirm("Remove these?", ConfirmDefault::Yes)? == Confirm::No {
            writeln!(input, "Cancelled. Nothing was removed.")?;
            return Ok(());
        }
    }
    removal.apply()?;

    if let Some(json_out) = out.for_json() {
        json_out.write_value(serde_json::json!({
            "skills": removal.skills,
            "instructionFiles": removal.instruction_files.keys().collect::<Vec<_>>(),
        }))?;
    } else if let Some(writer) = out.for_human() {
        if removal.is_empty() {
            writeln!(writer, "No GitButler skills or agent instructions found.")?;
        } else if confirmed_in_terminal {
            writeln!(writer, "{} Removed.", theme::get().sym().success)?;
        } else {
            writeln!(writer, "Removed GitButler from your coding agents:")?;
            write_removal(writer, &removal)?;
        }
    }
    Ok(())
}

/// Find GitButler skills and managed blocks under `home` and, inside a
/// repository, under its root. Uses the same skill formats as `but skill check`
/// and the same instruction files as `but agent setup`, for every agent.
fn find_removal(home: &Path, repo: Option<&RepoInfo>) -> Result<Removal> {
    let mut bases = vec![(home, true)];
    if let Some(repo) = repo {
        bases.push((&repo.root, false));
    }

    let mut skills = BTreeSet::new();
    let mut instruction_files = BTreeMap::new();
    for (base, global) in bases {
        skills.extend(skill::find_installations(base, global));
        for agent in AgentTarget::ALL {
            let components = if global {
                agent.global_instruction_components()
            } else {
                Some(agent.shared_instruction_components())
            };
            let Some(components) = components else {
                continue;
            };
            let path = join_components(base, components);
            if let Some(updated) = without_managed_blocks(&path)? {
                instruction_files.insert(path, updated);
            }
        }
    }
    Ok(Removal {
        skills,
        instruction_files,
    })
}

fn without_managed_blocks(path: &Path) -> Result<Option<String>> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("Failed to read {}", path.display())),
    };
    remove_managed_blocks(&content).with_context(|| {
        format!(
            "Could not remove the GitButler section from {}",
            path.display()
        )
    })
}

fn write_removal(writer: &mut (impl fmt::Write + ?Sized), removal: &Removal) -> fmt::Result {
    let t = theme::get();
    let bullet = t.info.paint("•");
    if !removal.skills.is_empty() {
        writeln!(writer)?;
        writeln!(writer, "{}", t.important.paint("Skills"))?;
        for path in &removal.skills {
            writeln!(
                writer,
                "  {bullet} {}",
                t.config_value.paint(display_path(path))
            )?;
        }
    }
    if !removal.instruction_files.is_empty() {
        writeln!(writer)?;
        writeln!(writer, "{}", t.important.paint("GitButler section in"))?;
        for path in removal.instruction_files.keys() {
            writeln!(
                writer,
                "  {bullet} {}",
                t.config_value.paint(display_path(path))
            )?;
        }
    }
    Ok(())
}
