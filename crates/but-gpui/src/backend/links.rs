//! `but://` links from outside the app: what one asks to open, read as `but gui` writes it, and
//! how they reach the app.
//!
//! macOS hands the running app its links itself. Windows and Linux launch the app again with
//! the link as an argument, so that launch hands its links to the running app and quits (`unix`,
//! `windows`). Dev builds skip this, so two can run side by side.

use std::path::PathBuf;

use but_path::AppChannel;
use but_path::link::{self, OpenLink};
pub use but_path::link::{FileAt, Page};
use futures::channel::mpsc;

use super::{Checkout, CommitId, FullName};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
// `self::`, as the `windows` crate has the same name.
#[cfg(windows)]
use self::windows as platform;

/// What reaches the app from outside.
#[derive(Debug, PartialEq)]
pub enum Incoming {
    Link(String),
    /// A launch without a link, which only brings the window forward.
    Launch,
}

/// This launch's links, and where later launches send theirs, if it listens.
pub struct IncomingLinks {
    links: Vec<String>,
    listener: Option<platform::Listener>,
}

impl IncomingLinks {
    /// Send this launch's links to `sender`, then later launches' as they come.
    pub fn receive(self, sender: mpsc::UnboundedSender<Incoming>) {
        for link in self.links {
            sender.unbounded_send(Incoming::Link(link)).ok();
        }
        if let Some(listener) = self.listener {
            receive_links(listener, sender);
        }
    }
}

/// What a `but://open` link asks to open: the project at `path`, at the place in it `at` names.
#[derive(Debug, PartialEq)]
pub struct Link {
    pub path: PathBuf,
    pub at: Option<Place>,
    /// It names a place that can't be, as a branch name that isn't a ref or a commit not by its
    /// full hash, and opens the project in its place.
    pub unreadable: bool,
}

/// A place in a project a link can open at.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    /// Some changes, at one of their files if `file` names one.
    Changes {
        of: Changes,
        file: Option<FileAt>,
    },
    Page(Page),
}

/// Changes a link can show.
#[derive(Debug, Clone, PartialEq)]
pub enum Changes {
    Branch(FullName),
    Commit(CommitId),
    Uncommitted(Checkout),
}

/// What `url` asks to open, or `None` if it isn't a `but://open` link. Whether it asks for a
/// new window doesn't matter, as one window shows every project, and a place it names that
/// isn't a valid one opens the project as it opens by itself.
pub fn read_link(url: &str) -> Option<Link> {
    let link = OpenLink::parse(url)?;
    let named = link.at.is_some();
    let at = link.at.and_then(place);
    Some(Link {
        path: link.path,
        unreadable: named && at.is_none(),
        at,
    })
}

/// `at` in the app's own terms, or `None` if the changes it names can't be.
fn place(at: link::Place) -> Option<Place> {
    Some(match at {
        link::Place::Changes { of, file } => Place::Changes {
            of: changes(of)?,
            file,
        },
        link::Place::Page(page) => Place::Page(page),
    })
}

/// `of` in the app's own terms, or `None` for a branch name that isn't a valid ref, or a commit
/// that isn't named by its full hash.
fn changes(of: link::Changes) -> Option<Changes> {
    Some(match of {
        link::Changes::Branch(name) => Changes::Branch(FullName::try_from(name.as_str()).ok()?),
        link::Changes::Commit(id) => {
            Changes::Commit(CommitId(gix::ObjectId::from_hex(id.as_bytes()).ok()?))
        }
        link::Changes::Uncommitted { worktree } => Changes::Uncommitted(
            worktree.map_or(Checkout::Main, |name| Checkout::Linked(name.into())),
        ),
    })
}

/// Hand this launch's links to the app already running, which comes forward with them, and
/// return `None`, as this launch is then done. Otherwise the links that reach it.
pub fn hand_off() -> Option<IncomingLinks> {
    let links = launch_links();
    if cfg!(target_os = "macos") || AppChannel::new() == AppChannel::Dev {
        return Some(IncomingLinks {
            links,
            listener: None,
        });
    }
    let listener = match platform::listen() {
        Ok(Some(listener)) => Some(listener),
        Ok(None) => match send_to_running_app(&links) {
            Ok(()) => {
                tracing::info!("handed this launch to the app already running");
                return None;
            }
            Err(err) => {
                tracing::warn!("couldn't hand this launch to the app already running: {err}");
                None
            }
        },
        Err(err) => {
            tracing::warn!("later launches will open apps of their own: {err}");
            None
        }
    };
    Some(IncomingLinks { links, listener })
}

/// Hand `links` to the app already running, failing if none listens: one message, a link a
/// line, or empty for a launch without one. The only code that sends the running app links.
fn send_to_running_app(links: &[String]) -> std::io::Result<()> {
    platform::send(links.join("\n").as_bytes())
}

/// The links this launch was given, as Windows and Linux give them. `args_os`, as `args`
/// panics on one that isn't Unicode, which is no link anyway.
fn launch_links() -> Vec<String> {
    std::env::args_os()
        .skip(1)
        .filter_map(|arg| arg.into_string().ok())
        .filter(|arg| read_link(arg).is_some())
        .collect()
}

/// Pass on what later launches hand over at `listener`: each message's links, or a launch for
/// an empty one.
fn receive_links(listener: platform::Listener, sender: mpsc::UnboundedSender<Incoming>) {
    let receiving = std::thread::Builder::new()
        .name("links".into())
        .spawn(move || {
            let received = platform::receive(listener, |message| {
                let message = String::from_utf8_lossy(&message);
                let mut links = message.lines().filter(|link| !link.is_empty()).peekable();
                if links.peek().is_none() {
                    sender.unbounded_send(Incoming::Launch).ok();
                }
                for link in links {
                    sender.unbounded_send(Incoming::Link(link.to_owned())).ok();
                }
            });
            if let Err(err) = received {
                tracing::warn!("later launches will open apps of their own: {err}");
            }
        });
    if let Err(err) = receiving {
        tracing::warn!("later launches will open apps of their own: {err}");
    }
}

#[cfg(test)]
mod tests {
    use but_testsupport::isolated_app_data_dir;
    use futures::StreamExt as _;

    use super::*;

    #[test]
    fn a_link_reads_as_the_place_it_names() {
        let at = |url| read_link(url).and_then(|link| link.at);
        let id = "8c5a2f0e4a2b7c43c0d9a3a8bfbb4e4b2b2d1f6a";
        assert_eq!(
            read_link("but://open?path=%2Frepo&branch=refs%2Fheads%2Fx&file=src%2Fa.rs&line=4"),
            Some(Link {
                path: "/repo".into(),
                at: Some(Place::Changes {
                    of: Changes::Branch(FullName::try_from("refs/heads/x").expect("a valid ref")),
                    file: Some(FileAt {
                        path: "src/a.rs".into(),
                        line: Some(4),
                    }),
                }),
                unreadable: false,
            }),
            "a branch at a line of one of its files"
        );
        let url = format!("but://open?path=%2Frepo&commit={id}");
        assert_eq!(
            at(&url),
            Some(Place::Changes {
                of: Changes::Commit(CommitId(
                    gix::ObjectId::from_hex(id.as_bytes()).expect("a hash")
                )),
                file: None,
            }),
            "a commit by its full hash"
        );
        assert_eq!(
            at("but://open?path=%2Frepo&uncommitted=1&worktree=feature"),
            Some(Place::Changes {
                of: Changes::Uncommitted(Checkout::Linked("feature".into())),
                file: None,
            }),
            "a linked worktree's uncommitted changes"
        );
        assert_eq!(
            at("but://open?path=%2Frepo&uncommitted=1"),
            Some(Place::Changes {
                of: Changes::Uncommitted(Checkout::Main),
                file: None,
            }),
            "without a worktree, the main worktree's"
        );
        assert_eq!(
            at("but://open?path=%2Frepo&page=graph"),
            Some(Place::Page(Page::Graph)),
            "a page as the link names it"
        );
        assert_eq!(
            read_link("but://open?path=%2Frepo").map(|link| (link.at, link.unreadable)),
            Some((None, false)),
            "a link naming no place opens the project as it opens by itself"
        );
        for url in [
            "but://open?path=%2Frepo&branch=not%20a..ref",
            "but://open?path=%2Frepo&commit=8c5a2f0e",
        ] {
            assert_eq!(
                read_link(url).map(|link| (link.at, link.unreadable)),
                Some((None, true)),
                "{url} names nothing that can be, so it opens the project and says so"
            );
        }
    }

    #[test]
    fn a_later_launch_hands_over_its_links() {
        isolated_app_data_dir(|| {
            let first = "but://open?path=%2Ffirst".to_owned();
            let listener = (platform::listen())
                .expect("the socket can be listened at")
                .expect("nothing listens there yet");
            assert!(
                platform::listen()
                    .expect("the socket can be asked")
                    .is_none(),
                "a later launch finds the app listening"
            );
            let (sender, incoming) = mpsc::unbounded();
            let incoming_links = IncomingLinks {
                links: vec![first.clone()],
                listener: Some(listener),
            };
            incoming_links.receive(sender);

            let links = [
                "but://open?path=%2Fa".to_owned(),
                "but://open?path=%2Fb".to_owned(),
            ];
            send_to_running_app(&links).expect("a launch with links hands them over");
            send_to_running_app(&[]).expect("a launch without one hands over itself");
            let incoming = futures::executor::block_on(incoming.take(4).collect::<Vec<_>>());
            let [a, b] = links;
            assert_eq!(
                incoming,
                [
                    Incoming::Link(first),
                    Incoming::Link(a),
                    Incoming::Link(b),
                    Incoming::Launch
                ],
                "the app gets its own links first, then each later launch's in turn"
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn a_launch_listens_in_place_of_an_app_that_quit() {
        isolated_app_data_dir(|| {
            // Closing the socket leaves its file behind, as an app that quits does.
            drop(platform::listen().expect("the socket can be listened at"));
            assert!(
                send_to_running_app(&[]).is_err(),
                "no app listens at a socket left behind"
            );
            let listener = platform::listen().expect("the socket can be listened at");
            assert!(listener.is_some(), "a launch listens there itself");
        });
    }

    #[cfg(unix)]
    #[test]
    fn only_the_user_can_reach_the_socket() {
        use std::os::unix::fs::PermissionsExt as _;

        isolated_app_data_dir(|| {
            let _listener = platform::listen().expect("the socket can be listened at");
            let path = platform::socket_path().expect("an app data directory");
            let mode = (std::fs::metadata(path).expect("the socket is there"))
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "other users can't hand the app links");
        });
    }
}
