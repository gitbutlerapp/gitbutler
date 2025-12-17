use std::{collections::BTreeMap, path::PathBuf};

type WorktreePathByRef = BTreeMap<gix::refs::FullName, Vec<PathBuf>>;

/// State for re-use in [`safe_delete()`].
#[derive(Debug)]
pub struct SafeDelete {
    worktrees_by_ref: WorktreePathByRef,
}

/// The outcome of [`SafeDelete::delete_reference()`]
#[derive(Debug, Clone)]
pub struct SafeDeleteOutcome<'parent> {
    /// The paths to the worktrees that have the to-be-deleted reference checked out.
    pub checked_out_in_worktree_dirs: Option<&'parent [PathBuf]>,
}

impl SafeDeleteOutcome<'_> {
    /// Return `true` if the reference was deleted.
    pub fn was_deleted(&self) -> bool {
        self.checked_out_in_worktree_dirs.is_none()
    }
}

/// Lifecycle
impl SafeDelete {
    /// Create a new instance, fetching the required information from `repo`.
    pub fn new(repo: &gix::Repository) -> anyhow::Result<Self> {
        Ok(SafeDelete {
            worktrees_by_ref: gix_copies::worktree_branches(repo)?,
        })
    }
}

impl SafeDelete {
    /// Delete the reference `rn` no `HEAD` in any worktree points to `rn` directly or indirectly.
    /// Return an outcome to indicate if it was deleted or not.
    pub fn delete_reference(&self, rn: &gix::Reference) -> anyhow::Result<SafeDeleteOutcome<'_>> {
        let out = if let Some(paths) = self.worktrees_by_ref.get(&rn.inner.name) {
            Some(paths.as_slice())
        } else {
            rn.delete()?;
            None
        };

        Ok(SafeDeleteOutcome {
            checked_out_in_worktree_dirs: out,
        })
    }
}

/// This code was copied from `gix` and it should rather be exposed there.
/// Everyone needs it for safe-deletion.
// TODO(gix): expose this in gix (but find this code, it already exists there)
mod gix_copies {
    use std::collections::BTreeMap;

    use crate::branch::WorktreePathByRef;

    fn insert_head(head: Option<gix::Head<'_>>, out: &mut WorktreePathByRef) -> anyhow::Result<()> {
        if let Some((head, wd)) = head.and_then(|head| head.repo.workdir().map(|wd| (head, wd))) {
            out.entry("HEAD".try_into().expect("added first so we always have it"))
                .or_default()
                .push(wd.to_owned());
            let mut ref_chain = Vec::new();
            let mut cursor = head.try_into_referent();
            while let Some(ref_) = cursor {
                ref_chain.push(ref_.name().to_owned());
                cursor = ref_.follow().transpose()?;
            }
            for name in ref_chain {
                out.entry(name).or_default().push(wd.to_owned());
            }
        }
        Ok(())
    }

    pub fn worktree_branches(repo: &gix::Repository) -> anyhow::Result<WorktreePathByRef> {
        let mut map = BTreeMap::new();
        insert_head(repo.head().ok(), &mut map)?;
        for proxy in repo.worktrees()? {
            let repo = proxy.into_repo_with_possibly_inaccessible_worktree()?;
            insert_head(repo.head().ok(), &mut map)?;
        }
        Ok(map)
    }
}
