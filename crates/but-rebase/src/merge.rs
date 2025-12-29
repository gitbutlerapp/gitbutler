use anyhow::{Result, anyhow, bail};
use bstr::{BString, ByteSlice};
use but_core::RepositoryExt;
use but_core::commit::TreeKind;
use gix::prelude::ObjectIdExt;

use crate::commit::DateMode;

/// Perform a three-base merge for each of the parents in `target_merge_commit` which serves as template for the merge.
/// This means that after merging, we will use it unchanged to create a new, possibly signed commit, after adjusting its
/// tree to point to the merge result of its parents.
/// If `target_merge_commit` only has two parents, it will be a normal merge.
///
/// The specialty of the octopus merge is that the merge-base is calculated once using [`gix::Repository::merge_base_octopus()`]
/// and then reused when merging subsequent `parent-commit^{tree}` into each other in a three-way merge, reusing the previous
/// result as *ours* until all parents are merged in.
///
/// Conflicts will cause the operation to fail, there is no hiding of conflicts as merge commits typically are
/// workspace tips which are implicit in the application. For consistency, there is no special treatment of merge-commits
/// which are part of the branches or stacks.
///
/// ### About Signing
///
/// Merges are special, and we will *not* sign it if it wasn't yet signed. That way workspace commits will naturally
/// remain unsigned.
/// However, if we re-merge a commit that was signed before it's likely a user-commit that should be treated accordingly.
/// Thanks to this logic, the caller shouldn't have to steer signing.
pub fn octopus(
    repo: &gix::Repository,
    mut target_merge_commit: gix::objs::Commit,
    graph: &mut gix::revwalk::Graph<
        '_,
        '_,
        gix::revwalk::graph::Commit<gix::revision::plumbing::merge_base::Flags>,
    >,
) -> Result<gix::ObjectId> {
    if target_merge_commit.parents.len() < 2 {
        bail!("An octopus merge commits must have at least two parents");
    }
    let parents_to_merge = target_merge_commit.parents.iter().copied();
    let merge_base = but_core::Commit::from_id(
        repo.merge_base_octopus_with_graph(parents_to_merge.clone(), graph)?,
    )?
    .tree_id_or_kind(TreeKind::Base)?
    .detach();
    let mut trees_to_merge = parents_to_merge
        .clone()
        .map(|commit_id| -> Result<_> {
            // TODO: as long as only cherry-picking is creating these trees, THEIRS
            //       is the original 'to_rebase'. However, if that changes we must know
            //       what created the special merge commit.
            Ok(but_core::Commit::from_id(commit_id.attach(repo))?
                .tree_id_or_kind(TreeKind::Theirs)?
                .detach())
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter();
    let mut ours = trees_to_merge.next().expect("two or more trees");
    let (merge_options, unresolved) = repo.merge_options_fail_fast()?;
    let mut successfully_merged = vec![ours];
    for tree_to_merge in trees_to_merge {
        let mut merge = repo.merge_trees(
            merge_base,
            ours,
            tree_to_merge,
            repo.default_merge_labels(),
            merge_options.clone(),
        )?;
        if merge.has_unresolved_conflicts(unresolved) {
            return Err(anyhow!(
                "Encountered conflict when merging tree {tree_to_merge}{details}",
                details = if successfully_merged.len() > 1 {
                    format!(
                        " after the trees {} were merged successfully",
                        successfully_merged
                            .iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                } else {
                    format!(" and tree {tree_to_merge}")
                }
            )
            .context(ConflictErrorContext {
                paths: merge
                    .conflicts
                    .iter()
                    .map(|c| c.ours.location().to_owned())
                    .collect(),
            }));
        }
        successfully_merged.push(tree_to_merge);
        ours = merge.tree.write()?.detach();
    }
    target_merge_commit.tree = ours;
    if but_core::commit::HeadersV2::try_from_commit(&target_merge_commit).is_none() {
        but_core::commit::HeadersV2::from_config(&repo.config_snapshot())
            .set_in_commit(&mut target_merge_commit);
    }
    if target_merge_commit
        .extra_headers()
        .pgp_signature()
        .is_some()
    {
        crate::commit::create(
            repo,
            target_merge_commit,
            DateMode::CommitterUpdateAuthorKeep,
        )
    } else {
        crate::commit::update_committer(repo, &mut target_merge_commit)?;
        Ok(repo.write_object(target_merge_commit)?.detach())
    }
}

/// A type that can be retrieved as an `anyhow` context to see if the rebase failed due to merge conflicts.
#[derive(Debug, Clone)]
pub struct ConflictErrorContext {
    /// All the paths that were involved, limited to the current location of `our` side.
    pub paths: Vec<BString>,
}

impl std::fmt::Display for ConflictErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} was/were conflicted when merging",
            self.paths
                .iter()
                .filter_map(|p| p.to_str().ok().map(|p| format!("{p:?}")))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
