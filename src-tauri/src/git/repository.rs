use std::path;

// wrapper around git2::Repository to get control over how it's used.
pub struct Repository(git2::Repository);

type Result<T> = std::result::Result<T, git2::Error>;

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

    pub fn inner(&self) -> &git2::Repository {
        &self.0
    }

    pub fn revparse_single(&self, spec: &str) -> Result<git2::Object> {
        self.0.revparse_single(spec)
    }

    pub fn find_reference(&self, name: &str) -> Result<git2::Reference> {
        self.0.find_reference(name)
    }

    pub fn head(&self) -> Result<git2::Reference> {
        self.0.head()
    }

    pub fn find_tree(&self, id: git2::Oid) -> Result<git2::Tree> {
        self.0.find_tree(id)
    }

    pub fn find_commit(&self, id: git2::Oid) -> Result<git2::Commit> {
        self.0.find_commit(id)
    }

    pub fn find_blob(&self, id: git2::Oid) -> Result<git2::Blob> {
        self.0.find_blob(id)
    }

    pub fn revwalk(&self) -> Result<git2::Revwalk> {
        self.0.revwalk()
    }

    pub fn branches(&self, filter: Option<git2::BranchType>) -> Result<git2::Branches> {
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
        tree: &git2::Tree<'_>,
        parents: &[&git2::Commit<'_>],
    ) -> Result<git2::Oid> {
        self.0
            .commit(update_ref, author, committer, message, tree, parents)
    }

    pub fn config(&self) -> Result<git2::Config> {
        self.0.config()
    }

    pub fn treebuilder(&self, tree: Option<&git2::Tree>) -> Result<git2::TreeBuilder> {
        self.0.treebuilder(tree)
    }

    pub fn path(&self) -> &path::Path {
        self.0.path()
    }

    pub fn remote_anonymous(&self, url: &str) -> Result<git2::Remote> {
        self.0.remote_anonymous(url)
    }

    #[cfg(test)]
    pub fn refname_to_id(&self, name: &str) -> Result<git2::Oid> {
        self.0.refname_to_id(name)
    }

    #[cfg(test)]
    pub fn checkout_head(&self, opts: Option<&mut git2::build::CheckoutBuilder<'_>>) -> Result<()> {
        self.0.checkout_head(opts)
    }

    #[cfg(test)]
    pub fn set_head(&self, refname: &str) -> Result<()> {
        self.0.set_head(refname)
    }

    #[cfg(test)]
    pub fn reference(
        &self,
        name: &str,
        id: git2::Oid,
        force: bool,
        log_message: &str,
    ) -> Result<git2::Reference> {
        self.0.reference(name, id, force, log_message)
    }

    #[cfg(test)]
    pub fn remote(&self, name: &str, url: &str) -> Result<git2::Remote> {
        self.0.remote(name, url)
    }

    #[cfg(test)]
    pub fn references(&self) -> Result<git2::References> {
        self.0.references()
    }
}
