//! Following `but://` links in the window: the one the app was launched with, and those that
//! come while it runs, to the place each names ([`Route`]). How they arrive is `backend::links`.
//!
//! Everywhere the window can be has a link, so an agent can open it: every kind of
//! [`Selection`] but a terminal, a page on any of its tabs, and every page of settings. A link
//! names its place by `to=`, its kind's own name, and what that kind takes, as
//! `to=branch&branch=refs/heads/x&tab=review`, `to=project&tab=graph` or `to=settings&page=git`,
//! and a file of a diff by `file=` and `line=`. Reading one matches every kind with no wildcard,
//! so a new kind doesn't build until it's read here, and the tests open a link to every one.

use but_path::link::OpenLink;
use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{App, Application, Context, Window, WindowHandle};

use crate::app::AppView;
use crate::backend::links::{Incoming, IncomingLinks};
use crate::backend::{Checkout, CommitId, FullName};
use crate::detail::{PageTab, has_tabs, shows_its_diff};
use crate::settings::{AppPage, Page};
use crate::state::{
    CodePlace, MeshPick, ProjectId, ProjectTab, Section, SectionKind, Selection, SelectionKind,
};

/// Where a link takes the window.
#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    /// What a row selects, on one of its page's tabs, or at a place in its diff.
    Select {
        selection: Selection,
        tab: Option<Tab>,
        place: Option<CodePlace>,
    },
    /// A page of settings.
    Settings(Page),
}

/// A tab a page opens on: one of a project's, or one of another page's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Project(ProjectTab),
    Page(PageTab),
}

/// `to=` for the app's settings and a project's, which aren't kinds of selection.
const SETTINGS: &str = "settings";
const PROJECT_SETTINGS: &str = "project-settings";

/// The place `params` name in `project`, or `None` if they name none this version can read.
pub fn read(params: &[(String, String)], project: &ProjectId) -> Option<Route> {
    let get = |key: &str| {
        (params.iter())
            .find(|(named, _)| named == key)
            .map(|(_, value)| value.as_str())
    };
    match get("to")? {
        SETTINGS => {
            let page: AppPage = get("page")?.parse().ok()?;
            // Developer only in a build that shows it.
            AppPage::shown().find(|shown| *shown == page)?;
            Some(Route::Settings(Page::App(page)))
        }
        PROJECT_SETTINGS => Some(Route::Settings(Page::Project(
            project.clone(),
            get("page")?.parse().ok()?,
        ))),
        kind => select_route(kind.parse().ok()?, &get, project),
    }
}

/// A selection of `kind` that `get` names in `project`, on one of its page's tabs and at a file
/// of its diff, where its page has them: a link elsewhere names a place that can't be.
fn select_route<'a>(
    kind: SelectionKind,
    get: &impl Fn(&str) -> Option<&'a str>,
    project: &ProjectId,
) -> Option<Route> {
    let selection = selection(kind, get, project)?;
    let tab = match get("tab") {
        None => None,
        Some(tab) if matches!(selection, Selection::Project(_)) => {
            Some(Tab::Project(tab.parse().ok()?))
        }
        Some(tab) if has_tabs(&selection) => Some(Tab::Page(tab.parse().ok()?)),
        Some(_) => return None,
    };
    // A file opens in the diff, which the page's other tabs show in place of.
    let in_diff = matches!(tab, None | Some(Tab::Page(PageTab::Changes)));
    let place = match get("file") {
        None => None,
        Some(path) if in_diff && shows_its_diff(&selection) => Some(CodePlace::File {
            path: path.to_owned(),
            // One that isn't a number opens the file at its top.
            line: get("line").and_then(|line| line.parse().ok()),
        }),
        Some(_) => return None,
    };
    Some(Route::Select {
        selection,
        tab,
        place,
    })
}

/// The selection of `kind` that `get` names in `project`.
fn selection<'a>(
    kind: SelectionKind,
    get: &impl Fn(&str) -> Option<&'a str>,
    project: &ProjectId,
) -> Option<Selection> {
    let project = project.clone();
    let branch = || FullName::try_from(get("branch")?).ok();
    // A commit by its full hash, as a short one could come to name another.
    let commit = || {
        gix::ObjectId::from_hex(get("commit")?.as_bytes())
            .ok()
            .map(CommitId)
    };
    let checkout = || get("worktree").map_or(Checkout::Main, |name| Checkout::Linked(name.into()));
    Some(match kind {
        SelectionKind::Project => Selection::Project(project),
        SelectionKind::Section => {
            let section = match get("section")?.parse().ok()? {
                SectionKind::Upstream => Section::Upstream(branch()?),
                SectionKind::Inactive => Section::Inactive,
                SectionKind::Mesh => Section::Mesh,
            };
            Selection::Section(project, section)
        }
        SelectionKind::Checkout => Selection::Checkout(project, checkout()),
        SelectionKind::Uncommitted => Selection::Uncommitted(project, checkout()),
        // Its id lasts only while the app runs.
        SelectionKind::Terminal => return None,
        SelectionKind::Branch => Selection::Branch(project, Some(branch()?)),
        SelectionKind::Stack => Selection::Stack(project, Some(branch()?)),
        SelectionKind::Commit => Selection::Commit(project, commit()?),
        SelectionKind::Upstream => Selection::Upstream(project, branch()?, commit()?),
        SelectionKind::Mesh => {
            // A branch on another machine by its name there (`name=`), not a ref here.
            let pick = match (get("name"), get("commit")) {
                (None, None) => MeshPick::Checkout,
                (Some(name), None) => MeshPick::Branch(name.to_owned()),
                (Some(name), Some(_)) => MeshPick::Commit(name.to_owned(), commit()?),
                (None, Some(_)) => return None,
            };
            Selection::Mesh(project, get("checkout")?.into(), pick)
        }
    })
}

/// Take `app`'s window where a link points in `project`, once the store has it open: `at`, as
/// [`read`] reads it, or where the project opens by itself if `at` names nothing it can read,
/// which it says. `existed` says whether the project was added before the link.
pub fn land(
    app: &mut AppView,
    project: ProjectId,
    existed: bool,
    at: &[(String, String)],
    window: &mut Window,
    cx: &mut Context<AppView>,
) {
    let route = read(at, &project);
    let page_tab = match &route {
        Some(Route::Select {
            selection,
            tab: Some(Tab::Page(tab)),
            ..
        }) => Some((selection.clone(), *tab)),
        _ => None,
    };
    let settings = (app.store).update(cx, |store, cx| match route {
        Some(Route::Select {
            selection: Selection::Project(id),
            tab: Some(Tab::Project(tab)),
            ..
        }) => {
            store.show_tab(id, tab, cx);
            None
        }
        Some(Route::Select {
            selection, place, ..
        }) => {
            store.select_linked(selection, place, cx);
            None
        }
        Some(Route::Settings(page)) => {
            store.select_opened(&project, existed, cx);
            Some(page)
        }
        None => {
            if !at.is_empty() {
                store.snack_warning("Couldn't read where the link points", cx);
            }
            store.select_opened(&project, existed, cx);
            None
        }
    });
    // As the navigator's marks for a pull request and a brief open a page on its tab.
    if let Some((selection, tab)) = page_tab
        && app.store.read(cx).selection.as_ref() == Some(&selection)
    {
        (app.detail).update(cx, |detail, cx| {
            detail.follow_selection(window, cx);
            detail.set_page_tab(tab, window, cx)
        });
    }
    if let Some(page) = settings {
        app.open_settings_at(page, window, cx);
    }
}

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
        let Some(link) = OpenLink::parse(&url) else {
            // Not the whole link, which can carry a sign-in token.
            let kind = url.split_once('?').map_or(&*url, |(kind, _)| kind);
            tracing::warn!("ignoring a link the app doesn't open: {kind}");
            return;
        };
        (app.store).update(cx, |store, cx| store.open_from_link(link.path, link.at, cx));
    }
    // An open modal, as Settings a link opened, keeps them.
    if app.modal.is_none() {
        app.focus_main(window, cx);
    }
    window.activate_window();
    cx.activate(true);
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator as _;

    use super::*;
    use crate::settings::ProjectPage;

    const HASH: &str = "8c5a2f0e4a2b7c43c0d9a3a8bfbb4e4b2b2d1f6a";

    fn project() -> ProjectId {
        "a-project-id".into()
    }

    fn feature() -> FullName {
        FullName::try_from("refs/heads/feature").expect("a valid ref")
    }

    fn commit_id() -> CommitId {
        CommitId(gix::ObjectId::from_hex(HASH.as_bytes()).expect("a hash"))
    }

    /// Where `link`, a link's place, takes the window, read from the URL it's in.
    fn route_of(link: &str) -> Option<Route> {
        let url = format!("but://open?path=%2Frepo&{link}");
        let link = OpenLink::parse(&url).expect("an open link");
        read(&link.at, &project())
    }

    /// Links to places of `kind`, as an agent writes them, and what each selects, or `None` for a
    /// kind no link names. No wildcard: a new kind doesn't build until it's decided here.
    fn links_to(kind: SelectionKind) -> Option<Vec<(String, Selection)>> {
        let (p, linked) = (project(), Checkout::Linked("feature".into()));
        Some(match kind {
            SelectionKind::Project => vec![("to=project".into(), Selection::Project(p))],
            SelectionKind::Section => vec![
                (
                    "to=section&section=upstream&branch=refs/heads/feature".into(),
                    Selection::Section(p.clone(), Section::Upstream(feature())),
                ),
                (
                    "to=section&section=inactive".into(),
                    Selection::Section(p.clone(), Section::Inactive),
                ),
                (
                    "to=section&section=mesh".into(),
                    Selection::Section(p, Section::Mesh),
                ),
            ],
            SelectionKind::Checkout => vec![
                (
                    "to=checkout".into(),
                    Selection::Checkout(p.clone(), Checkout::Main),
                ),
                (
                    "to=checkout&worktree=feature".into(),
                    Selection::Checkout(p, linked),
                ),
            ],
            SelectionKind::Uncommitted => vec![
                (
                    "to=uncommitted".into(),
                    Selection::Uncommitted(p.clone(), Checkout::Main),
                ),
                (
                    "to=uncommitted&worktree=feature".into(),
                    Selection::Uncommitted(p, linked),
                ),
            ],
            SelectionKind::Terminal => return None,
            SelectionKind::Branch => vec![(
                "to=branch&branch=refs/heads/feature".into(),
                Selection::Branch(p, Some(feature())),
            )],
            SelectionKind::Stack => vec![(
                "to=stack&branch=refs/heads/feature".into(),
                Selection::Stack(p, Some(feature())),
            )],
            SelectionKind::Commit => vec![(
                format!("to=commit&commit={HASH}"),
                Selection::Commit(p, commit_id()),
            )],
            SelectionKind::Upstream => vec![(
                format!("to=upstream&branch=refs/heads/feature&commit={HASH}"),
                Selection::Upstream(p, feature(), commit_id()),
            )],
            SelectionKind::Mesh => {
                let mesh = |pick| Selection::Mesh(p.clone(), "machine".into(), pick);
                vec![
                    ("to=mesh&checkout=machine".into(), mesh(MeshPick::Checkout)),
                    (
                        "to=mesh&checkout=machine&name=feature".into(),
                        mesh(MeshPick::Branch("feature".into())),
                    ),
                    (
                        format!("to=mesh&checkout=machine&name=feature&commit={HASH}"),
                        mesh(MeshPick::Commit("feature".into(), commit_id())),
                    ),
                ]
            }
        })
    }

    #[test]
    fn every_kind_of_place_opens_from_its_link() {
        for kind in SelectionKind::iter() {
            let Some(links) = links_to(kind) else {
                let name: &str = kind.into();
                assert_eq!(
                    route_of(&format!("to={name}")),
                    None,
                    "{kind:?} is left out, so its link opens the project and says so"
                );
                continue;
            };
            assert!(!links.is_empty(), "{kind:?} isn't left out, so it has one");
            for (link, selection) in links {
                assert_eq!(
                    route_of(&link),
                    Some(Route::Select {
                        selection,
                        tab: None,
                        place: None,
                    }),
                    "{link} takes the window where it names"
                );
            }
        }
    }

    #[test]
    fn a_page_opens_on_any_of_its_tabs() {
        for tab in ProjectTab::ALL {
            let name: &str = tab.into();
            assert_eq!(
                route_of(&format!("to=project&tab={name}")),
                Some(Route::Select {
                    selection: Selection::Project(project()),
                    tab: Some(Tab::Project(tab)),
                    place: None,
                }),
                "the project's page on its {name} tab"
            );
        }
        for (name, tab) in [
            ("changes", PageTab::Changes),
            ("review", PageTab::Review),
            ("brief", PageTab::Brief),
        ] {
            assert_eq!(
                route_of(&format!("to=branch&branch=refs/heads/feature&tab={name}")),
                Some(Route::Select {
                    selection: Selection::Branch(project(), Some(feature())),
                    tab: Some(Tab::Page(tab)),
                    place: None,
                }),
                "a branch's page on its {name} tab"
            );
        }
    }

    #[test]
    fn a_diff_opens_at_a_line_of_a_file() {
        let at = |line| {
            Some(Route::Select {
                selection: Selection::Branch(project(), Some(feature())),
                tab: None,
                place: Some(CodePlace::File {
                    path: "src/a b.rs".into(),
                    line,
                }),
            })
        };
        let link = "to=branch&branch=refs/heads/feature&file=src/a%20b.rs";
        assert_eq!(
            route_of(&format!("{link}&line=4")),
            at(Some(4)),
            "at the hunk with that line"
        );
        assert_eq!(
            route_of(&format!("{link}&line=x")),
            at(None),
            "a line that isn't a number opens the file at its top"
        );
        assert_eq!(
            route_of(&format!("{link}&tab=changes")),
            Some(Route::Select {
                selection: Selection::Branch(project(), Some(feature())),
                tab: Some(Tab::Page(PageTab::Changes)),
                place: Some(CodePlace::File {
                    path: "src/a b.rs".into(),
                    line: None,
                }),
            }),
            "on the Changes tab, which shows the diff"
        );
    }

    #[test]
    fn every_page_of_settings_opens_from_its_link() {
        // Tests run as a dev build, which shows the Developer page too.
        for page in AppPage::ALL {
            let name: &str = page.into();
            assert_eq!(
                route_of(&format!("to=settings&page={name}")),
                Some(Route::Settings(Page::App(page))),
                "the app's {name} settings"
            );
        }
        for page in ProjectPage::ALL {
            let name: &str = page.into();
            assert_eq!(
                route_of(&format!("to=project-settings&page={name}")),
                Some(Route::Settings(Page::Project(project(), page))),
                "the project's {name} settings"
            );
        }
    }

    #[test]
    fn a_link_naming_what_cant_be_reads_as_nothing() {
        let links = [
            "to=timeline",
            "to=branch",
            "to=branch&branch=not%20a..ref",
            "to=commit&commit=8c5a2f0e",
            "to=project&tab=timeline",
            "to=branch&branch=refs/heads/x&tab=graph",
            "to=checkout&file=src/a.rs",
            // Review shows in place of the diff the file would open in.
            "to=branch&branch=refs/heads/x&tab=review&file=src/a.rs",
            "to=section&section=elsewhere",
            "to=settings&page=nowhere",
            "to=project-settings&page=ai",
            "branch=refs/heads/x",
        ]
        .map(String::from)
        .into_iter()
        .chain([
            // Only a branch's page and an upstream commit's have tabs past Changes.
            format!("to=commit&commit={HASH}&tab=review"),
            format!("to=mesh&checkout=x&commit={HASH}"),
        ]);
        for link in links {
            assert_eq!(
                route_of(&link),
                None,
                "{link} opens the project and says it couldn't read where"
            );
        }
    }
}
