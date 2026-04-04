use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;
use std::rc::Rc;

use anyhow::{Context as _, bail};
use bstr::BString;
use but_ctx::Context;
use colored::Colorize;
use itertools::Itertools as _;

use crate::{
    CliId, IdMap,
    utils::{OutputChannel, shorten_object_id},
};

fn do_evo(
    ctx: &mut Context,
    _guard: &but_core::sync::RepoShared,
    remote: String,
    local: String,
    graph: String,
) -> anyhow::Result<gix::ObjectId> {
    let repo = &*ctx.repo.get()?;

    type RevCommit =
        gix::revision::plumbing::graph::Commit<gix::revision::plumbing::merge_base::Flags>;
    let mut gix_graph: gix::revision::plumbing::Graph<'_, '_, RevCommit> =
        gix::revision::plumbing::Graph::new(&repo.objects, None);
    let remote_commit_id = repo.rev_parse_single(&*remote)?.detach();
    let local_commit_id = repo.rev_parse_single(&*local)?.detach();
    let merge_base =
        gix::revision::plumbing::merge_base(remote_commit_id, &[local_commit_id], &mut gix_graph)?
            .context("missing merge base")?
            .first()
            .to_owned();

    fn push_parents_then_self(
        gix_graph: &gix::revision::plumbing::Graph<'_, '_, RevCommit>,
        commit_id: &gix::ObjectId,
        merge_base: &gix::ObjectId,
        reverse_topology: &mut Vec<gix::ObjectId>,
    ) -> anyhow::Result<()> {
        if commit_id == merge_base {
            return Ok(());
        }
        if reverse_topology.contains(commit_id) {
            return Ok(());
        }
        let commit = gix_graph.get(commit_id).context("missing")?;
        for parent_id in &commit.parents {
            push_parents_then_self(gix_graph, parent_id, merge_base, reverse_topology)?;
        }
        reverse_topology.push(commit_id.to_owned());
        Ok(())
    }
    let mut remote_reverse_topology: Vec<gix::ObjectId> = Vec::new();
    push_parents_then_self(
        &gix_graph,
        &remote_commit_id,
        &merge_base,
        &mut remote_reverse_topology,
    )?;
    let mut local_reverse_topology: Vec<gix::ObjectId> = Vec::new();
    push_parents_then_self(
        &gix_graph,
        &local_commit_id,
        &merge_base,
        &mut local_reverse_topology,
    )?;

    // Assumes that family is family, no matter how distantly related (thus, this union-find structure is sufficient).
    // We'll need to switch to something that can distinguish close family from distant family.
    // TODO link to a doc describing this
    #[derive(Debug, Default)]
    struct Family<'repo> {
        chars: BTreeSet<u8>,
        /// In reverse topological order.
        remote_commits: Vec<gix::Commit<'repo>>,
        /// In reverse topological order.
        local_commits: Vec<gix::Commit<'repo>>,
    }
    type FamilyCell<'repo> = Rc<RefCell<Family<'repo>>>;
    let mut char_to_family = HashMap::<u8, FamilyCell>::new();
    for chars in graph.as_bytes().chunks(2) {
        let [char1, char2] = chars else {
            anyhow::bail!("graph must have even chars");
        };
        match (char_to_family.get(char1), char_to_family.get(char2)) {
            (None, None) => {
                let mut ref_cell = RefCell::<Family>::default();
                ref_cell.get_mut().chars.insert(*char1);
                ref_cell.get_mut().chars.insert(*char2);
                let family_cell = Rc::new(ref_cell);
                char_to_family.insert(*char1, family_cell.clone());
                char_to_family.insert(*char2, family_cell);
            }
            (None, Some(family_cell)) => {
                family_cell.borrow_mut().chars.insert(*char1);
                char_to_family.insert(*char1, family_cell.clone());
            }
            (Some(family_cell), None) => {
                family_cell.borrow_mut().chars.insert(*char2);
                char_to_family.insert(*char2, family_cell.clone());
            }
            (Some(family_cell1), Some(family_cell2)) => {
                family_cell1
                    .borrow_mut()
                    .chars
                    .extend(family_cell2.borrow_mut().chars.iter());
                char_to_family.insert(*char2, family_cell1.clone());
            }
        }
    }
    let mut remote_commit_id_to_family = HashMap::<gix::ObjectId, FamilyCell>::new();
    for commit_id in remote_reverse_topology.iter() {
        let commit = repo.find_commit(*commit_id)?;
        let message = commit.message_raw()?;
        if message.get(0) == message.get(1)
            && let Some(char) = message.get(0)
            && let Some(family) = char_to_family.get(char)
        {
            family.borrow_mut().remote_commits.push(commit);
            remote_commit_id_to_family.insert(*commit_id, family.clone());
        }
    }
    for commit_id in local_reverse_topology.iter() {
        let commit = repo.find_commit(*commit_id)?;
        let message = commit.message_raw()?;
        if message.get(0) == message.get(1)
            && let Some(char) = message.get(0)
            && let Some(family) = char_to_family.get(char)
        {
            family.borrow_mut().local_commits.push(commit);
        }
    }

    fn write_parents_then_self(
        repo: &gix::Repository,
        gix_graph: &gix::revision::plumbing::Graph<'_, '_, RevCommit>,
        remote_commit_id_to_family: &HashMap<gix::ObjectId, FamilyCell>,
        commit_id: &gix::ObjectId,
        merge_base: &gix::ObjectId,
        written_commit_ids: &mut HashMap<gix::ObjectId, gix::ObjectId>,
    ) -> anyhow::Result<gix::ObjectId> {
        if commit_id == merge_base {
            return Ok(*commit_id);
        }
        if let Some(rewritten_commit_id) = written_commit_ids.get(commit_id) {
            return Ok(*rewritten_commit_id);
        }
        let commit = repo.find_commit(*commit_id)?;
        let mut new_parent_ids = Vec::<gix::ObjectId>::new();
        for parent_id in commit.parent_ids() {
            new_parent_ids.push(write_parents_then_self(
                repo,
                gix_graph,
                remote_commit_id_to_family,
                &parent_id.detach(),
                merge_base,
                written_commit_ids,
            )?);
        }
        let message = if let Some(family) = remote_commit_id_to_family.get(commit_id) {
            let local_summary = family
                .borrow()
                .local_commits
                .iter()
                .map(|commit| commit.message().expect("message should be present").title)
                .join(",");
            BString::from(format!(
                "merge remote {} + local {}",
                commit.message()?.title,
                local_summary
            ))
        } else {
            BString::from(commit.message_raw()?)
        };
        let new_commit = gix::objs::Commit {
            tree: repo.empty_tree().id,
            parents: new_parent_ids.into(),
            author: commit.author()?.to_owned()?,
            committer: commit.committer()?.to_owned()?,
            encoding: None,
            message,
            extra_headers: Vec::new(),
        };
        let new_commit_id = repo.write_object(new_commit)?.detach();
        written_commit_ids.insert(*commit_id, new_commit_id);
        Ok(new_commit_id)
    }
    let mut written_commit_ids = HashMap::<gix::ObjectId, gix::ObjectId>::new();
    for commit_id in local_reverse_topology.iter() {
        let commit = repo.find_commit(*commit_id)?;
        let message = commit.message_raw()?;
        // Compare the first two bytes. Clippy doesn't like get(0), hence the first().
        if message.first() == message.get(1)
            && let Some(char) = message.first()
            && let Some(family) = char_to_family.get(char)
        {
            let mut borrowed_family = family.borrow_mut();
            let remote_commits = std::mem::take(&mut borrowed_family.remote_commits);
            std::mem::drop(borrowed_family);
            for remote_commit in remote_commits {
                write_parents_then_self(
                    repo,
                    &gix_graph,
                    &remote_commit_id_to_family,
                    &remote_commit.id,
                    &merge_base,
                    &mut written_commit_ids,
                )?;
            }
        }
    }

    Ok(written_commit_ids
        .get(&remote_commit_id)
        .context("remote commit id should have been rewritten")?
        .to_owned())
}

pub async fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch_id: &str,
    remote: Option<String>,
    local: Option<String>,
    graph: Option<String>,
) -> anyhow::Result<()> {
    let guard = ctx.exclusive_worktree_access();

    if let (Some(remote), Some(local), Some(graph)) = (remote, local, graph) {
        println!(
            "{}",
            do_evo(ctx, guard.read_permission(), remote, local, graph)?.to_hex()
        );
        return Ok(());
    }

    let mut progress = out.progress_channel();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

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
        writeln!(
            progress,
            "Merging branch {} into target {}",
            branch_name.bright_cyan(),
            format!("{}/{}", target_remote, base_branch.branch_name).bright_cyan()
        )?;

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

        writeln!(
            progress,
            "Merging {} ({}) into {} ({})",
            branch_name.bright_cyan(),
            shorten_object_id(&repo, merge_in_branch_head_oid).bright_black(),
            local_branch_name.bright_cyan(),
            shorten_object_id(&repo, local_branch_head_oid).bright_black()
        )?;

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

        if merge_result
            .tree_merge
            .has_unresolved_conflicts(Default::default())
        {
            bail!(
                "Merge resulted in conflicts, please run `but pull` to update {local_branch_name}"
            );
        }

        // write the merge commit and update the local branch
        let commit_message = format!("Merge branch '{branch_name}'");
        let merge_commit = repo.new_commit(
            commit_message,
            merge_result.tree_merge.tree.write()?,
            vec![merge_in_branch_head_oid, local_branch_head_oid],
        )?;

        writeln!(progress, "\nUpdating {}", local_branch_name.blue())?;

        // update the local branch
        let branch_ref_name: gix::refs::FullName =
            format!("refs/heads/{local_branch_name}").try_into()?;
        repo.reference(
            branch_ref_name.clone(),
            merge_commit.id(),
            gix::refs::transaction::PreviousValue::Any,
            "GitButler local merge",
        )?;

        // TODO: Drop the guard as we can't keep it across await, and `handle` will obtain its own as well.
        drop(guard);
        crate::command::legacy::pull::handle(ctx, out, false).await?;

        writeln!(
            progress,
            "\n{}",
            "Merge and update complete!".green().bold()
        )?;
    } else {
        bail!(
            "Target remote is {target_remote}, not gb-local. This command only works with gb-local targets."
        );
    }

    Ok(())
}
