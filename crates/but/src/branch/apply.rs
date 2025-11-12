use std::{ops::Deref, str::FromStr};

use bstr::ByteSlice;

/// Apply a branch to the workspace
///
/// Look first in through the local references, then the remote references.
pub fn apply(project: &gitbutler_project::Project, branch_name: &str, json: bool) -> anyhow::Result<()> {
    let repo = project.open()?;

    if let Some(reference) = (&repo).try_find_reference(branch_name)? {
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
            project.id,
            ref_name,
            remote_ref_name,
            None,
        )?;
        if !json {
            println!("Applied branch '{branch_name}' to workspace");
        }
    } else if let Some(remote_ref) = find_remote_reference(&repo, branch_name)? {
        let remote = remote_ref.remote();
        let name = remote_ref.branch();
        // Look for the branch in the remote references
        let ref_name =
            gitbutler_reference::Refname::from_str(&format!("refs/remotes/{remote}/{name}"))?;
        but_api::virtual_branches::create_virtual_branch_from_branch(
            project.id,
            ref_name,
            Some(remote_ref),
            None,
        )?;
        if !json {
            println!("Applied remote branch '{branch_name}' to workspace");
        }
    } else if !json {
        println!("Could not find branch '{branch_name}' in local repository");
    }

    Ok(())
}

fn find_remote_reference(
    repo: &gix::Repository,
    branch_name: &str,
) -> anyhow::Result<Option<gitbutler_reference::RemoteRefname>> {
    for remote in repo.remote_names().iter().map(|r| r.deref()) {
        let remote_ref_name = gitbutler_reference::RemoteRefname::new(remote.to_str()?, branch_name);
        if repo
            .try_find_reference(&remote_ref_name.fullname())?
            .is_some()
        {
            return Ok(Some(remote_ref_name));
        }
    }
    Ok(None)
}
