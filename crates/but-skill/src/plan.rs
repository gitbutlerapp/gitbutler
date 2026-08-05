//! What an agent-setup run would write, computed before anything touches disk.
//!
//! `Plan` is deliberately inert: building one performs no writes, so both the
//! CLI wizard and the desktop app can show the exact set of paths for review
//! before the user confirms.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};

use crate::target::AgentTarget;

/// Where agent artifacts are written: this repository, the user's home
/// directory, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Only the current repository.
    Repository,
    /// Only the user's home directory, applying to all their projects.
    Global,
    /// Both of the above.
    Both,
}

/// The repository an agent-setup run targets.
#[derive(Debug, Clone)]
pub struct RepoInfo {
    /// Absolute path to the repository worktree root.
    pub root: PathBuf,
    /// Whether the repository still needs `but setup` before GitButler works.
    pub needs_setup: bool,
}

#[derive(Debug)]
pub struct Plan {
    pub scope: Scope,
    pub policy: String,
    pub skill_installs: Vec<SkillInstallPlan>,
    pub instruction_writes: Vec<InstructionWritePlan>,
    pub print_only_notes: Vec<String>,
    pub setup_needed: bool,
}

impl Plan {
    pub fn new(
        repo: Option<&RepoInfo>,
        scope: Scope,
        agents: Vec<AgentTarget>,
        policy: String,
    ) -> Result<Self> {
        let skill_installs = collect_skill_installs(&agents, scope, repo)?;
        let (instruction_writes, print_only_notes) =
            collect_instruction_writes(&agents, scope, repo)?;
        let setup_needed = repository_setup_needed(repo, scope);
        Ok(Self {
            scope,
            policy,
            skill_installs,
            instruction_writes,
            print_only_notes,
            setup_needed,
        })
    }
}

#[derive(Debug)]
pub struct SkillInstallPlan {
    pub agent: AgentTarget,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct InstructionWritePlan {
    pub path: PathBuf,
    pub agents: Vec<AgentTarget>,
}

pub fn collect_skill_installs(
    agents: &[AgentTarget],
    scope: Scope,
    repo: Option<&RepoInfo>,
) -> Result<Vec<SkillInstallPlan>> {
    // Resolve each concrete install location (a single-location scope) to its
    // base directory once, expanding `Both` into global + repository.
    let mut locations: Vec<(Scope, PathBuf)> = Vec::new();
    if matches!(scope, Scope::Global | Scope::Both) {
        let home = but_path::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        locations.push((Scope::Global, home));
    }
    if matches!(scope, Scope::Repository | Scope::Both) {
        let root = repo
            .map(|repo| repo.root.clone())
            .context("Repository skill install requested outside a repository")?;
        locations.push((Scope::Repository, root));
    }

    let mut installs = Vec::new();
    for agent in agents {
        for (location, base_dir) in &locations {
            if let Some(components) = agent.skill_path_components(*location) {
                installs.push(SkillInstallPlan {
                    agent: *agent,
                    path: join_components(base_dir, components),
                });
            }
        }
    }
    Ok(installs)
}

pub fn repository_setup_needed(repo: Option<&RepoInfo>, scope: Scope) -> bool {
    repo.is_some_and(|repo| repo.needs_setup && matches!(scope, Scope::Repository | Scope::Both))
}

pub fn collect_instruction_writes(
    agents: &[AgentTarget],
    scope: Scope,
    repo: Option<&RepoInfo>,
) -> Result<(Vec<InstructionWritePlan>, Vec<String>)> {
    let mut by_path: BTreeMap<PathBuf, Vec<AgentTarget>> = BTreeMap::new();
    let mut print_only_notes = Vec::new();
    let home = if matches!(scope, Scope::Global | Scope::Both) {
        Some(but_path::home_dir().context("Could not determine home directory")?)
    } else {
        None
    };
    for agent in agents {
        if matches!(scope, Scope::Repository | Scope::Both) {
            let repo = repo.context("Repository instructions requested outside a repository")?;
            by_path
                .entry(join_components(
                    &repo.root,
                    agent.shared_instruction_components(),
                ))
                .or_default()
                .push(*agent);
        }

        if let Some(home) = &home {
            if let Some(components) = agent.global_instruction_components() {
                by_path
                    .entry(join_components(home, components))
                    .or_default()
                    .push(*agent);
            } else {
                print_only_notes.push(format!(
                    "{} has no supported global instructions file; copy the generated policy below into it manually.",
                    agent.name()
                ));
            }
        }
    }

    Ok((
        by_path
            .into_iter()
            .map(|(path, agents)| InstructionWritePlan { path, agents })
            .collect(),
        print_only_notes,
    ))
}

pub fn join_components(base: &Path, components: &[&str]) -> PathBuf {
    components
        .iter()
        .fold(base.to_path_buf(), |path, component| path.join(component))
}

/// Whether `base` joined with `marker`'s components exists on disk. `None` marker
/// (the agent has no such location) is never present.
pub fn marker_exists(base: &Path, marker: Option<&'static [&'static str]>) -> bool {
    marker.is_some_and(|components| join_components(base, components).exists())
}
