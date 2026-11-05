//! The `but://` links the GUI opens, written by `but gui` and read by the apps that open them.
//!
//! Both sides go through [`OpenLink`], so what one writes is what the other reads.

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
    /// Where in the project to open, or `None` for where it opens by itself.
    pub at: Option<Place>,
}

/// A place in a project a link can open at, each named by query parameters of its own.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Place {
    /// One of the project's pages (`page=`).
    Page(Page),
    /// Some changes, at one of their files if `file` names one.
    Changes { of: Changes, file: Option<FileAt> },
}

/// A project's page a link can open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Page {
    Branches,
    History,
    Graph,
}

/// Changes a link can show.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Changes {
    /// A branch's, by its full ref name, as `refs/heads/feature` (`branch=`).
    Branch(String),
    /// A commit's, by its full hash (`commit=`).
    Commit(String),
    /// A worktree's uncommitted ones (`uncommitted=1`): a linked worktree's by its name, the
    /// folder name under `.git/worktrees/` (`worktree=`), or else the main worktree's.
    Uncommitted { worktree: Option<String> },
}

/// A file in changes, relative to the project, with `/` (`file=`), at the hunk with a line of
/// its new side if `line` names one (`line=`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileAt {
    pub path: String,
    pub line: Option<u32>,
}

impl Page {
    /// Every page. As `name` does, a new page fails to build here until it's in the list.
    pub const ALL: [Page; 3] = {
        match Page::Graph {
            Page::Branches | Page::History | Page::Graph => {}
        }
        [Page::Branches, Page::History, Page::Graph]
    };

    /// The page's name in a link.
    pub const fn name(self) -> &'static str {
        match self {
            Page::Branches => "branches",
            Page::History => "history",
            Page::Graph => "graph",
        }
    }
}

impl Place {
    /// The query parameters that name the place.
    fn params(&self) -> Vec<(&'static str, String)> {
        let (of, file) = match self {
            Place::Page(page) => return vec![("page", page.name().into())],
            Place::Changes { of, file } => (of, file),
        };
        let mut params = match of {
            Changes::Branch(name) => vec![("branch", name.clone())],
            Changes::Commit(id) => vec![("commit", id.clone())],
            Changes::Uncommitted { worktree } => {
                let worktree = worktree.iter().map(|name| ("worktree", name.clone()));
                [("uncommitted", "1".into())]
                    .into_iter()
                    .chain(worktree)
                    .collect()
            }
        };
        if let Some(FileAt { path, line }) = file {
            params.push(("file", path.clone()));
            params.extend(line.map(|line| ("line", line.to_string())));
        }
        params
    }

    /// The place `params` name, or `None` if they name none, or one this version doesn't know.
    /// Changes come before a page, a branch's before a commit's, and a commit's before
    /// uncommitted ones.
    fn from_params(params: &HashMap<String, String>) -> Option<Self> {
        let param = |key| params.get(key).cloned();
        let of = if let Some(name) = param("branch") {
            Changes::Branch(name)
        } else if let Some(id) = param("commit") {
            Changes::Commit(id)
        } else if params.get("uncommitted").is_some_and(|value| value == "1") {
            Changes::Uncommitted {
                worktree: param("worktree"),
            }
        } else {
            let page = params.get("page")?;
            return (Page::ALL.into_iter())
                .find(|known| known.name() == page)
                .map(Place::Page);
        };
        let file = param("file").map(|path| FileAt {
            path,
            // One that isn't a number opens the file at its top.
            line: params.get("line").and_then(|line| line.parse().ok()),
        });
        Some(Place::Changes { of, file })
    }
}

impl OpenLink {
    /// The link as `channel`'s app takes it. `timestamp` makes each link differ from the last,
    /// so the platform doesn't drop a repeat of one it just delivered.
    pub fn to_url(&self, channel: AppChannel, timestamp: u128) -> anyhow::Result<url::Url> {
        let mut url = url::Url::parse(&format!("{}://open", channel.scheme()))?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("path", &self.path.to_string_lossy())
                .append_pair("t", &timestamp.to_string())
                .append_pair("new_window", if self.new_window { "1" } else { "0" });
            for (key, value) in self.at.iter().flat_map(Place::params) {
                query.append_pair(key, &value);
            }
        }
        Ok(url)
    }

    /// Read `url` as an open link of any channel's scheme, or `None` if it isn't one or names
    /// no path. Parameters it doesn't know are left alone, so newer links still open the
    /// project in older apps, as does a place this version doesn't know.
    pub fn parse(url: &str) -> Option<Self> {
        let url = url::Url::parse(url).ok()?;
        let known_scheme = (AppChannel::ALL.iter()).any(|channel| channel.scheme() == url.scheme());
        if !known_scheme || url.host_str() != Some("open") {
            return None;
        }
        let params: HashMap<String, String> = (url.query_pairs())
            .filter(|(_, value)| !value.is_empty())
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        Some(OpenLink {
            path: PathBuf::from(params.get("path")?),
            new_window: params.get("new_window").is_some_and(|value| value == "1"),
            at: Place::from_params(&params),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::path::Path;

    use super::{Changes, FileAt, OpenLink, Page, Place};
    use crate::AppChannel;

    /// `of`, at `file` and its line if it names one.
    fn changes(of: Changes, file: Option<(&str, Option<u32>)>) -> Option<Place> {
        Some(Place::Changes {
            of,
            file: file.map(|(path, line)| FileAt {
                path: path.into(),
                line,
            }),
        })
    }

    #[test]
    fn to_url_encodes_query_parameters() -> anyhow::Result<()> {
        let path = Path::new(r"C:\project & tools\#one");
        let link = OpenLink {
            path: path.to_owned(),
            new_window: true,
            at: None,
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
            None,
            changes(Changes::Branch("refs/heads/feature".into()), None),
            changes(
                Changes::Branch("refs/heads/a & b".into()),
                Some(("src/with space/#one.rs", None)),
            ),
            changes(
                Changes::Commit("8c5a2f0e4a2b7c43c0d9a3a8bfbb4e4b2b2d1f6a".into()),
                Some(("src/a.rs", Some(42))),
            ),
            changes(Changes::Uncommitted { worktree: None }, None),
            changes(
                Changes::Uncommitted {
                    worktree: Some("feature".into()),
                },
                Some(("src/a.rs", Some(7))),
            ),
        ]
        .into_iter()
        .chain(Page::ALL.map(|page| Some(Place::Page(page))));
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
                        "every channel's link reads back as written, and every place"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn parse_reads_the_place_a_link_names() {
        let at = |url| OpenLink::parse(url).and_then(|link| link.at);
        assert_eq!(
            at("but://open?path=%2Frepo&branch=refs%2Fheads%2Fx&file=src%2Fa.rs"),
            changes(
                Changes::Branch("refs/heads/x".into()),
                Some(("src/a.rs", None))
            ),
            "a branch, at one of its files"
        );
        assert_eq!(
            at("but://open?path=%2Frepo&commit=8c5a2f0e&file=src%2Fa.rs&line=42"),
            changes(
                Changes::Commit("8c5a2f0e".into()),
                Some(("src/a.rs", Some(42)))
            ),
            "a commit, at a line of one of its files"
        );
        assert_eq!(
            at("but://open?path=%2Frepo&uncommitted=1&worktree=feature"),
            changes(
                Changes::Uncommitted {
                    worktree: Some("feature".into())
                },
                None
            ),
            "a linked worktree's uncommitted changes"
        );
        assert_eq!(
            at("but://open?path=%2Frepo&uncommitted=1&file=src%2Fa.rs&line=x"),
            changes(
                Changes::Uncommitted { worktree: None },
                Some(("src/a.rs", None))
            ),
            "a line that isn't a number opens the file"
        );
        assert_eq!(
            at("but://open?path=%2Frepo&page=graph"),
            Some(Place::Page(Page::Graph)),
            "a page"
        );
        assert_eq!(
            at("but://open?path=%2Frepo&commit=8c5a2f0e&branch=refs%2Fheads%2Fx&page=graph"),
            changes(Changes::Branch("refs/heads/x".into()), None),
            "a branch comes before a commit, and changes before a page"
        );
        for url in [
            "but://open?path=%2Frepo&page=timeline",
            "but://open?path=%2Frepo&file=src%2Fa.rs",
            "but://open?path=%2Frepo&worktree=feature",
            "but://open?path=%2Frepo&uncommitted=0",
            "but://open?path=%2Frepo&branch=",
        ] {
            assert_eq!(
                at(url),
                None,
                "{url} names no place this version knows, so it opens the project"
            );
        }
    }

    #[test]
    fn parse_ignores_unknown_parameters() {
        assert_eq!(
            OpenLink::parse("but://open?path=%2Frepo&view=history&new_window=0"),
            Some(OpenLink {
                path: "/repo".into(),
                new_window: false,
                at: None,
            }),
            "parameters a newer app writes don't stop an older one opening the project"
        );
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
