use anyhow::{Context, Result};

use crate::{git, reader};

use super::{Session, SessionError};

pub struct SessionsIterator<'iterator> {
    git_repository: &'iterator git::Repository,
    iter: git2::Revwalk<'iterator>,
}

impl<'iterator> SessionsIterator<'iterator> {
    pub(crate) fn new(git_repository: &'iterator git::Repository) -> Result<Self> {
        let mut iter = git_repository
            .revwalk()
            .context("failed to create revwalk")?;

        iter.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)
            .context("failed to set sorting")?;

        let branches = git_repository.branches(None)?;
        for branch in branches {
            let (branch, _) = branch.context("failed to get branch")?;
            iter.push(branch.peel_to_commit()?.id().into())
                .with_context(|| format!("failed to push branch {:?}", branch.name()))?;
        }

        Ok(Self {
            git_repository,
            iter,
        })
    }
}

impl<'iterator> Iterator for SessionsIterator<'iterator> {
    type Item = Result<Session>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(Result::Ok(oid)) => {
                let commit = match self.git_repository.find_commit(oid.into()) {
                    Result::Ok(commit) => commit,
                    Err(err) => return Some(Err(err.into())),
                };

                if commit.parent_count() == 0 {
                    // skip initial commit, as it's impossible to get a list of files from it
                    // it's only used to bootstrap the history
                    return self.next();
                }

                let commit_reader = match reader::Reader::from_commit(self.git_repository, &commit)
                {
                    Result::Ok(commit_reader) => commit_reader,
                    Err(err) => return Some(Err(err)),
                };
                let session = match Session::try_from(&commit_reader) {
                    Result::Ok(session) => session,
                    Err(SessionError::NoSession) => return None,
                    Err(err) => return Some(Err(err.into())),
                };
                Some(Ok(session))
            }
            Some(Err(err)) => Some(Err(err.into())),
            None => None,
        }
    }
}
