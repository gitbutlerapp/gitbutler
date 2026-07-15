use but_core::HunkHeader;
use serde::Serialize;

use crate::{
    CliId, CliResult, IdMap,
    args::atoms::CliIdArg,
    theme::Theme,
    utils::{CliOutput, CliOutputHuman, WriteWithUtils},
};

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Resource {
    Commit {
        change_id: Option<String>,
        commit_id: String,
    },
    Branch {
        short_id: String,
        name: String,
    },
    UncommittedFile {
        path: String,
    },
    UncommittedHunk {
        path: String,
        hunk_header: String,
    },
    CommittedFile {
        commit_id: String,
        path: String,
    },
    PathPrefix {
        path: String,
    },
    Uncommitted,
    Stack {
        stack_id: String,
    },
    Worktree {
        name: String,
    },
    WorktreeChange {
        name: String,
        path: String,
        hunk_header: Option<String>,
    },
}

impl std::fmt::Display for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Resource::Commit {
                change_id,
                commit_id,
            } => {
                write!(f, "commit:")?;
                if let Some(change_id) = change_id {
                    write!(f, " {change_id}")?;
                }
                write!(f, " {commit_id}")
            }
            Resource::Branch { short_id, name } => write!(f, "branch: {short_id} {name}"),
            Resource::UncommittedFile { path } => write!(f, "uncommitted file: {path}"),
            Resource::UncommittedHunk { path, hunk_header } => {
                write!(f, "uncommitted hunk: {path} {hunk_header}")
            }
            Resource::CommittedFile { commit_id, path } => {
                write!(f, "committed file: {commit_id} {path}")
            }
            Resource::PathPrefix { path } => write!(f, "path prefix: {path}"),
            Resource::Uncommitted => f.write_str("uncommitted area"),
            Resource::Stack { stack_id } => write!(f, "stack: {stack_id}"),
            Resource::Worktree { name } => write!(f, "worktree: {name}"),
            Resource::WorktreeChange {
                name,
                path,
                hunk_header,
            } => {
                write!(f, "worktree change: {name} {path}")?;
                if let Some(hunk_header) = hunk_header {
                    write!(f, " {hunk_header}")?;
                }
                Ok(())
            }
        }
    }
}

pub struct ExpandOutcome {
    resources: Vec<Resource>,
}

impl CliOutputHuman for ExpandOutcome {
    fn on_human(self, out: &mut dyn WriteWithUtils, _theme: &'static Theme) -> anyhow::Result<()> {
        writeln!(out, "Matches: {}", self.resources.len())?;
        writeln!(out)?;
        for resource in self.resources {
            writeln!(out, "{resource}")?;
        }
        Ok(())
    }
}

impl CliOutput for ExpandOutcome {
    fn on_shell(self, out: &mut dyn WriteWithUtils) -> anyhow::Result<()> {
        for resource in self.resources {
            writeln!(out, "{resource}")?;
        }
        Ok(())
    }

    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            matches: usize,
            resources: Vec<Resource>,
        }

        Output {
            matches: self.resources.len(),
            resources: self.resources,
        }
    }
}

pub fn handle(ctx: &but_ctx::Context, cli_id: CliIdArg) -> CliResult<ExpandOutcome> {
    let guard = ctx.shared_worktree_access();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;
    let repo = ctx.repo.get()?;
    let matches = cli_id.parse(&repo, &id_map)?;
    let resources = matches
        .into_iter()
        .flat_map(resources_from_cli_id)
        .collect();

    Ok(ExpandOutcome { resources })
}

fn resources_from_cli_id(cli_id: CliId) -> Vec<Resource> {
    match cli_id {
        CliId::Commit {
            commit_id,
            change_id,
            ..
        } => vec![Resource::Commit {
            change_id: change_id.map(|id| id.to_string()),
            commit_id: commit_id.to_string(),
        }],
        CliId::Branch { name, id, .. } => vec![Resource::Branch { short_id: id, name }],
        CliId::UncommittedHunkOrFile(uncommitted) if uncommitted.is_entire_file => {
            vec![Resource::UncommittedFile {
                path: uncommitted.hunk_assignments.first().path_bytes.to_string(),
            }]
        }
        CliId::UncommittedHunkOrFile(uncommitted) => uncommitted
            .hunk_assignments
            .into_iter()
            .map(|assignment| Resource::UncommittedHunk {
                path: assignment.path_bytes.to_string(),
                hunk_header: assignment
                    .hunk_header
                    .map(format_hunk_header)
                    .unwrap_or_else(|| "<no hunk header>".to_string()),
            })
            .collect(),
        CliId::CommittedFile {
            commit_id, path, ..
        } => vec![Resource::CommittedFile {
            commit_id: commit_id.to_string(),
            path: path.to_string(),
        }],
        CliId::PathPrefix { id, .. } => vec![Resource::PathPrefix { path: id }],
        CliId::Uncommitted { .. } => vec![Resource::Uncommitted],
        CliId::Stack { stack_id, .. } => vec![Resource::Stack {
            stack_id: stack_id.to_string(),
        }],
        CliId::Worktree { name, .. } => vec![Resource::Worktree {
            name: name.to_string(),
        }],
        CliId::WorktreeChange(change) => vec![Resource::WorktreeChange {
            name: change.name.to_string(),
            path: change.path.to_string(),
            hunk_header: change.hunk_header.map(format_hunk_header),
        }],
    }
}

fn format_hunk_header(header: HunkHeader) -> String {
    format!(
        "@@ -{},{} +{},{} @@",
        header.old_start, header.old_lines, header.new_start, header.new_lines
    )
}
