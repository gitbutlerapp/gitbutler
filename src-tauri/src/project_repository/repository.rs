use std::{collections::HashMap, env, path};

use anyhow::{Context, Result};
use serde::Serialize;
use walkdir::WalkDir;

use crate::{git, keys, project_repository::activity, projects, reader, ssh};

use super::branch;

pub struct Repository<'repository> {
    pub(crate) git_repository: git2::Repository,
    project: &'repository projects::Project,
}

impl<'project> TryFrom<&'project projects::Project> for Repository<'project> {
    type Error = git2::Error;

    fn try_from(project: &'project projects::Project) -> std::result::Result<Self, Self::Error> {
        let git_repository = git2::Repository::open(&project.path)?;
        Ok(Self {
            git_repository,
            project,
        })
    }
}

impl<'repository> Repository<'repository> {
    pub fn path(&self) -> &path::Path {
        path::Path::new(&self.project.path)
    }

    pub fn open(project: &'repository projects::Project) -> Result<Self> {
        let git_repository = git2::Repository::open(&project.path)
            .with_context(|| format!("{}: failed to open git repository", project.path))?;
        Ok(Self {
            git_repository,
            project,
        })
    }

    pub fn get_head(&self) -> Result<git2::Reference, git2::Error> {
        let head = self.git_repository.head()?;
        Ok(head)
    }

    pub fn is_path_ignored<P: AsRef<std::path::Path>>(&self, path: P) -> Result<bool> {
        let path = path.as_ref();
        let ignored = self.git_repository.is_path_ignored(path)?;
        Ok(ignored)
    }

    pub fn get_wd_reader(&self) -> reader::DirReader {
        reader::DirReader::open(self.root().to_path_buf())
    }

    pub fn root(&self) -> &std::path::Path {
        self.git_repository.path().parent().unwrap()
    }

    pub fn git_activity(&self, start_time_ms: Option<u128>) -> Result<Vec<activity::Activity>> {
        let head_logs_path = self.git_repository.path().join("logs").join("HEAD");

        if !head_logs_path.exists() {
            return Ok(Vec::new());
        }

        let activity = std::fs::read_to_string(head_logs_path)
            .with_context(|| "failed to read HEAD logs")?
            .lines()
            .filter_map(|line| activity::parse_reflog_line(line).ok())
            .collect::<Vec<activity::Activity>>();

        let activity = if let Some(start_timestamp_ms) = start_time_ms {
            activity
                .into_iter()
                .filter(|activity| activity.timestamp_ms > start_timestamp_ms)
                .collect::<Vec<activity::Activity>>()
        } else {
            activity
        };

        Ok(activity)
    }

    fn unstaged_statuses(&self) -> Result<HashMap<String, FileStatusType>> {
        let mut options = git2::StatusOptions::new();
        options.include_untracked(true);
        options.recurse_untracked_dirs(true);
        options.include_ignored(false);
        options.show(git2::StatusShow::Workdir);

        // get the status of the repository
        let statuses = self
            .git_repository
            .statuses(Some(&mut options))
            .with_context(|| "failed to get repository status")?;

        let files = statuses
            .iter()
            .filter_map(|entry| {
                entry
                    .path()
                    .map(|path| (path.to_string(), FileStatusType::from(entry.status())))
            })
            .collect();

        Ok(files)
    }

    fn staged_statuses(&self) -> Result<HashMap<String, FileStatusType>> {
        let mut options = git2::StatusOptions::new();
        options.include_untracked(true);
        options.include_ignored(false);
        options.recurse_untracked_dirs(true);
        options.show(git2::StatusShow::Index);

        // get the status of the repository
        let statuses = self
            .git_repository
            .statuses(Some(&mut options))
            .with_context(|| "failed to get repository status")?;

        let files = statuses
            .iter()
            .filter_map(|entry| {
                entry
                    .path()
                    .map(|path| (path.to_string(), FileStatusType::from(entry.status())))
            })
            .collect();

        Ok(files)
    }

    pub fn git_status(&self) -> Result<HashMap<String, FileStatus>> {
        let staged_statuses = self.staged_statuses()?;
        let unstaged_statuses = self.unstaged_statuses()?;
        let mut statuses = HashMap::new();
        unstaged_statuses
            .iter()
            .for_each(|(path, unstaged_status_type)| {
                statuses.insert(
                    path.clone(),
                    FileStatus {
                        unstaged: Some(*unstaged_status_type),
                        staged: None,
                    },
                );
            });
        staged_statuses
            .iter()
            .for_each(|(path, stages_status_type)| {
                if let Some(status) = statuses.get_mut(path) {
                    status.staged = Some(*stages_status_type);
                } else {
                    statuses.insert(
                        path.clone(),
                        FileStatus {
                            unstaged: None,
                            staged: Some(*stages_status_type),
                        },
                    );
                }
            });

        Ok(statuses)
    }

    pub fn git_match_paths(&self, pattern: &str) -> Result<Vec<String>> {
        let workdir = self
            .git_repository
            .workdir()
            .with_context(|| "failed to get working directory")?;

        let pattern = pattern.to_lowercase();
        let mut files = vec![];
        for entry in WalkDir::new(workdir)
                    .into_iter()
                    .filter_entry(|entry| {
                        // need to remove workdir so we're not matching it
                        let relative_path = entry
                            .path()
                            .strip_prefix(workdir)
                            .unwrap()
                            .to_str()
                            .unwrap();
                        // this is to make it faster, so we dont have to traverse every directory if it is ignored by git
                        entry.path().to_str() == workdir.to_str()  // but we need to traverse the first one
                            || ((entry.file_type().is_dir() // traverse all directories if they are not ignored by git
                                || relative_path.to_lowercase().contains(&pattern)) // but only pass on files that match the regex
                                && !self.git_repository.is_path_ignored(entry.path()).unwrap_or(true))
                    })
                    .filter_map(Result::ok)
                {
                    if entry.file_type().is_file() {
                        // only save the matching files, not the directories
                        let path = entry.path();
                        let path = path
                            .strip_prefix::<&std::path::Path>(workdir.as_ref())
                            .with_context(|| {
                                format!(
                                    "failed to strip prefix from path {}",
                                    path.to_str().unwrap()
                                )
                            })?;
                        let path = path.to_str().unwrap().to_string();
                        files.push(path);
                    }
                }
        files.sort();
        Ok(files)
    }

    pub fn git_branches(&self) -> Result<Vec<branch::LocalName>> {
        self.git_repository
            .branches(Some(git2::BranchType::Local))?
            .flatten()
            .map(|(branch, _)| branch)
            .map(|branch| {
                branch::LocalName::try_from(&branch)
                    .context("failed to convert branch to local name")
            })
            .collect::<Result<Vec<branch::LocalName>>>()
    }

    pub fn git_remote_branches(&self) -> Result<Vec<branch::RemoteName>> {
        self.git_repository
            .branches(Some(git2::BranchType::Remote))?
            .flatten()
            .map(|(branch, _)| branch)
            .map(|branch| {
                branch::RemoteName::try_from(&branch)
                    .context("failed to convert branch to remote name")
            })
            .collect::<Result<Vec<branch::RemoteName>>>()
    }

    // returns a list of commit oids from the first oid to the second oid
    pub fn l(&self, from: git2::Oid, to: LogUntil) -> Result<Vec<git2::Oid>> {
        match to {
            LogUntil::Commit(oid) => {
                let mut revwalk = self
                    .git_repository
                    .revwalk()
                    .context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                revwalk
                    .hide(oid)
                    .context(format!("failed to push {}", oid))?;
                revwalk.collect::<Result<Vec<_>, _>>()
            }
            LogUntil::Take(n) => {
                let mut revwalk = self
                    .git_repository
                    .revwalk()
                    .context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                revwalk.take(n).collect::<Result<Vec<_>, _>>()
            }
            LogUntil::When(cond) => {
                let mut revwalk = self
                    .git_repository
                    .revwalk()
                    .context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                let mut oids = vec![];
                for oid in revwalk {
                    let oid = oid.context("failed to get oid")?;
                    oids.push(oid);

                    let commit = self
                        .git_repository
                        .find_commit(oid)
                        .context("failed to find commit")?;

                    if cond(&commit).context("failed to check condition")? {
                        break;
                    }
                }
                Ok(oids)
            }
            LogUntil::End => {
                let mut revwalk = self
                    .git_repository
                    .revwalk()
                    .context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                revwalk.collect::<Result<Vec<_>, _>>()
            }
        }
        .context("failed to collect oids")
    }

    // returns a list of commits from the first oid to the second oid
    pub fn log(&self, from: git2::Oid, to: LogUntil) -> Result<Vec<git2::Commit>> {
        self.l(from, to)?
            .into_iter()
            .map(|oid| self.git_repository.find_commit(oid))
            .collect::<Result<Vec<_>, _>>()
            .context("failed to collect commits")
    }

    // returns the number of commits between the first oid to the second oid
    pub fn distance(&self, from: git2::Oid, to: git2::Oid) -> Result<u32> {
        let oids = self.l(from, LogUntil::Commit(to))?;
        Ok(oids.len().try_into()?)
    }

    pub fn git_switch_branch(&self, branch: &str) -> Result<()> {
        let branch = self
            .git_repository
            .find_branch(branch, git2::BranchType::Local)?;
        let branch = branch.into_reference();
        self.git_repository
            .set_head(branch.name().unwrap())
            .context("failed to set head")?;
        self.git_repository
            .checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))
            .context("failed to checkout head")?;
        Ok(())
    }

    pub fn git_stage_files<P: AsRef<std::path::Path>>(&self, paths: Vec<P>) -> Result<()> {
        let mut index = self.git_repository.index()?;
        for path in paths {
            let path = path.as_ref();
            // to "stage" a file means to:
            // - remove it from the index if file is deleted
            // - overwrite it in the index otherwise
            if !std::path::Path::new(&self.project.path).join(path).exists() {
                index.remove_path(path).with_context(|| {
                    format!("failed to remove path {} from index", path.display())
                })?;
            } else {
                index
                    .add_path(path)
                    .with_context(|| format!("failed to add path {} to index", path.display()))?;
            }
        }
        index.write().with_context(|| "failed to write index")?;
        Ok(())
    }

    pub fn git_unstage_files<P: AsRef<std::path::Path>>(&self, paths: Vec<P>) -> Result<()> {
        let head_tree = self.git_repository.head()?.peel_to_tree()?;
        let mut head_index = git2::Index::new()?;
        head_index.read_tree(&head_tree)?;
        let mut index = self.git_repository.index()?;
        for path in paths {
            let path = path.as_ref();
            // to "unstage" a file means to:
            // - put head version of the file in the index if it exists
            // - remove it from the index otherwise
            let head_index_entry = head_index.iter().find(|entry| {
                let entry_path = String::from_utf8(entry.path.clone());
                entry_path.as_ref().unwrap() == path.to_str().unwrap()
            });
            if let Some(entry) = head_index_entry {
                index
                    .add(&entry)
                    .with_context(|| format!("failed to add path {} to index", path.display()))?;
            } else {
                index.remove_path(path).with_context(|| {
                    format!("failed to remove path {} from index", path.display())
                })?;
            }
        }
        index.write().with_context(|| "failed to write index")?;
        Ok(())
    }

    // returns a remote and makes sure that the push url is an ssh url
    // if url is already ssh, or not set at all, then it returns the remote as is.
    fn get_remote(&'repository self, name: &str) -> Result<git2::Remote<'repository>> {
        let remote = self
            .git_repository
            .find_remote(name)
            .context("failed to find remote")?;

        if let Some(url) = remote.url() {
            if matches!(url_type(url), URLType::Ssh) {
                return Ok(remote);
            }

            let url = to_ssh_url(url);
            if !matches!(url_type(&url), URLType::Ssh) {
                return Err(Error::NonSSHUrl(url.to_string()).into());
            }

            ssh::check_known_host(url_host(&url)).context("failed to check known host")?;

            Ok(self
                .git_repository
                .remote_anonymous(&url)
                .context("failed to get anonymous")?)
        } else {
            Err(Error::NoUrl.into())
        }
    }

    pub fn push(
        &self,
        head: &git2::Oid,
        branch: &branch::RemoteName,
        key: &keys::PrivateKey,
    ) -> Result<(), Error> {
        let mut remote = self
            .get_remote(branch.remote())
            .context("failed to get remote")
            .map_err(Error::Other)?;

        for credential_callback in git::credentials::for_key(key) {
            let mut remote_callbacks = git2::RemoteCallbacks::new();
            remote_callbacks.credentials(credential_callback);

            tracing::info!(
                "{}: git push {} {}:refs/heads/{}",
                self.project.id,
                branch.remote(),
                head,
                branch.branch()
            );

            match remote.push(
                &[format!("{}:refs/heads/{}", head, branch.branch())],
                Some(&mut git2::PushOptions::new().remote_callbacks(remote_callbacks)),
            ) {
                Ok(()) => {
                    tracing::info!("{}: git push succeeded", self.project.id);
                    return Ok(());
                }
                Err(e) => {
                    if e.code() == git2::ErrorCode::Auth {
                        tracing::info!("{}: git push failed: {:#}", self.project.id, e);
                        continue;
                    } else {
                        return Err(Error::Other(e.into()));
                    }
                }
            }
        }

        Err(Error::AuthError)
    }

    pub fn fetch(&self, remote_name: &str, key: &keys::PrivateKey) -> Result<(), Error> {
        let mut remote = self
            .get_remote(remote_name)
            .context("failed to get remote")
            .map_err(|e| {
                println!("failed to get remote: {:#}", e);
                Error::Other(e)
            })?;

        for credential_callback in git::credentials::for_key(key) {
            let mut remote_callbacks = git2::RemoteCallbacks::new();
            remote_callbacks.credentials(credential_callback);
            remote_callbacks.push_update_reference(|refname, message| {
                if let Some(msg) = message {
                    tracing::info!(
                        "{}: push update reference: {}: {}",
                        self.project.id,
                        refname,
                        msg
                    );
                }
                Ok(())
            });
            remote_callbacks.push_negotiation(|proposals| {
                tracing::info!(
                    "{}: push negotiation: {:?}",
                    self.project.id,
                    proposals
                        .iter()
                        .map(|p| format!(
                            "src_refname: {}, dst_refname: {}",
                            p.src_refname().unwrap_or(&p.src().to_string()),
                            p.dst_refname().unwrap_or(&p.dst().to_string())
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                Ok(())
            });
            remote_callbacks.push_transfer_progress(|one, two, three| {
                tracing::info!(
                    "{}: push transfer progress: {} {} {}",
                    self.project.id,
                    one,
                    two,
                    three
                );
            });

            let mut fetch_opts = git2::FetchOptions::new();
            fetch_opts.remote_callbacks(remote_callbacks);
            fetch_opts.prune(git2::FetchPrune::On);

            let refspec = format!("+refs/heads/*:refs/remotes/{}/*", remote_name);
            tracing::info!("{}: git fetch {}", self.project.id, &refspec);

            match remote.fetch(&[refspec], Some(&mut fetch_opts), None) {
                Ok(()) => {
                    tracing::info!("{}: fetched {}", self.project.id, remote_name);
                    return Ok(());
                }
                Err(e) => {
                    if e.code() == git2::ErrorCode::Auth {
                        tracing::warn!("{}: auth error", self.project.id);
                        continue;
                    } else {
                        return Err(Error::Other(e.into()));
                    }
                }
            }
        }

        Err(Error::AuthError)
    }

    pub fn git_commit(&self, message: &str, push: bool) -> Result<()> {
        let config = self
            .git_repository
            .config()
            .with_context(|| "failed to get config")?;
        let name = config
            .get_string("user.name")
            .with_context(|| "failed to get user.name")?;
        let email = config
            .get_string("user.email")
            .with_context(|| "failed to get user.email")?;

        // Get the default signature for the repository
        let signature =
            git2::Signature::now(&name, &email).with_context(|| "failed to get signature")?;

        // Create the commit with current index
        let tree_id = self.git_repository.index()?.write_tree()?;
        let tree = self.git_repository.find_tree(tree_id)?;
        let parent_commit = self.git_repository.head()?.peel_to_commit()?;
        let commit = self.git_repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent_commit],
        )?;
        tracing::info!("{}: created commit {}", self.project.id, commit);

        if push {
            // Get a reference to the current branch
            let head = self.git_repository.head()?;
            let branch = head.name().unwrap();

            let branch_remote = self.git_repository.branch_upstream_remote(branch)?;
            let branch_remote_name = branch_remote.as_str().unwrap();
            let branch_name = self.git_repository.branch_upstream_name(branch)?;

            tracing::info!(
                "{}: pushing {} to {} as {}",
                self.project.id,
                branch,
                branch_remote_name,
                branch_name.as_str().unwrap()
            );

            // Set the remote's callbacks
            let mut callbacks = git2::RemoteCallbacks::new();

            callbacks.push_update_reference(move |refname, message| {
                tracing::info!("pushing reference '{}': {:?}", refname, message);
                Ok(())
            });
            callbacks.push_transfer_progress(move |one, two, three| {
                tracing::info!("transferred {}/{}/{} objects", one, two, three);
            });

            // create ssh key if it's not there

            // try to auth with creds from an ssh-agent
            callbacks.credentials(|_url, username_from_url, _allowed_types| {
                git2::Cred::ssh_key(
                    username_from_url.unwrap(),
                    None,
                    std::path::Path::new(&format!("{}/.ssh/id_ed25519", env::var("HOME").unwrap())),
                    None,
                )
            });

            let mut push_options = git2::PushOptions::new();
            push_options.remote_callbacks(callbacks);

            // Push to the remote
            let mut remote = self.git_repository.find_remote(branch_remote_name)?;
            remote
                .push(&[branch], Some(&mut push_options))
                .with_context(|| {
                    format!("failed to push {:?} to {:?}", branch, branch_remote_name)
                })?;
        }

        Ok(())
    }
}

#[derive(Serialize, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileStatus {
    pub staged: Option<FileStatusType>,
    pub unstaged: Option<FileStatusType>,
}

#[derive(Serialize, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum FileStatusType {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChange,
    Other,
}

impl From<git2::Status> for FileStatusType {
    fn from(status: git2::Status) -> Self {
        if status.is_index_new() || status.is_wt_new() {
            FileStatusType::Added
        } else if status.is_index_modified() || status.is_wt_modified() {
            FileStatusType::Modified
        } else if status.is_index_deleted() || status.is_wt_deleted() {
            FileStatusType::Deleted
        } else if status.is_index_renamed() || status.is_wt_renamed() {
            FileStatusType::Renamed
        } else if status.is_index_typechange() || status.is_wt_typechange() {
            FileStatusType::TypeChange
        } else {
            FileStatusType::Other
        }
    }
}

enum URLType {
    Ssh,
    Https,
    Unknown,
}

fn url_host(url: &str) -> &str {
    if url.starts_with("git@") {
        url.split(':').next().unwrap().split('@').nth(1).unwrap()
    } else if url.starts_with("https://") {
        url.split('/').nth(2).unwrap()
    } else {
        ""
    }
}

fn url_type(url: &str) -> URLType {
    if url.starts_with("git@") {
        URLType::Ssh
    } else if url.starts_with("https://") {
        URLType::Https
    } else {
        URLType::Unknown
    }
}

fn to_ssh_url(url: &str) -> String {
    if !url.starts_with("https://") {
        return url.to_string();
    }
    let mut url = url.to_string();
    url.replace_range(..8, "git@");
    url.replace("https://", ":").replace(".com/", ".com:")
}

#[test]
fn test_to_ssh_url() {
    assert_eq!(
        to_ssh_url("https://github.com/gitbutlerapp/gitbutler-client.git"),
        "git@github.com:gitbutlerapp/gitbutler-client.git"
    );
    assert_eq!(
        to_ssh_url("git@github.com:gitbutlerapp/gitbutler-client.git"),
        "git@github.com:gitbutlerapp/gitbutler-client.git"
    );
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("git url is empty")]
    NoUrl,
    #[error("git url is not ssh: {0}")]
    NonSSHUrl(String),
    #[error("authentication failed")]
    AuthError,
    #[error(transparent)]
    Other(anyhow::Error),
}

type OidFilter = dyn Fn(&git2::Commit) -> Result<bool>;

pub enum LogUntil {
    Commit(git2::Oid),
    Take(usize),
    When(Box<OidFilter>),
    End,
}
