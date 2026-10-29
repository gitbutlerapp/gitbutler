use anyhow::{Context as _, Result};
use but_core::RepositoryExt as _;
use but_ctx::{Context, access::RepoShared};

/// A snapshot of the workspace at a point in time.
#[derive(Debug)]
pub struct WorkspaceState {
    /// The heads of the stacks in the workspace.
    heads: Vec<gix::ObjectId>,
    /// The base of the workspace.
    base: gix::ObjectId,
}

impl WorkspaceState {
    pub fn create(ctx: &Context, perm: &RepoShared) -> Result<Self> {
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;
        let target_base_oid = ws
            .target_commit
            .as_ref()
            .map(|target| target.commit_id)
            .context("failed to get target base oid")?;
        let head_oids = ws
            .stacks
            .iter()
            .map(|stack| stack.tip_skip_empty().unwrap_or(target_base_oid))
            .collect::<Vec<_>>();
        Self::create_from_heads_and_target(&repo, &head_oids, target_base_oid)
    }

    pub fn create_from_heads_and_target(
        repo: &gix::Repository,
        head_oids: &[gix::ObjectId],
        target_base_oid: gix::ObjectId,
    ) -> Result<Self> {
        let heads = head_oids
            .iter()
            .map(|head| -> Result<gix::ObjectId> {
                let commit = but_core::Commit::try_from(repo.find_commit(*head)?)?;
                Ok(commit.tree_id_or_auto_resolution()?.detach())
            })
            .collect::<Result<Vec<_>>>()?;

        let base_tree_id = if head_oids.is_empty() {
            but_core::Commit::try_from(repo.find_commit(target_base_oid)?)?
                .tree_id_or_auto_resolution()?
                .detach()
        } else if head_oids.len() == 1 {
            but_core::Commit::try_from(
                repo.find_commit(*head_oids.first().expect("Head oids is len 1"))?,
            )?
            .tree_id_or_auto_resolution()?
            .detach()
        } else {
            but_core::Commit::try_from(
                repo.merge_base_octopus(head_oids.iter().cloned())?
                    .context("Could not find a merge-base for the workspace heads")?
                    .object()?
                    .into_commit(),
            )?
            .tree_id_or_auto_resolution()?
            .detach()
        };

        Ok(WorkspaceState {
            heads,
            base: base_tree_id,
        })
    }
}

/// Take the changes on top of one workspace and return what they would look
/// like if they were on top of the new workspace, retaining conflicts for the caller.
pub fn move_tree_between_workspaces<'repo>(
    repo: &'repo gix::Repository,
    tree: gix::ObjectId,
    old: &WorkspaceState,
    new: &WorkspaceState,
) -> Result<gix::merge::tree::Outcome<'repo>> {
    let old_workspace = merge_workspace(repo, old)?;
    let new_workspace = merge_workspace(repo, new)?;
    Ok(repo.merge_trees(
        old_workspace,
        tree,
        new_workspace,
        gix::merge::blob::builtin_driver::text::Labels {
            ancestor: Some("ancestor".into()),
            ..repo.default_merge_labels()
        },
        repo.tree_merge_options()?,
    )?)
}

/// Octopus merge using gix, which correctly resolves adjacent-hunk changes that git2 treats as conflicts.
/// Takes N trees and a base tree and merges all the heads together with respect to the given base.
///
/// If there are no heads provided, the base will be returned.
fn merge_workspace(repo: &gix::Repository, workspace: &WorkspaceState) -> Result<gix::ObjectId> {
    if workspace.heads.is_empty() {
        return Ok(workspace.base);
    } else if workspace.heads.len() == 1 {
        return Ok(*workspace.heads.first().expect("List is length 1"));
    }

    let mut output = workspace.base;
    let base = workspace.base;

    let (merge_options, conflict_kind) = repo.merge_options_fail_fast()?;

    for head in &workspace.heads {
        let mut merge = repo.merge_trees(
            base,
            output,
            *head,
            repo.default_merge_labels(),
            merge_options.clone(),
        )?;

        if merge.has_unresolved_conflicts(conflict_kind) {
            anyhow::bail!("merge conflict when computing workspace tree");
        }
        output = merge.tree.write()?.detach();
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use gix::object::tree::EntryKind;

    use super::*;

    fn tree(repo: &gix::Repository, files: &[(&str, &str)]) -> Result<gix::ObjectId> {
        let mut tree = repo.empty_tree().edit()?;
        for (path, contents) in files {
            tree.upsert(
                *path,
                EntryKind::Blob,
                repo.write_blob(contents.as_bytes())?.detach(),
            )?;
        }
        Ok(tree.write()?.detach())
    }

    #[test]
    fn merges_separated_hunks_from_both_sides() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let repo = gix::init(temp.path())?;
        let base = tree(
            &repo,
            &[(
                "file",
                "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n",
            )],
        )?;
        let a = tree(
            &repo,
            &[(
                "file",
                "a1\na2\na3\na4\na5\n6\n7\n8\n9\n10\na11\na12\na13\na14\na15\n",
            )],
        )?;
        let b = tree(
            &repo,
            &[(
                "file",
                "1\n2\n3\n4\n5\n6\nb7\nb8\nb9\n10\n11\n12\n13\n14\n15\n",
            )],
        )?;
        let expected = tree(
            &repo,
            &[(
                "file",
                "a1\na2\na3\na4\na5\n6\nb7\nb8\nb9\n10\na11\na12\na13\na14\na15\n",
            )],
        )?;

        assert_eq!(
            merge_workspace(
                &repo,
                &WorkspaceState {
                    heads: vec![a, b],
                    base,
                },
            )?,
            expected,
            "separated changes from both stacks are merged"
        );
        Ok(())
    }

    #[test]
    fn merges_diverged_stacks_against_their_shared_base() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let repo = gix::init(temp.path())?;
        let base = tree(&repo, &[("a", "a\n"), ("b", "b\n"), ("c", "c\n")])?;
        let c = tree(&repo, &[("x", "x\n"), ("y", "y\n"), ("c", "c\n")])?;
        let d = tree(&repo, &[("a", "a\n"), ("b", "b\n"), ("z", "z\n")])?;
        let expected = tree(&repo, &[("x", "x\n"), ("y", "y\n"), ("z", "z\n")])?;

        assert_eq!(
            merge_workspace(
                &repo,
                &WorkspaceState {
                    heads: vec![c, d],
                    base,
                },
            )?,
            expected,
            "both stacks are merged against their shared base"
        );
        Ok(())
    }
}
