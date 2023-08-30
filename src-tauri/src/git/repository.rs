use std::path;

use super::{Branch, Commit, Reference, Result, Tree};

// wrapper around git2::Repository to get control over how it's used.
pub struct Repository(git2::Repository);

impl<'a> From<&'a Repository> for &'a git2::Repository {
    fn from(repo: &'a Repository) -> Self {
        &repo.0
    }
}

impl Repository {
    pub fn init<P: AsRef<path::Path>>(path: P) -> Result<Self> {
        let inner = git2::Repository::init(path)?;
        Ok(Repository(inner))
    }

    pub fn open<P: AsRef<path::Path>>(path: P) -> Result<Self> {
        let inner = git2::Repository::open(path)?;
        Ok(Repository(inner))
    }
    pub fn init_opts<P: AsRef<path::Path>>(
        path: P,
        opts: &git2::RepositoryInitOptions,
    ) -> Result<Self> {
        let inner = git2::Repository::init_opts(path, opts)?;
        Ok(Repository(inner))
    }

    pub fn odb(&self) -> Result<git2::Odb> {
        self.0.odb()
    }

    pub fn revparse_single(&self, spec: &str) -> Result<git2::Object> {
        self.0.revparse_single(spec)
    }

    pub fn find_annotated_commit(&self, id: git2::Oid) -> Result<git2::AnnotatedCommit<'_>> {
        self.0.find_annotated_commit(id)
    }

    pub fn rebase(
        &self,
        branch: Option<&git2::AnnotatedCommit<'_>>,
        upstream: Option<&git2::AnnotatedCommit<'_>>,
        onto: Option<&git2::AnnotatedCommit<'_>>,
        opts: Option<&mut git2::RebaseOptions<'_>>,
    ) -> Result<git2::Rebase<'_>> {
        self.0.rebase(branch, upstream, onto, opts)
    }

    pub fn merge_base(&self, one: git2::Oid, two: git2::Oid) -> Result<git2::Oid> {
        self.0.merge_base(one, two)
    }

    pub fn merge_trees(
        &self,
        ancestor_tree: &Tree<'_>,
        our_tree: &Tree<'_>,
        their_tree: &Tree<'_>,
    ) -> Result<git2::Index> {
        self.0.merge_trees(
            ancestor_tree.into(),
            our_tree.into(),
            their_tree.into(),
            None,
        )
    }

    pub fn diff_tree_to_tree(
        &self,
        old_tree: Option<&Tree<'_>>,
        new_tree: Option<&Tree<'_>>,
        opts: Option<&mut git2::DiffOptions>,
    ) -> Result<git2::Diff<'_>> {
        self.0.diff_tree_to_tree(
            old_tree.map(|tree| tree.into()),
            new_tree.map(|tree| tree.into()),
            opts,
        )
    }

    pub fn diff_tree_to_workdir(
        &self,
        old_tree: Option<&Tree<'_>>,
        opts: Option<&mut git2::DiffOptions>,
    ) -> Result<git2::Diff<'_>> {
        self.0
            .diff_tree_to_workdir(old_tree.map(|tree| tree.into()), opts)
    }

    pub fn reset(
        &self,
        commit: &Commit<'_>,
        kind: git2::ResetType,
        checkout: Option<&mut git2::build::CheckoutBuilder<'_>>,
    ) -> Result<()> {
        let commit: &git2::Commit = commit.into();
        self.0.reset(commit.as_object(), kind, checkout)
    }

    pub fn find_reference(&self, name: &str) -> Result<Reference> {
        self.0.find_reference(name).map(Reference::from)
    }

    pub fn head(&self) -> Result<Reference> {
        self.0.head().map(Reference::from)
    }

    pub fn find_tree(&self, id: git2::Oid) -> Result<Tree> {
        self.0.find_tree(id).map(Tree::from)
    }

    pub fn find_commit(&self, id: git2::Oid) -> Result<Commit> {
        self.0.find_commit(id).map(Commit::from)
    }

    pub fn find_blob(&self, id: git2::Oid) -> Result<git2::Blob> {
        self.0.find_blob(id)
    }

    pub fn revwalk(&self) -> Result<git2::Revwalk> {
        self.0.revwalk()
    }

    pub fn is_path_ignored<P: AsRef<path::Path>>(&self, path: P) -> Result<bool> {
        self.0.is_path_ignored(path)
    }

    pub fn branches(
        &self,
        filter: Option<git2::BranchType>,
    ) -> Result<impl Iterator<Item = Result<(git2::Branch, git2::BranchType)>>> {
        self.0.branches(filter)
    }

    pub fn index(&self) -> Result<git2::Index> {
        self.0.index()
    }

    pub fn blob_path(&self, path: &path::Path) -> Result<git2::Oid> {
        self.0.blob_path(path)
    }

    pub fn blob(&self, data: &[u8]) -> Result<git2::Oid> {
        self.0.blob(data)
    }

    pub fn commit(
        &self,
        update_ref: Option<&str>,
        author: &git2::Signature<'_>,
        committer: &git2::Signature<'_>,
        message: &str,
        tree: &Tree<'_>,
        parents: &[&Commit<'_>],
    ) -> Result<git2::Oid> {
        let parents: Vec<&git2::Commit> = parents
            .iter()
            .map(|c| c.to_owned().into())
            .collect::<Vec<_>>();
        self.0.commit(
            update_ref,
            author,
            committer,
            message,
            tree.into(),
            &parents,
        )
    }

    pub fn config(&self) -> Result<git2::Config> {
        self.0.config()
    }

    pub fn treebuilder(&self, tree: Option<&Tree>) -> Result<git2::TreeBuilder> {
        self.0.treebuilder(tree.map(|t| t.into()))
    }

    pub fn path(&self) -> &path::Path {
        self.0.path()
    }

    pub fn workdir(&self) -> Option<&path::Path> {
        self.0.workdir()
    }

    pub fn branch_upstream_name(&self, branch_name: &str) -> Result<String> {
        self.0
            .branch_upstream_name(branch_name)
            .map(|s| s.as_str().unwrap().to_string())
    }

    pub fn branch_remote_name(&self, refname: &str) -> Result<String> {
        self.0
            .branch_remote_name(refname)
            .map(|s| s.as_str().unwrap().to_string())
    }

    pub fn branch_upstream_remote(&self, branch_name: &str) -> Result<String> {
        self.0
            .branch_upstream_remote(branch_name)
            .map(|s| s.as_str().unwrap().to_string())
    }

    pub fn statuses(
        &self,
        options: Option<&mut git2::StatusOptions>,
    ) -> Result<git2::Statuses<'_>> {
        self.0.statuses(options)
    }

    pub fn remote_anonymous(&self, url: &str) -> Result<git2::Remote> {
        self.0.remote_anonymous(url)
    }

    pub fn find_remote(&self, name: &str) -> Result<git2::Remote> {
        self.0.find_remote(name)
    }

    pub fn find_branch(&self, name: &str, branch_type: git2::BranchType) -> Result<Branch> {
        self.0.find_branch(name, branch_type).map(Branch::from)
    }

    pub fn refname_to_id(&self, name: &str) -> Result<git2::Oid> {
        self.0.refname_to_id(name)
    }

    pub fn checkout_head(&self, opts: Option<&mut git2::build::CheckoutBuilder<'_>>) -> Result<()> {
        self.0.checkout_head(opts)
    }

    pub fn checkout_index(
        &self,
        index: Option<&mut git2::Index>,
        opts: Option<&mut git2::build::CheckoutBuilder<'_>>,
    ) -> Result<()> {
        self.0.checkout_index(index, opts)
    }

    pub fn checkout_tree(
        &self,
        tree: &Tree<'_>,
        opts: Option<&mut git2::build::CheckoutBuilder<'_>>,
    ) -> Result<()> {
        let tree: &git2::Tree = tree.into();
        self.0.checkout_tree(tree.as_object(), opts)
    }

    pub fn set_head(&self, refname: &str) -> Result<()> {
        self.0.set_head(refname)
    }

    pub fn reference(
        &self,
        name: &str,
        id: git2::Oid,
        force: bool,
        log_message: &str,
    ) -> Result<Reference> {
        self.0
            .reference(name, id, force, log_message)
            .map(Reference::from)
    }

    #[cfg(test)]
    pub fn remote(&self, name: &str, url: &str) -> Result<git2::Remote> {
        self.0.remote(name, url)
    }

    #[cfg(test)]
    pub fn references(&self) -> Result<impl Iterator<Item = Result<super::Reference>>> {
        self.0
            .references()
            .map(|iter| iter.map(|reference| reference.map(Reference::from)))
    }
}
