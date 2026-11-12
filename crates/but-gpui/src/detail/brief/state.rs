//! What's kept for a brief: for each page, how its brief is being looked at, and for the view, what
//! the tab is doing now. Here rather than in the page and the view themselves, so that they hold one
//! field for it and it can go without touching them.

use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

use gpui::{Bounds, Entity, ListAlignment, ListOffset, ListState, Pixels, Subscription, Task};

use super::ReadKey;
use super::test_facts::TestFacts;
use crate::backend::accounts::GitButlerUser;
use crate::backend::briefs::{Activity, BriefAnchor, BriefPage, CommentTo, Found, Note};
use crate::detail::DiffKey;
use crate::ui::fixed;
use crate::ui::text_input::TextInput;

/// What the view holds for the Brief tab it's showing, whichever page that's on.
pub(in crate::detail) struct BriefView {
    /// The branch the page last showed, and the asking about it: see `Detail::ask_briefs`.
    pub(in crate::detail) asked: Option<BriefAnchor>,
    /// Who was signed in then. Another account's answer isn't this one's, so someone else signing
    /// in asks again, which drops the ask under way.
    pub(super) asked_as: Option<GitButlerUser>,
    pub(super) _ask: Task<()>,
    /// The version of a brief being read for the tab, and the reading.
    pub(in crate::detail) reading: Option<(ReadKey, Task<()>)>,
    /// What's written is on its way out. It's here as a page outlives a send to it.
    pub(super) sending: bool,
    /// How wide the tab was drawn, in design pixels, for its rows to go by.
    pub(super) room: Cell<f32>,
    /// What the comment the pointer is on quotes, which tints those words more strongly.
    pub(super) said_hovered: Option<String>,
    /// The words of a brief last pointed at, by what tells them apart, whose pencil shows.
    pub(super) hovered: Option<usize>,
    /// The version whose talk is kept current while its tab shows, and that keeping.
    pub(super) talk: Option<((String, u32), Task<()>)>,
    /// A page with a brief opens on it, until a tab with its changes is picked over it: whoever's
    /// reading one brief after another stays reading.
    pub(in crate::detail) first: bool,
    pub(super) input: Option<BriefInput>,
    pub(in crate::detail) list: ListState,
}

impl BriefView {
    pub(in crate::detail) fn new() -> Self {
        Self {
            asked: None,
            asked_as: None,
            _ask: Task::ready(()),
            reading: None,
            sending: false,
            room: Cell::new(0.),
            said_hovered: None,
            hovered: None,
            talk: None,
            first: true,
            input: None,
            list: ListState::new(0, ListAlignment::Top, fixed(200.)),
        }
    }
}

/// A comment or a shortcut being written, with its field.
pub(in crate::detail) struct BriefInput {
    pub(super) field: Entity<TextInput>,
    pub(super) writing: Writing,
    /// The comment not sent yet that it would take the place of, which doesn't show meanwhile.
    pub(super) replaces: Option<i64>,
    /// What it's about, which can stop being what shows.
    pub(super) version: (String, u32),
    /// What's in the field so far, for keeping it where the field can't be asked.
    pub(super) written: String,
    /// Where it was drawn, once it has been, for having it in view.
    pub(super) drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub(super) _listen: Subscription,
}

impl BriefInput {
    pub(super) fn is_about(&self, id: &str, version: u32) -> bool {
        self.version.0 == id && self.version.1 == version
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::detail) enum Writing {
    /// A note, kept until it's sent.
    Note(CommentTo),
    /// A shortcut to a common note, at `editing` of them or a new one, written beside `to`.
    Shortcut {
        editing: Option<usize>,
        to: CommentTo,
    },
}

impl Writing {
    /// What it's about, or beside.
    pub(in crate::detail) fn about(&self) -> &CommentTo {
        match self {
            Writing::Note(to) | Writing::Shortcut { to, .. } => to,
        }
    }
}

/// What can be opened on a Brief tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::detail) enum BriefFold {
    /// A fold of blocks, by its first.
    Part(usize),
    /// The files it shows no code of, listed.
    Unshown,
    /// What's gone since the version last read, listed.
    Removed,
    /// What hasn't changed since the version last read, shown too.
    Unchanged,
    /// All the branch changes in the file of a code card, by its block.
    WholeFile(usize),
}

/// The brief of a page's branch and how it's being looked at.
#[derive(Default)]
pub(in crate::detail) struct BriefState {
    /// The brief of the branch, as the store knows it.
    pub(in crate::detail) found: Option<Found>,
    /// Its version, read as its tab first shows, or why it couldn't be. It stays while a newer
    /// version is read.
    pub(in crate::detail) shown: Option<Result<BriefPage, String>>,
    pub(in crate::detail) open: HashSet<BriefFold>,
    /// The version picked over the one that matched the branch.
    pub(in crate::detail) version: Option<u32>,
    /// What's been said about it, as last asked.
    pub(in crate::detail) activity: Activity,
    /// What's written about it and not sent.
    pub(in crate::detail) notes: Vec<Note>,
    /// Where the tab was as a link in it was followed, to go back to.
    pub(in crate::detail) back: Option<ListOffset>,
    /// The row ↓ and ↑ don't, j and k do: where a note goes without a passage selected.
    pub(in crate::detail) cursor: Option<usize>,
    /// It was the keys that put it there, so it's marked.
    pub(in crate::detail) cursor_shown: bool,
    /// Notes went out with no agent waiting on them, so one has to be told.
    pub(in crate::detail) unheard: bool,
    /// Which tab it opens on has been settled. A brief that lands after that doesn't take the
    /// page from what's being read.
    pub(in crate::detail) settled: bool,
    /// The diff and version the names of code in what's shown were last joined up from.
    pub(in crate::detail) linked: Option<(DiffKey, u32, Option<crate::backend::CommitId>)>,
    /// The branch's own diff was gone to from here, which the key back returns from.
    pub(in crate::detail) left_for_code: bool,
    /// What that diff says about the branch's tests.
    pub(in crate::detail) tests: Option<TestFacts>,
    /// The files that diff changes which tend to matter and the brief neither shows nor names.
    pub(in crate::detail) unnamed: Vec<String>,
    /// How far the tab was scrolled as the page was left, for when its rows are next drawn.
    pub(in crate::detail) top: Cell<Option<ListOffset>>,
}

impl BriefState {
    /// The version read, unless it's not in yet or couldn't be read.
    pub(in crate::detail) fn page(&self) -> Option<&BriefPage> {
        self.shown.as_ref()?.as_ref().ok()
    }

    pub(in crate::detail) fn page_mut(&mut self) -> Option<&mut BriefPage> {
        self.shown.as_mut()?.as_mut().ok()
    }

    pub(in crate::detail) fn is_open(&self, fold: BriefFold) -> bool {
        self.open.contains(&fold)
    }

    pub(in crate::detail) fn toggle(&mut self, fold: BriefFold) {
        if !self.open.remove(&fold) {
            self.open.insert(fold);
        }
    }
}
