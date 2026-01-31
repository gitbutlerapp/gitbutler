use std::{ops::Deref, str::FromStr};

use anyhow::bail;
use bstr::ByteSlice;
use but_ctx::Context;
use gitbutler_reference::RemoteRefname;
use gix::reference::Category;

use crate::utils::OutputChannel;

/// Apply a branch to the workspace, and return the full ref name to it.
///
/// Look first in through the local references, then the remote references.
pub fn apply(ctx: &mut Context, branch_name: &str, out: &mut OutputChannel) -> anyhow::Result<()> {
    let repo = ctx.repo.get()?;
    let reference = if let Some(r) = repo.try_find_reference(branch_name)? {
        // Look for the branch in the local repository
        let ref_name = gitbutler_reference::Refname::from_str(&r.name().to_string())?;
        let remote_ref_name = r
            .remote_ref_name(gix::remote::Direction::Push)
            .transpose()?
            .as_deref()
            .and_then(|ref_name| {
                gitbutler_reference::RemoteRefname::from_str(&ref_name.to_string()).ok()
            });

        let r = r.detach();
        drop(repo);
        but_api::legacy::virtual_branches::create_virtual_branch_from_branch(
            ctx,
            ref_name,
            remote_ref_name,
            None,
        )?;
        r
    } else if let Some((remote_ref, r)) = find_remote_reference(&repo, branch_name)? {
        let remote = remote_ref.remote();
        let name = remote_ref.branch();
        // Look for the branch in the remote references
        let ref_name =
            gitbutler_reference::Refname::from_str(&format!("refs/remotes/{remote}/{name}"))?;
        let r = r.detach();
        drop(repo);
        but_api::legacy::virtual_branches::create_virtual_branch_from_branch(
            ctx,
            ref_name,
            Some(remote_ref.clone()),
            None,
        )?;
        r
    } else {
        // NOTE: this is aligned with the 'modern' version for now which doesn't handle this specifically.
        //       Once there is only one impl, this can be adjusted more easily.
        bail!("The reference '{branch_name}' did not exist");
    };

    if let Some(out) = out.for_human() {
        let short_name = reference.name.shorten();
        let is_remote_reference = reference
            .name
            .category()
            .is_some_and(|c| c == Category::RemoteBranch);
        if is_remote_reference {
            writeln!(out, "Applied remote branch '{short_name}' to workspace")
        } else {
            writeln!(out, "Applied branch '{short_name}' to workspace")
        }?;
    } else if let Some(out) = out.for_shell() {
        writeln!(out, "{reference_name}", reference_name = reference.name)?;
    }

    if let Some(out) = out.for_json() {
        out.write_value(but_api::json::Reference::from(reference))?;
    }
    Ok(())
}

fn find_remote_reference<'repo>(
    repo: &'repo gix::Repository,
    branch_name: &str,
) -> anyhow::Result<Option<(RemoteRefname, gix::Reference<'repo>)>> {
    for remote in repo.remote_names().iter().map(|r| r.deref()) {
        let remote_ref_name = RemoteRefname::new(remote.to_str()?, branch_name);
        if let Some(reference) = repo.try_find_reference(&remote_ref_name.fullname())? {
            return Ok(Some((remote_ref_name, reference)));
        }
    }
    Ok(None)
}
