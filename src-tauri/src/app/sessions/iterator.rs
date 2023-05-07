use anyhow::{Context, Result};

use crate::app::reader::{CommitReader, Reader};

use super::{cache, Session, SessionError};

pub struct SessionsIterator<'iterator> {
    git_repository: &'iterator git2::Repository,
    iter: git2::Revwalk<'iterator>,
}

impl<'iterator> SessionsIterator<'iterator> {
    pub(crate) fn new(git_repository: &'iterator git2::Repository) -> Result<Self> {
        let mut iter = git_repository
            .revwalk()
            .context("failed to create revwalk")?;
        iter.push_head().context("failed to push HEAD to revwalk")?;
        iter.set_sorting(git2::Sort::TOPOLOGICAL)
            .context("failed to set sorting")?;
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
                let commit = match self.git_repository.find_commit(oid) {
                    Result::Ok(commit) => commit,
                    Err(err) => return Some(Err(err.into())),
                };

                if commit.parent_count() == 0 {
                    // skip initial commit, as it's impossible to get a list of files from it
                    // it's only used to bootstrap the history
                    return None;
                }

                let commit_reader = match CommitReader::from_commit(self.git_repository, commit) {
                    Result::Ok(commit_reader) => commit_reader,
                    Err(err) => return Some(Err(err)),
                };
                let session = match Session::try_from(commit_reader) {
                    Result::Ok(session) => session,
                    Err(SessionError::NoSession) => return None,
                    Err(err) => return Some(Err(err.into())),
                };
                cache::set_hash_mapping(&session.id, &oid);
                Some(Ok(session))
            }
            Some(Err(err)) => Some(Err(err.into())),
            None => None,
        }
    }
}

pub struct SessionsIdsIterator<'iterator> {
    git_repository: &'iterator git2::Repository,
    iter: git2::Revwalk<'iterator>,
}

impl<'iterator> SessionsIdsIterator<'iterator> {
    pub(crate) fn new(git_repository: &'iterator git2::Repository) -> Result<Self> {
        let mut iter = git_repository
            .revwalk()
            .context("failed to create revwalk")?;
        iter.push_head().context("failed to push HEAD to revwalk")?;
        iter.set_sorting(git2::Sort::TOPOLOGICAL)
            .context("failed to set sorting")?;
        Ok(Self {
            git_repository,
            iter,
        })
    }
}

impl<'iterator> Iterator for SessionsIdsIterator<'iterator> {
    type Item = Result<(git2::Oid, String)>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(Result::Ok(oid)) => {
                let commit = match self.git_repository.find_commit(oid) {
                    Result::Ok(commit) => commit,
                    Err(err) => return Some(Err(err.into())),
                };
                let commit_reader = match CommitReader::from_commit(self.git_repository, commit) {
                    Result::Ok(commit_reader) => commit_reader,
                    Err(err) => return Some(Err(err)),
                };
                match commit_reader.read_to_string("session/meta/id") {
                    Ok(sid) => {
                        cache::set_hash_mapping(&sid, &oid);
                        Some(Ok((oid, sid)))
                    }
                    Err(e) => Some(Err(e.into())),
                }
            }
            Some(Err(err)) => Some(Err(err.into())),
            None => None,
        }
    }
}
