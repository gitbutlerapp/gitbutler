//! Following `but://` links in the window: the one the app was launched with, and those that
//! come while it runs. How they arrive, and what they say, is `backend::links`.

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{App, Application, Context, Window, WindowHandle};

use crate::app::AppView;
use crate::backend;
use crate::backend::links::{Incoming, IncomingLinks};

/// Collect links from before the window opens, for [`follow`] to open once it has:
/// `incoming_links`, and what macOS hands over.
pub fn listen(
    app: &Application,
    incoming_links: IncomingLinks,
) -> mpsc::UnboundedReceiver<Incoming> {
    let (sender, incoming) = mpsc::unbounded();
    incoming_links.receive(sender.clone());
    app.on_open_urls(move |urls| {
        for url in urls {
            sender.unbounded_send(Incoming::Link(url)).ok();
        }
    });
    incoming
}

/// Open each link as it comes, in `window`, which comes forward with the keys where a click
/// on a row leaves them.
pub fn follow(
    mut incoming: mpsc::UnboundedReceiver<Incoming>,
    window: WindowHandle<AppView>,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        while let Some(incoming) = incoming.next().await {
            (window.update(cx, |app, window, cx| open(incoming, app, window, cx))).ok();
        }
    })
    .detach();
}

/// Open `incoming` in `app`'s window, as [`follow`] does each link that comes.
pub fn open(incoming: Incoming, app: &mut AppView, window: &mut Window, cx: &mut Context<AppView>) {
    if let Incoming::Link(url) = incoming {
        let Some(link) = backend::links::read_link(&url) else {
            // Not the whole link, which can carry a sign-in token.
            let kind = url.split_once('?').map_or(&*url, |(kind, _)| kind);
            tracing::warn!("ignoring a link the app doesn't open: {kind}");
            return;
        };
        (app.store).update(cx, |store, cx| {
            store.open_from_link(link.path, link.at, cx);
            if link.unreadable {
                store.snack_warning("Couldn't read where the link points", cx);
            }
        });
    }
    app.focus_main(window, cx);
    window.activate_window();
    cx.activate(true);
}
