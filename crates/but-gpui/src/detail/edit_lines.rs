//! Editing an uncommitted file in its diff, line by line where the lines are shown, as in an
//! editor: a click puts the caret there, the diff follows what's typed, and ⌘S saves it. What
//! isn't saved yet shows a dot after the file's name, and stays when the caret goes; leaving the
//! file, or the app quitting, saves it.
//!
//! The file's new side is held as lines while it's edited, and its diff is worked out again
//! from them on every change and shown in place of the one loaded, the whole file at once.
//! Only the line with the caret is a field; the rest draw as any diff line does.
//!
//! An edit's first write is recorded for undo, so ⌘Z takes back all of it. Its writes aren't
//! operations, which would hold up
//! commits and pushes: they take GitButler's lock, and the store counts them as its own.

use std::cmp::Ordering;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use bstr::BString;
use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, Focusable, ListOffset, Pixels, SharedString,
    Subscription, WeakEntity, Window, div, prelude::*,
};

use super::{Detail, DiffKey, TextPoint};
use crate::backend::edit_lines::{self, Sides, WriteFile};
use crate::backend::{self, ChangeKind, Checkout, DiffBody, DiffLine, FileDiff, LineKind};
use crate::clipboard;
use crate::state::{ProjectId, snack};
use crate::theme::Theme;
use crate::ui::diff::DiffRow;
use crate::ui::text_input::{InputEvent, LineEvent, TextInput};
use crate::ui::{ButtonStyle, Icon, button, dp, h_flex, small_icon};

/// So every line of the file is a row while it's edited, and the caret can go to any of them.
const WHOLE: u32 = u32::MAX / 4;

/// A file being edited, by its path in the diff of its project's checkout.
pub(super) struct FileEdit {
    project: ProjectId,
    checkout: Checkout,
    /// As git names it, which writing takes, and as the diff lists it.
    path: BString,
    shown_path: String,
    /// What the file's loaded diff hashed to when it last read as written here: another means
    /// it may have been written to since, as by an agent.
    fingerprint: u64,
    /// The file as it was when the edit began, which Reset puts back.
    original: String,
    /// The file as it was last read or written here, which a write has to find there still,
    /// and its lines.
    read: String,
    read_lines: Vec<String>,
    /// What a write under way puts in the file.
    writing: Option<String>,
    /// The edit's first write is recorded for undo, and the rest go in that step.
    recorded: bool,
    old: Vec<String>,
    old_shown: Vec<DiffLine>,
    /// Its new side as typed, a line each without its ending, and each line as it's shown,
    /// highlighted as read until it's typed in.
    lines: Vec<String>,
    shown: Vec<Option<DiffLine>>,
    newline: &'static str,
    /// The last line ends with a newline.
    ends: bool,
    /// What Tab indents a line by, as the file's lines are indented.
    indent: String,
    /// The line with the caret, if any has it.
    active: Option<Active>,
    /// Text selected over more than one line, which the caret's field shows its part of.
    selection: Option<Selection>,
    /// The lines the last copy took from, which blink as the diff's copied text does.
    copied: Option<std::ops::RangeInclusive<usize>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// The line a run of typing is on, which undo takes back at once.
    typing: Option<usize>,
    /// Something else wrote to the file while it was edited: nothing's saved until the user
    /// keeps what's typed or drops it.
    changed_on_disk: bool,
    /// Opened on a tracked file with no changes: how the diff shows it while it has none.
    opened: Option<FileDiff>,
}

/// A place in the file being edited: a line, and a byte offset in it.
type Spot = (usize, usize);

/// Text selected over more than one line: where it began, and where the caret took it.
#[derive(Clone, Copy, PartialEq)]
struct Selection {
    anchor: Spot,
    head: Spot,
}

impl Selection {
    /// Its start and its end, in the file's order.
    fn ordered(self) -> (Spot, Spot) {
        match self.anchor <= self.head {
            true => (self.anchor, self.head),
            false => (self.head, self.anchor),
        }
    }
}

struct Active {
    line: usize,
    input: Entity<TextInput>,
    _listen: [Subscription; 2],
}

/// The lines as they were before a change, and where the caret was.
struct Snapshot {
    lines: Vec<String>,
    shown: Vec<Option<DiffLine>>,
    line: usize,
    caret: usize,
}

impl FileEdit {
    /// Typed since it was last saved.
    fn dirty(&self) -> bool {
        self.lines != self.read_lines
    }

    /// The file isn't as it was when the edit began, as typed or as saved.
    fn changed(&self) -> bool {
        self.read != self.original || self.dirty()
    }

    /// The file as typed, its lines ended as when it was read.
    fn text(&self) -> String {
        let mut text = self.lines.join(self.newline);
        if self.ends && !self.lines.is_empty() {
            text.push_str(self.newline);
        }
        text
    }

    /// Keep the lines as they are, for undo to come back to, unless typing goes on with the
    /// run undo already takes back.
    fn remember(&mut self, line: usize, caret: usize, typing: bool) {
        if typing && self.typing == Some(line) {
            return;
        }
        self.undo.push(Snapshot {
            lines: self.lines.clone(),
            shown: self.shown.clone(),
            line,
            caret,
        });
        self.redo.clear();
        self.typing = typing.then_some(line);
    }

    /// The text `selection` takes, its lines parted by newlines.
    fn selected_text(&self, selection: Selection) -> String {
        let ((a, from), (b, to)) = selection.ordered();
        (a..=b)
            .filter_map(|n| {
                let raw = self.lines.get(n)?;
                let start = if n == a { from } else { 0 };
                let end = if n == b { to } else { raw.len() };
                raw.get(start..end)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Take what's selected out, the lines it ran over joined where it began, which is where
    /// the caret goes.
    fn take_selection(&mut self) -> Option<Spot> {
        let ((a, from), (b, to)) = self.selection.take()?.ordered();
        let head = self.lines.get(a)?.get(..from)?.to_owned();
        let tail = self.lines.get(b)?.get(to..)?;
        let joined = head + tail;
        if b < self.lines.len() {
            self.lines.splice(a..=b, [joined]);
            self.shown.splice(a..=b, [None]);
        }
        Some((a, from))
    }

    /// Put `parts` in place of line `line`, as typed.
    fn replace_line(&mut self, line: usize, parts: Vec<String>) {
        let count = parts.len();
        if line < self.lines.len() {
            self.lines.splice(line..=line, parts);
            self.shown
                .splice(line..=line, std::iter::repeat_n(None, count));
        }
    }

    /// The file's diff as typed.
    fn body(&self) -> DiffBody {
        edit_lines::diff_between(&self.old, &self.lines, WHOLE, |kind, old, new| {
            let index = |n: Option<u32>| n.and_then(|n| usize::try_from(n).ok()?.checked_sub(1));
            let shown = match kind {
                LineKind::Added => index(new).and_then(|ix| {
                    let typed = || Some(edit_lines::typed_line(self.lines.get(ix)?));
                    self.shown.get(ix)?.clone().or_else(typed)
                }),
                LineKind::Removed | LineKind::Context => {
                    index(old).and_then(|ix| self.old_shown.get(ix).cloned())
                }
            };
            DiffLine {
                kind,
                old,
                new,
                ..shown.unwrap_or_else(|| DiffLine::new(kind, old, new, ""))
            }
        })
    }

    /// Whether `file` of the diff of `key` is the one edited.
    fn of(&self, key: &DiffKey, file: &FileDiff) -> bool {
        key.project == self.project
            && key.checkout().as_ref() == Some(&self.checkout)
            && file.file.change.path == self.path
    }
}

/// From a byte offset in a line as the diff shows it, tabs as spaces, the offset in the line.
fn raw_offset(raw: &str, shown: usize) -> usize {
    let mut at = 0;
    for (ix, c) in raw.char_indices() {
        if at >= shown {
            return ix;
        }
        at += match c {
            '\t' => 4,
            c => c.len_utf8(),
        };
    }
    raw.len()
}

/// What Tab indents a line of `lines` by: a tab where a line starts with one, else the fewest
/// spaces any line starts with, from two to eight, or four where none does.
fn indent_of(lines: &[String]) -> String {
    if lines.iter().any(|line| line.starts_with('\t')) {
        return "\t".into();
    }
    let spaces = (lines.iter())
        .map(|line| line.len() - line.trim_start_matches(' ').len())
        .filter(|&spaces| spaces > 0)
        .min()
        .unwrap_or(4);
    " ".repeat(spaces.clamp(2, 8))
}

/// How much of `raw` ⇧Tab takes off its start: a tab, or as many of the step `indent`'s spaces
/// as it starts with.
fn outdent(raw: &str, indent: &str) -> usize {
    match raw.starts_with('\t') {
        true => 1,
        false => (raw.bytes().take(indent.len().max(1)))
            .take_while(|&b| b == b' ')
            .count(),
    }
}

/// From byte `at` of line `raw`, the byte offset in the line as the diff shows it, tabs as
/// spaces: the other way from [`raw_offset`].
fn shown_offset(raw: &str, at: usize) -> usize {
    (raw.char_indices())
        .take_while(|(ix, _)| *ix < at)
        .map(|(_, c)| if c == '\t' { 4 } else { c.len_utf8() })
        .sum()
}

/// `text` split at byte `at`, or the nearest character boundary before it.
fn split(text: &str, at: usize) -> (&str, &str) {
    let at = (0..=at.min(text.len()))
        .rev()
        .find(|&at| text.is_char_boundary(at))
        .unwrap_or(0);
    text.split_at(at)
}

/// What a write of an edit came to.
enum Wrote {
    Written {
        span: (i64, i64),
        recorded: Option<backend::SnapshotId>,
    },
    /// The file wasn't what the edit last saw, as an agent wrote to it: nothing was written.
    ChangedOnDisk,
    Failed(String),
}

impl Detail {
    /// Whether file `file` of the loaded diff can be edited, or why not.
    pub(super) fn editable(&self, file: usize, cx: &App) -> Result<(), &'static str> {
        let diff = self.diff.as_ref().ok_or("No diff to edit")?;
        diff.key
            .checkout()
            .ok_or("Only uncommitted changes can be edited")?;
        let file_diff = diff
            .files
            .get(file)
            .ok_or("That file isn't shown anymore")?;
        if file_diff.file.conflicted {
            return Err("Resolve its conflicts first");
        }
        if file_diff.file.converted {
            return Err("Its diff isn't the file's own lines");
        }
        if file_diff.file.kind == ChangeKind::Deleted
            || !matches!(file_diff.body, DiffBody::Hunks { .. })
        {
            return Err("It has no lines to edit");
        }
        if self.store.read(cx).prefs.split {
            return Err("Switch to Unified to edit lines");
        }
        Ok(())
    }

    /// The index in the loaded diff of the file being edited.
    fn edited_file(&self) -> Option<usize> {
        let (edit, diff) = (self.file_edit.as_ref()?, self.diff.as_ref()?);
        diff.files.iter().position(|f| edit.of(&diff.key, f))
    }

    /// Put the caret on new-side line `line` (1-based) of file `file`, at byte `shown` of the
    /// line as shown, starting to edit the file if it isn't yet.
    pub(super) fn edit_at(
        &mut self,
        file: usize,
        line: u32,
        shown: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ix = (line as usize).saturating_sub(1);
        if self.edited_file() == Some(file) {
            let caret = (self.file_edit.as_ref())
                .and_then(|edit| edit.lines.get(ix))
                .map_or(0, |raw| raw_offset(raw, shown));
            if let Some(edit) = &mut self.file_edit {
                edit.selection = None;
            }
            return self.activate(ix, caret, window, cx);
        }
        if let Err(why) = self.editable(file, cx) {
            return snack(&self.store, why, cx);
        }
        // Another file's edit ends, what's typed in it saved.
        self.end_edit(cx);
        let Some(diff) = &self.diff else {
            return;
        };
        let (Some(root), Some(file_diff)) = (
            self.store.read(cx).path(&diff.key.project),
            diff.files.get(file),
        ) else {
            return;
        };
        let (project, checkout) = (diff.key.project.clone(), diff.key.files_checkout());
        let (change, fingerprint) = (file_diff.file.clone(), file_diff.fingerprint);
        let worktree = checkout.clone();
        self._read_sides = cx.spawn_in(window, async move |this, cx| {
            let read = backend::run({
                let change = change.clone();
                move || edit_lines::read_sides(&root, worktree.linked(), &change)
            })
            .await;
            (this.update_in(cx, |this, window, cx| {
                let sides = match read {
                    Ok(Some(sides)) => sides,
                    Ok(None) => return snack(&this.store, "It isn't text to edit", cx),
                    Err(err) => return snack(&this.store, format!("{err:#}"), cx),
                };
                // Changed while it was read: the click was on lines that aren't there now.
                let now = (this.diff.as_ref()).and_then(|diff| diff.files.get(file));
                if now.is_none_or(|f| f.fingerprint != fingerprint || f.file.path != change.path) {
                    return;
                }
                let edit = new_edit(project, checkout, &change, fingerprint, sides);
                let caret = (edit.lines.get(ix)).map_or(0, |raw| raw_offset(raw, shown));
                // Where the line is on screen, which it keeps as the file opens whole around
                // it: the lines a box above it stood for would push it down.
                let held = (this.line_row(file, line)).and_then(|row| this.row_offset(row));
                this.file_edit = Some(edit);
                this.refresh_edit(cx);
                this.activate(ix, caret, window, cx);
                match (held, this.edit_row()) {
                    (Some(offset), Some(row)) => this.hold_row(row, offset, cx),
                    _ => this.reveal_edit_row(cx),
                }
            }))
            .ok();
        });
    }

    /// Edit the file the cursor's in, at the first line its hunk changed.
    pub(super) fn edit_cursor_hunk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self.cursor.in_file() else {
            return snack(&self.store, "Go to a file to edit its lines", cx);
        };
        let hunk = self.cursor.at.near().unwrap_or(0);
        self.edit_hunk(file, hunk, window, cx);
    }

    /// Edit file `file` at the first line hunk `hunk` changed.
    pub(super) fn edit_hunk(
        &mut self,
        file: usize,
        hunk: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let found = (self.diff.as_ref())
            .and_then(|diff| diff.files.get(file)?.hunks().get(hunk))
            .map(|found| {
                (found.lines.iter())
                    .find(|line| line.kind == LineKind::Added)
                    .and_then(|line| line.new)
                    .unwrap_or(found.key.0.new_start.max(1))
            });
        self.edit_at(file, found.unwrap_or(1), 0, window, cx);
    }

    /// The project and checkout whose files can be opened to edit: the one whose uncommitted
    /// changes the diff shows.
    pub fn openable_checkout(&self) -> Option<(ProjectId, Checkout)> {
        let diff = self.worktree_diff()?;
        Some((diff.key.project.clone(), diff.key.checkout()?))
    }

    /// The file opened to edit in `project`'s `checkout` while it has no changes there, as
    /// the diff shows it: one not among `changes`.
    pub(super) fn opened_file(
        &self,
        project: &ProjectId,
        checkout: &Checkout,
        changes: &[backend::FileChange],
    ) -> Option<&backend::FileChange> {
        let edit = (self.file_edit.as_ref()).filter(|edit| {
            edit.opened.is_some() && edit.project == *project && edit.checkout == *checkout
        })?;
        if changes.iter().any(|f| f.change.path == edit.path) {
            return None;
        }
        let diff = self.worktree_diff()?;
        (diff.files.iter())
            .find(|f| edit.of(&diff.key, f))
            .map(|f| &f.file)
    }

    /// Opening a file failed, `why`: a checkout listed for it no longer is.
    fn not_opened(&mut self, why: impl Into<SharedString>, cx: &mut Context<Self>) {
        if self
            .file_edit
            .as_ref()
            .is_none_or(|edit| edit.opened.is_none())
        {
            self.store
                .update(cx, |store, cx| store.set_opened(None, cx));
        }
        snack(&self.store, why, cx);
    }

    /// Whether `file` is one opened to edit with no changes.
    pub(super) fn opened_unchanged(&self, file: &backend::FileChange) -> bool {
        (self.file_edit.as_ref())
            .is_some_and(|edit| edit.opened.is_some() && file.change.path == edit.path)
    }

    /// Edit `path`, a file `project`'s `checkout` tracks, from its first line: here if its
    /// changes are shown, else once selecting them shows them, with none if need be.
    pub fn open_file_in(
        &mut self,
        project: ProjectId,
        checkout: Checkout,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = (project, checkout);
        if self.openable_checkout().as_ref() == Some(&at) {
            return self.open_file(path, window, cx);
        }
        let selection = crate::state::Selection::Uncommitted(at.0.clone(), at.1.clone());
        self.store.update(cx, |store, cx| {
            store.set_opened(Some(at.clone()), cx);
            store.select(Some(selection), cx);
        });
        let this = cx.entity();
        self._open_when_shown = Some(cx.observe_in(&this, window, move |this, _, window, cx| {
            if this.loading.is_some() {
                return;
            }
            this._open_when_shown = None;
            match this.openable_checkout().as_ref() == Some(&at) {
                true => this.open_file(path.clone(), window, cx),
                // Something else selected meanwhile.
                false => this
                    .store
                    .update(cx, |store, cx| store.set_opened(None, cx)),
            }
        }));
    }

    /// Edit `path`, a file the checkout whose changes are shown tracks, from its first line:
    /// where it's shown if it has changes, else opened with none, shown among them as long as
    /// it's edited.
    pub fn open_file(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some((project, checkout)) = self.openable_checkout() else {
            return snack(
                &self.store,
                "Go to a checkout's changes to edit its files",
                cx,
            );
        };
        let shown = (self.diff.as_ref())
            .and_then(|diff| diff.files.iter().position(|f| f.file.path == path));
        // At its first change, as the file menu.s Edit file is.
        if let Some(file) = shown {
            return self.edit_hunk(file, 0, window, cx);
        }
        if self.store.read(cx).prefs.split {
            return snack(&self.store, "Switch to Unified to edit lines", cx);
        }
        let Some(root) = self.store.read(cx).path(&project) else {
            return;
        };
        self.end_edit(cx);
        let worktree = checkout.clone();
        self._read_sides = cx.spawn_in(window, async move |this, cx| {
            let read =
                backend::run(move || edit_lines::read_unchanged(&root, worktree.linked(), &path))
                    .await;
            (this.update_in(cx, |this, window, cx| {
                let (change, sides) = match read {
                    Ok(Some(read)) => read,
                    Ok(None) => return this.not_opened("It isn't a text file to edit", cx),
                    Err(err) => return this.not_opened(format!("{err:#}"), cx),
                };
                // Another diff shown meanwhile, or this one with changes to the file now.
                if this.openable_checkout().as_ref() != Some(&(project.clone(), checkout.clone())) {
                    return (this.store).update(cx, |store, cx| store.set_opened(None, cx));
                }
                let shown = (this.diff.as_ref()).and_then(|diff| {
                    (diff.files.iter()).position(|f| f.file.change.path == change.change.path)
                });
                if let Some(file) = shown {
                    return this.edit_at(file, 1, 0, window, cx);
                }
                let opened = FileDiff {
                    file: change.clone(),
                    body: DiffBody::Hunks {
                        hunks: Vec::new(),
                        added: 0,
                        removed: 0,
                        widest: [0, 0],
                        conflicts: Vec::new(),
                    },
                    fingerprint: 0,
                    annotated: true,
                    scope_lines: None,
                    drawing: None,
                    made_by: Vec::new(),
                };
                // The filter would hide it.
                if this.filtering && !super::keeps_path(&this.filter_terms(cx), &change.path) {
                    this.close_filter(window, cx);
                }
                let at = (project.clone(), checkout.clone());
                let mut edit = new_edit(project, checkout, &change, 0, sides);
                edit.opened = Some(opened);
                this.file_edit = Some(edit);
                // Its checkout's changes stay listed while it's edited, though there are none.
                (this.store).update(cx, |store, cx| store.set_opened(Some(at), cx));
                let Some((key, files)) =
                    (this.diff.as_ref()).map(|diff| (diff.key.clone(), diff.files.clone()))
                else {
                    return;
                };
                let files = this.live_files(&key, files, cx);
                if let Some(diff) = &mut this.diff {
                    diff.files = files;
                }
                this.refresh_edit(cx);
                this.activate(0, 0, window, cx);
                this.reveal_edit_row(cx);
            }))
            .ok();
        });
    }

    /// The file and new-side line number of the line at row `row`, an uncommitted one that
    /// isn't removed.
    pub(super) fn line_place(&self, row: usize) -> Option<(usize, u32)> {
        let diff = self.diff.as_ref()?;
        diff.key.checkout()?;
        match *diff.rows.get(row)? {
            DiffRow::Line { file, hunk, line } => {
                let found = diff.files.get(file)?.hunks().get(hunk)?.lines.get(line)?;
                (found.kind != LineKind::Removed).then_some((file, found.new?))
            }
            DiffRow::Context { file, new, .. } => Some((file, new)),
            _ => None,
        }
    }

    /// A double click at `at` in the diff, or a click in a file being edited: on a line of an
    /// uncommitted file, the caret goes there.
    pub(super) fn click_to_edit(
        &mut self,
        at: TextPoint,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((file, line)) = self.line_place(at.0) {
            self.edit_at(file, line, at.1, window, cx);
        }
    }

    /// Give the caret to line `line` of the file being edited, at byte `caret`. Nothing
    /// scrolls: a click is on the line already, and the keys reveal it themselves.
    fn activate(&mut self, line: usize, caret: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = &mut self.file_edit else {
            return;
        };
        let Some(text) = edit.lines.get(line).cloned() else {
            return;
        };
        if let Some(active) = &edit.active
            && active.line == line
        {
            active
                .input
                .update(cx, |input, cx| input.place_caret(caret, cx));
            return window.focus(&active.input.focus_handle(cx), cx);
        }
        // The file's lines scroll sideways together, the field with them, following its caret.
        let sideways = (self.sideways.get(&edit.shown_path)).map(|[_, new]| new.clone());
        // Coloured as the diff showed the line, while it's as read: a typed line is plain there.
        let theme = Theme::get(cx);
        let styles: Vec<(Range<usize>, gpui::HighlightStyle)> = (edit.shown.get(line))
            .and_then(Option::as_ref)
            .filter(|shown| *shown.text == *text.replace('\t', "    "))
            .map(|shown| {
                (shown.spans.iter())
                    .map(|(range, token)| (range.clone(), theme.token(*token).into()))
                    .collect()
            })
            .unwrap_or_default();
        let input = cx.new(|cx| {
            let input = TextInput::new("", false, cx).line_editor();
            let mut input = match sideways {
                Some(scroll) => input.scrolled_by(scroll),
                None => input,
            };
            input.load(text, cx);
            input.set_styles(styles);
            input.place_caret(caret, cx);
            input
        });
        let typed =
            cx.subscribe_in(
                &input,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Changed => this.typed(window, cx),
                    InputEvent::Submit | InputEvent::Cancel => this.leave_edit(window, cx),
                },
            );
        let keys = cx.subscribe_in(&input, window, |this, _, event: &LineEvent, window, cx| {
            this.line_key(event.clone(), window, cx)
        });
        window.focus(&input.focus_handle(cx), cx);
        edit.active = Some(Active {
            line,
            input,
            _listen: [typed, keys],
        });
        cx.notify();
    }

    /// The caret's line, and where on it.
    fn caret(&self, cx: &App) -> Option<(usize, usize)> {
        let active = self.file_edit.as_ref()?.active.as_ref()?;
        Some((active.line, active.input.read(cx).caret()))
    }

    fn typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((line, caret)) = self.caret(cx) else {
            return;
        };
        let Some(edit) = &mut self.file_edit else {
            return;
        };
        let Some(text) = (edit.active.as_ref()).map(|a| a.input.read(cx).text().to_owned()) else {
            return;
        };
        if edit.lines.get(line) == Some(&text) {
            return;
        }
        // Typed over a selection of more lines: the field took its line's part, and the rest
        // of it goes too, the lines it ran over joined at the caret.
        if let Some(selection) = edit.selection.take() {
            edit.remember(line, caret, false);
            let ((a, from), (b, to)) = selection.ordered();
            let (joined, at) = match selection.head.0 == b {
                // On the last line: the first's start, then the field's.
                true => {
                    let Some(start) = edit.lines.get(a).and_then(|raw| raw.get(..from)) else {
                        return;
                    };
                    (start.to_owned() + &text, from + caret)
                }
                // On the first: the field's, then the last's end.
                false => {
                    let Some(end) = edit.lines.get(b).and_then(|raw| raw.get(to..)) else {
                        return;
                    };
                    (text + end, caret)
                }
            };
            if b < edit.lines.len() {
                edit.lines.splice(a..=b, [joined]);
                edit.shown.splice(a..=b, [None]);
            }
            return self.after_edit(a, at, window, cx);
        }
        edit.remember(line, caret, true);
        if let (Some(raw), Some(shown)) = (edit.lines.get_mut(line), edit.shown.get_mut(line)) {
            (*raw, *shown) = (text, None);
        }
        self.refresh_edit(cx);
    }

    /// What goes past the caret's line: a line split or joined, the caret stepping to the
    /// line next to it, undo, saving, or lines pasted.
    fn line_key(&mut self, event: LineEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some((line, caret)) = self.caret(cx) else {
            return;
        };
        let anchor = (self.file_edit.as_ref())
            .and_then(|edit| edit.active.as_ref())
            .map_or(caret, |active| active.input.read(cx).anchor());
        let Some(edit) = &mut self.file_edit else {
            return;
        };
        // The field moves the caret's end of it within its line, as ⇧→ does.
        if let Some(selection) = &mut edit.selection
            && selection.head.0 == line
        {
            selection.head.1 = caret;
        }
        let selected = edit.selection;
        // What acts on a selection of more lines whole.
        match event {
            LineEvent::Moved => {
                if edit.selection.take().is_some() {
                    cx.notify();
                }
                return;
            }
            LineEvent::Select(step) => {
                let Some(to) = line
                    .checked_add_signed(step)
                    .filter(|&to| to < edit.lines.len())
                else {
                    return;
                };
                let at = edit.lines.get(to).map_or(0, |raw| caret.min(raw.len()));
                return self.select_to((to, at), (line, anchor), window, cx);
            }
            LineEvent::SelectAcross(step, at_end) => {
                let Some(to) = line
                    .checked_add_signed(step)
                    .filter(|&to| to < edit.lines.len())
                else {
                    return;
                };
                let at = match at_end {
                    true => edit.lines.get(to).map_or(0, String::len),
                    false => 0,
                };
                return self.select_to((to, at), (line, anchor), window, cx);
            }
            LineEvent::SelectEdge(start) => {
                let last = edit.lines.len().saturating_sub(1);
                let head = match start {
                    true => (0, 0),
                    false => (last, edit.lines.get(last).map_or(0, String::len)),
                };
                return self.select_to(head, (line, anchor), window, cx);
            }
            LineEvent::Copy { cut, own } => return self.copy_edit(cut, own, window, cx),
            LineEvent::Indent(more) if selected.is_some() => {
                return self.indent_selection(more, window, cx);
            }
            LineEvent::DeleteLine if selected.is_some() => {
                return self.delete_selected_lines(window, cx);
            }
            _ => {}
        }
        // Typing, a line split or joined, or lines pasted over it take a selection out first,
        // the caret going where it began.
        let taking = matches!(
            event,
            LineEvent::Split(_) | LineEvent::JoinBack | LineEvent::JoinNext | LineEvent::Paste(_)
        );
        let (line, caret, fresh) = match selected.filter(|_| taking) {
            Some(_) => {
                edit.remember(line, caret, false);
                let Some((line, caret)) = edit.take_selection() else {
                    return;
                };
                if matches!(event, LineEvent::JoinBack | LineEvent::JoinNext) {
                    return self.after_edit(line, caret, window, cx);
                }
                (line, caret, false)
            }
            None => (line, caret, true),
        };
        let Some(raw) = edit.lines.get(line).cloned() else {
            return;
        };
        // What moves on from it, or undoes, drops it.
        edit.selection = None;
        let (to, at) = match event {
            LineEvent::Save => return self.save_now(cx),
            LineEvent::Step(step, at) => {
                let Some(to) = line
                    .checked_add_signed(step)
                    .filter(|&to| to < edit.lines.len())
                else {
                    return;
                };
                self.activate(to, at, window, cx);
                return self.reveal_edit_row(cx);
            }
            LineEvent::Undo | LineEvent::Redo => {
                let (from, to) = match event {
                    LineEvent::Undo => (&mut edit.undo, &mut edit.redo),
                    _ => (&mut edit.redo, &mut edit.undo),
                };
                let Some(was) = from.pop() else {
                    return;
                };
                to.push(Snapshot {
                    lines: std::mem::replace(&mut edit.lines, was.lines),
                    shown: std::mem::replace(&mut edit.shown, was.shown),
                    line,
                    caret,
                });
                edit.typing = None;
                (was.line, was.caret)
            }
            LineEvent::Split(at) => {
                if fresh {
                    edit.remember(line, caret, false);
                }
                let (head, rest) = split(&raw, if fresh { at } else { caret });
                // The new line starts as indented as the one it came from.
                let indent: String = head.chars().take_while(|c| c.is_whitespace()).collect();
                let caret = indent.len();
                edit.replace_line(line, vec![head.to_owned(), indent + rest.trim_start()]);
                (line + 1, caret)
            }
            LineEvent::JoinBack => {
                let Some(before) = line.checked_sub(1) else {
                    return;
                };
                let Some(prev) = edit.lines.get(before).cloned() else {
                    return;
                };
                edit.remember(line, caret, false);
                edit.lines.remove(line);
                edit.shown.remove(line);
                edit.replace_line(before, vec![prev.clone() + &raw]);
                (before, prev.len())
            }
            LineEvent::JoinNext => {
                let Some(next) = edit.lines.get(line + 1).cloned() else {
                    return;
                };
                edit.remember(line, caret, false);
                edit.lines.remove(line + 1);
                edit.shown.remove(line + 1);
                edit.replace_line(line, vec![raw.clone() + &next]);
                (line, raw.len())
            }
            LineEvent::Edge(start) => {
                let last = edit.lines.len().saturating_sub(1);
                let (to, at) = match start {
                    true => (0, 0),
                    false => (last, edit.lines.get(last).map_or(0, String::len)),
                };
                self.activate(to, at, window, cx);
                return self.reveal_edit_row(cx);
            }
            LineEvent::Indent(true) => {
                edit.remember(line, caret, false);
                let indent = edit.indent.clone();
                edit.replace_line(line, vec![indent.clone() + &raw]);
                (line, caret + indent.len())
            }
            LineEvent::Indent(false) => {
                let cut = outdent(&raw, &edit.indent);
                let Some(rest) = raw.get(cut..).filter(|_| cut > 0) else {
                    return;
                };
                edit.remember(line, caret, false);
                edit.replace_line(line, vec![rest.to_owned()]);
                (line, caret.saturating_sub(cut))
            }
            LineEvent::MoveLine(step) => {
                let Some(to) = line
                    .checked_add_signed(step)
                    .filter(|&to| to < edit.lines.len())
                else {
                    return;
                };
                edit.remember(line, caret, false);
                // As shown too, so a line moved keeps its highlighting.
                edit.lines.swap(line, to);
                edit.shown.swap(line, to);
                (to, caret)
            }
            LineEvent::DeleteLine => {
                edit.remember(line, caret, false);
                match edit.lines.len() {
                    // The last line left is emptied rather than taken out.
                    0 | 1 => edit.replace_line(line, vec![String::new()]),
                    _ => {
                        edit.lines.remove(line);
                        edit.shown.remove(line);
                    }
                }
                (line.min(edit.lines.len().saturating_sub(1)), 0)
            }
            LineEvent::Paste(text) => {
                if fresh {
                    edit.remember(line, caret, false);
                }
                let (head, tail) = split(&raw, caret);
                let text = text.replace("\r\n", "\n").replace('\r', "\n");
                let mut parts: Vec<String> = text.split('\n').map(str::to_owned).collect();
                if let Some(first) = parts.first_mut() {
                    first.insert_str(0, head);
                }
                let count = parts.len();
                let caret = parts.last().map_or(0, String::len);
                if let Some(last) = parts.last_mut() {
                    last.push_str(tail);
                }
                edit.replace_line(line, parts);
                (line + count.saturating_sub(1), caret)
            }
            // Taken above.
            LineEvent::Moved
            | LineEvent::Select(_)
            | LineEvent::SelectAcross(..)
            | LineEvent::SelectEdge(_)
            | LineEvent::Copy { .. } => return,
        };
        self.after_edit(to, at, window, cx);
    }

    /// After the lines changed: the diff shown as they are, the caret on line `to` at byte `at`,
    /// in view, and a save once typing rests.
    fn after_edit(&mut self, to: usize, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(edit) = &mut self.file_edit {
            // The field shows its line as it is now.
            edit.active = None;
        }
        self.refresh_edit(cx);
        self.activate(to, at, window, cx);
        self.reveal_edit_row(cx);
    }

    /// Carry the selection on to `head`, from where it began, else from `from`, the caret's
    /// line and where its field's selection began.
    fn select_to(&mut self, head: Spot, from: Spot, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = &mut self.file_edit else {
            return;
        };
        let anchor = edit.selection.map_or(from, |selection| selection.anchor);
        edit.selection = Some(Selection { anchor, head });
        self.show_selection(window, cx);
    }

    /// Give the caret to the selection's head, its field selecting its line's part of it. Back
    /// on one line, the field's own selection is all of it.
    fn show_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection { anchor, head }) = self.file_edit.as_ref().and_then(|e| e.selection)
        else {
            return;
        };
        self.activate(head.0, head.1, window, cx);
        let Some(edit) = &mut self.file_edit else {
            return;
        };
        let len = edit.lines.get(head.0).map_or(0, String::len);
        let (range, reversed) = match anchor.0.cmp(&head.0) {
            Ordering::Less => (0..head.1, false),
            Ordering::Greater => (head.1..len, true),
            Ordering::Equal => (
                anchor.1.min(head.1)..anchor.1.max(head.1),
                head.1 < anchor.1,
            ),
        };
        if anchor.0 == head.0 {
            edit.selection = None;
        }
        if let Some(active) = &edit.active {
            (active.input).update(cx, |input, cx| input.select_range(range, reversed, cx));
        }
        self.reveal_edit_row(cx);
        cx.notify();
    }

    /// ⌘C, or with `cut` ⌘X: what's selected over more lines, or else `own`, what the caret's
    /// line has selected, goes on the clipboard.
    fn copy_edit(&mut self, cut: bool, own: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some((line, caret)) = self.caret(cx) else {
            return;
        };
        let Some(edit) = &mut self.file_edit else {
            return;
        };
        let (text, lines) = match edit.selection {
            Some(selection) => {
                // A selection ending at a line's very start takes nothing from it.
                let ((a, _), (b, to)) = selection.ordered();
                let last = if to == 0 && b > a { b - 1 } else { b };
                (edit.selected_text(selection), a..=last)
            }
            None => (own, line..=line),
        };
        if !text.is_empty() {
            // Said as the diff's own copy says it.
            let verb = if cut { "Cut" } else { "Copied" };
            let message = clipboard::took(verb, &text);
            clipboard::copy_saying(&self.store, text, message, cx);
            // As the diff's copied text blinks, so a copy says it took.
            edit.copied = Some(lines);
            self.blink_copied(super::Copied::Text, cx);
        }
        let Some(edit) = &mut self.file_edit else {
            return;
        };
        if !cut {
            return;
        }
        match edit.selection.is_some() {
            true => {
                edit.remember(line, caret, false);
                if let Some((line, caret)) = edit.take_selection() {
                    self.after_edit(line, caret, window, cx);
                }
            }
            // The line's own, which typing over it then saves.
            false => {
                if let Some(input) = edit.active.as_ref().map(|active| active.input.clone()) {
                    input.update(cx, |input, cx| input.delete_selected(window, cx));
                }
            }
        }
    }

    /// Tab (`more`) or ⇧Tab over a selection of more lines: each of them indented a step, or
    /// outdented, the selection staying on the same text.
    fn indent_selection(&mut self, more: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((line, caret)) = self.caret(cx) else {
            return;
        };
        let Some(edit) = &mut self.file_edit else {
            return;
        };
        let Some(selection) = edit.selection else {
            return;
        };
        edit.remember(line, caret, false);
        let ((a, _), (b, _)) = selection.ordered();
        let indent = edit.indent.clone();
        let mut shifts = Vec::new();
        for n in a..=b {
            let (Some(raw), Some(shown)) = (edit.lines.get_mut(n), edit.shown.get_mut(n)) else {
                break;
            };
            let shift = match more {
                true => {
                    raw.insert_str(0, &indent);
                    indent.len() as isize
                }
                false => {
                    let cut = outdent(raw, &indent);
                    raw.drain(..cut);
                    -(cut as isize)
                }
            };
            *shown = None;
            shifts.push(shift);
        }
        let moved = |(line, at): Spot| {
            let shift = line.checked_sub(a).and_then(|ix| shifts.get(ix)).copied();
            (line, at.saturating_add_signed(shift.unwrap_or(0)))
        };
        edit.selection = Some(Selection {
            anchor: moved(selection.anchor),
            head: moved(selection.head),
        });
        edit.active = None;
        self.refresh_edit(cx);
        self.show_selection(window, cx);
    }

    /// ⇧⌘K over a selection of more lines: every one of them taken out.
    fn delete_selected_lines(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((line, caret)) = self.caret(cx) else {
            return;
        };
        let Some(edit) = &mut self.file_edit else {
            return;
        };
        let Some(selection) = edit.selection.take() else {
            return;
        };
        edit.remember(line, caret, false);
        let ((a, _), (b, _)) = selection.ordered();
        match b + 1 >= edit.lines.len() && a == 0 {
            // All of them: the file keeps one empty line.
            true => {
                edit.lines = vec![String::new()];
                edit.shown = vec![None];
            }
            false => {
                let end = (b + 1).min(edit.lines.len());
                edit.lines.drain(a..end);
                edit.shown.drain(a..end);
            }
        }
        let to = a.min(edit.lines.len().saturating_sub(1));
        self.after_edit(to, 0, window, cx);
    }

    /// Take the caret's field away, as a press on another of the file's lines does, and what
    /// it had selected over more lines: the lines all draw as text till the press lets go.
    pub(super) fn put_field_away(&mut self, cx: &mut Context<Self>) {
        if let Some(edit) = &mut self.file_edit
            && (edit.active.is_some() || edit.selection.is_some())
        {
            (edit.active, edit.selection) = (None, None);
            cx.notify();
        }
    }

    /// Whether `row` shows a line of the file being edited that the last copy took from.
    pub(super) fn edit_copied(&self, row: DiffRow) -> bool {
        let (Some(edit), Some(diff)) = (&self.file_edit, &self.diff) else {
            return false;
        };
        let DiffRow::Line { file, hunk, line } = row else {
            return false;
        };
        let found = (diff.files.get(file))
            .filter(|f| edit.of(&diff.key, f))
            .and_then(|f| f.hunks().get(hunk)?.lines.get(line));
        let new = found
            .filter(|found| found.kind != LineKind::Removed)
            .and_then(|f| f.new);
        let n = new.and_then(|n| usize::try_from(n).ok()?.checked_sub(1));
        n.zip(edit.copied.as_ref())
            .is_some_and(|(n, copied)| copied.contains(&n))
    }

    /// The part of line `line` of hunk `hunk` of file `file` selected over more lines, as the
    /// line's shown, but the caret's, whose field shows its own.
    pub(super) fn edit_selected(
        &self,
        file: usize,
        hunk: usize,
        line: usize,
    ) -> Option<Range<usize>> {
        let edit = self.file_edit.as_ref()?;
        let selection = edit.selection?;
        if self.edited_file() != Some(file) {
            return None;
        }
        let found = (self
            .diff
            .as_ref()?
            .files
            .get(file)?
            .hunks()
            .get(hunk)?
            .lines)
            .get(line)?;
        if found.kind == LineKind::Removed {
            return None;
        }
        let n = usize::try_from(found.new?).ok()?.checked_sub(1)?;
        if edit.active.as_ref().is_some_and(|active| active.line == n) {
            return None;
        }
        let ((a, from), (b, to)) = selection.ordered();
        if n < a || n > b {
            return None;
        }
        let raw = edit.lines.get(n)?;
        let start = if n == a { from } else { 0 };
        let end = if n == b { to } else { raw.len() };
        let len = found.text.len();
        Some(shown_offset(raw, start).min(len)..shown_offset(raw, end).min(len))
    }

    /// A drag let go over the lines of the file being edited: its text selected for editing,
    /// as with the keys, the caret where the drag ended.
    pub(super) fn drag_to_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((from, to)) = self.text_selection.filter(|(from, to)| from != to) else {
            return;
        };
        let Some(to) = drag_end(from.0, to, |row| self.line_place(row).is_some()) else {
            return;
        };
        let (Some((start_file, start)), Some((end_file, end))) =
            (self.line_place(from.0), self.line_place(to.0))
        else {
            return;
        };
        if self
            .edited_file()
            .is_none_or(|file| file != start_file || file != end_file)
        {
            return;
        }
        let Some(edit) = &mut self.file_edit else {
            return;
        };
        let spot = |line: u32, shown: usize| {
            let n = (line as usize).saturating_sub(1);
            (n, edit.lines.get(n).map_or(0, |raw| raw_offset(raw, shown)))
        };
        let (anchor, head) = (spot(start, from.1), spot(end, to.1));
        edit.selection = Some(Selection { anchor, head });
        self.text_selection = None;
        self.show_selection(window, cx);
    }

    /// Show the file being edited as typed: its diff worked out again, in place of the one
    /// loaded.
    fn refresh_edit(&mut self, cx: &mut Context<Self>) {
        let (Some(edit), Some(diff)) = (&self.file_edit, &mut self.diff) else {
            return;
        };
        let body = edit.body();
        let key = diff.key.clone();
        let files = Arc::make_mut(&mut diff.files);
        if let Some(file) = files.iter_mut().find(|f| edit.of(&key, f)) {
            file.body = body;
        }
        self.rebuild_rows(cx);
    }

    /// The files of the diff of `key` just loaded, with the one being edited as typed. One
    /// whose file changed since it was last read here is read again, to tell its own writes
    /// from anyone else's.
    pub(super) fn live_files(
        &mut self,
        key: &DiffKey,
        mut files: Arc<Vec<FileDiff>>,
        cx: &mut Context<Self>,
    ) -> Arc<Vec<FileDiff>> {
        let Some(edit) = &self.file_edit else {
            return files;
        };
        let Some(found) = files.iter().position(|f| edit.of(key, f)) else {
            // A file opened with no changes, which has none yet, or none again as it's typed
            // back, stays where the files are listed as it would be with some.
            if let Some(opened) = edit.opened.as_ref().filter(|_| {
                key.project == edit.project
                    && key.checkout().as_ref() == Some(&edit.checkout)
                    && edit.read_lines == edit.old
            }) {
                let file = FileDiff {
                    body: edit.body(),
                    ..opened.clone()
                };
                let order = self.diff_file_order(cx);
                let files_mut = Arc::make_mut(&mut files);
                let at = (files_mut.iter()).position(|f| order(&file, f).is_lt());
                files_mut.insert(at.unwrap_or(files_mut.len()), file);
                return files;
            }
            // Gone from the diff, as when it's committed or discarded elsewhere, or another
            // diff is shown: the edit ends, what's typed saved.
            self.end_edit(cx);
            return files;
        };
        let fingerprint = files.get(found).map(|f| f.fingerprint);
        if fingerprint.is_some_and(|now| now != edit.fingerprint) {
            self.check_disk(fingerprint.unwrap_or_default(), cx);
        }
        let Some(edit) = &self.file_edit else {
            return files;
        };
        let body = edit.body();
        if let Some(file) = Arc::make_mut(&mut files).get_mut(found) {
            file.body = body;
        }
        files
    }

    /// Read the file being edited, its diff now hashing to `fingerprint`: as it was last
    /// written here, or as a write under way leaves it, the change was this edit's own; else
    /// something else wrote to it.
    fn check_disk(&mut self, fingerprint: u64, cx: &mut Context<Self>) {
        let Some(edit) = &self.file_edit else {
            return;
        };
        let Some(root) = self.store.read(cx).path(&edit.project) else {
            return;
        };
        let (worktree, path) = (edit.checkout.clone(), edit.path.clone());
        cx.spawn(async move |this, cx| {
            let now = backend::run(move || {
                edit_lines::read_text(&root, worktree.linked(), path.as_ref())
            })
            .await;
            (this.update(cx, |this, cx| {
                let Some(edit) = &mut this.file_edit else {
                    return;
                };
                let ours = (now.ok().flatten())
                    .is_some_and(|now| now == edit.read || Some(&now) == edit.writing.as_ref());
                match ours {
                    true => edit.fingerprint = fingerprint,
                    false => edit.changed_on_disk = true,
                }
                cx.notify();
            }))
            .ok();
        })
        .detach();
    }

    /// Save what's typed now, as before a commit, which would otherwise leave the last of it
    /// out.
    pub fn flush_edits(&mut self, cx: &mut Context<Self>) {
        self.save_now(cx);
    }

    /// Write what's typed, as ⌘S does, unless that's saved, a write is under way, or something
    /// else wrote to the file meanwhile.
    pub(super) fn save_now(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = &self.file_edit else {
            return;
        };
        if !edit.dirty() || edit.changed_on_disk || edit.writing.is_some() {
            return;
        }
        let write = WriteFile {
            checkout: edit.checkout.clone(),
            path: edit.path.clone(),
            read: edit.read.clone(),
            text: edit.text(),
            record: !edit.recorded,
        };
        self.write_edit(write, cx);
    }

    /// Run `write` of the file being edited, then take what it came to. Not cancelled when the
    /// edit ends, so a write that lands after still counts as the app's own.
    fn write_edit(&mut self, write: WriteFile, cx: &mut Context<Self>) {
        let Some(edit) = &mut self.file_edit else {
            return;
        };
        let Some(root) = self.store.read(cx).path(&edit.project) else {
            return;
        };
        edit.writing = Some(write.text.clone());
        Self::spawn_write(root, edit.project.clone(), write, cx);
        cx.notify();
    }

    /// Write `write` to a file of `project`, at `root`, then take what it came to.
    fn spawn_write(root: PathBuf, project: ProjectId, write: WriteFile, cx: &mut Context<Self>) {
        let (text, started) = (write.text.clone(), Instant::now());
        cx.spawn(async move |this, cx| {
            let wrote = match backend::run(move || backend::apply(&root, write)).await {
                Ok((true, ran)) => Wrote::Written {
                    span: ran.span,
                    recorded: ran.recorded,
                },
                Ok((false, _)) => Wrote::ChangedOnDisk,
                Err(err) => Wrote::Failed(format!("{err:#}")),
            };
            (this.update(cx, |this, cx| this.wrote(project, text, wrote, started, cx))).ok();
        })
        .detach();
    }

    /// What a write of `text` to the file of an edit in `project` came to.
    fn wrote(
        &mut self,
        project: ProjectId,
        text: String,
        wrote: Wrote,
        started: Instant,
        cx: &mut Context<Self>,
    ) {
        if let Wrote::Written { span, recorded } = &wrote {
            let (span, recorded) = (*span, *recorded);
            self.store.update(cx, |store, _| {
                store.count_own_write(&project, span, recorded, started)
            });
        }
        let Some(edit) = self
            .file_edit
            .as_mut()
            .filter(|edit| edit.writing.as_ref() == Some(&text))
        else {
            // The edit ended while it saved: what was typed since goes now, and the diff shows
            // the file as written.
            if let Some(ix) = (self.write_after.iter()).position(|(_, write)| write.read == text) {
                let (project, write) = self.write_after.swap_remove(ix);
                if let Some(root) = self.store.read(cx).path(&project) {
                    Self::spawn_write(root, project, write, cx);
                }
            }
            self.refresh_diff(false, cx);
            return;
        };
        edit.writing = None;
        match wrote {
            Wrote::Written { .. } => {
                edit.read_lines = edit_lines::lines(&text).map(str::to_owned).collect();
                edit.read = text;
                edit.recorded = true;
                // Saved once the caret had gone, with nothing typed since: nothing keeps it.
                if edit.active.is_none() && !edit.dirty() {
                    self.keep_view();
                    self.end_edit(cx);
                }
            }
            Wrote::ChangedOnDisk => edit.changed_on_disk = true,
            Wrote::Failed(why) => snack(&self.store, why, cx),
        }
        cx.notify();
    }

    /// Put the caret away, as Esc does, the keys going back to the diff. What isn't saved stays
    /// as typed, its dot showing; with nothing to save, the diff goes back to its hunks. With the
    /// caret away already, Esc again closes the file, asking first whether to save what's typed:
    /// only ⌘S or a yes writes it.
    pub(super) fn leave_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let esc = (self.file_edit.as_ref()).map(|edit| on_esc(edit, self.has_caret()));
        match esc {
            Some(Esc::AskToSave) => return self.ask_to_save(window, cx),
            Some(Esc::PutCaretAway) => {
                if let Some(edit) = &mut self.file_edit {
                    (edit.active, edit.selection) = (None, None);
                }
                self.refresh_edit(cx);
            }
            Some(Esc::Close) => {
                self.keep_view();
                self.end_edit(cx);
            }
            None => {}
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// End the edit, as another file's begins or the diff goes, saving what's typed first, unless
    /// something else wrote to the file: then what's typed goes, and the file stays as that left
    /// it.
    fn end_edit(&mut self, cx: &mut Context<Self>) {
        let saving = (self.file_edit.as_ref())
            .filter(|edit| edit.dirty() && !edit.changed_on_disk && edit.writing.is_none())
            .map(|edit| edit.shown_path.clone());
        self.save_now(cx);
        let writing = (self.file_edit.as_ref()).is_some_and(|edit| edit.writing.is_some());
        // Typed after a write already under way began, which goes once that lands. Not what's
        // put back as read, as Don't save does, which would undo that write.
        if let Some(edit) =
            (self.file_edit.as_ref()).filter(|edit| edit.dirty() && !edit.changed_on_disk)
            && let Some(written) = (edit.writing.clone()).filter(|written| *written != edit.text())
        {
            let write = WriteFile {
                checkout: edit.checkout.clone(),
                path: edit.path.clone(),
                read: written,
                text: edit.text(),
                record: false,
            };
            self.write_after.push((edit.project.clone(), write));
        }
        // Saved without ⌘S, so it says so.
        if let Some(path) = saving {
            let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
            snack(&self.store, format!("Saved {name}"), cx);
        }
        let Some(edit) = self.file_edit.take() else {
            return;
        };
        if edit.opened.is_some() {
            self.store
                .update(cx, |store, cx| store.set_opened(None, cx));
        }
        if edit.changed_on_disk && edit.dirty() {
            let message = format!(
                "Your edits to {} weren't saved: it changed on disk",
                edit.shown_path
            );
            snack(&self.store, message, cx);
        }
        // Once the write lands, rather than show the file as it was before it for a moment.
        if !writing {
            self.refresh_diff(false, cx);
        }
    }

    /// As the edit ends where it is, keep the file shown whole as typed, so the lines on screen
    /// stay there as the diff loads again.
    fn keep_view(&mut self) {
        let Some(edit) = &self.file_edit else {
            return;
        };
        let lines: Vec<DiffLine> = (edit.lines.iter().enumerate())
            .map(|(ix, raw)| {
                let shown = (edit.shown.get(ix).cloned().flatten())
                    .unwrap_or_else(|| edit_lines::typed_line(raw));
                DiffLine {
                    kind: LineKind::Context,
                    old: None,
                    new: u32::try_from(ix + 1).ok(),
                    ..shown
                }
            })
            .collect();
        let widest = (lines.iter())
            .map(|line| backend::columns(&line.text))
            .max();
        let path = edit.shown_path.clone();
        self.whole.insert(
            path,
            Some((super::AS_TYPED, Arc::new(lines), widest.unwrap_or(0))),
        );
    }

    /// Close the file being edited, asking whether to save what's typed, or drop it, first.
    fn ask_to_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = &self.file_edit else {
            return;
        };
        let name = (edit.shown_path.rsplit('/').next()).unwrap_or(&edit.shown_path);
        let answer = window.prompt(
            gpui::PromptLevel::Info,
            &format!("Save changes to {name}?"),
            Some("Your edits are lost if you don't save them."),
            &["Save", "Don't save", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let answer = answer.await;
            (this.update_in(cx, |this, window, cx| {
                match answer {
                    Ok(0) => {}
                    // As read last: nothing to save, so the edit ends without writing.
                    Ok(1) => {
                        if let Some(edit) = &mut this.file_edit {
                            edit.lines = edit.read_lines.clone();
                        }
                    }
                    _ => return,
                }
                this.keep_view();
                this.end_edit(cx);
                window.focus(&this.focus, cx);
                cx.notify();
            }))
            .ok();
        })
        .detach();
    }

    /// Drop what's typed, the file staying as something else wrote it.
    fn discard_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(edit) = &mut self.file_edit {
            (edit.lines, edit.changed_on_disk) = (edit.read_lines.clone(), false);
        }
        self.leave_edit(window, cx);
    }

    /// Keep what's typed over what was written to the file meanwhile: the file's read again
    /// to start from, and what's typed saved over it.
    fn keep_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(edit), Some(diff)) = (&self.file_edit, &self.diff) else {
            return;
        };
        let Some(root) = self.store.read(cx).path(&edit.project) else {
            return;
        };
        let Some(file) = diff.files.iter().find(|f| edit.of(&diff.key, f)) else {
            return;
        };
        let (change, fingerprint) = (file.file.clone(), file.fingerprint);
        let worktree = edit.checkout.clone();
        self._read_sides = cx.spawn_in(window, async move |this, cx| {
            let read =
                backend::run(move || edit_lines::read_sides(&root, worktree.linked(), &change))
                    .await;
            (this.update(cx, |this, cx| {
                let Some(edit) = &mut this.file_edit else {
                    return;
                };
                match read {
                    Ok(Some(sides)) => {
                        edit.read_lines =
                            edit_lines::lines(&sides.text).map(str::to_owned).collect();
                        (edit.read, edit.old, edit.old_shown) =
                            (sides.text, sides.old, sides.old_shown);
                        (edit.fingerprint, edit.changed_on_disk) = (fingerprint, false);
                        this.refresh_edit(cx);
                        this.save_now(cx);
                    }
                    Ok(None) => snack(&this.store, "It isn't text anymore", cx),
                    Err(err) => snack(&this.store, format!("{err:#}"), cx),
                }
            }))
            .ok();
        });
    }

    /// Scroll the caret's line just into view, if the keys took it off it or to the edge.
    /// Judged from where the list is scrolled, as its rows were just built again and aren't laid
    /// out yet: the file's lines are a line tall each.
    fn reveal_edit_row(&mut self, cx: &App) {
        let Some(row) = self.edit_row() else {
            return;
        };
        let top = self.list.logical_scroll_top().item_ix;
        let height = self.list.viewport_bounds().size.height;
        let rows = (height / super::code_line(cx)).floor().max(1.) as usize;
        // A line to spare above it, and a couple below, so it's never at the very edge.
        if row <= top {
            self.scroll_to_row(row.saturating_sub(1), false);
        } else if row + 3 > top + rows {
            self.scroll_to_row((row + 3).saturating_sub(rows), false);
        }
    }

    /// Where row `row` is on screen, from the top of the diff, if it was laid out.
    fn row_offset(&self, row: usize) -> Option<Pixels> {
        let item = self.list.bounds_for_item(row)?;
        Some(item.top() - self.list.viewport_bounds().top())
    }

    /// Scroll so row `row` is `offset` from the top of the diff, going back over the rows above
    /// it as tall as the list takes them to be until they fill that.
    fn hold_row(&mut self, row: usize, offset: Pixels, cx: &App) {
        let (mut top, mut above) = (row, Pixels::ZERO);
        while above < offset && top > 0 {
            top -= 1;
            above += self.row_height(top, cx);
        }
        self.list.scroll_to(ListOffset {
            item_ix: top,
            offset_in_item: (above - offset).max(Pixels::ZERO),
        });
    }

    /// The row showing new-side line `line` (1-based) of file `file`.
    fn line_row(&self, file: usize, line: u32) -> Option<usize> {
        let diff = self.diff.as_ref()?;
        self.row_of(|row| match *row {
            DiffRow::Line {
                file: f,
                hunk,
                line: l,
            } if f == file => (diff.files.get(f))
                .and_then(|f| f.hunks().get(hunk)?.lines.get(l))
                .is_some_and(|found| found.kind != LineKind::Removed && found.new == Some(line)),
            DiffRow::Context { file: f, new, .. } => f == file && new == line,
            _ => false,
        })
    }

    /// The row the caret's line is on.
    fn edit_row(&self) -> Option<usize> {
        let line = self.file_edit.as_ref()?.active.as_ref()?.line as u32 + 1;
        self.line_row(self.edited_file()?, line)
    }

    /// Whether file `file` of the loaded diff is the one being edited.
    pub(super) fn edits_file_at(&self, file: usize) -> bool {
        self.edited_file() == Some(file)
    }

    /// The field for line `line` of hunk `hunk` of file `file`, when it's the caret's.
    pub(super) fn edit_field(
        &self,
        file: usize,
        hunk: usize,
        line: usize,
    ) -> Option<Entity<TextInput>> {
        let active = self.file_edit.as_ref()?.active.as_ref()?;
        if self.edited_file() != Some(file) {
            return None;
        }
        let found = (self
            .diff
            .as_ref()?
            .files
            .get(file)?
            .hunks()
            .get(hunk)?
            .lines)
            .get(line)?;
        (found.kind != LineKind::Removed && found.new == Some(active.line as u32 + 1))
            .then(|| active.input.clone())
    }

    /// Whether `file` of the loaded diff is being edited, which shows it whole, as typed.
    pub(super) fn edits_file(&self, file: &FileDiff) -> bool {
        let (Some(edit), Some(diff)) = (&self.file_edit, &self.diff) else {
            return false;
        };
        edit.of(&diff.key, file)
    }

    /// The file being edited, by its name, and how its edit stands: saved, saving, or with
    /// something else having written to it, a warning. For the status bar, which says what a
    /// mode is doing.
    pub fn edit_status(&self) -> Option<(String, &'static str, bool)> {
        let edit = self.file_edit.as_ref()?;
        let name = (edit.shown_path.rsplit('/').next()).unwrap_or(&edit.shown_path);
        let says = match (edit.changed_on_disk, edit.writing.is_some(), edit.dirty()) {
            (true, ..) => "changed on disk",
            (false, true, _) => "saving",
            (false, false, true) => "not saved",
            (false, false, false) if edit.changed() => "saved",
            (false, false, false) => "nothing changed yet",
        };
        Some((name.to_owned(), says, edit.changed_on_disk))
    }

    /// What the header of `file_diff` says while it's edited, on its solid ground when `on`:
    /// that it's being edited, or once something else wrote to the file meanwhile, ways to
    /// discard or keep what's typed.
    pub(super) fn edit_controls(
        &self,
        file_diff: &FileDiff,
        on: bool,
        entity: WeakEntity<Detail>,
        cx: &App,
    ) -> Option<AnyElement> {
        let edit = (self.file_edit.as_ref()).filter(|_| self.edits_file(file_diff))?;
        let theme = Theme::get(cx);
        if !edit.changed_on_disk {
            let color = match on {
                true => theme.muted_on_solid(),
                false => theme.text_2,
            };
            let editing = h_flex()
                .flex_none()
                .gap(dp(4.))
                .text_size(dp(12.))
                .text_color(color)
                .child(small_icon(Icon::Edit).text_color(color))
                .child("Editing");
            return Some(editing.into_any_element());
        }
        let act = |f: fn(&mut Detail, &mut Window, &mut Context<Detail>)| {
            let entity = entity.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                // Not a click on the header, which folds the file.
                cx.stop_propagation();
                (entity.update(cx, |this, cx| f(this, window, cx))).ok();
            }
        };
        let controls = h_flex()
            .flex_none()
            .gap(dp(4.))
            .text_size(dp(12.))
            // Beside the buttons, which only make sense with it.
            .child(
                div()
                    .mr(dp(2.))
                    .text_color(theme.warn)
                    .child("Changed on disk"),
            )
            .child(
                button("edit-discard", "Discard my edits")
                    .small()
                    .style(ButtonStyle::Ghost)
                    .tooltip("Keep the file as it was written")
                    .on_click(act(Self::discard_edit)),
            )
            .child(
                button("edit-keep", "Keep mine")
                    .small()
                    .style(ButtonStyle::Gray)
                    .tooltip("Save over what was written")
                    .on_click(act(Self::keep_edit)),
            );
        Some(controls.into_any_element())
    }

    /// What has the keys while a line's typed in: its field.
    pub(super) fn caret_focus(&self, cx: &App) -> Option<gpui::FocusHandle> {
        let active = self.file_edit.as_ref()?.active.as_ref()?;
        Some(active.input.focus_handle(cx))
    }

    /// Whether Esc closes the file being edited, asking first to save what's typed: the caret's
    /// away, and what's typed can be saved, as nothing else wrote to the file meanwhile.
    pub fn esc_closes_edit(&self) -> bool {
        !self.has_caret()
            && (self.file_edit.as_ref())
                .is_some_and(|edit| matches!(on_esc(edit, false), Esc::Close | Esc::AskToSave))
    }

    /// Whether a file's being typed in: the caret's in a line of it, or text of it selected.
    pub fn has_caret(&self) -> bool {
        (self.file_edit.as_ref())
            .is_some_and(|edit| edit.active.is_some() || edit.selection.is_some())
    }

    /// Whether `file` of the loaded diff is being edited with what's typed not saved yet.
    pub(super) fn unsaved(&self, file: &FileDiff) -> bool {
        (self.file_edit.as_ref())
            .is_some_and(|edit| (edit.dirty() || edit.writing.is_some()) && self.edits_file(file))
    }

    /// Save what's typed at once, as the app quits: it may not wait for a write of its own.
    pub(super) fn save_at_quit(&mut self, cx: &App) {
        let Some(edit) = (self.file_edit.as_ref())
            .filter(|edit| edit.dirty() && !edit.changed_on_disk && edit.writing.is_none())
        else {
            return;
        };
        let Some(root) = self.store.read(cx).path(&edit.project) else {
            return;
        };
        let write = WriteFile {
            checkout: edit.checkout.clone(),
            path: edit.path.clone(),
            read: edit.read.clone(),
            text: edit.text(),
            record: !edit.recorded,
        };
        if let Err(err) = backend::apply(&root, write) {
            tracing::warn!("Couldn't save {} as the app quit: {err:#}", edit.shown_path);
        }
    }
}

/// What Esc does to the file being edited.
#[derive(Debug, PartialEq)]
enum Esc {
    /// Takes the caret out of the line, what's typed staying as it is.
    PutCaretAway,
    /// Closes the file, which has nothing to save.
    Close,
    /// Asks whether to save what's typed, then closes the file.
    AskToSave,
}

/// What Esc does to `edit`, the caret in a line of it or not (`caret`): with the caret in, it
/// comes out, unless nothing's kept the file open, which then closes; with the caret out, the
/// file closes, asking to save what's typed. Once something else wrote to the file, the caret
/// comes out and the file stays, for its header's choice between what's typed and that.
fn on_esc(edit: &FileEdit, caret: bool) -> Esc {
    let (dirty, writing) = (edit.dirty(), edit.writing.is_some());
    esc_for(caret, dirty, writing, edit.changed_on_disk)
}

fn esc_for(caret: bool, dirty: bool, writing: bool, changed_on_disk: bool) -> Esc {
    match (caret, changed_on_disk) {
        (false, false) if dirty => Esc::AskToSave,
        (false, false) => Esc::Close,
        _ if dirty || writing || changed_on_disk => Esc::PutCaretAway,
        _ => Esc::Close,
    }
}

/// Where a drag over the diff's rows from row `from` ends, let go at `to`: there on one of the
/// file's lines (`is_line`); on a row that isn't, as a removed line above the lines that replace
/// it, at the nearest that is towards where it began, at its start going up and its end going
/// down. `None` with none between them.
fn drag_end(from: usize, to: TextPoint, is_line: impl Fn(usize) -> bool) -> Option<TextPoint> {
    let step: isize = if to.0 < from { 1 } else { -1 };
    let mut row = to.0;
    loop {
        if is_line(row) {
            let at = match (row == to.0, step) {
                (true, _) => to.1,
                (false, 1) => 0,
                (false, _) => usize::MAX,
            };
            return Some((row, at));
        }
        if row == from {
            return None;
        }
        row = row.saturating_add_signed(step);
    }
}

/// An edit of `change`, from both of its `sides`, read when its diff hashed to `fingerprint`.
fn new_edit(
    project: ProjectId,
    checkout: Checkout,
    change: &backend::FileChange,
    fingerprint: u64,
    sides: Sides,
) -> FileEdit {
    let lines: Vec<String> = edit_lines::lines(&sides.text).map(str::to_owned).collect();
    let newline = match sides.text.contains("\r\n") {
        true => "\r\n",
        false => "\n",
    };
    FileEdit {
        project,
        checkout,
        path: change.change.path.clone(),
        shown_path: change.path.clone(),
        fingerprint,
        ends: sides.text.ends_with('\n'),
        indent: indent_of(&lines),
        shown: sides.new_shown.into_iter().map(Some).collect(),
        read_lines: lines.clone(),
        lines,
        original: sides.text.clone(),
        read: sides.text,
        writing: None,
        recorded: false,
        old: sides.old,
        old_shown: sides.old_shown,
        newline,
        active: None,
        selection: None,
        copied: None,
        undo: Vec::new(),
        redo: Vec::new(),
        typing: None,
        changed_on_disk: false,
        opened: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Esc, Selection, drag_end, esc_for, indent_of, outdent, raw_offset, shown_offset, split,
    };

    #[test]
    fn esc_puts_the_caret_away_then_closes_asking_to_save() {
        // (caret in a line, typed and not saved, a write under way, changed on disk)
        assert_eq!(
            esc_for(true, false, false, false),
            Esc::Close,
            "with nothing typed, the file closes at once"
        );
        assert_eq!(
            esc_for(true, true, false, false),
            Esc::PutCaretAway,
            "with something typed, the caret comes out and the file stays"
        );
        assert_eq!(
            esc_for(false, true, false, false),
            Esc::AskToSave,
            "then Esc asks before anything's written or dropped"
        );
        assert_eq!(
            esc_for(false, false, true, false),
            Esc::Close,
            "saving already: nothing's left to ask about"
        );
        assert_eq!(
            esc_for(false, true, false, true),
            Esc::PutCaretAway,
            "changed on disk, Esc never saves over it nor drops what's typed"
        );
    }

    #[test]
    fn a_drag_let_go_off_the_files_lines_ends_on_the_nearest() {
        // Rows 3 and 4 are a removed line and a hunk's box, which aren't the file's lines.
        let is_line = |row: usize| !matches!(row, 3 | 4);
        assert_eq!(
            drag_end(6, (2, 5), is_line),
            Some((2, 5)),
            "on a line, there"
        );
        assert_eq!(
            drag_end(6, (3, 5), is_line),
            Some((5, 0)),
            "going up onto a removed line, the start of the line below it"
        );
        assert_eq!(
            drag_end(1, (4, 5), is_line),
            Some((2, usize::MAX)),
            "going down onto one, the end of the line above it"
        );
        assert_eq!(drag_end(3, (4, 0), is_line), None, "no line between them");
    }

    #[test]
    fn a_click_on_shown_text_finds_its_place_in_the_line() {
        assert_eq!(raw_offset("abc", 2), 2, "plain text is as shown");
        assert_eq!(raw_offset("\tx", 4), 1, "a tab shows as four spaces");
        assert_eq!(
            raw_offset("\tx", 2),
            1,
            "a click inside a tab's spaces is past it"
        );
        assert_eq!(raw_offset("ab", 9), 2, "past the end is the end");
    }

    #[test]
    fn shown_and_file_offsets_go_both_ways() {
        assert_eq!(shown_offset("\tx", 1), 4, "past a tab is four spaces on");
        assert_eq!(raw_offset("\tx", shown_offset("\tx", 1)), 1, "and back");
        assert_eq!(outdent("\t\tx", "\t"), 1, "one tab at a time");
        assert_eq!(outdent("   x", "  "), 2, "a step of spaces");
        assert_eq!(outdent(" x", "    "), 1, "fewer, all of them");
    }

    #[test]
    fn a_selection_runs_from_its_start_whichever_way_it_was_made() {
        let up = Selection {
            anchor: (3, 1),
            head: (1, 4),
        };
        assert_eq!(
            up.ordered(),
            ((1, 4), (3, 1)),
            "made upwards, it starts at its head"
        );
    }

    #[test]
    fn tab_indents_as_the_file_does() {
        let lines = |text: &str| text.lines().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            indent_of(&lines("a\n\tb\n")),
            "\t",
            "a file indented with tabs"
        );
        assert_eq!(
            indent_of(&lines("a\n  b\n    c\n")),
            "  ",
            "two spaces a step"
        );
        assert_eq!(
            indent_of(&lines("a\nb\n")),
            "    ",
            "nothing indented yet: four"
        );
        assert_eq!(
            indent_of(&lines("a\n b\n")),
            "  ",
            "a lone space is alignment, not a step"
        );
    }

    #[test]
    fn a_line_splits_on_a_character_boundary() {
        assert_eq!(split("héllo", 2), ("h", "éllo"), "inside é, before it");
        assert_eq!(split("ab", 9), ("ab", ""), "past the end, at it");
    }
}
