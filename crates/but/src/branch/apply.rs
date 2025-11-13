use anyhow::bail;
use bstr::ByteSlice;
use gitbutler_reference::RemoteRefname;
use std::{ops::Deref, str::FromStr};

/// Apply a branch to the workspace, and return the full ref name to it.
///
/// Look first in through the local references, then the remote references.
pub fn apply(
    ctx: &but_ctx::Context,
    branch_name: &str,
    json: bool,
) -> anyhow::Result<but_api::json::Reference> {
    let legacy_project = &ctx.legacy_project;
    let ctx = ctx.legacy_ctx()?;
    let repo = ctx.gix_repo()?;

    let reference = if let Some(reference) = repo.try_find_reference(branch_name)? {
        // Look for the branch in the local repository
        let ref_name = gitbutler_reference::Refname::from_str(&reference.name().to_string())?;
        let remote_ref_name = reference
            .remote_ref_name(gix::remote::Direction::Push)
            .transpose()?
            .as_deref()
            .and_then(|ref_name| {
                gitbutler_reference::RemoteRefname::from_str(&ref_name.to_string()).ok()
            });

        but_api::virtual_branches::create_virtual_branch_from_branch(
            legacy_project.id,
            ref_name,
            remote_ref_name,
            None,
        )?;
        if !json {
            println!("Applied branch '{branch_name}' to workspace");
        }
        reference
    } else if let Some((remote_ref, reference)) = find_remote_reference(&repo, branch_name)? {
        let remote = remote_ref.remote();
        let name = remote_ref.branch();
        // Look for the branch in the remote references
        let ref_name =
            gitbutler_reference::Refname::from_str(&format!("refs/remotes/{remote}/{name}"))?;
        but_api::virtual_branches::create_virtual_branch_from_branch(
            legacy_project.id,
            ref_name,
            Some(remote_ref.clone()),
            None,
        )?;
        if !json {
            println!("Applied remote branch '{branch_name}' to workspace");
        }
        reference
    } else {
        bail!("Could not find branch '{branch_name}' in local repository");
    };

    Ok(reference.inner.into())
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
