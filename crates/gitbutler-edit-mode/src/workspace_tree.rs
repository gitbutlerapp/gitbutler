use anyhow::{Context as _, Result};
use but_core::RepositoryExt as _;
use but_ctx::{Context, access::RepoExclusive};
use but_oxidize::ObjectIdExt as _;
use git2::build::CheckoutBuilder;

#[derive(Debug)]
pub(crate) struct WorkspaceTrees {
    head_trees: Vec<gix::ObjectId>,
    base_tree: gix::ObjectId,
}

impl WorkspaceTrees {
    pub(crate) fn from_workspace(
        repo: &gix::Repository,
        ws: &but_graph::Workspace,
    ) -> Result<Self> {
        let target = ws
            .target_commit
            .as_ref()
            .map(|target| target.commit_id)
            .context("failed to get target base oid")?;
        let head_ids = ws
            .stacks
            .iter()
            .map(|stack| stack.tip_skip_empty().unwrap_or(target))
            .collect::<Vec<_>>();
        let tree = |commit_id: gix::ObjectId| -> Result<gix::ObjectId> {
            let commit = but_core::Commit::try_from(repo.find_commit(commit_id)?)?;
            Ok(commit.tree_id_or_auto_resolution()?.detach())
        };

        let head_trees = head_ids
            .iter()
            .map(|head| tree(*head))
            .collect::<Result<_>>()?;
        let base_tree = match head_ids.as_slice() {
            [] => tree(target)?,
            [head] => tree(*head)?,
            _ => tree(
                repo.merge_base_octopus(head_ids.iter().copied())?
                    .context("Could not find a merge-base for the workspace heads")?
                    .detach(),
            )?,
        };
        Ok(WorkspaceTrees {
            head_trees,
            base_tree,
        })
    }

    fn merge(&self, repo: &gix::Repository) -> Result<gix::ObjectId> {
        match self.head_trees.as_slice() {
            [] => return Ok(self.base_tree),
            [head] => return Ok(*head),
            _ => {}
        }

        let (merge_options, conflict_kind) = repo.merge_options_fail_fast()?;
        let mut output = self.base_tree;
        for head in &self.head_trees {
            let mut merge = repo.merge_trees(
                self.base_tree,
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
}

pub(crate) fn checkout_uncommitted_changes(
    ctx: &Context,
    old: &WorkspaceTrees,
    new: &WorkspaceTrees,
    uncommitted: gix::ObjectId,
    _perm: &mut RepoExclusive,
) -> Result<()> {
    let gix_repo = ctx.clone_repo_for_merging()?;
    #[expect(
        deprecated,
        reason = "git2 builds the conflicted index that it then checks out"
    )]
    let repo = &*ctx.git2_repo.get()?;
    let mut index = repo.merge_trees(
        &repo.find_tree(old.merge(&gix_repo)?.to_git2())?,
        &repo.find_tree(uncommitted.to_git2())?,
        &repo.find_tree(new.merge(&gix_repo)?.to_git2())?,
        None,
    )?;
    repo.checkout_index(
        Some(&mut index),
        Some(
            CheckoutBuilder::new()
                .force()
                .remove_untracked(true)
                .conflict_style_diff3(true),
        ),
    )?;
    Ok(())
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
            WorkspaceTrees {
                head_trees: vec![a, b],
                base_tree: base,
            }
            .merge(&repo)?,
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
            WorkspaceTrees {
                head_trees: vec![c, d],
                base_tree: base,
            }
            .merge(&repo)?,
            expected,
            "both stacks are merged against their shared base"
        );
        Ok(())
    }
}
