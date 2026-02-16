use std::fmt::Write;

use anyhow::bail;
use but_ctx::Context;
use colored::Colorize;

use crate::{CliId, IdMap, utils::OutputChannel};

pub async fn handle(ctx: &mut Context, out: &mut OutputChannel, branch_id: &str) -> anyhow::Result<()> {
    let mut progress = out.progress_channel();

    let id_map = IdMap::new_from_context(ctx, None)?;

    // Resolve the branch ID
    let resolved_ids = id_map.parse_using_context(branch_id, ctx)?;
    if resolved_ids.is_empty() {
        bail!("Could not find branch: {branch_id}");
    }
    if resolved_ids.len() > 1 {
        bail!("Ambiguous branch '{branch_id}', matches multiple items");
    }

    let cli_id = &resolved_ids[0];
    let branch_name = match cli_id {
        CliId::Branch { name, .. } => name.clone(),
        _ => bail!("Expected a branch ID, got {}", cli_id.kind_for_humans()),
    };

    // Get the base branch data to find the target
    let base_branch = but_api::legacy::virtual_branches::get_base_branch_data(ctx)?
        .ok_or_else(|| anyhow::anyhow!("No base branch configured"))?;

    let target_remote = base_branch.remote_name;

    // Check if target is gb-local
    if target_remote == "gb-local" {
        if out.for_human().is_some() {
            writeln!(
                progress,
                "Merging branch {} into target {}",
                branch_name.bright_cyan(),
                format!("{}/{}", target_remote, base_branch.branch_name).bright_cyan()
            )?;
        }

        // Extract the local branch name from the base branch
        // The branch_name might be "gb-local/main" or "gb-local/feature/foo", so strip the "gb-local/" prefix
        let local_branch_name = base_branch
            .branch_name
            .strip_prefix("gb-local/")
            .unwrap_or(&base_branch.branch_name)
            .to_string();

        // look up the local branch in gix
        let repo = gix::open(ctx.gitdir.as_path())?;
        let local_branch = repo
            .try_find_reference(&local_branch_name)?
            .ok_or_else(|| anyhow::anyhow!("Local branch {local_branch_name} not found"))?;
        let local_branch_head_oid = local_branch.into_fully_peeled_id()?;

        // get the oid of the branch we're merging in
        let merge_in_branch_head_oid = repo
            .try_find_reference(&branch_name)?
            .ok_or_else(|| anyhow::anyhow!("Branch {branch_name} not found"))?
            .into_fully_peeled_id()?;

        if out.for_human().is_some() {
            writeln!(
                progress,
                "Merging {} ({}) into {} ({})",
                branch_name.bright_cyan(),
                merge_in_branch_head_oid.to_string()[..7].bright_black(),
                local_branch_name.bright_cyan(),
                local_branch_head_oid.to_string()[..7].bright_black()
            )?;
        }

        // do the merge
        let mut merge_result = repo.merge_commits(
            merge_in_branch_head_oid,
            local_branch_head_oid,
            gix::merge::blob::builtin_driver::text::Labels {
                ancestor: Some("base".into()),
                current: Some("ours".into()),
                other: Some("theirs".into()),
            },
            gix::merge::commit::Options::default(),
        )?;

        if merge_result.tree_merge.has_unresolved_conflicts(Default::default()) {
            bail!("Merge resulted in conflicts, please run `but pull` to update {local_branch_name}");
        }

        // write the merge commit and update the local branch
        let commit_message = format!("Merge branch '{branch_name}'");
        let merge_commit = repo.new_commit(
            commit_message,
            merge_result.tree_merge.tree.write()?,
            vec![merge_in_branch_head_oid, local_branch_head_oid],
        )?;

        if out.for_human().is_some() {
            writeln!(progress, "\nUpdating {}", local_branch_name.blue())?;
        }

        // update the local branch
        let branch_ref_name: gix::refs::FullName = format!("refs/heads/{local_branch_name}").try_into()?;
        repo.reference(
            branch_ref_name.clone(),
            merge_commit.id(),
            gix::refs::transaction::PreviousValue::Any,
            "GitButler local merge",
        )?;

        crate::command::legacy::pull::handle(ctx, out, false).await?;

        if out.for_human().is_some() {
            writeln!(progress, "\n{}", "Merge and update complete!".green().bold())?;
        }
    } else {
        bail!("Target remote is {target_remote}, not gb-local. This command only works with gb-local targets.");
    }

    Ok(())
}
