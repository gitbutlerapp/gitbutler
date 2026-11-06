//! `but://` links from outside the app: how they reach it. What one names is `crate::links`'s
//! to read.
//!
//! macOS hands the running app its links itself. Windows and Linux launch the app again with
//! the link as an argument, so that launch hands its links to the running app and quits (`unix`,
//! `windows`). Dev builds skip this, so two can run side by side.

use but_path::AppChannel;
use but_path::link::OpenLink;
use futures::channel::mpsc;

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
        .filter(|arg| OpenLink::parse(arg).is_some())
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
