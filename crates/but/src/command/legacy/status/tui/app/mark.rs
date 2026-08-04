use std::{convert::Infallible, ops::Deref};

use nonempty::NonEmpty;
use strum::VariantArray;

use crate::{
    CliId,
    command::legacy::status::{
        StatusOutputLine,
        output::StatusOutputLineData,
        tui::{
            Selectable,
            app::{
                App, CommandReturnMode, normal_mode::NormalMode, pick_changes_mode::PickChangesMode,
            },
            mode::Mode,
        },
    },
    id::{
        BranchId, BranchIdRef, CommitId, CommitIdRef, CommittedFileId, CommittedFileIdRef,
        IdAndHunk, UncommittedHunkOrFile,
    },
};

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Marks {
    #[default]
    Empty,
    Hunks(NonEmpty<UncommittedHunkOrFile>),
    Commits(NonEmpty<CommitId>),
    CommittedFiles(NonEmpty<CommittedFileId>),
    Branches(NonEmpty<BranchId>),
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

    pub fn as_commits(&self) -> Option<&NonEmpty<CommitId>> {
        match self {
            Self::Commits(commits) => Some(commits),
            _ => None,
        }
    }

    pub fn as_committed_files(&self) -> Option<&NonEmpty<CommittedFileId>> {
        match self {
            Self::CommittedFiles(files) => Some(files),
            _ => None,
        }
    }

    pub fn as_branches(&self) -> Option<&NonEmpty<BranchId>> {
        match self {
            Self::Branches(branches) => Some(branches),
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
            Self::Branches(branches) => MarksRef::from_branches(branches),
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
        head: &'a CommitId,
        tail: &'a [CommitId],
    },
    CommittedFiles {
        head: &'a CommittedFileId,
        tail: &'a [CommittedFileId],
    },
    Branches {
        head: &'a BranchId,
        tail: &'a [BranchId],
    },
}

impl<'a> MarksRef<'a> {
    pub fn from_hunks(hunks: &'a NonEmpty<UncommittedHunkOrFile>) -> Self {
        Self::Hunks {
            head: &hunks.head,
            tail: &hunks.tail,
        }
    }

    pub fn from_hunk_ref(hunk: &'a UncommittedHunkOrFile) -> Self {
        Self::Hunks {
            head: hunk,
            tail: &[],
        }
    }

    pub fn from_commits(commits: &'a NonEmpty<CommitId>) -> Self {
        Self::Commits {
            head: &commits.head,
            tail: &commits.tail,
        }
    }

    pub fn from_commit_ref(commit: &'a CommitId) -> Self {
        Self::Commits {
            head: commit,
            tail: &[],
        }
    }

    pub fn from_committed_files(commits: &'a NonEmpty<CommittedFileId>) -> Self {
        Self::CommittedFiles {
            head: &commits.head,
            tail: &commits.tail,
        }
    }

    pub fn from_committed_file_ref(commit: &'a CommittedFileId) -> Self {
        Self::CommittedFiles {
            head: commit,
            tail: &[],
        }
    }

    pub fn from_branches(branches: &'a NonEmpty<BranchId>) -> Self {
        Self::Branches {
            head: &branches.head,
            tail: &branches.tail,
        }
    }

    pub fn from_branch_ref(branch: &'a BranchId) -> Self {
        Self::Branches {
            head: branch,
            tail: &[],
        }
    }

    pub fn is_empty(self) -> bool {
        matches!(self, Self::Empty)
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn len(self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Hunks { head: _, tail } => 1 + tail.len(),
            Self::Commits { head: _, tail } => 1 + tail.len(),
            Self::CommittedFiles { head: _, tail } => 1 + tail.len(),
            Self::Branches { head: _, tail } => 1 + tail.len(),
        }
    }

    pub fn contains_cli_id(self, cli_id: &CliId) -> bool {
        match self {
            MarksRef::Empty => false,
            MarksRef::Hunks { head, tail } => {
                let CliId::UncommittedHunkOrFile(uncommitted) = cli_id else {
                    return false;
                };
                std::iter::once(head)
                    .chain(tail)
                    .any(|hunk| hunk == uncommitted)
            }
            MarksRef::Commits { head, tail } => {
                let CliId::Commit {
                    commit: other,
                    id: _,
                } = cli_id
                else {
                    return false;
                };
                std::iter::once(head)
                    .chain(tail)
                    .any(|commit| other == commit)
            }
            MarksRef::CommittedFiles { head, tail } => {
                let CliId::CommittedFile {
                    committed_file: other,
                    id: _,
                } = cli_id
                else {
                    return false;
                };
                std::iter::once(head).chain(tail).any(|file| other == file)
            }
            MarksRef::Branches { head, tail } => {
                let CliId::Branch(other) = cli_id else {
                    return false;
                };
                std::iter::once(head)
                    .chain(tail)
                    .any(|branch| branch == other)
            }
        }
    }

    pub fn to_owned(self) -> Marks {
        match self {
            MarksRef::Empty => Marks::Empty,
            MarksRef::Hunks { head, tail } => Marks::Hunks(NonEmpty {
                head: head.clone(),
                tail: tail.to_vec(),
            }),
            MarksRef::Commits { head, tail } => Marks::Commits(NonEmpty {
                head: head.clone(),
                tail: tail.to_vec(),
            }),
            MarksRef::CommittedFiles { head, tail } => Marks::CommittedFiles(NonEmpty {
                head: head.clone(),
                tail: tail.to_vec(),
            }),
            MarksRef::Branches { head, tail } => Marks::Branches(NonEmpty {
                head: head.clone(),
                tail: tail.to_vec(),
            }),
        }
    }

    pub fn iter(self) -> impl Iterator<Item = MarkableRef<'a>> {
        match self {
            MarksRef::Empty => Box::new(std::iter::empty()) as Box<dyn Iterator<Item = _>>,
            MarksRef::Hunks { head, tail } => {
                let iter = std::iter::once(head)
                    .chain(tail)
                    .map(MarkableRef::Uncommitted);
                Box::new(iter)
            }
            MarksRef::Commits { head, tail } => {
                let iter = std::iter::once(head)
                    .chain(tail)
                    .map(|commit| commit.as_ref())
                    .map(MarkableRef::Commit);
                Box::new(iter)
            }
            MarksRef::CommittedFiles { head, tail } => {
                let iter = std::iter::once(head)
                    .chain(tail)
                    .map(|file| file.as_ref())
                    .map(MarkableRef::CommittedFile);
                Box::new(iter)
            }
            MarksRef::Branches { head, tail } => {
                let iter = std::iter::once(head)
                    .chain(tail)
                    .map(|file| file.as_ref())
                    .map(MarkableRef::Branch);
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

            hunk_is_child_of(parent, child)
        })
    }
}

pub fn hunk_is_child_of(parent: &UncommittedHunkOrFile, child: &UncommittedHunkOrFile) -> bool {
    parent.is_entire_file
        && !child.is_entire_file
        && child.hunks.iter().any(|child_hunk| {
            parent
                .hunks
                .iter()
                .any(|parent_hunk| parent_hunk.hunk.identifies_same_hunk(&child_hunk.hunk))
        })
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
            Markable::Branch(branch) => self.contains_mark(branch),
        }
    }

    fn insert_mark(&mut self, mark: Markable) -> Result<(), Self::Error> {
        match mark {
            Markable::Uncommitted(hunk) => self.insert_mark(hunk),
            Markable::Commit(commit) => self.insert_mark(commit),
            Markable::CommittedFile(file) => self.insert_mark(file),
            Markable::Branch(branch) => self.insert_mark(branch),
        }
    }

    fn remove_mark(&mut self, mark: &Markable) {
        match mark {
            Markable::Uncommitted(hunk) => self.remove_mark(hunk),
            Markable::Commit(commit) => self.remove_mark(commit),
            Markable::CommittedFile(file) => self.remove_mark(file),
            Markable::Branch(branch) => self.remove_mark(branch),
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

impl MarkStore<CommitId> for Marks {
    type Error = anyhow::Error;

    fn contains_mark(&self, mark: &CommitId) -> bool {
        self.as_commits()
            .is_some_and(|commits| commits.iter().any(|commit| commit == mark))
    }

    fn insert_mark(&mut self, mark: CommitId) -> Result<(), Self::Error> {
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

    fn remove_mark(&mut self, mark: &CommitId) {
        let Self::Commits(commits) = self else {
            return;
        };

        if remove_from_non_empty(commits, |marked| marked == mark) {
            *self = Self::Empty;
        }
    }
}

impl MarkStore<CommittedFileId> for Marks {
    type Error = anyhow::Error;

    fn contains_mark(&self, mark: &CommittedFileId) -> bool {
        self.as_committed_files()
            .is_some_and(|files| files.iter().any(|file| file == mark))
    }

    fn insert_mark(&mut self, mark: CommittedFileId) -> Result<(), Self::Error> {
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

    fn remove_mark(&mut self, mark: &CommittedFileId) {
        let Self::CommittedFiles(files) = self else {
            return;
        };

        if remove_from_non_empty(files, |marked| marked == mark) {
            *self = Self::Empty;
        }
    }
}

impl MarkStore<BranchId> for Marks {
    type Error = anyhow::Error;

    fn contains_mark(&self, mark: &BranchId) -> bool {
        self.as_branches()
            .is_some_and(|branches| branches.iter().any(|file| file == mark))
    }

    fn insert_mark(&mut self, mark: BranchId) -> Result<(), Self::Error> {
        if self.contains_mark(&mark) {
            return Ok(());
        }
        match self {
            Self::Empty => *self = Self::Branches(NonEmpty::new(mark)),
            Self::Branches(branches) => {
                branches.push(mark);
            }
            _ => anyhow::bail!("cannot mix mark sources"),
        }
        Ok(())
    }

    fn remove_mark(&mut self, mark: &BranchId) {
        let Self::Branches(branches) = self else {
            return;
        };

        if remove_from_non_empty(branches, |marked| marked == mark) {
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
    Commit(CommitId),
    CommittedFile(CommittedFileId),
    Branch(BranchId),
}

impl Markable {
    pub fn into_selectable(self) -> Selectable {
        match self {
            Markable::Uncommitted(uncommitted_cli_id) => {
                Selectable::UncommittedHunkOrFile(uncommitted_cli_id)
            }
            Markable::Commit(commit) => Selectable::Commit(commit),
            Markable::CommittedFile(file) => Selectable::CommittedFile(file),
            Markable::Branch(branch) => Selectable::Branch(branch),
        }
    }

    pub fn as_ref(&self) -> MarkableRef<'_> {
        match self {
            Markable::Uncommitted(hunk) => MarkableRef::Uncommitted(hunk),
            Markable::Commit(inner) => MarkableRef::Commit(inner.as_ref()),
            Markable::CommittedFile(inner) => MarkableRef::CommittedFile(inner.as_ref()),
            Markable::Branch(inner) => MarkableRef::Branch(inner.as_ref()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::VariantArray))]
pub enum MarkableRef<'a> {
    Uncommitted(&'a UncommittedHunkOrFile),
    Commit(CommitIdRef<'a>),
    CommittedFile(CommittedFileIdRef<'a>),
    Branch(BranchIdRef<'a>),
}

impl<'a> MarkableRef<'a> {
    pub fn try_from_cli_id(cli_id: &'a CliId) -> Option<Self> {
        for variant in MarkableRefDiscriminants::VARIANTS {
            match variant {
                MarkableRefDiscriminants::Uncommitted => {
                    if let CliId::UncommittedHunkOrFile(uncommitted) = cli_id {
                        return Some(Self::Uncommitted(uncommitted));
                    }
                }
                MarkableRefDiscriminants::Commit => {
                    if let CliId::Commit { commit, id: _ } = cli_id {
                        return Some(Self::Commit(commit.as_ref()));
                    }
                }
                MarkableRefDiscriminants::CommittedFile => {
                    if let CliId::CommittedFile {
                        committed_file: file,
                        id: _,
                    } = cli_id
                    {
                        return Some(Self::CommittedFile(file.as_ref()));
                    }
                }
                MarkableRefDiscriminants::Branch => {
                    if let CliId::Branch(branch) = cli_id {
                        return Some(Self::Branch(branch.as_ref()));
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
            MarkableRef::Branch(inner) => Markable::Branch(inner.to_owned()),
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

impl App {
    pub fn handle_mark(&mut self) -> anyhow::Result<()> {
        let Some(selection) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|selection| selection.data.cli_id())
        else {
            return Ok(());
        };

        match &**selection {
            CliId::Branch(..)
            | CliId::Commit { .. }
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
            CliId::Uncommitted { .. } => {
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
                    Mode::Squash(..)
                    | Mode::InlineReword(..)
                    | Mode::Command(..)
                    | Mode::Commit(..)
                    | Mode::Move(..)
                    | Mode::Details(..)
                    | Mode::MoveStack(..)
                    | Mode::Jump(..)
                    | Mode::CherryPick(..)
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
            Mode::Squash(..)
            | Mode::InlineReword(..)
            | Mode::Commit(..)
            | Mode::Move(..)
            | Mode::Stack(..)
            | Mode::MoveStack(..)
            | Mode::CherryPick(..)
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
        | Mode::Squash(..)
        | Mode::Command(..)
        | Mode::Commit(..)
        | Mode::Move(..)
        | Mode::Stack(..)
        | Mode::MoveStack(..)
        | Mode::Jump(..)
        | Mode::CherryPick(..)
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
// This relies on the fact that `UncommittedHunkOrFile::eq` just uses `hunks` and
// `is_entire_file` and _not_ `id`. So we're free to invent a fake id for our synthetic hunk and
// it'll still match the real hunk and which will then appear marked. This check is made by the
// rendering when deciding to show the checkmark or not.
//
// When the hunks are actually used later they'll be deduplicated so duplicates or overlapping
// hunks don't break mutations.
fn synthetic_hunk(
    base_id: &str,
    idx: usize,
    hunks: NonEmpty<IdAndHunk>,
    is_entire_file: bool,
) -> UncommittedHunkOrFile {
    UncommittedHunkOrFile {
        id: format!("{base_id}:synthetic-id-{idx}"),
        hunks,
        is_entire_file,
    }
}

pub fn synthetic_parent_hunk(
    base_id: &str,
    idx: usize,
    hunks: NonEmpty<IdAndHunk>,
) -> UncommittedHunkOrFile {
    synthetic_hunk(base_id, idx, hunks, true)
}

pub fn synthetic_child_hunk(
    base_id: &str,
    idx: usize,
    hunks: NonEmpty<IdAndHunk>,
) -> UncommittedHunkOrFile {
    synthetic_hunk(base_id, idx, hunks, false)
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

        for (idx, child) in hunk.hunks.iter().enumerate() {
            let child_hunk = synthetic_child_hunk(&hunk.id, idx, NonEmpty::new(child.clone()));
            match outcome {
                ToggleMarkablesOutcome::Marked => marks.insert_mark(child_hunk)?,
                ToggleMarkablesOutcome::Unmarked => marks.remove_mark(&child_hunk),
                ToggleMarkablesOutcome::NoMarkables => {}
            }
        }
    }
    Ok(())
}
