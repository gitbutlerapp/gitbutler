use but_core::ref_metadata::StackId;
use serde::Serialize;

use crate::{CliId, CliResult, IdMap, bad_input, id::ShortId, utils::shorten_object_id};

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct CliIdArg(pub String);

impl std::str::FromStr for CliIdArg {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_owned()))
    }
}

impl std::fmt::Display for CliIdArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl CliIdArg {
    pub fn resolve(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
        purpose: Purpose,
    ) -> CliResult<CliId> {
        let mut target_ids = id_map.parse_using_context(&self.0, ctx)?;
        if target_ids.is_empty() {
            return Err(bad_input(format!("Could not find {purpose}: {self}")).into());
        }
        if target_ids.len() > 1 {
            return Err(bad_input(format!(
                "Ambiguous {purpose} '{self}', matches multiple items"
            ))
            .into());
        }
        Ok(target_ids.swap_remove(0))
    }

    pub fn resolve_commit_or_branch(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
        purpose: Purpose,
    ) -> CliResult<CommitOrBranchCliId> {
        let target_id = self.resolve(ctx, id_map, purpose)?;
        match self.resolve(ctx, id_map, purpose)? {
            CliId::Commit { commit_id, id } => Ok(CommitOrBranchCliId::Commit { commit_id, id }),
            CliId::Branch { name, id, stack_id } => {
                Ok(CommitOrBranchCliId::Branch { name, id, stack_id })
            }
            _ => Err(bad_input(format!(
                "Invalid {purpose} type: {}, expected commit or branch",
                target_id.kind_for_humans()
            ))
            .into()),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Purpose {
    Anchor,
}

impl std::fmt::Display for Purpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Purpose::Anchor => f.write_str("anchor"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CommitOrBranchCliId {
    Commit {
        commit_id: gix::ObjectId,
        id: ShortId,
    },
    Branch {
        name: String,
        id: ShortId,
        stack_id: Option<StackId>,
    },
}

impl CommitOrBranchCliId {
    pub fn display(&self, repo: &gix::Repository) -> impl std::fmt::Display {
        std::fmt::from_fn(|f| match self {
            CommitOrBranchCliId::Commit { commit_id, .. } => {
                let short = shorten_object_id(repo, *commit_id);
                std::fmt::Display::fmt(&short, f)
            }
            CommitOrBranchCliId::Branch { name, .. } => std::fmt::Display::fmt(name, f),
        })
    }
}
