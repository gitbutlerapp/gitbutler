use std::fmt;

use serde::Serialize;

#[derive(Debug)]
pub struct RemoteName {
    // contains name of the remote, e.x. "origin" or "upstream"
    remote: String,
    // contains name of the branch, e.x. "master" or "main"
    branch: String,
}

impl fmt::Display for RemoteName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.remote, self.branch)
    }
}

impl Serialize for RemoteName {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl TryFrom<&git2::Branch<'_>> for RemoteName {
    type Error = git2::Error;

    fn try_from(value: &git2::Branch<'_>) -> std::result::Result<Self, Self::Error> {
        if !value.get().is_remote() {
            return Err(git2::Error::from_str("not a remote branch"));
        }
        let name = String::from_utf8(value.name_bytes()?.to_vec())
            .map_err(|e| git2::Error::from_str(&e.to_string()))?;

        if let Some((remote, branch)) = name.split_once('/') {
            Ok(Self {
                remote: remote.to_string(),
                branch: branch.to_string(),
            })
        } else {
            Err(git2::Error::from_str("invalid remote branch name"))
        }
    }
}

#[derive(Debug)]
pub struct LocalName {
    // contains name of the branch, e.x. "master" or "main"
    branch: String,
    // contains name of the remote branch, if the local branch is tracking a remote branch
    remote: Option<RemoteName>,
}

impl Serialize for LocalName {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.branch)
    }
}

impl fmt::Display for LocalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.branch)
    }
}

impl TryFrom<&git2::Branch<'_>> for LocalName {
    type Error = git2::Error;

    fn try_from(value: &git2::Branch<'_>) -> std::result::Result<Self, Self::Error> {
        if value.get().is_remote() {
            return Err(git2::Error::from_str("not a local branch"));
        }
        let name = String::from_utf8(value.name_bytes()?.to_vec())
            .map_err(|e| git2::Error::from_str(&e.to_string()))?;
        match value.upstream() {
            Ok(upstream) => {
                let remote_branch_name = RemoteName::try_from(&upstream)?;
                Ok(Self {
                    branch: name,
                    remote: Some(remote_branch_name),
                })
            }
            Err(error) => {
                if error.code() == git2::ErrorCode::NotFound {
                    Ok(Self {
                        branch: name,
                        remote: None,
                    })
                } else {
                    Err(error)
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum Name {
    Remote(RemoteName),
    Local(LocalName),
}

impl TryFrom<&git2::Branch<'_>> for Name {
    type Error = git2::Error;

    fn try_from(value: &git2::Branch<'_>) -> std::result::Result<Self, Self::Error> {
        if value.get().is_remote() {
            Ok(Self::Remote(RemoteName::try_from(value)?))
        } else {
            Ok(Self::Local(LocalName::try_from(value)?))
        }
    }
}
