use gix::url::Scheme;

use crate::ForgeName;

/// Parsed parts of a remote URL needed to identify a forge repository.
pub(crate) struct RemoteUrl {
    pub(crate) protocol: String,
    pub(crate) host: String,
    pub(crate) port: Option<u16>,
    path: String,
}

impl RemoteUrl {
    pub(crate) fn parse(input: &str) -> Option<Self> {
        let parsed = gix::url::parse(input.as_bytes()).ok()?;
        if !matches!(
            parsed.scheme,
            Scheme::Git | Scheme::Ssh | Scheme::Http | Scheme::Https
        ) {
            return None;
        }

        let path = std::str::from_utf8(parsed.path.as_ref()).ok()?;
        let path = path.split(['?', '#']).next()?.trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path).to_string();

        Some(Self {
            protocol: parsed.scheme.as_str().to_string(),
            host: parsed.host?,
            port: parsed.port,
            path,
        })
    }

    pub(crate) fn repository_parts(&self, forge: &ForgeName) -> Option<(String, String)> {
        match forge {
            ForgeName::GitHub | ForgeName::Bitbucket => {
                let (owner, repo) = self.path.split_once('/')?;
                (!owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
                    .then(|| (owner.to_string(), repo.to_string()))
            }
            ForgeName::GitLab => {
                let (owner, repo) = self.path.rsplit_once('/')?;
                (!owner.is_empty() && !repo.is_empty())
                    .then(|| (owner.to_string(), repo.to_string()))
            }
            ForgeName::Azure => self.azure_repository_parts(),
        }
    }

    fn azure_repository_parts(&self) -> Option<(String, String)> {
        let mut segments = self.path.split('/');
        let first = segments.next()?;
        let (org, project, marker_or_repo, repo) = if first == "v3" {
            (
                segments.next()?,
                segments.next()?,
                segments.next()?,
                segments.next(),
            )
        } else {
            (first, segments.next()?, segments.next()?, segments.next())
        };
        let repo = match (marker_or_repo, repo) {
            ("_git", Some(repo)) => repo,
            (repo, None) => repo,
            _ => return None,
        };
        if org.is_empty() || project.is_empty() || repo.is_empty() || segments.next().is_some() {
            return None;
        }
        Some((format!("{org}/{project}"), repo.to_string()))
    }
}
