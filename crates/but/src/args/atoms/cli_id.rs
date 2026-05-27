use serde::Serialize;

use crate::{CliId, CliResult, IdMap, args::atoms::BranchArg, bad_input};

/// An argument atom for cli ids that can match multiple things like branches, commits, files, etc.
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
    pub fn try_resolve(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
        purpose: Purpose,
    ) -> CliResult<Option<CliId>> {
        let mut target_ids = id_map.parse_using_context(&self.0, ctx)?;
        if target_ids.is_empty() {
            return Ok(None);
        }
        if target_ids.len() > 1 {
            return Err(bad_input(format!(
                "Ambiguous {purpose} '{self}', matches multiple items"
            ))
            .into());
        }
        Ok(Some(target_ids.swap_remove(0)))
    }

    pub fn resolve(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
        purpose: Purpose,
    ) -> CliResult<CliId> {
        if let Some(cli_id) = self.try_resolve(ctx, id_map, purpose)? {
            Ok(cli_id)
        } else {
            Err(bad_input(format!("Could not find {purpose}: {self}")).into())
        }
    }

    pub fn resolve_commit_or_branch(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
        purpose: Purpose,
    ) -> CliResult<CommitOrBranchCliId> {
        let target_id = self.resolve(ctx, id_map, purpose)?;
        match target_id {
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

    pub fn try_resolve_branch(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
        purpose: Purpose,
    ) -> CliResult<Option<Branch>> {
        let Some(target_id) = self.try_resolve(ctx, id_map, purpose)? else {
            return Ok(None);
        };
        match target_id {
            CliId::Branch { name, id, stack_id } => Ok(Some(Branch { name, id, stack_id })),
            _ => Err(bad_input(format!(
                "Invalid {purpose} type: {}, expected branch",
                target_id.kind_for_humans()
            ))
            .into()),
        }
    }

    pub fn resolve_branch_arg(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
        purpose: Purpose,
    ) -> BranchNameArg {
        self.try_resolve_branch(ctx, id_map, purpose)
            // TODO: IdMap returns an error if you look up single letter ids, however branches
            // might be single letters which shouldn't be considered an error. We should fix that
            // so it returns `None` instead of erroring.
            .ok()
            .flatten()
            .map(|branch| BranchNameArg(branch.name))
            // the branch might be unapplied in which case it wont be in the IdMap
            .unwrap_or_else(|| BranchNameArg(self.0.clone()))
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Purpose {
    Anchor,
    Branch,
    Target,
}

impl std::fmt::Display for Purpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Purpose::Anchor => f.write_str("anchor"),
            Purpose::Branch => f.write_str("branch"),
            Purpose::Target => f.write_str("target"),
        }
    }
}

/// A [`CliIdArg`] that has actually been resolved.
#[derive(Debug, Clone)]
pub enum ResolvedCliIdArg {
    #[expect(missing_docs)]
    Commit(gix::ObjectId),
    #[expect(missing_docs)]
    Branch(BranchArg),
}

impl std::fmt::Display for ResolvedCliIdArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvedCliIdArg::Commit(inner) => inner.to_hex_with_len(7).fmt(f),
            ResolvedCliIdArg::Branch(inner) => inner.fmt(f),
        }
    }
}
