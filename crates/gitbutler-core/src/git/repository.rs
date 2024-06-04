use super::{Oid, Refname, Result, Url};
use git2::Submodule;
use git2_hooks::HookResult;
use std::{path::Path, str};

// wrapper around git2::Repository to get control over how it's used.
pub struct Repository(git2::Repository);

impl<'a> From<&'a Repository> for &'a git2::Repository {
    fn from(repo: &'a Repository) -> Self {
        &repo.0
    }
}

impl From<git2::Repository> for Repository {
    fn from(repo: git2::Repository) -> Self {
        Self(repo)
    }
}

impl Repository {
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self> {
        let inner = git2::Repository::init(path)?;
        Ok(Repository(inner))
    }

    pub fn init_opts<P: AsRef<Path>>(path: P, opts: &git2::RepositoryInitOptions) -> Result<Self> {
        let inner = git2::Repository::init_opts(path, opts)?;
        Ok(Repository(inner))
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let inner = git2::Repository::open(path)?;
        Ok(Repository(inner))
    }

    pub fn add_submodule<P: AsRef<Path>>(&self, url: &Url, path: P) -> Result<Submodule<'_>> {
        self.0
            .submodule(&url.to_string(), path.as_ref(), false)
            .map_err(Into::into)
    }

    pub fn rebase(
        &self,
        branch_oid: Option<Oid>,
        upstream_oid: Option<Oid>,
        onto_oid: Option<Oid>,
        opts: Option<&mut git2::RebaseOptions<'_>>,
    ) -> Result<git2::Rebase<'_>> {
        let annotated_branch = if let Some(branch) = branch_oid {
            Some(self.0.find_annotated_commit(branch.into())?)
        } else {
            None
        };

        let annotated_upstream = if let Some(upstream) = upstream_oid {
            Some(self.0.find_annotated_commit(upstream.into())?)
        } else {
            None
        };

        let annotated_onto = if let Some(onto) = onto_oid {
            Some(self.0.find_annotated_commit(onto.into())?)
        } else {
            None
        };

        self.0
            .rebase(
                annotated_branch.as_ref(),
                annotated_upstream.as_ref(),
                annotated_onto.as_ref(),
                opts,
            )
            .map_err(Into::into)
    }

    pub fn merge_base(&self, one: Oid, two: Oid) -> Result<Oid> {
        self.0
            .merge_base(one.into(), two.into())
            .map(Oid::from)
            .map_err(Into::into)
    }

    pub fn merge_trees(
        &self,
        ancestor_tree: &git2::Tree<'_>,
        our_tree: &git2::Tree<'_>,
        their_tree: &git2::Tree<'_>,
    ) -> Result<git2::Index> {
        self.0
            .merge_trees(ancestor_tree, our_tree, their_tree, None)
            .map_err(Into::into)
    }

    pub fn diff_tree_to_tree(
        &self,
        old_tree: Option<&git2::Tree<'_>>,
        new_tree: Option<&git2::Tree<'_>>,
        opts: Option<&mut git2::DiffOptions>,
    ) -> Result<git2::Diff<'_>> {
        self.0
            .diff_tree_to_tree(old_tree, new_tree, opts)
            .map_err(Into::into)
    }

    pub fn diff_tree_to_workdir(
        &self,
        old_tree: Option<&git2::Tree<'_>>,
        opts: Option<&mut git2::DiffOptions>,
    ) -> Result<git2::Diff<'_>> {
        if let Ok(mut index) = self.0.index() {
            index.update_all(vec!["*"], None)?;
        }
        self.0
            .diff_tree_to_workdir_with_index(old_tree, opts)
            .map_err(Into::into)
    }

    pub fn reset(
        &self,
        commit: &git2::Commit<'_>,
        kind: git2::ResetType,
        checkout: Option<&mut git2::build::CheckoutBuilder<'_>>,
    ) -> Result<()> {
        let commit: &git2::Commit = commit;
        self.0
            .reset(commit.as_object(), kind, checkout)
            .map_err(Into::into)
    }

    pub fn find_reference(&self, name: &Refname) -> Result<git2::Reference> {
        self.0.find_reference(&name.to_string()).map_err(Into::into)
    }

    pub fn head(&self) -> Result<git2::Reference> {
        self.0.head().map_err(Into::into)
    }

    pub fn find_tree(&self, id: Oid) -> Result<git2::Tree> {
        self.0.find_tree(id.into()).map_err(Into::into)
    }

    pub fn find_commit(&self, id: Oid) -> Result<git2::Commit> {
        self.0.find_commit(id.into()).map_err(Into::into)
    }

    pub fn revwalk(&self) -> Result<git2::Revwalk> {
        self.0.revwalk().map_err(Into::into)
    }

    pub fn is_path_ignored<P: AsRef<Path>>(&self, path: P) -> Result<bool> {
        self.0.is_path_ignored(path).map_err(Into::into)
    }

    pub fn branches(
        &self,
        filter: Option<git2::BranchType>,
    ) -> Result<impl Iterator<Item = Result<git2::Branch>>> {
        self.0
            .branches(filter)
            .map(|branches| {
                branches.map(|branch| branch.map(|(branch, _)| branch).map_err(Into::into))
            })
            .map_err(Into::into)
    }

    pub fn index(&self) -> Result<git2::Index> {
        self.0.index().map_err(Into::into)
    }

    pub fn index_size(&self) -> Result<usize> {
        Ok(self.0.index()?.len())
    }

    pub fn blob_path<P: AsRef<Path>>(&self, path: P) -> Result<Oid> {
        self.0
            .blob_path(path.as_ref())
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn cherry_pick(&self, base: &git2::Commit, target: &git2::Commit) -> Result<git2::Index> {
        self.0
            .cherrypick_commit(target, base, 0, None)
            .map_err(Into::into)
    }

    pub fn blob(&self, data: &[u8]) -> Result<Oid> {
        self.0.blob(data).map(Into::into).map_err(Into::into)
    }

    pub fn config(&self) -> Result<git2::Config> {
        self.0.config().map_err(Into::into)
    }

    pub fn path(&self) -> &Path {
        self.0.path()
    }

    pub fn workdir(&self) -> Option<&Path> {
        self.0.workdir()
    }

    pub fn statuses(
        &self,
        options: Option<&mut git2::StatusOptions>,
    ) -> Result<git2::Statuses<'_>> {
        self.0.statuses(options).map_err(Into::into)
    }

    pub fn remote_anonymous(&self, url: &super::Url) -> Result<git2::Remote> {
        self.0
            .remote_anonymous(&url.to_string())
            .map_err(Into::into)
    }

    pub fn find_remote(&self, name: &str) -> Result<git2::Remote> {
        self.0.find_remote(name).map_err(Into::into)
    }

    pub fn find_branch(&self, name: &Refname) -> Result<git2::Branch> {
        self.0
            .find_branch(
                &name.simple_name(),
                match name {
                    Refname::Virtual(_) | Refname::Local(_) | Refname::Other(_) => {
                        git2::BranchType::Local
                    }
                    Refname::Remote(_) => git2::BranchType::Remote,
                },
            )
            .map_err(Into::into)
    }

    pub fn refname_to_id(&self, name: &str) -> Result<Oid> {
        self.0
            .refname_to_id(name)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn checkout_head(&self, opts: Option<&mut git2::build::CheckoutBuilder>) -> Result<()> {
        self.0.checkout_head(opts).map_err(Into::into)
    }

    pub fn checkout_index<'a>(&'a self, index: &'a mut git2::Index) -> CheckoutIndexBuilder {
        CheckoutIndexBuilder {
            index,
            repo: &self.0,
            checkout_builder: git2::build::CheckoutBuilder::new(),
        }
    }

    pub fn checkout_index_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut builder = git2::build::CheckoutBuilder::new();
        builder.path(path.as_ref());
        builder.force();

        let mut index = self.0.index()?;
        self.0
            .checkout_index(Some(&mut index), Some(&mut builder))?;

        Ok(())
    }

    pub fn checkout_tree<'a>(&'a self, tree: &'a git2::Tree<'a>) -> CheckoutTreeBuidler {
        CheckoutTreeBuidler {
            tree,
            repo: &self.0,
            checkout_builder: git2::build::CheckoutBuilder::new(),
        }
    }

    pub fn set_head(&self, refname: &Refname) -> Result<()> {
        self.0.set_head(&refname.to_string()).map_err(Into::into)
    }

    pub fn set_head_detached(&self, commitish: Oid) -> Result<()> {
        self.0
            .set_head_detached(commitish.into())
            .map_err(Into::into)
    }

    pub fn reference(
        &self,
        name: &Refname,
        id: Oid,
        force: bool,
        log_message: &str,
    ) -> Result<git2::Reference> {
        self.0
            .reference(&name.to_string(), id.into(), force, log_message)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn remote(&self, name: &str, url: &Url) -> Result<git2::Remote> {
        self.0.remote(name, &url.to_string()).map_err(Into::into)
    }

    pub fn references(&self) -> Result<impl Iterator<Item = Result<git2::Reference>>> {
        self.0
            .references()
            .map(|iter| iter.map(|reference| reference.map(Into::into).map_err(Into::into)))
            .map_err(Into::into)
    }

    pub fn references_glob(
        &self,
        glob: &str,
    ) -> Result<impl Iterator<Item = Result<git2::Reference>>> {
        self.0
            .references_glob(glob)
            .map(|iter| iter.map(|reference| reference.map(Into::into).map_err(Into::into)))
            .map_err(Into::into)
    }

    pub fn run_hook_pre_commit(&self) -> Result<HookResult> {
        let res = git2_hooks::hooks_pre_commit(&self.0, Some(&["../.husky"]))?;
        Ok(res)
    }

    pub fn run_hook_commit_msg(&self, msg: &mut String) -> Result<HookResult> {
        let res = git2_hooks::hooks_commit_msg(&self.0, Some(&["../.husky"]), msg)?;
        Ok(res)
    }

    pub fn run_hook_post_commit(&self) -> Result<()> {
        git2_hooks::hooks_post_commit(&self.0, Some(&["../.husky"]))?;
        Ok(())
    }

    /// Returns a list of remotes
    ///
    /// Returns `Vec<String>` instead of StringArray because StringArray cannot safly be sent between threads
    pub fn remotes(&self) -> Result<Vec<String>> {
        self.0
            .remotes()
            .map(|string_array| {
                string_array
                    .iter()
                    .filter_map(|s| s.map(String::from))
                    .collect()
            })
            .map_err(super::Error::Remotes)
    }

    pub fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        self.0.remote(name, url)?;
        Ok(())
    }
}

pub struct CheckoutTreeBuidler<'a> {
    repo: &'a git2::Repository,
    tree: &'a git2::Tree<'a>,
    checkout_builder: git2::build::CheckoutBuilder<'a>,
}

impl CheckoutTreeBuidler<'_> {
    pub fn force(&mut self) -> &mut Self {
        self.checkout_builder.force();
        self
    }

    pub fn remove_untracked(&mut self) -> &mut Self {
        self.checkout_builder.remove_untracked(true);
        self
    }

    pub fn checkout(&mut self) -> Result<()> {
        self.repo
            .checkout_tree(self.tree.as_object(), Some(&mut self.checkout_builder))
            .map_err(Into::into)
    }
}

pub struct CheckoutIndexBuilder<'a> {
    repo: &'a git2::Repository,
    index: &'a mut git2::Index,
    checkout_builder: git2::build::CheckoutBuilder<'a>,
}

impl CheckoutIndexBuilder<'_> {
    pub fn force(&mut self) -> &mut Self {
        self.checkout_builder.force();
        self
    }

    pub fn allow_conflicts(&mut self) -> &mut Self {
        self.checkout_builder.allow_conflicts(true);
        self
    }

    pub fn conflict_style_merge(&mut self) -> &mut Self {
        self.checkout_builder.conflict_style_merge(true);
        self
    }

    pub fn checkout(&mut self) -> Result<()> {
        self.repo
            .checkout_index(Some(&mut self.index), Some(&mut self.checkout_builder))
            .map_err(Into::into)
    }
}
