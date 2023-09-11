use std::path;

use super::{
    AnnotatedCommit, Branch, BranchName, Commit, Index, Oid, Reference, Remote, Result, Tree,
    TreeBuilder,
};

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

    pub fn add_disk_alternate(&self, path: &str) -> Result<()> {
        self.0
            .odb()
            .and_then(|odb| odb.add_disk_alternate(path))
            .map_err(Into::into)
    }

    pub fn find_annotated_commit(&self, id: Oid) -> Result<AnnotatedCommit<'_>> {
        self.0
            .find_annotated_commit(id.into())
            .map(AnnotatedCommit::from)
            .map_err(Into::into)
    }

    pub fn rebase(
        &self,
        branch: Option<&AnnotatedCommit<'_>>,
        upstream: Option<&AnnotatedCommit<'_>>,
        onto: Option<&AnnotatedCommit<'_>>,
        opts: Option<&mut git2::RebaseOptions<'_>>,
    ) -> Result<git2::Rebase<'_>> {
        self.0
            .rebase(
                branch.map(|commit| commit.into()),
                upstream.map(|commit| commit.into()),
                onto.map(|commit| commit.into()),
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
        ancestor_tree: &Tree<'_>,
        our_tree: &Tree<'_>,
        their_tree: &Tree<'_>,
    ) -> Result<Index> {
        self.0
            .merge_trees(
                ancestor_tree.into(),
                our_tree.into(),
                their_tree.into(),
                None,
            )
            .map(Index::from)
            .map_err(Into::into)
    }

    pub fn diff_tree_to_tree(
        &self,
        old_tree: Option<&Tree<'_>>,
        new_tree: Option<&Tree<'_>>,
        opts: Option<&mut git2::DiffOptions>,
    ) -> Result<git2::Diff<'_>> {
        self.0
            .diff_tree_to_tree(
                old_tree.map(|tree| tree.into()),
                new_tree.map(|tree| tree.into()),
                opts,
            )
            .map_err(Into::into)
    }

    pub fn diff_tree_to_workdir(
        &self,
        old_tree: Option<&Tree<'_>>,
        opts: Option<&mut git2::DiffOptions>,
    ) -> Result<git2::Diff<'_>> {
        self.0
            .diff_tree_to_workdir(old_tree.map(|tree| tree.into()), opts)
            .map_err(Into::into)
    }

    pub fn reset(
        &self,
        commit: &Commit<'_>,
        kind: git2::ResetType,
        checkout: Option<&mut git2::build::CheckoutBuilder<'_>>,
    ) -> Result<()> {
        let commit: &git2::Commit = commit.into();
        self.0
            .reset(commit.as_object(), kind, checkout)
            .map_err(Into::into)
    }

    pub fn find_reference(&self, name: &str) -> Result<Reference> {
        self.0
            .find_reference(name)
            .map(Reference::from)
            .map_err(Into::into)
    }

    pub fn head(&self) -> Result<Reference> {
        self.0.head().map(Reference::from).map_err(Into::into)
    }

    pub fn find_tree(&self, id: Oid) -> Result<Tree> {
        self.0
            .find_tree(id.into())
            .map(Tree::from)
            .map_err(Into::into)
    }

    pub fn find_commit(&self, id: Oid) -> Result<Commit> {
        self.0
            .find_commit(id.into())
            .map(Commit::from)
            .map_err(Into::into)
    }

    pub fn find_blob(&self, id: Oid) -> Result<git2::Blob> {
        self.0.find_blob(id.into()).map_err(Into::into)
    }

    pub fn revwalk(&self) -> Result<git2::Revwalk> {
        self.0.revwalk().map_err(Into::into)
    }

    pub fn is_path_ignored<P: AsRef<path::Path>>(&self, path: P) -> Result<bool> {
        self.0.is_path_ignored(path).map_err(Into::into)
    }

    pub fn branches(
        &self,
        filter: Option<git2::BranchType>,
    ) -> Result<impl Iterator<Item = Result<(Branch, git2::BranchType)>>> {
        self.0
            .branches(filter)
            .map(|branches| {
                branches.map(|branch| {
                    branch
                        .map(|(branch, branch_type)| (Branch::from(branch), branch_type))
                        .map_err(Into::into)
                })
            })
            .map_err(Into::into)
    }

    pub fn index(&self) -> Result<Index> {
        self.0.index().map(Into::into).map_err(Into::into)
    }

    pub fn blob_path(&self, path: &path::Path) -> Result<Oid> {
        self.0.blob_path(path).map(Into::into).map_err(Into::into)
    }

    pub fn blob(&self, data: &[u8]) -> Result<Oid> {
        self.0.blob(data).map(Into::into).map_err(Into::into)
    }

    pub fn commit(
        &self,
        update_ref: Option<&str>,
        author: &git2::Signature<'_>,
        committer: &git2::Signature<'_>,
        message: &str,
        tree: &Tree<'_>,
        parents: &[&Commit<'_>],
    ) -> Result<Oid> {
        let parents: Vec<&git2::Commit> = parents
            .iter()
            .map(|c| c.to_owned().into())
            .collect::<Vec<_>>();
        self.0
            .commit(
                update_ref,
                author,
                committer,
                message,
                tree.into(),
                &parents,
            )
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn commit_create_buffer(
        &self,
        author: &git2::Signature<'_>,
        committer: &git2::Signature<'_>,
        message: &str,
        tree: &Tree<'_>,
        parents: &[&Commit<'_>],
    ) -> Result<git2::Buf> {
        let parents: Vec<&git2::Commit> = parents
            .iter()
            .map(|c| c.to_owned().into())
            .collect::<Vec<_>>();
        let buf = self
            .0
            .commit_create_buffer(author, committer, message, tree.into(), &parents)?;
        Ok(buf)
    }

    pub fn commit_signed(&self, commit_content: &str, signature: &str) -> Result<Oid> {
        self.0
            .commit_signed(commit_content, signature, None)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn config(&self) -> Result<git2::Config> {
        self.0.config().map_err(Into::into)
    }

    pub fn treebuilder<'repo>(&'repo self, tree: Option<&'repo Tree>) -> TreeBuilder<'repo> {
        TreeBuilder::new(self, tree)
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
            .map_err(Into::into)
    }

    pub fn branch_remote_name(&self, refname: &str) -> Result<String> {
        self.0
            .branch_remote_name(refname)
            .map(|s| s.as_str().unwrap().to_string())
            .map_err(Into::into)
    }

    pub fn branch_upstream_remote(&self, branch_name: &str) -> Result<String> {
        self.0
            .branch_upstream_remote(branch_name)
            .map(|s| s.as_str().unwrap().to_string())
            .map_err(Into::into)
    }

    pub fn statuses(
        &self,
        options: Option<&mut git2::StatusOptions>,
    ) -> Result<git2::Statuses<'_>> {
        self.0.statuses(options).map_err(Into::into)
    }

    pub fn remote_anonymous(&self, url: &str) -> Result<Remote> {
        self.0
            .remote_anonymous(url)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn find_remote(&self, name: &str) -> Result<Remote> {
        self.0.find_remote(name).map(Into::into).map_err(Into::into)
    }

    pub fn find_branch(&self, name: &BranchName) -> Result<Branch> {
        self.0
            .find_branch(
                &match name {
                    BranchName::Local(local) => local.branch().to_string(),
                    BranchName::Remote(remote) => {
                        format!("{}/{}", remote.remote(), remote.branch())
                    }
                },
                match name {
                    BranchName::Local(_) => git2::BranchType::Local,
                    BranchName::Remote(_) => git2::BranchType::Remote,
                },
            )
            .map(Into::into)
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

    pub fn checkout_index(
        &self,
        index: Option<&mut Index>,
        opts: Option<&mut git2::build::CheckoutBuilder<'_>>,
    ) -> Result<()> {
        self.0
            .checkout_index(index.map(Into::into), opts)
            .map_err(Into::into)
    }

    pub fn checkout_tree(
        &self,
        tree: &Tree<'_>,
        opts: Option<&mut git2::build::CheckoutBuilder<'_>>,
    ) -> Result<()> {
        let tree: &git2::Tree = tree.into();
        self.0
            .checkout_tree(tree.as_object(), opts)
            .map_err(Into::into)
    }

    pub fn set_head(&self, refname: &str) -> Result<()> {
        self.0.set_head(refname).map_err(Into::into)
    }

    pub fn reference(
        &self,
        name: &str,
        id: Oid,
        force: bool,
        log_message: &str,
    ) -> Result<Reference> {
        self.0
            .reference(name, id.into(), force, log_message)
            .map(Into::into)
            .map_err(Into::into)
    }

    #[cfg(test)]
    pub fn remote(&self, name: &str, url: &str) -> Result<Remote> {
        self.0.remote(name, url).map(Into::into).map_err(Into::into)
    }

    #[cfg(test)]
    pub fn references(&self) -> Result<impl Iterator<Item = Result<Reference>>> {
        self.0
            .references()
            .map(|iter| iter.map(|reference| reference.map(Into::into).map_err(Into::into)))
            .map_err(Into::into)
    }
}
