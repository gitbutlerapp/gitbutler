use std::{convert::Infallible, ops::Deref};

use anyhow::Context as _;
use bstr::{BStr, BString};
use but_core::{ChangeId, ref_metadata::StackId};
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;
use nonempty::NonEmpty;
use strum::VariantArray;

use crate::{
    CliId, IdMap,
    command::legacy::status::{
        StatusOutputLine,
        output::StatusOutputLineData,
        tui::{
            app::{
                App, CommandReturnMode, normal_mode::NormalMode, pick_changes_mode::PickChangesMode,
            },
            mode::Mode,
        },
    },
    id::{ShortId, UncommittedHunkOrFile},
};

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Marks {
    #[default]
    Empty,
    Hunks(NonEmpty<UncommittedHunkOrFile>),
    Commits(NonEmpty<MarkedCommit>),
    CommittedFiles(NonEmpty<MarkedCommittedFile>),
}

impl Marks {
    pub fn clear(&mut self) {
        *self = Self::Empty;
    }

    pub fn as_hunks(&self) -> Option<&NonEmpty<UncommittedHunkOrFile>> {
        match self {
            Self::Hunks(hunks) => Some(hunks),
            _ => None,
        }
    }

    pub fn as_commits(&self) -> Option<&NonEmpty<MarkedCommit>> {
        match self {
            Self::Commits(commits) => Some(commits),
            _ => None,
        }
    }

    pub fn as_committed_files(&self) -> Option<&NonEmpty<MarkedCommittedFile>> {
        match self {
            Self::CommittedFiles(files) => Some(files),
            _ => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    pub fn contains_cli_id(&self, cli_id: &CliId) -> bool {
        self.iter().any(|mark| mark.matches_cli_id(cli_id))
    }

    pub fn iter(&self) -> impl Iterator<Item = MarkableRef<'_>> {
        self.as_ref().iter()
    }

    pub fn as_ref(&self) -> MarksRef<'_> {
        match self {
            Self::Empty => MarksRef::Empty,
            Self::Hunks(hunks) => MarksRef::from_hunks(hunks),
            Self::Commits(commits) => MarksRef::from_commits(commits),
            Self::CommittedFiles(files) => MarksRef::from_committed_files(files),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MarksRef<'a> {
    Empty,
    Hunks {
        head: &'a UncommittedHunkOrFile,
        tail: &'a [UncommittedHunkOrFile],
    },
    Commits {
        head: &'a MarkedCommit,
        tail: &'a [MarkedCommit],
    },
    CommittedFiles {
        head: &'a MarkedCommittedFile,
        tail: &'a [MarkedCommittedFile],
    },
}

impl<'a> MarksRef<'a> {
    pub fn from_hunks(hunks: &'a NonEmpty<UncommittedHunkOrFile>) -> Self {
        Self::Hunks {
            head: &hunks.head,
            tail: &hunks.tail,
        }
    }

    #[expect(dead_code)]
    pub fn from_hunk_slice(hunks: &'a [UncommittedHunkOrFile]) -> Self {
        let Some((head, tail)) = hunks.split_first() else {
            return Self::Empty;
        };
        Self::Hunks { head, tail }
    }

    pub fn from_commits(commits: &'a NonEmpty<MarkedCommit>) -> Self {
        Self::Commits {
            head: &commits.head,
            tail: &commits.tail,
        }
    }

    pub fn from_committed_files(commits: &'a NonEmpty<MarkedCommittedFile>) -> Self {
        Self::CommittedFiles {
            head: &commits.head,
            tail: &commits.tail,
        }
    }

    pub fn is_empty(self) -> bool {
        matches!(self, Self::Empty)
    }

    pub fn contains_cli_id(self, cli_id: &CliId) -> bool {
        match self {
            Self::Empty => false,
            Self::Hunks { head, tail } => {
                let CliId::UncommittedHunkOrFile(uncommitted) = cli_id else {
                    return false;
                };
                std::iter::once(head)
                    .chain(tail)
                    .any(|hunk| hunk == uncommitted)
            }
            Self::Commits { head, tail } => {
                let CliId::Commit {
                    commit_id,
                    id,
                    change_id: _,
                } = cli_id
                else {
                    return false;
                };
                std::iter::once(head)
                    .chain(tail)
                    .any(|commit| commit.commit_id == *commit_id && commit.id == *id)
            }
            Self::CommittedFiles { head, tail } => {
                let CliId::CommittedFile {
                    commit_id,
                    path,
                    id,
                } = cli_id
                else {
                    return false;
                };
                std::iter::once(head).chain(tail).any(|file| {
                    file.commit_id == *commit_id && &file.path == path && &file.id == id
                })
            }
        }
    }

    pub fn to_owned(self) -> Marks {
        match self {
            Self::Empty => Marks::Empty,
            Self::Hunks { head, tail } => {
                let mut hunks = NonEmpty::new(head.clone());
                hunks.extend(tail.iter().cloned());
                Marks::Hunks(hunks)
            }
            Self::Commits { head, tail } => {
                let mut commits = NonEmpty::new(head.clone());
                commits.extend(tail.iter().cloned());
                Marks::Commits(commits)
            }
            Self::CommittedFiles { head, tail } => {
                let mut files = NonEmpty::new(head.clone());
                files.extend(tail.iter().cloned());
                Marks::CommittedFiles(files)
            }
        }
    }

    pub fn iter(self) -> impl Iterator<Item = MarkableRef<'a>> {
        match self {
            Self::Empty => Box::new(std::iter::empty()) as Box<dyn Iterator<Item = _>>,
            Self::Hunks { head, tail } => {
                let iter = std::iter::once(head)
                    .chain(tail)
                    .map(MarkableRef::Uncommitted);
                Box::new(iter)
            }
            Self::Commits { head, tail } => {
                let iter = std::iter::once(head)
                    .chain(tail)
                    .map(|commit| commit.as_ref())
                    .map(MarkableRef::Commit);
                Box::new(iter)
            }
            Self::CommittedFiles { head, tail } => {
                let iter = std::iter::once(head)
                    .chain(tail)
                    .map(|file| file.as_ref())
                    .map(MarkableRef::CommittedFile);
                Box::new(iter)
            }
        }
    }

    pub fn contains_child_of(self, cli_id: &CliId) -> bool {
        let CliId::UncommittedHunkOrFile(parent) = cli_id else {
            return false;
        };
        if !parent.is_entire_file {
            return false;
        }

        self.iter().any(|mark| {
            let MarkableRef::Uncommitted(child) = mark else {
                return false;
            };

            !child.is_entire_file
                && child.hunk_assignments.iter().any(|child_assignment| {
                    parent
                        .hunk_assignments
                        .iter()
                        .any(|parent_assignment| parent_assignment == child_assignment)
                })
        })
    }
}

pub trait MarkStore<T> {
    type Error;

    fn contains_mark(&self, mark: &T) -> bool;
    fn insert_mark(&mut self, mark: T) -> Result<(), Self::Error>;
    fn remove_mark(&mut self, mark: &T);
}

impl MarkStore<Markable> for Marks {
    type Error = anyhow::Error;

    fn contains_mark(&self, mark: &Markable) -> bool {
        match mark {
            Markable::Uncommitted(hunk) => self.contains_mark(hunk),
            Markable::Commit(commit) => self.contains_mark(commit),
            Markable::CommittedFile(file) => self.contains_mark(file),
        }
    }

    fn insert_mark(&mut self, mark: Markable) -> Result<(), Self::Error> {
        match mark {
            Markable::Uncommitted(hunk) => self.insert_mark(hunk),
            Markable::Commit(commit) => self.insert_mark(commit),
            Markable::CommittedFile(file) => self.insert_mark(file),
        }
    }

    fn remove_mark(&mut self, mark: &Markable) {
        match mark {
            Markable::Uncommitted(hunk) => self.remove_mark(hunk),
            Markable::Commit(commit) => self.remove_mark(commit),
            Markable::CommittedFile(file) => self.remove_mark(file),
        }
    }
}

impl MarkStore<UncommittedHunkOrFile> for Marks {
    type Error = anyhow::Error;

    fn contains_mark(&self, mark: &UncommittedHunkOrFile) -> bool {
        self.as_hunks()
            .is_some_and(|hunks| hunks.iter().any(|hunk| hunk == mark))
    }

    fn insert_mark(&mut self, mark: UncommittedHunkOrFile) -> Result<(), Self::Error> {
        if self.contains_mark(&mark) {
            return Ok(());
        }
        match self {
            Self::Empty => *self = Self::Hunks(NonEmpty::new(mark)),
            Self::Hunks(hunks) => hunks.push(mark),
            _ => anyhow::bail!("cannot mix mark sources"),
        }
        Ok(())
    }

    fn remove_mark(&mut self, mark: &UncommittedHunkOrFile) {
        let Self::Hunks(hunks) = self else {
            return;
        };

        if remove_from_non_empty(hunks, |marked| marked == mark) {
            *self = Self::Empty;
        }
    }
}

impl MarkStore<MarkedCommit> for Marks {
    type Error = anyhow::Error;

    fn contains_mark(&self, mark: &MarkedCommit) -> bool {
        self.as_commits()
            .is_some_and(|commits| commits.iter().any(|commit| commit == mark))
    }

    fn insert_mark(&mut self, mark: MarkedCommit) -> Result<(), Self::Error> {
        if self.contains_mark(&mark) {
            return Ok(());
        }
        match self {
            Self::Empty => *self = Self::Commits(NonEmpty::new(mark)),
            Self::Commits(commits) => commits.push(mark),
            _ => anyhow::bail!("cannot mix mark sources"),
        }
        Ok(())
    }

    fn remove_mark(&mut self, mark: &MarkedCommit) {
        let Self::Commits(commits) = self else {
            return;
        };

        if remove_from_non_empty(commits, |marked| marked.commit_id == mark.commit_id) {
            *self = Self::Empty;
        }
    }
}

impl MarkStore<MarkedCommittedFile> for Marks {
    type Error = anyhow::Error;

    fn contains_mark(&self, mark: &MarkedCommittedFile) -> bool {
        self.as_committed_files()
            .is_some_and(|files| files.iter().any(|file| file == mark))
    }

    fn insert_mark(&mut self, mark: MarkedCommittedFile) -> Result<(), Self::Error> {
        if self.contains_mark(&mark) {
            return Ok(());
        }
        match self {
            Self::Empty => *self = Self::CommittedFiles(NonEmpty::new(mark)),
            Self::CommittedFiles(files) => {
                if files.head.commit_id != mark.commit_id {
                    anyhow::bail!("cannot mark files from multiple commits");
                }
                files.push(mark);
            }
            _ => anyhow::bail!("cannot mix mark sources"),
        }
        Ok(())
    }

    fn remove_mark(&mut self, mark: &MarkedCommittedFile) {
        let Self::CommittedFiles(files) = self else {
            return;
        };

        if remove_from_non_empty(files, |marked| {
            marked.commit_id == mark.commit_id && marked.path == mark.path
        }) {
            *self = Self::Empty;
        }
    }
}

#[derive(Debug, Clone)]
pub struct SingleSourceMarks<T>(Vec<T>);

impl<T> Default for SingleSourceMarks<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> SingleSourceMarks<T> {
    #[expect(dead_code)]
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl<T> Deref for SingleSourceMarks<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> MarkStore<T> for SingleSourceMarks<T>
where
    T: PartialEq,
{
    type Error = Infallible;

    fn contains_mark(&self, mark: &T) -> bool {
        self.0.iter().any(|hunk| hunk == mark)
    }

    fn insert_mark(&mut self, mark: T) -> Result<(), Self::Error> {
        if self.contains_mark(&mark) {
            return Ok(());
        }
        self.0.push(mark);
        Ok(())
    }

    fn remove_mark(&mut self, mark: &T) {
        if let Some(index) = self.0.iter().position(|hunk| hunk == mark) {
            self.0.remove(index);
        }
    }
}

fn remove_from_non_empty<T>(items: &mut NonEmpty<T>, predicate: impl Fn(&T) -> bool) -> bool {
    let Some(index) = items.iter().position(predicate) else {
        return false;
    };

    if index == 0 {
        if items.tail.is_empty() {
            true
        } else {
            items.head = items.tail.remove(0);
            false
        }
    } else {
        items.tail.remove(index - 1);
        false
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Markable {
    Uncommitted(UncommittedHunkOrFile),
    Commit(MarkedCommit),
    CommittedFile(MarkedCommittedFile),
}

impl Markable {
    pub fn into_cli_id(self) -> CliId {
        match self {
            Markable::Uncommitted(uncommitted_cli_id) => {
                CliId::UncommittedHunkOrFile(uncommitted_cli_id)
            }
            Markable::Commit(MarkedCommit {
                commit_id,
                id,
                change_id,
            }) => CliId::Commit {
                commit_id,
                id,
                change_id,
            },
            Markable::CommittedFile(MarkedCommittedFile {
                commit_id,
                path,
                id,
            }) => CliId::CommittedFile {
                commit_id,
                path,
                id,
            },
        }
    }

    pub fn as_ref(&self) -> MarkableRef<'_> {
        match self {
            Markable::Uncommitted(hunk) => MarkableRef::Uncommitted(hunk),
            Markable::Commit(inner) => MarkableRef::Commit(inner.as_ref()),
            Markable::CommittedFile(inner) => MarkableRef::CommittedFile(inner.as_ref()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkedCommit {
    pub commit_id: gix::ObjectId,
    pub id: ShortId,
    pub change_id: Option<ChangeId>,
}

impl MarkedCommit {
    pub fn as_ref(&self) -> MarkedCommitRef<'_> {
        MarkedCommitRef {
            commit_id: self.commit_id,
            id: &self.id,
            change_id: self.change_id.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::VariantArray))]
pub enum MarkableRef<'a> {
    Uncommitted(&'a UncommittedHunkOrFile),
    Commit(MarkedCommitRef<'a>),
    CommittedFile(MarkedCommittedFileRef<'a>),
}

impl<'a> MarkableRef<'a> {
    pub fn try_from_cli_id(cli_id: &'a CliId) -> Option<Self> {
        for variant in MarkableRefDiscriminants::VARIANTS {
            match variant {
                MarkableRefDiscriminants::Uncommitted => {
                    if let CliId::UncommittedHunkOrFile(uncommitted) = cli_id {
                        if uncommitted
                            .hunk_assignments
                            .iter()
                            .any(|hunk| hunk.stack_id.is_some())
                        {
                            return None;
                        }
                        return Some(Self::Uncommitted(uncommitted));
                    }
                }
                MarkableRefDiscriminants::Commit => {
                    if let CliId::Commit {
                        commit_id,
                        id,
                        change_id,
                    } = cli_id
                    {
                        return Some(Self::Commit(MarkedCommitRef {
                            commit_id: *commit_id,
                            id,
                            change_id: change_id.as_ref(),
                        }));
                    }
                }
                MarkableRefDiscriminants::CommittedFile => {
                    if let CliId::CommittedFile {
                        commit_id,
                        path,
                        id,
                    } = cli_id
                    {
                        return Some(Self::CommittedFile(MarkedCommittedFileRef {
                            commit_id: *commit_id,
                            path: path.as_ref(),
                            id,
                        }));
                    }
                }
            }
        }

        None
    }

    pub fn matches_cli_id(&self, cli_id: &CliId) -> bool {
        MarkableRef::try_from_cli_id(cli_id).is_some_and(|id| self == &id)
    }

    pub fn to_owned(self) -> Markable {
        match self {
            MarkableRef::Uncommitted(hunk) => Markable::Uncommitted(hunk.clone()),
            MarkableRef::Commit(inner) => Markable::Commit(inner.to_owned()),
            MarkableRef::CommittedFile(inner) => Markable::CommittedFile(inner.to_owned()),
        }
    }
}

impl PartialEq<MarkableRef<'_>> for Markable {
    fn eq(&self, other: &MarkableRef<'_>) -> bool {
        <MarkableRef<'_> as PartialEq>::eq(&self.as_ref(), other)
    }
}

impl PartialEq<Markable> for MarkableRef<'_> {
    fn eq(&self, other: &Markable) -> bool {
        <MarkableRef<'_> as PartialEq>::eq(&other.as_ref(), self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkedCommitRef<'a> {
    pub commit_id: gix::ObjectId,
    pub id: &'a str,
    pub change_id: Option<&'a ChangeId>,
}

impl MarkedCommitRef<'_> {
    pub fn to_owned(self) -> MarkedCommit {
        MarkedCommit {
            commit_id: self.commit_id,
            id: self.id.to_owned(),
            change_id: self.change_id.cloned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkedCommittedFile {
    pub commit_id: gix::ObjectId,
    pub path: BString,
    pub id: ShortId,
}

impl MarkedCommittedFile {
    pub fn as_ref(&self) -> MarkedCommittedFileRef<'_> {
        MarkedCommittedFileRef {
            commit_id: self.commit_id,
            path: self.path.as_ref(),
            id: &self.id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkedCommittedFileRef<'a> {
    pub commit_id: gix::ObjectId,
    pub path: &'a BStr,
    pub id: &'a str,
}

impl MarkedCommittedFileRef<'_> {
    pub fn to_owned(self) -> MarkedCommittedFile {
        MarkedCommittedFile {
            commit_id: self.commit_id,
            path: self.path.to_owned(),
            id: self.id.to_owned(),
        }
    }
}

impl App {
    pub fn handle_mark(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let Some(selection) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|selection| selection.data.cli_id())
        else {
            return Ok(());
        };

        match &**selection {
            CliId::Commit { .. }
            | CliId::UncommittedHunkOrFile(..)
            | CliId::CommittedFile { .. } => {
                if handle_mark_cli_id(
                    selection,
                    self.mode
                        .get_mut_and_i_promise_not_to_switch_to_a_different_state(),
                )? && let Some(new_cursor) = self.cursor.move_after_mark(
                    &self.status_lines,
                    &self.mode,
                    self.flags.show_files,
                ) {
                    self.cursor = new_cursor;
                }
            }
            CliId::Branch {
                name,
                id: _,
                stack_id,
            } => {
                // you cannot select branches in rub mode so we don't need to care about that
                if let Some(stack_id) = *stack_id {
                    match self
                        .mode
                        .get_mut_and_i_promise_not_to_switch_to_a_different_state()
                    {
                        Mode::Normal(NormalMode { marks }) => {
                            handle_mark_branch(marks, ctx, stack_id, name)?;
                        }
                        Mode::PickChanges(..) => {}
                        Mode::Rub(..)
                        | Mode::InlineReword(..)
                        | Mode::Command(..)
                        | Mode::Commit(..)
                        | Mode::Move(..)
                        | Mode::Details(..)
                        | Mode::MoveStack(..)
                        | Mode::Jump(..)
                        | Mode::Stack(..) => {}
                    }
                }
            }
            CliId::Uncommitted { .. } => {
                // you cannot select uncommitted changes in rub mode so we don't need to care about that
                match self
                    .mode
                    .get_mut_and_i_promise_not_to_switch_to_a_different_state()
                {
                    Mode::Normal(NormalMode { marks }) => {
                        handle_mark_uncommitted(marks, &self.status_lines)?;
                    }
                    Mode::PickChanges(PickChangesMode { marks }) => {
                        handle_mark_uncommitted(marks, &self.status_lines)?;
                    }
                    Mode::Rub(..)
                    | Mode::InlineReword(..)
                    | Mode::Command(..)
                    | Mode::Commit(..)
                    | Mode::Move(..)
                    | Mode::Details(..)
                    | Mode::MoveStack(..)
                    | Mode::Jump(..)
                    | Mode::Stack(..) => {}
                }
            }
            CliId::PathPrefix { .. } | CliId::Stack { .. } => {}
        }

        if self.marks_ref().is_empty() {
            self.backstack.remove_mark();
        } else {
            self.backstack.push_mark();
        }

        Ok(())
    }

    pub fn handle_clear_marks(&mut self) {
        let did_clear_marks = match self
            .mode
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        {
            Mode::Normal(normal_mode) => {
                normal_mode.marks.clear();
                true
            }
            Mode::Details(details_mode) => {
                details_mode.return_mode.marks_mut().clear();
                true
            }
            Mode::PickChanges(pick_changes_mode) => {
                pick_changes_mode.marks.clear();
                true
            }
            Mode::Command(command_mode) => {
                match &mut command_mode.return_mode {
                    CommandReturnMode::Normal(normal_mode) => normal_mode.marks.clear(),
                    CommandReturnMode::Details(details_mode) => {
                        details_mode.return_mode.marks_mut().clear()
                    }
                }
                true
            }
            Mode::Rub(..)
            | Mode::InlineReword(..)
            | Mode::Commit(..)
            | Mode::Move(..)
            | Mode::Stack(..)
            | Mode::MoveStack(..)
            | Mode::Jump(..) => false,
        };

        if did_clear_marks {
            self.backstack.remove_mark();
        }
    }

    pub fn marks_ref(&self) -> MarksRef<'_> {
        self.mode.marks_ref()
    }
}

fn handle_mark_cli_id(commit: &CliId, mode: &mut Mode) -> anyhow::Result<bool> {
    let Some(markable) = MarkableRef::try_from_cli_id(commit) else {
        return Ok(false);
    };

    match mode {
        Mode::Normal(normal_mode) => {
            toggle_markables(&mut normal_mode.marks, [markable.to_owned()])?;
        }
        Mode::PickChanges(pick_uncommitted_mode) => {
            let MarkableRef::Uncommitted(..) = markable else {
                return Ok(false);
            };
            toggle_markables(&mut pick_uncommitted_mode.marks, [markable.to_owned()])?;
        }
        Mode::InlineReword(..)
        | Mode::Rub(..)
        | Mode::Command(..)
        | Mode::Commit(..)
        | Mode::Move(..)
        | Mode::Stack(..)
        | Mode::MoveStack(..)
        | Mode::Jump(..)
        | Mode::Details(..) => {
            return Ok(false);
        }
    }

    Ok(true)
}

// Create a fake synthetic hunk that makes the parent (entire-file) hunk or child (individual hunks
// inside a multi hunk change) hunk get marked/unmarked.
//
// When you mark an uncommitted file in the status that should be equivalent to marking all the
// individual hunks in the detail view. We do that by creating synthetic child hunks and also
// insert those into the mark store, when marking the parent. The detail view does the inverse when
// marking a child hunk.
//
// This relies on the fact that `UncommittedHunkOrFile::eq` just uses `hunk_assignments` and
// `is_entire_file` and _not_ `id`. So we're free to invent a fake id for our synthetic hunk and
// it'll still match the real hunk and which will then appear marked. This check is made by the
// rendering when deciding to show the checkmark or not.
//
// When the hunks are actually used later they'll be deduplicated so duplicates or overlapping
// hunks don't break mutations.
fn synthetic_hunk(
    base_id: &str,
    idx: usize,
    hunk_assignments: NonEmpty<HunkAssignment>,
    is_entire_file: bool,
) -> UncommittedHunkOrFile {
    UncommittedHunkOrFile {
        id: format!("{base_id}:synthetic-id-{idx}"),
        hunk_assignments,
        is_entire_file,
    }
}

pub fn synthetic_parent_hunk(
    base_id: &str,
    idx: usize,
    hunk_assignments: NonEmpty<HunkAssignment>,
) -> UncommittedHunkOrFile {
    synthetic_hunk(base_id, idx, hunk_assignments, true)
}

pub fn synthetic_child_hunk(
    base_id: &str,
    idx: usize,
    hunk_assignments: NonEmpty<HunkAssignment>,
) -> UncommittedHunkOrFile {
    synthetic_hunk(base_id, idx, hunk_assignments, false)
}

fn handle_mark_branch(
    marks: &mut Marks,
    ctx: &Context,
    stack_id: StackId,
    name: &str,
) -> anyhow::Result<()> {
    let commits =
        commits_on_branch(ctx, stack_id, name)?
            .into_iter()
            .map(|(commit_id, id, change_id)| {
                Markable::Commit(MarkedCommit {
                    commit_id,
                    id,
                    change_id,
                })
            });

    toggle_markables(marks, commits)?;

    Ok(())
}

fn handle_mark_uncommitted(
    marks: &mut Marks,
    status_lines: &[StatusOutputLine],
) -> anyhow::Result<()> {
    let uncommitted_files = status_lines
        .iter()
        .filter_map(|line| match &line.data {
            StatusOutputLineData::UncommittedFile { cli_id } => {
                match MarkableRef::try_from_cli_id(cli_id) {
                    Some(MarkableRef::Uncommitted(hunk)) => {
                        Some(Markable::Uncommitted(hunk.clone()))
                    }
                    Some(_) | None => None,
                }
            }
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::Branch { .. }
            | StatusOutputLineData::Commit { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => None,
        })
        .collect::<Vec<_>>();

    toggle_markables(marks, uncommitted_files)?;

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum ToggleMarkablesOutcome {
    Marked,
    Unmarked,
    NoMarkables,
}

pub fn toggle_markables(
    marks: &mut Marks,
    markables: impl IntoIterator<Item = Markable>,
) -> anyhow::Result<ToggleMarkablesOutcome> {
    let markables = markables.into_iter().collect::<Vec<_>>();
    let mut marked = Vec::new();
    let mut saw_unmarked = false;

    for markable in &markables {
        if marks.contains_mark(markable) {
            if !saw_unmarked {
                marked.push(markable);
            }
        } else {
            saw_unmarked = true;
            marked.clear();
            marks.insert_mark(markable.clone())?;
        }
    }

    let outcome = if saw_unmarked {
        ToggleMarkablesOutcome::Marked
    } else if marked.is_empty() {
        ToggleMarkablesOutcome::NoMarkables
    } else {
        for markable in marked {
            marks.remove_mark(markable);
        }
        ToggleMarkablesOutcome::Unmarked
    };

    propagate_marks_from_parent_to_children(marks, &markables, outcome)?;

    Ok(outcome)
}

fn propagate_marks_from_parent_to_children(
    marks: &mut Marks,
    markables: &[Markable],
    outcome: ToggleMarkablesOutcome,
) -> anyhow::Result<()> {
    for markable in markables {
        let Markable::Uncommitted(hunk) = markable else {
            continue;
        };
        if !hunk.is_entire_file {
            continue;
        }

        for (idx, hunk_assignment) in hunk.hunk_assignments.iter().enumerate() {
            let child_hunk =
                synthetic_child_hunk(&hunk.id, idx, NonEmpty::new(hunk_assignment.clone()));
            match outcome {
                ToggleMarkablesOutcome::Marked => marks.insert_mark(child_hunk)?,
                ToggleMarkablesOutcome::Unmarked => marks.remove_mark(&child_hunk),
                ToggleMarkablesOutcome::NoMarkables => {}
            }
        }
    }
    Ok(())
}

pub fn commits_on_branch(
    ctx: &Context,
    stack_id: StackId,
    name: &str,
) -> anyhow::Result<Vec<(gix::ObjectId, String, Option<ChangeId>)>> {
    let guard = ctx.shared_worktree_access();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

    let segment = id_map
        .stacks()
        .iter()
        .filter(|stack| stack.id.is_some_and(|id| id == stack_id))
        .flat_map(|stack| &stack.segments)
        .find(|segment| {
            segment
                .branch_name()
                .is_some_and(|branch_name| branch_name == name)
        })
        .context("segment not found")?;

    let commits = segment
        .workspace_commits
        .iter()
        .map(|commit| {
            (
                commit.commit_id(),
                commit.short_id.clone(),
                commit
                    .change_id
                    .as_ref()
                    .map(|change_id| change_id.change_id.clone()),
            )
        })
        .collect::<Vec<_>>();

    Ok(commits)
}
