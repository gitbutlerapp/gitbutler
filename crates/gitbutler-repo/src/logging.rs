use anyhow::{Context, Result};

pub trait RepositoryExt {
    fn l(&self, from: git2::Oid, to: LogUntil, include_all_parents: bool)
        -> Result<Vec<git2::Oid>>;
    fn list_commits(&self, from: git2::Oid, to: git2::Oid) -> Result<Vec<git2::Commit>>;
    fn log(
        &self,
        from: git2::Oid,
        to: LogUntil,
        include_all_parents: bool,
    ) -> Result<Vec<git2::Commit>>;
}

impl RepositoryExt for git2::Repository {
    // returns a list of commit oids from the first oid to the second oid
    // if `include_all_parents` is true it will include commits from all sides of merge commits,
    // otherwise, only the first parent of each commit is considered
    fn l(
        &self,
        from: git2::Oid,
        to: LogUntil,
        include_all_parents: bool,
    ) -> Result<Vec<git2::Oid>> {
        match to {
            LogUntil::Commit(oid) => {
                let mut revwalk = self.revwalk().context("failed to create revwalk")?;
                if !include_all_parents {
                    revwalk.simplify_first_parent()?;
                }
                revwalk
                    .push(from)
                    .context(format!("failed to push {from}"))?;
                revwalk.hide(oid).context(format!("failed to hide {oid}"))?;
                revwalk.collect::<Result<Vec<_>, _>>()
            }
            LogUntil::Take(n) => {
                let mut revwalk = self.revwalk().context("failed to create revwalk")?;
                if !include_all_parents {
                    revwalk.simplify_first_parent()?;
                }
                revwalk
                    .push(from)
                    .context(format!("failed to push {from}"))?;
                revwalk.take(n).collect::<Result<Vec<_>, _>>()
            }
            LogUntil::When(cond) => {
                let mut revwalk = self.revwalk().context("failed to create revwalk")?;
                if !include_all_parents {
                    revwalk.simplify_first_parent()?;
                }
                revwalk
                    .push(from)
                    .context(format!("failed to push {from}"))?;
                let mut oids: Vec<git2::Oid> = vec![];
                for oid in revwalk {
                    let oid = oid.context("failed to get oid")?;
                    oids.push(oid);

                    let commit = self.find_commit(oid).context("failed to find commit")?;

                    if cond(&commit).context("failed to check condition")? {
                        break;
                    }
                }
                Ok(oids)
            }
            LogUntil::End => {
                let mut revwalk = self.revwalk().context("failed to create revwalk")?;
                if !include_all_parents {
                    revwalk.simplify_first_parent()?;
                }
                revwalk
                    .push(from)
                    .context(format!("failed to push {from}"))?;
                revwalk.collect::<Result<Vec<_>, _>>()
            }
        }
        .context("failed to collect oids")
    }

    fn list_commits(&self, from: git2::Oid, to: git2::Oid) -> Result<Vec<git2::Commit<'_>>> {
        Ok(self
            .l(from, LogUntil::Commit(to), false)?
            .into_iter()
            .map(|oid| self.find_commit(oid))
            .collect::<Result<Vec<_>, _>>()?)
    }

    // returns a list of commits from the first oid to the second oid
    fn log(
        &self,
        from: git2::Oid,
        to: LogUntil,
        include_all_parents: bool,
    ) -> Result<Vec<git2::Commit<'_>>> {
        self.l(from, to, include_all_parents)?
            .into_iter()
            .map(|oid| self.find_commit(oid))
            .collect::<Result<Vec<_>, _>>()
            .context("failed to collect commits")
    }
}

type OidFilter = dyn Fn(&git2::Commit) -> Result<bool>;

/// Generally, all traversals will use no particular ordering, it's implementation defined in `git2`.
pub enum LogUntil {
    /// Traverse until one sees (or gets commits older than) the given commit.
    /// Do not return that commit or anything older than that.
    Commit(git2::Oid),
    /// Traverse the given `n` commits.
    Take(usize),
    /// Traverse all commits until the given condition returns `false` for a commit.
    /// Note that this commit-id will also be returned.
    When(Box<OidFilter>),
    /// Traverse the whole graph until it is exhausted.
    End,
}
