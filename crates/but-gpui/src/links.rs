//! Following `but://` links in the window: the one the app was launched with, and those that
//! come while it runs, to the place each names ([`Route`]). How they arrive is `backend::links`.
//!
//! Everywhere the window can be has a link, so an agent can open it: every kind of
//! [`Selection`] but a terminal, a page on any of its tabs, and every page of settings. A link
//! names its place by `to=`, its kind's own name, and what that kind takes, as
//! `to=branch&branch=refs/heads/x&tab=review`, `to=project&tab=graph` or `to=settings&page=git`,
//! and a file of a diff by `file=` and `line=`. Reading one matches every kind with no wildcard,
//! so a new kind doesn't build until it's read here, and the tests open a link to every one and
//! write `LINKS.md`, the list of them agents read.

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
    CodePlace, MeshPick, MeshPickKind, ProjectId, ProjectTab, Section, SectionKind, Selection,
    SelectionKind,
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

/// What the app says when the place of `kind` a link named isn't there now. No wildcard: a new
/// kind doesn't build until it has its own.
fn missing(kind: SelectionKind) -> &'static str {
    match kind {
        SelectionKind::Project => "The link's project isn't open now",
        SelectionKind::Section => "The link's section is empty now",
        SelectionKind::Checkout => "The link's worktree isn't there now",
        SelectionKind::Uncommitted => "The link's worktree has nothing uncommitted now",
        SelectionKind::Terminal => "The link's terminal isn't open now",
        SelectionKind::Branch => "The link's branch isn't in the workspace now",
        SelectionKind::Stack => "The link's stack isn't in the workspace now",
        SelectionKind::Commit => "The link's commit isn't in the workspace now",
        SelectionKind::Upstream => "The link's commit isn't on the branch's remote now",
        SelectionKind::Mesh => "The link's checkout on another machine isn't there now",
    }
}

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
    // A linked worktree by `worktree=`, and with none, the main one. A new kind of checkout
    // doesn't build until it's read here.
    match Checkout::Main {
        Checkout::Main | Checkout::Linked(_) => {}
    }
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
            let name = || get("name").map(str::to_owned);
            let pick = match get("pick")?.parse().ok()? {
                MeshPickKind::Checkout => MeshPick::Checkout,
                MeshPickKind::Branch => MeshPick::Branch(name()?),
                MeshPickKind::Commit => MeshPick::Commit(name()?, commit()?),
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
            let missing = missing(SelectionKind::from(&selection));
            store.select_linked(selection, place, missing, cx);
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
    use but_path::AppChannel;
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

    /// A link to a place of `kind`, as an agent writes one, or `None` for a kind no link names.
    /// `LINKS.md` lists these. No wildcard: a new kind doesn't build until it has one, or none
    /// on purpose, and the tests read each back.
    fn example(kind: SelectionKind) -> Option<&'static str> {
        Some(match kind {
            SelectionKind::Project => "to=project",
            SelectionKind::Section => "to=section&section=upstream&branch=refs/heads/feature",
            SelectionKind::Checkout => "to=checkout&worktree=feature",
            SelectionKind::Uncommitted => "to=uncommitted&worktree=feature",
            // Its id lasts only while the app runs.
            SelectionKind::Terminal => return None,
            SelectionKind::Branch => "to=branch&branch=refs/heads/feature",
            SelectionKind::Stack => "to=stack&branch=refs/heads/feature",
            SelectionKind::Commit => "to=commit&commit=8c5a2f0e4a2b7c43c0d9a3a8bfbb4e4b2b2d1f6a",
            SelectionKind::Upstream => {
                "to=upstream&branch=refs/heads/feature&commit=8c5a2f0e4a2b7c43c0d9a3a8bfbb4e4b2b2d1f6a"
            }
            SelectionKind::Mesh => "to=mesh&checkout=other-machine&pick=branch&name=feature",
        })
    }

    /// Every shape of place a link names, and what it selects: each kind's example, then the other
    /// shapes a kind comes in.
    fn shapes() -> Vec<(String, Selection)> {
        let example = |kind| example(kind).expect("a link names it").to_owned();
        let (p, linked) = (project(), Checkout::Linked("feature".into()));
        let mesh = |pick| Selection::Mesh(p.clone(), "other-machine".into(), pick);
        vec![
            (
                example(SelectionKind::Project),
                Selection::Project(p.clone()),
            ),
            (
                example(SelectionKind::Section),
                Selection::Section(p.clone(), Section::Upstream(feature())),
            ),
            (
                "to=section&section=inactive".into(),
                Selection::Section(p.clone(), Section::Inactive),
            ),
            (
                "to=section&section=mesh".into(),
                Selection::Section(p.clone(), Section::Mesh),
            ),
            (
                example(SelectionKind::Checkout),
                Selection::Checkout(p.clone(), linked.clone()),
            ),
            (
                "to=checkout".into(),
                Selection::Checkout(p.clone(), Checkout::Main),
            ),
            (
                example(SelectionKind::Uncommitted),
                Selection::Uncommitted(p.clone(), linked),
            ),
            (
                "to=uncommitted".into(),
                Selection::Uncommitted(p.clone(), Checkout::Main),
            ),
            (
                example(SelectionKind::Branch),
                Selection::Branch(p.clone(), Some(feature())),
            ),
            (
                example(SelectionKind::Stack),
                Selection::Stack(p.clone(), Some(feature())),
            ),
            (
                example(SelectionKind::Commit),
                Selection::Commit(p.clone(), commit_id()),
            ),
            (
                example(SelectionKind::Upstream),
                Selection::Upstream(p.clone(), feature(), commit_id()),
            ),
            (
                example(SelectionKind::Mesh),
                mesh(MeshPick::Branch("feature".into())),
            ),
            (
                "to=mesh&checkout=other-machine&pick=checkout".into(),
                mesh(MeshPick::Checkout),
            ),
            (
                format!("to=mesh&checkout=other-machine&pick=commit&name=feature&commit={HASH}"),
                mesh(MeshPick::Commit("feature".into(), commit_id())),
            ),
        ]
    }

    #[test]
    fn every_kind_of_place_opens_from_its_example() {
        for kind in SelectionKind::iter() {
            let name: &str = kind.into();
            let Some(example) = example(kind) else {
                assert_eq!(
                    route_of(&format!("to={name}")),
                    None,
                    "{kind:?} is left out, so its link opens the project and says so"
                );
                continue;
            };
            let opened = match route_of(example) {
                Some(Route::Select { selection, .. }) => Some(SelectionKind::from(&selection)),
                Some(Route::Settings(_)) | None => None,
            };
            assert_eq!(opened, Some(kind), "{example} opens a {name}");
        }
    }

    #[test]
    fn every_shape_of_place_reads_whole() {
        for (link, selection) in shapes() {
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
        for tab in PageTab::iter() {
            let name: &str = tab.into();
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
            // A mesh link says which pick it names.
            format!("to=mesh&checkout=x&commit={HASH}"),
            // A commit on another machine by the branch it's listed under.
            format!("to=mesh&checkout=x&pick=commit&commit={HASH}"),
            "to=mesh&checkout=x&pick=elsewhere".into(),
        ]);
        for link in links {
            assert_eq!(
                route_of(&link),
                None,
                "{link} opens the project and says it couldn't read where"
            );
        }
    }

    /// `names` as a sentence lists them: `` `a`, `b` or `c` ``.
    fn either(names: &[&str]) -> String {
        let names: Vec<String> = names.iter().map(|name| format!("`{name}`")).collect();
        match names.split_last() {
            Some((last, [])) => last.clone(),
            Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
            None => String::new(),
        }
    }

    /// `LINKS.md`, the list of places agents read, as the app's own lists make it: each kind of
    /// place by its example, and the tabs, files and settings pages a link opens, as reading a
    /// link takes them.
    fn list_of_places() -> String {
        let examples: Vec<_> = (SelectionKind::iter())
            .filter_map(|kind| Some((<&str>::from(kind), example(kind)?)))
            .collect();
        let left_out: Vec<&str> = (SelectionKind::iter())
            .filter(|kind| example(*kind).is_none())
            .map(Into::into)
            .collect();
        let taking = |extra: &str| -> Vec<&str> {
            (examples.iter())
                .filter(|(_, example)| route_of(&format!("{example}&{extra}")).is_some())
                .map(|(name, _)| *name)
                .collect()
        };
        let schemes = AppChannel::ALL.map(AppChannel::scheme);
        let sections: Vec<&str> = SectionKind::iter().map(Into::into).collect();
        let picks: Vec<&str> = MeshPickKind::iter().map(Into::into).collect();
        let project_tabs = ProjectTab::ALL.map(<&str>::from);
        let page_tabs: Vec<&str> = PageTab::iter().map(Into::into).collect();
        let app_pages = AppPage::ALL.map(<&str>::from);
        let project_pages = ProjectPage::ALL.map(<&str>::from);
        let places: Vec<String> = (examples.iter())
            .map(|(name, example)| format!("| `{name}` | `{example}` |"))
            .collect();
        format!(
            "# Links into the GitButler app

<!-- Written by `but-gpui`'s `links` tests from the app's own lists, so it's never edited by hand:
change the app, then run `SNAPSHOTS=overwrite cargo test -p but-gpui links`. -->

`but://open?path=<project>&<place>` opens the project in the directory `<project>`, percent-encoded,
at the place the parameters after it name, and without any, where the project opens by itself.
The scheme is the build's: {schemes}.

## Places

| `to=` | A link to one |
| --- | --- |
{places}

No link opens a {left_out}. `section=` is {sections}, and `pick=` is {picks}.

## Tabs

`tab=` opens a `project` on {project_tabs}. Added to the link above to a {page_tab_kinds},
it opens that page on {page_tabs}.

## Files

Added to the link above to a {file_kinds}, `file=` opens its diff at a file, relative to the
project with `/`, and `line=` at the hunk with that line of its new side. With a `tab=`, only on
`changes`.

## Settings

`to=settings&page=` opens the app's settings on {app_pages}, and `to=project-settings&page=`
the project's on {project_pages}.
",
            schemes = either(&schemes),
            places = places.join("\n"),
            left_out = either(&left_out),
            sections = either(&sections),
            picks = either(&picks),
            project_tabs = either(&project_tabs),
            page_tab_kinds = either(&taking("tab=changes")),
            page_tabs = either(&page_tabs),
            file_kinds = either(&taking("file=a.rs")),
            app_pages = either(&app_pages),
            project_pages = either(&project_pages),
        )
    }

    #[test]
    fn the_list_of_places_is_current() {
        // Written from the app's own lists, so a place added, renamed or dropped changes it.
        snapbox::assert_data_eq!(list_of_places(), snapbox::file!["../LINKS.md"].raw());
    }

    /// What the CLI writes for the changes its IDs name reads as those changes, so `but _open
    /// --gui` and the app agree. No wildcard: a new kind of changes doesn't build until it's read.
    #[test]
    fn the_changes_cli_ids_name_read_as_them() {
        use but_path::link::{Changes, FileAt};

        let file = FileAt {
            path: "src/a.rs".into(),
            line: Some(4),
        };
        for changes in [
            Changes::Branch("refs/heads/feature".into()),
            Changes::Commit(HASH.into()),
            Changes::Uncommitted { worktree: None },
            Changes::Uncommitted {
                worktree: Some("feature".into()),
            },
        ] {
            let selection = match &changes {
                Changes::Branch(_) => Selection::Branch(project(), Some(feature())),
                Changes::Commit(_) => Selection::Commit(project(), commit_id()),
                Changes::Uncommitted { worktree } => Selection::Uncommitted(
                    project(),
                    (worktree.clone()).map_or(Checkout::Main, |name| Checkout::Linked(name.into())),
                ),
            };
            assert_eq!(
                read(&changes.at(None), &project()),
                Some(Route::Select {
                    selection: selection.clone(),
                    tab: None,
                    place: None,
                }),
                "{changes:?}"
            );
            assert_eq!(
                read(&changes.at(Some(&file)), &project()),
                Some(Route::Select {
                    selection,
                    tab: None,
                    place: Some(CodePlace::File {
                        path: "src/a.rs".into(),
                        line: Some(4),
                    }),
                }),
                "{changes:?} at a line of one of its files"
            );
        }
    }
}
