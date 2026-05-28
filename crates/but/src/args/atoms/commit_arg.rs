use crate::{CliResult, bad_input};

/// An argument atom for commits.
#[derive(Debug, Clone)]
pub struct CommitArg(pub String);

impl std::str::FromStr for CommitArg {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_owned()))
    }
}

impl std::fmt::Display for CommitArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl CommitArg {
    /// Resolve the argument to a commit that exists in the repository.
    pub fn resolve(&self, repo: &gix::Repository) -> CliResult<gix::ObjectId> {
        let Ok(prefix) = gix::hash::Prefix::from_hex(&self.0) else {
            return Err(bad_input(format!("'{self}' is not a valid commit")).into());
        };
        match repo.objects.lookup_prefix(prefix, None)? {
            Some(Ok(commit)) => Ok(commit),
            Some(Err(_)) => Err(bad_input(format!(
                "Commit prefix '{self}' is ambiguous, matches multiple commits"
            ))
            .into()),
            None => Err(bad_input(format!("Could not find commit '{self}'")).into()),
        }
    }
}
