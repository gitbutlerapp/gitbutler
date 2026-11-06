//! The `but://` links the GUI opens, written by `but gui` and `but _open --gui` and read by the
//! apps that open them.
//!
//! A link names its project by `path`. Where in it to open is the app's to spell and read, so
//! this crate only carries it, as the parameters past the link's own, but for [`Changes`], which
//! the CLI names by its IDs and so writes as the app reads them.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::AppChannel;

/// A `but://open?path=…` link: open the project at `path`, adding it first if it's new, at the
/// place in it `at` names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenLink {
    /// The project's directory.
    pub path: PathBuf,
    /// Open it in a window of its own rather than the one in front.
    pub new_window: bool,
    /// Where in the project to open, as the app names it, in the parameters' order, or none for
    /// where it opens by itself.
    pub at: Vec<(String, String)>,
}

/// The link's own parameters, which [`OpenLink::parse`] keeps out of the place.
const OWN: [&str; 3] = ["path", "t", "new_window"];

/// Changes the CLI names by its IDs, as the app reads them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Changes {
    /// A branch's, by its full ref name, as `refs/heads/feature`.
    Branch(String),
    /// A commit's, by its full hash.
    Commit(String),
    /// A worktree's uncommitted ones: a linked worktree's by its name, the folder name under
    /// `.git/worktrees/`, or else the main worktree's.
    Uncommitted { worktree: Option<String> },
}

/// A file in changes, relative to the project, with `/`, at the hunk with a line of its new side
/// if `line` names one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileAt {
    pub path: String,
    pub line: Option<u32>,
}

impl Changes {
    /// The place these changes are, at `file` if it names one, as [`OpenLink::at`] takes it.
    pub fn at(&self, file: Option<&FileAt>) -> Vec<(String, String)> {
        let mut params = match self {
            Changes::Branch(name) => vec![("to", "branch".into()), ("branch", name.clone())],
            Changes::Commit(id) => vec![("to", "commit".into()), ("commit", id.clone())],
            Changes::Uncommitted { worktree } => {
                let mut params = vec![("to", "uncommitted".into())];
                params.extend(worktree.clone().map(|name| ("worktree", name)));
                params
            }
        };
        if let Some(FileAt { path, line }) = file {
            params.push(("file", path.clone()));
            params.extend(line.map(|line| ("line", line.to_string())));
        }
        (params.into_iter())
            .map(|(key, value)| (key.to_owned(), value))
            .collect()
    }
}

impl OpenLink {
    /// The link as `channel`'s app takes it. `timestamp` makes each link differ from the last,
    /// so the platform doesn't drop a repeat of one it just delivered. A place can't name one of
    /// the link's own parameters, which would stand in for the link's.
    pub fn to_url(&self, channel: AppChannel, timestamp: u128) -> anyhow::Result<url::Url> {
        if let Some((key, _)) = (self.at.iter()).find(|(key, _)| OWN.contains(&key.as_str())) {
            anyhow::bail!("`{key}` is the link's own parameter, not a place's");
        }
        let mut url = url::Url::parse(&format!("{}://open", channel.scheme()))?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("path", &self.path.to_string_lossy())
                .append_pair("t", &timestamp.to_string())
                .append_pair("new_window", if self.new_window { "1" } else { "0" });
            for (key, value) in &self.at {
                query.append_pair(key, value);
            }
        }
        Ok(url)
    }

    /// Read `url` as an open link of any channel's scheme, or `None` if it isn't one or names
    /// no path. What it names in the project is left for the app to read, which leaves what it
    /// doesn't know alone, so newer links still open the project in older apps.
    pub fn parse(url: &str) -> Option<Self> {
        let url = url::Url::parse(url).ok()?;
        let known_scheme = (AppChannel::ALL.iter()).any(|channel| channel.scheme() == url.scheme());
        if !known_scheme || url.host_str() != Some("open") {
            return None;
        }
        let (own, at): (Vec<_>, Vec<_>) = (url.query_pairs())
            .filter(|(_, value)| !value.is_empty())
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .partition(|(key, _)| OWN.contains(&key.as_str()));
        let own: HashMap<_, _> = own.into_iter().collect();
        Some(OpenLink {
            path: PathBuf::from(own.get("path")?),
            new_window: own.get("new_window").is_some_and(|value| value == "1"),
            at,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::path::Path;

    use super::OpenLink;
    use crate::AppChannel;

    /// `pairs` as a link's place.
    fn at(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        (pairs.iter())
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn to_url_encodes_query_parameters() -> anyhow::Result<()> {
        let path = Path::new(r"C:\project & tools\#one");
        let link = OpenLink {
            path: path.to_owned(),
            new_window: true,
            at: Vec::new(),
        };
        let url = link.to_url(AppChannel::Release, 123)?;

        assert_eq!(
            url.as_str(),
            "but://open?path=C%3A%5Cproject+%26+tools%5C%23one&t=123&new_window=1",
            "reserved path characters are encoded"
        );
        assert_eq!(url.scheme(), "but", "the requested app channel is retained");
        assert_eq!(url.host_str(), Some("open"), "the open command is retained");
        assert_eq!(
            url.query_pairs().collect::<HashMap<_, _>>(),
            HashMap::from([
                (Cow::Borrowed("path"), path.to_string_lossy()),
                (Cow::Borrowed("t"), Cow::Borrowed("123")),
                (Cow::Borrowed("new_window"), Cow::Borrowed("1")),
            ]),
            "query values round-trip after encoding"
        );
        Ok(())
    }

    #[test]
    fn parse_reads_what_to_url_writes() -> anyhow::Result<()> {
        let places = [
            Vec::new(),
            at(&[("to", "branch"), ("branch", "refs/heads/a & b")]),
            at(&[
                ("to", "commit"),
                ("commit", "8c5a2f0e"),
                ("file", "src/with space/#one.rs"),
                ("line", "42"),
            ]),
            at(&[("to", "settings"), ("page", "ai")]),
        ];
        for at in places {
            for channel in AppChannel::ALL {
                for new_window in [false, true] {
                    let link = OpenLink {
                        path: r"/Users/me/project & tools/#one".into(),
                        new_window,
                        at: at.clone(),
                    };
                    let url = link.to_url(channel, 123)?;
                    assert_eq!(
                        OpenLink::parse(url.as_str()),
                        Some(link),
                        "every channel's link reads back as written, place and all"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn parse_keeps_what_it_doesnt_know_for_the_app() {
        assert_eq!(
            OpenLink::parse("but://open?path=%2Frepo&to=timeline&view=x&new_window=0&file="),
            Some(OpenLink {
                path: "/repo".into(),
                new_window: false,
                at: at(&[("to", "timeline"), ("view", "x")]),
            }),
            "the app reads the place, and an older one opens the project at a place it doesn't know"
        );
    }

    #[test]
    fn to_url_refuses_a_place_naming_the_links_own_parameters() {
        for key in ["path", "t", "new_window"] {
            let link = OpenLink {
                path: "/repo".into(),
                new_window: false,
                at: at(&[("to", "project"), (key, "x")]),
            };
            assert!(
                link.to_url(AppChannel::Release, 123).is_err(),
                "`{key}` would stand in for the link's own, as opening another project"
            );
        }
    }

    #[test]
    fn parse_rejects_other_links() {
        for url in [
            "https://open?path=%2Frepo",
            "but://login?access_token=x&t=1",
            "but://app/project/x/workspace",
            "but://open?t=1",
            "but://open?path=",
            "not a url",
        ] {
            assert_eq!(OpenLink::parse(url), None, "{url} is not an open link");
        }
    }
}
