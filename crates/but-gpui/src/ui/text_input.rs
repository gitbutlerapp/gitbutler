//! A small text field, single- or multi-line with soft wrapping.
//!
//! GPUI ships no text input; this follows the structure of its `input.rs` example
//! (Apache-2.0): the entity owns the text and selection, a custom element shapes and
//! paints it, and `EntityInputHandler` connects it to the platform's IME.

use std::ops::Range;
use std::time::Duration;

use gpui::{
    App, Bounds, ContentMask, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    Role, ScrollHandle, SharedString, Style, Task, TextRun, UTF16Selection, UnderlineStyle, Window,
    WrappedLine, actions, div, fill, point, prelude::*, relative, size,
};
use unicode_segmentation::UnicodeSegmentation as _;

use super::fixed;
use crate::clipboard;
use crate::theme::{Highlight, Theme};

actions!(
    text_input,
    [
        SelectAll,
        Newline,
        Submit,
        Cancel,
        Paste,
        Cut,
        Copy,
        Undo,
        Redo,
        Save,
        Indent,
        Outdent,
        MoveLineUp,
        MoveLineDown,
        DeleteLine,
        SelectParagraphBackward,
        SelectParagraphForward
    ]
);

/// What the last change to a field's text was, for undoing runs of them as one.
#[derive(Clone, Copy, PartialEq)]
enum Edit {
    /// A character typed after another, spaces too, as the platform's fields undo a run of
    /// typing at once; or text an input method is still composing.
    Typed,
    /// A character deleted after another.
    Deleted,
    /// Text set from outside, as AI writing a message sets it again and again.
    Set,
    Other,
}

/// Where a motion or deletion goes from the cursor.
#[derive(Clone, Copy, PartialEq)]
enum Target {
    Left,
    Right,
    Up,
    Down,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    Start,
    End,
}

/// Move the cursor, or with `select` the selection's moving end.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = text_input, no_json)]
struct Move {
    to: Target,
    select: bool,
}

/// Delete the selection, or up to the target when nothing is selected.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = text_input, no_json)]
struct Delete {
    to: Target,
}

const CONTEXT: &str = "TextInput";

impl Target {
    fn words(self) -> &'static str {
        match self {
            Target::Left => "left",
            Target::Right => "right",
            Target::Up => "up",
            Target::Down => "down",
            Target::WordLeft => "word left",
            Target::WordRight => "word right",
            Target::LineStart => "line start",
            Target::LineEnd => "line end",
            Target::Start => "start",
            Target::End => "end",
        }
    }
}

/// Keys [`bind_keys`] binds.
pub struct BoundKey {
    /// What they do, as `Move` and `Delete` run keys for many targets.
    pub title: String,
    pub action: &'static str,
    pub label: SharedString,
}

/// The keys [`bind_keys`] binds on this platform, for the command editor.
pub fn bound_keys(cx: &App) -> Vec<BoundKey> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    (keymap.bindings())
        .filter(|binding| {
            let in_field = binding
                .predicate()
                .is_some_and(|p| p.to_string() == CONTEXT);
            in_field && binding.action().name().starts_with("text_input::")
        })
        .map(|binding| BoundKey {
            title: title_of(binding.action()),
            action: binding.action().name(),
            label: super::keys_of(binding),
        })
        .collect()
}

fn title_of(action: &dyn gpui::Action) -> String {
    if let Some(Move { to, select }) = action.as_any().downcast_ref::<Move>() {
        let verb = if *select { "Select to" } else { "Move to" };
        return format!("{verb} {}", to.words());
    }
    if let Some(Delete { to }) = action.as_any().downcast_ref::<Delete>() {
        return format!("Delete to {}", to.words());
    }
    crate::commands::title_of(action.name())
}

/// The platform's text keys: on macOS the Cocoa text system's, ⌘ for lines, ⌥ for words and
/// Emacs's ⌃ keys; elsewhere ctrl for words and the document, home and end for lines.
pub fn bind_keys(cx: &mut App) {
    use Target::*;
    let c = Some(CONTEXT);
    let go = |keys, to| KeyBinding::new(keys, Move { to, select: false }, c);
    let extend = |keys, to| KeyBinding::new(keys, Move { to, select: true }, c);
    let delete = |keys, to| KeyBinding::new(keys, Delete { to }, c);
    cx.bind_keys([
        delete("backspace", Left),
        delete("delete", Right),
        go("left", Left),
        go("right", Right),
        go("up", Up),
        go("down", Down),
        go("home", LineStart),
        go("end", LineEnd),
        extend("shift-left", Left),
        extend("shift-right", Right),
        extend("shift-up", Up),
        extend("shift-down", Down),
        extend("shift-home", LineStart),
        extend("shift-end", LineEnd),
        KeyBinding::new("secondary-a", SelectAll, c),
        KeyBinding::new("enter", Newline, c),
        KeyBinding::new("shift-enter", Newline, c),
        KeyBinding::new("secondary-enter", Submit, c),
        KeyBinding::new("escape", Cancel, c),
        KeyBinding::new("secondary-v", Paste, c),
        KeyBinding::new("secondary-c", Copy, c),
        KeyBinding::new("secondary-x", Cut, c),
        KeyBinding::new("secondary-z", Undo, c),
        KeyBinding::new("secondary-shift-z", Redo, c),
        KeyBinding::new("secondary-s", Save, c),
    ]);
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        delete("alt-backspace", WordLeft),
        delete("alt-delete", WordRight),
        delete("cmd-backspace", LineStart),
        delete("cmd-delete", LineEnd),
        delete("ctrl-h", Left),
        delete("ctrl-d", Right),
        delete("ctrl-k", LineEnd),
        go("alt-left", WordLeft),
        go("alt-right", WordRight),
        go("cmd-left", LineStart),
        go("cmd-right", LineEnd),
        go("cmd-up", Start),
        go("cmd-down", End),
        go("ctrl-a", LineStart),
        go("ctrl-e", LineEnd),
        go("ctrl-b", Left),
        go("ctrl-f", Right),
        go("ctrl-p", Up),
        go("ctrl-n", Down),
        extend("alt-shift-left", WordLeft),
        extend("alt-shift-right", WordRight),
        extend("cmd-shift-left", LineStart),
        extend("cmd-shift-right", LineEnd),
        extend("cmd-shift-up", Start),
        extend("cmd-shift-down", End),
        extend("ctrl-shift-a", LineStart),
        extend("ctrl-shift-e", LineEnd),
    ]);
    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        delete("ctrl-backspace", WordLeft),
        delete("ctrl-delete", WordRight),
        go("ctrl-left", WordLeft),
        go("ctrl-right", WordRight),
        go("ctrl-home", Start),
        go("ctrl-end", End),
        extend("ctrl-shift-left", WordLeft),
        extend("ctrl-shift-right", WordRight),
        extend("ctrl-shift-home", Start),
        extend("ctrl-shift-end", End),
        KeyBinding::new("ctrl-y", Redo, c),
    ]);
}

#[derive(Clone, Copy)]
pub enum InputEvent {
    Changed,
    /// Enter in a single-line field, or cmd-enter in any field.
    Submit,
    Cancel,
}

/// What a [`TextInput::line_editor`] hands its owner: the keys that go past its one line.
#[derive(Clone)]
pub enum LineEvent {
    /// Enter, splitting the line at the caret, at this byte offset.
    Split(usize),
    /// Backspace at the start, joining the line to the one before.
    JoinBack,
    /// Delete at the end, joining the next line to this one.
    JoinNext,
    /// ↑ (-1) or ↓ (1), to the line before or after, from this byte offset.
    Step(isize, usize),
    /// ⌘Z and ⇧⌘Z, which go back and forth through the edits of every line.
    Undo,
    Redo,
    /// ⌘S.
    Save,
    /// Text pasted that runs over more than one line, as the clipboard has it.
    Paste(String),
    /// Tab (true) and ⇧Tab (false): the line indented a step, or outdented.
    Indent(bool),
    /// ⌥↑ (-1) and ⌥↓ (1): the line moved past the one before or after it.
    MoveLine(isize),
    /// ⇧⌘K: the line taken out.
    DeleteLine,
    /// ⌘↑ (true) and ⌘↓ (false): to the start of the first line, or the end of the last.
    Edge(bool),
    /// ⇧↑ (-1) and ⇧↓ (1): the selection carried on to the line before or after.
    Select(isize),
    /// ⇧⌘↑ (true) and ⇧⌘↓ (false): the selection carried on to the file's start or end.
    SelectEdge(bool),
    /// The selection carried past this line's start (-1) or end (1), as ⇧← at its start or ⇧→
    /// at its end: onto the line before or after, at its end when `true`, else its start.
    SelectAcross(isize, bool),
    /// The caret moved within the line, or a click put it there: a selection over more lines
    /// than this one goes.
    Moved,
    /// ⌘C, or with `cut` ⌘X, with what this line has selected: the owner copies that, or all
    /// it has selected over more lines.
    Copy {
        cut: bool,
        own: String,
    },
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: String,
    placeholder: SharedString,
    multiline: bool,
    /// Enter submits though the text wraps, for what's short but may not fit on a line.
    enter_submits: bool,
    /// Draw `*` instead of the text, for passwords.
    masked: bool,
    /// Type a dash for a space, as in a branch name.
    dashes: bool,
    /// One line of many, as a line of a diff being edited: what goes past its ends, Enter, and
    /// ↑ and ↓ go to the owner as events. It draws tabs as four spaces, as the diff does.
    line_editor: bool,
    /// A line editor's text drawn as the diff draws it, with tabs as four spaces, and the text
    /// it's for: once that's typed in, it's plain, as the diff shows a typed line.
    styles: Option<(String, Vec<SpanStyle>)>,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    /// Layout of the last paint, one entry per `\n`-separated line, with its byte offset.
    last_layout: Vec<(usize, WrappedLine)>,
    last_bounds: Option<Bounds<Pixels>>,
    last_line_height: Pixels,
    /// Height the text wanted on the last paint, used to size the next layout.
    wanted_height: Option<Pixels>,
    /// The box that scrolls a long text, and the selection it last scrolled to show.
    scroll: Option<(ScrollHandle, Range<usize>)>,
    /// How far a single line longer than the field is slid left, to keep the caret in it.
    scroll_x: Pixels,
    /// The text and selection before each change, and the ones undone, newest last.
    undo: Vec<(String, Range<usize>)>,
    redo: Vec<(String, Range<usize>)>,
    last_edit: Edit,
    is_selecting: bool,
    cursor_visible: bool,
    blink: Task<()>,
    /// `blink` runs, which it stops doing once the field loses the keys.
    blinking: bool,
}

impl EventEmitter<InputEvent> for TextInput {}
impl EventEmitter<LineEvent> for TextInput {}

impl TextInput {
    pub fn new(
        placeholder: impl Into<SharedString>,
        multiline: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            // Where Tab goes in a dialog: in the window, Tab moves between panes instead.
            focus_handle: cx.focus_handle().tab_stop(true),
            content: String::new(),
            placeholder: placeholder.into(),
            multiline,
            enter_submits: false,
            masked: false,
            dashes: false,
            line_editor: false,
            styles: None,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: Vec::new(),
            last_bounds: None,
            last_line_height: fixed(18.),
            scroll: None,
            scroll_x: Pixels::ZERO,
            undo: Vec::new(),
            redo: Vec::new(),
            // So filling the new field, as one opening on a message, is no change to undo.
            last_edit: Edit::Set,
            wanted_height: None,
            is_selecting: false,
            cursor_visible: true,
            blink: Task::ready(()),
            blinking: false,
        }
    }

    /// Make it one line of many, which hands what goes past its ends to its owner as
    /// [`LineEvent`]s.
    pub fn line_editor(mut self) -> Self {
        self.line_editor = true;
        self
    }

    /// Draw the text as it is now with `styles`, over it as shown, tabs as four spaces, as a
    /// diff's line colours its syntax.
    pub fn set_styles(&mut self, styles: Vec<SpanStyle>) {
        self.styles = Some((self.content.clone(), styles));
    }

    /// Where byte `raw` of the text is in it as drawn: a line editor's tabs are four spaces.
    fn shown_at(&self, raw: usize) -> usize {
        match self.line_editor {
            true => raw + TAB_EXTRA * tabs_before(&self.content, raw),
            false => raw,
        }
    }

    /// The byte of the text at `shown` in it as drawn: see [`raw_for`].
    fn raw_at(&self, shown: usize) -> usize {
        match self.line_editor {
            true => raw_for(&self.content, shown),
            false => shown,
        }
    }

    pub fn enter_submits(mut self) -> Self {
        self.enter_submits = true;
        self
    }

    pub fn masked(mut self) -> Self {
        self.masked = true;
        self
    }

    /// Type a dash for a space, as the TUI does in a branch name, which Git doesn't allow spaces
    /// in. A pasted name keeps its spaces, for the name's check to change.
    pub fn spaces_as_dashes(mut self) -> Self {
        self.dashes = true;
        self
    }

    /// Keep the caret in view of `scroll`, the box this input scrolls in, as it moves.
    pub fn scrolled_by(mut self, scroll: ScrollHandle) -> Self {
        self.scroll = Some((scroll, 0..0));
        self
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    /// The byte offset of the caret.
    pub fn caret(&self) -> usize {
        self.cursor_offset()
    }

    /// Where the selection began, which the caret's end of it moves away from.
    pub fn anchor(&self) -> usize {
        match self.selection_reversed {
            true => self.selected_range.end,
            false => self.selected_range.start,
        }
    }

    /// Select `range`, the caret at its start when `reversed`, else at its end.
    pub fn select_range(&mut self, range: Range<usize>, reversed: bool, cx: &mut Context<Self>) {
        let len = self.content.len();
        let at = |n: usize| {
            (0..=n.min(len))
                .rev()
                .find(|&n| self.content.is_char_boundary(n))
                .unwrap_or(0)
        };
        self.selected_range = at(range.start)..at(range.end);
        self.selection_reversed = reversed;
        cx.notify();
    }

    /// Take out what's selected, as typing over it would.
    pub fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            self.insert("", window, cx);
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.remember(Edit::Set);
        self.content = cleaned(&text.into(), self.multiline || self.line_editor);
        self.selected_range = self.content.len()..self.content.len();
        self.marked_range = None;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    /// Show `text` as a new field would: another text rather than an edit of this one, with
    /// nothing to undo back to.
    pub fn load(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.set_text(text, cx);
        (self.undo, self.redo) = (Vec::new(), Vec::new());
    }

    /// Show `text` as the field was left, which nothing needs to hear of: no one typed it.
    pub fn restore(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.content = cleaned(&text.into(), self.multiline || self.line_editor);
        self.selected_range = self.content.len()..self.content.len();
        self.marked_range = None;
        (self.undo, self.redo) = (Vec::new(), Vec::new());
        self.break_undo_run();
        cx.notify();
    }

    /// Put the caret at byte `offset`, or the end of the text before it, as where a field opens
    /// on lines it was asked for.
    pub fn place_caret(&mut self, offset: usize, cx: &mut Context<Self>) {
        let at = (0..=offset.min(self.content.len()))
            .rev()
            .find(|&at| self.content.is_char_boundary(at))
            .unwrap_or(0);
        self.selected_range = at..at;
        self.selection_reversed = false;
        cx.notify();
    }

    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        cx.notify();
    }

    /// Show the caret and blink it while the field has focus in the window in front, which
    /// redraws it twice a second.
    fn restart_blink(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.cursor_visible = true;
        self.blinking = true;
        self.blink = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(530))
                    .await;
                let blinking = this.update_in(cx, |this, window, cx| {
                    let blinking =
                        this.focus_handle.is_focused(window) && window.is_window_active();
                    // Leave the caret on for when focus comes back, or the next key or click.
                    this.cursor_visible = !blinking || !this.cursor_visible;
                    this.blinking = blinking;
                    cx.notify();
                    blinking
                });
                if !matches!(blinking, Ok(true)) {
                    break;
                }
            }
        });
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn move_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        // Typing somewhere else starts a new undo step.
        self.last_edit = Edit::Other;
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        window.notify_text_selection_changed(&self.focus_handle);
        self.restart_blink(window, cx);
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        self.last_edit = Edit::Other;
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        window.notify_text_selection_changed(&self.focus_handle);
        self.restart_blink(window, cx);
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    fn previous_word(&self, offset: usize) -> usize {
        self.content
            .unicode_word_indices()
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_word(&self, offset: usize) -> usize {
        self.content
            .unicode_word_indices()
            .find_map(|(idx, word)| (idx + word.len() > offset).then_some(idx + word.len()))
            .unwrap_or(self.content.len())
    }

    fn line_start(&self, offset: usize) -> usize {
        (self.content.get(..offset))
            .and_then(|before| before.rfind('\n'))
            .map_or(0, |ix| ix + 1)
    }

    fn line_end(&self, offset: usize) -> usize {
        (self.content.get(offset..))
            .and_then(|after| after.find('\n'))
            .map_or(self.content.len(), |ix| offset + ix)
    }

    /// The offset one visual line above or below `offset`, or `None` past the edges.
    fn vertical(&self, offset: usize, delta: f32) -> Option<usize> {
        let position = self.position_for_offset(offset)?;
        let target = point(position.x, position.y + self.last_line_height * delta);
        if target.y < Pixels::ZERO {
            return None;
        }
        let index = self.offset_for_position(target);
        (index != offset).then_some(index)
    }

    fn position_for_offset(&self, offset: usize) -> Option<Point<Pixels>> {
        position_in(
            &self.last_layout,
            self.last_line_height,
            self.shown_at(offset),
        )
    }

    /// `offset` clamped into the text and onto a character boundary.
    fn clamp(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.content.len());
        while !self.content.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    fn offset_for_position(&self, position: Point<Pixels>) -> usize {
        // The layout is the placeholder's when there's no text.
        if self.content.is_empty() {
            return 0;
        }
        let mut y = Pixels::ZERO;
        for (start, line) in &self.last_layout {
            let height = row_height(line, self.last_line_height);
            if position.y < y + height {
                let local = point(position.x.max(Pixels::ZERO), position.y - y);
                let index = match line.closest_index_for_position(local, self.last_line_height) {
                    Ok(index) | Err(index) => index,
                };
                return self.clamp(self.raw_at(start + index));
            }
            y += height;
        }
        self.content.len()
    }

    fn offset_for_mouse(&self, position: Point<Pixels>) -> usize {
        let Some(bounds) = self.last_bounds else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        self.offset_for_position(position - bounds.origin)
    }

    fn insert(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, text, window, cx);
    }

    fn target(&self, to: Target) -> usize {
        let at = self.cursor_offset();
        match to {
            Target::Left => self.previous_boundary(at),
            Target::Right => self.next_boundary(at),
            Target::Up => self.vertical(at, -1.).unwrap_or(0),
            Target::Down => self.vertical(at, 1.).unwrap_or(self.content.len()),
            Target::WordLeft => self.previous_word(at),
            Target::WordRight => self.next_word(at),
            Target::LineStart => self.line_start(at),
            Target::LineEnd => self.line_end(at),
            Target::Start => 0,
            Target::End => self.content.len(),
        }
    }

    fn move_cursor(&mut self, action: &Move, window: &mut Window, cx: &mut Context<Self>) {
        if self.line_editor && action.select {
            let at = self.cursor_offset();
            match action.to {
                // Past the line's ends, onto the next, as in an editor.
                Target::Left | Target::WordLeft | Target::LineStart if at == 0 => {
                    return cx.emit(LineEvent::SelectAcross(-1, true));
                }
                Target::Right | Target::WordRight | Target::LineEnd if at == self.content.len() => {
                    return cx.emit(LineEvent::SelectAcross(1, false));
                }
                _ => {}
            }
            match action.to {
                Target::Up => return cx.emit(LineEvent::Select(-1)),
                Target::Down => return cx.emit(LineEvent::Select(1)),
                Target::Start => return cx.emit(LineEvent::SelectEdge(true)),
                Target::End => return cx.emit(LineEvent::SelectEdge(false)),
                _ => {}
            }
        }
        if self.line_editor && !action.select {
            let step = match action.to {
                Target::Up => Some(-1),
                Target::Down => Some(1),
                _ => None,
            };
            if let Some(step) = step {
                return cx.emit(LineEvent::Step(step, self.cursor_offset()));
            }
            match action.to {
                Target::Start => return cx.emit(LineEvent::Edge(true)),
                Target::End => return cx.emit(LineEvent::Edge(false)),
                _ => {}
            }
        }
        if action.select {
            return self.select_to(self.target(action.to), window, cx);
        }
        // Left or right from a selection lands on its edge, as in native fields.
        let target = match action.to {
            Target::Left if !self.selected_range.is_empty() => self.selected_range.start,
            Target::Right if !self.selected_range.is_empty() => self.selected_range.end,
            to => self.target(to),
        };
        self.move_to(target, window, cx);
        if self.line_editor {
            cx.emit(LineEvent::Moved);
        }
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
        self.select_to(self.content.len(), window, cx)
    }

    /// Keep the text as it is before a change of kind `edit`, unless it carries on a run of
    /// the same kind, which undoes as one step.
    fn remember(&mut self, edit: Edit) {
        // A password or key isn't kept around to bring back.
        if self.masked {
            return;
        }
        if edit == Edit::Other || edit != self.last_edit {
            if self.undo.len() == 200 {
                self.undo.remove(0);
            }
            (self.undo).push((self.content.clone(), self.selected_range.clone()));
            self.redo.clear();
        }
        self.last_edit = edit;
    }

    /// Start a new undo step with the next change, as before AI writes into the field, so
    /// undoing what it wrote brings back what was there.
    pub fn break_undo_run(&mut self) {
        self.last_edit = Edit::Other;
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        match self.line_editor {
            true => cx.emit(LineEvent::Undo),
            false => self.step(true, cx),
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        match self.line_editor {
            true => cx.emit(LineEvent::Redo),
            false => self.step(false, cx),
        }
    }

    /// ⇧⌥↑ and ⇧⌥↓ in a line editor, as macOS selects by paragraph: to this line's start, or
    /// `forward` its end, and from there on to the line before's start or the next's end.
    fn select_paragraph(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.line_editor {
            return cx.propagate();
        }
        let (at, end) = (self.cursor_offset(), self.content.len());
        match (forward, at) {
            (false, 0) => cx.emit(LineEvent::SelectAcross(-1, false)),
            (true, at) if at == end => cx.emit(LineEvent::SelectAcross(1, true)),
            (false, _) => self.select_to(0, window, cx),
            (true, _) => self.select_to(end, window, cx),
        }
    }

    /// A line editor's line keys, which its key context alone binds: see [`LineEvent`].
    fn line_action(&mut self, event: LineEvent, cx: &mut Context<Self>) {
        match self.line_editor {
            true => cx.emit(event),
            false => cx.propagate(),
        }
    }

    /// Only a line editor's owner saves: in any other field ⌘S goes on to what's around it.
    fn save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        match self.line_editor {
            true => cx.emit(LineEvent::Save),
            false => cx.propagate(),
        }
    }

    /// Go back to the text before the last change, or with `back` false forward again.
    fn step(&mut self, back: bool, cx: &mut Context<Self>) {
        let (from, to) = match back {
            true => (&mut self.undo, &mut self.redo),
            false => (&mut self.redo, &mut self.undo),
        };
        let Some((content, selected)) = from.pop() else {
            return;
        };
        to.push((
            std::mem::replace(&mut self.content, content),
            self.selected_range.clone(),
        ));
        self.selected_range = selected;
        self.marked_range = None;
        self.last_edit = Edit::Other;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn delete(&mut self, action: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.line_editor && self.selected_range.is_empty() {
            let at = self.cursor_offset();
            match action.to {
                Target::Left | Target::WordLeft | Target::LineStart if at == 0 => {
                    return cx.emit(LineEvent::JoinBack);
                }
                Target::Right | Target::WordRight | Target::LineEnd if at == self.content.len() => {
                    return cx.emit(LineEvent::JoinNext);
                }
                _ => {}
            }
        }
        // Set directly rather than with `select_to`, which would end the run of deletes that
        // undo takes back in one step.
        if self.selected_range.is_empty() {
            let (at, to) = (self.cursor_offset(), self.target(action.to));
            self.selected_range = at.min(to)..at.max(to);
        }
        // Nothing to delete, as at the start, changes nothing.
        if !self.selected_range.is_empty() {
            self.insert("", window, cx)
        }
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.line_editor {
            return cx.emit(LineEvent::Split(self.cursor_offset()));
        }
        if self.multiline && !self.enter_submits {
            self.insert("\n", window, cx);
        } else {
            cx.emit(InputEvent::Submit);
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Submit);
    }

    fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        // A drag started where pressing doesn't take focus, as a diff line's grip, lets go
        // here, as it does wherever else Escape goes.
        if cx.stop_active_drag(window) {
            return;
        }
        cx.emit(InputEvent::Cancel);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = clipboard::text(cx) {
            if self.line_editor && text.contains(['\n', '\r']) {
                return cx.emit(LineEvent::Paste(text));
            }
            self.insert(&text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.line_editor {
            let own = (self.content.get(self.selected_range.clone())).unwrap_or_default();
            return cx.emit(LineEvent::Copy {
                cut: false,
                own: own.to_owned(),
            });
        }
        if !self.selected_range.is_empty()
            && !self.masked
            && let Some(text) = self.content.get(self.selected_range.clone())
        {
            clipboard::copy_silently(text.to_string(), cx);
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.line_editor {
            let own = (self.content.get(self.selected_range.clone())).unwrap_or_default();
            return cx.emit(LineEvent::Copy {
                cut: true,
                own: own.to_owned(),
            });
        }
        if !self.selected_range.is_empty() && !self.masked {
            self.copy(&Copy, window, cx);
            self.insert("", window, cx)
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A line editor's owner places the caret and selects, as its lines are one text.
        if self.line_editor {
            return;
        }
        self.is_selecting = true;
        let offset = self.offset_for_mouse(event.position);
        // Three clicks take the paragraph, as in the platform's fields.
        if event.click_count >= 3 {
            self.move_to(self.line_start(offset), window, cx);
            self.select_to(self.line_end(offset), window, cx);
        } else if event.click_count == 2 {
            // From past the click, so one at a word's very start selects that word.
            let start = self.previous_word(self.next_boundary(offset));
            let end = self.next_word(start);
            self.move_to(start, window, cx);
            self.select_to(end, window, cx);
        } else if event.modifiers.shift {
            self.select_to(offset, window, cx);
        } else {
            self.move_to(offset, window, cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_selecting {
            self.select_to(self.offset_for_mouse(event.position), window, cx);
        }
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    /// Put `new_text` over the UTF-16 range the platform gave, or else the marked text or
    /// the selection, and return the byte range it now takes.
    fn splice(&mut self, range_utf16: Option<Range<usize>>, new_text: &str) -> Range<usize> {
        // Tab isn't typing in a one-line field, as a branch name, as in the platform's own.
        if !self.multiline && !self.line_editor && new_text == "\t" {
            let at = self.cursor_offset();
            return at..at;
        }
        let new_text = match new_text {
            " " if self.dashes => "-",
            text => text,
        };
        let new_text = &cleaned(new_text, self.multiline || self.line_editor);
        let range = range_utf16
            .map(|range_utf16| self.range_from_utf16(&range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let typed = self.marked_range.is_some()
            || (range.is_empty() && new_text.chars().count() == 1) && new_text != "\n";
        // A range that doesn't fit the text, as one the platform gave backwards, changes nothing.
        let Some(replaced) = self.content.get(range.clone()) else {
            let at = self.cursor_offset();
            return at..at;
        };
        let deleted = new_text.is_empty() && replaced.chars().count() == 1;
        // Deleting nothing, as backspace at the start, changes nothing to undo.
        if !range.is_empty() || !new_text.is_empty() {
            self.remember(match (typed, deleted) {
                (true, _) => Edit::Typed,
                (_, true) => Edit::Deleted,
                _ => Edit::Other,
            });
        }
        self.content.replace_range(range.clone(), new_text);
        range.start..range.start + new_text.len()
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        // Input methods, dictation and accessibility read text through here; a password
        // isn't theirs to read.
        if self.masked {
            return None;
        }
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        self.content.get(range).map(str::to_string)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = self.range_from_utf16(&range_utf16);
        self.selection_reversed = false;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.content.encode_utf16().count())
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked_range = None;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let end = self.splice(range_utf16, new_text).end;
        self.selected_range = end..end;
        self.selection_reversed = false;
        self.marked_range.take();
        window.notify_text_input_changed(&self.focus_handle);
        self.restart_blink(window, cx);
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let inserted = self.splice(range_utf16, new_text);
        self.marked_range = (!inserted.is_empty()).then(|| inserted.clone());
        let insertion_start_utf16 = self.offset_to_utf16(inserted.start);
        let inserted_length_utf16 = new_text.encode_utf16().count();
        self.selected_range = new_selected_range_utf16
            .map(|selected| {
                self.range_from_utf16(
                    &(insertion_start_utf16 + selected.start.min(inserted_length_utf16)
                        ..insertion_start_utf16 + selected.end.min(inserted_length_utf16)),
                )
            })
            .unwrap_or(inserted.end..inserted.end);
        self.selection_reversed = false;
        window.notify_text_input_changed(&self.focus_handle);
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let start = self.position_for_offset(range.start)?;
        let end = self.position_for_offset(range.end)?;
        Some(Bounds::from_corners(
            bounds.origin + start,
            bounds.origin + point(end.x, end.y + self.last_line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let local = self.last_bounds?.localize(&point)?;
        Some(self.offset_to_utf16(self.offset_for_position(local)))
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Back with the keys, as after a press the list around it took them a moment, or in a
        // field made with them: the caret blinks again. Focus moving draws the window again.
        if !self.blinking && self.focus_handle.is_focused(window) && window.is_window_active() {
            self.restart_blink(window, cx);
        }
        super::shown_when_focused(div().id("text-input"))
            .role(if self.multiline {
                Role::MultilineTextInput
            } else {
                Role::TextInput
            })
            .aria_label(self.placeholder.clone())
            // A line editor's own keys, as Tab indenting, bind to it alone.
            .key_context(match self.line_editor {
                true => "TextInput LineEditor",
                false => CONTEXT,
            })
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .w_full()
            .on_action(cx.listener(Self::move_cursor))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::save))
            .on_action(
                cx.listener(|this, _: &SelectParagraphBackward, window, cx| {
                    this.select_paragraph(false, window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &SelectParagraphForward, window, cx| {
                this.select_paragraph(true, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &Indent, _, cx| {
                    this.line_action(LineEvent::Indent(true), cx)
                }),
            )
            .on_action(cx.listener(|this, _: &Outdent, _, cx| {
                this.line_action(LineEvent::Indent(false), cx)
            }))
            .on_action(cx.listener(|this, _: &MoveLineUp, _, cx| {
                this.line_action(LineEvent::MoveLine(-1), cx)
            }))
            .on_action(cx.listener(|this, _: &MoveLineDown, _, cx| {
                this.line_action(LineEvent::MoveLine(1), cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteLine, _, cx| {
                this.line_action(LineEvent::DeleteLine, cx)
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(TextElement { input: cx.entity() })
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct Prepaint {
    lines: Vec<(usize, WrappedLine)>,
    line_height: Pixels,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
    scroll_x: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let input = self.input.read(cx);
        let line_height = window.line_height();
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = input
            .wanted_height
            .unwrap_or(line_height)
            .max(line_height)
            .into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let theme = Theme::get(cx);
        let style = window.text_style();
        let line_height = window.line_height();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let wrap_width = input.multiline.then_some(bounds.size.width);
        let is_empty = input.content.is_empty();
        let text: SharedString = if is_empty {
            input.placeholder.clone()
        } else if input.masked {
            // One `*` per byte keeps every offset into the text valid for the layout.
            "*".repeat(input.content.len()).into()
        } else if input.line_editor {
            input.content.replace('\t', TAB).into()
        } else {
            input.content.clone().into()
        };
        // Its syntax, while it's as it was when given them.
        let styles = (input.styles.as_ref())
            .filter(|(text, _)| !is_empty && *text == input.content)
            .map(|(_, styles)| styles.as_slice());
        let color = if is_empty { theme.text_3 } else { style.color };

        let mut lines = Vec::new();
        let mut offset = 0;
        for raw in text.split('\n') {
            let base = TextRun {
                len: raw.len(),
                font: style.font(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let mut runs = match styles {
                Some(styles) => styled_runs(raw.len(), &base, styles),
                None => vec![base],
            };
            if let Some(marked) = input.marked_range.as_ref().filter(|_| !is_empty) {
                let marked = input.shown_at(marked.start)..input.shown_at(marked.end);
                let line_range = offset..offset + raw.len();
                let start = marked.start.max(line_range.start);
                let end = marked.end.min(line_range.end);
                if start < end
                    && let Some(base) = runs.first().cloned()
                {
                    runs = [
                        (start - offset, None),
                        (
                            end - start,
                            Some(UnderlineStyle {
                                color: Some(color),
                                thickness: fixed(1.),
                                wavy: false,
                            }),
                        ),
                        (line_range.end - end, None),
                    ]
                    .into_iter()
                    .filter(|(len, _)| *len > 0)
                    .map(|(len, underline)| TextRun {
                        len,
                        underline,
                        ..base.clone()
                    })
                    .collect();
                }
            }
            // Without a newline the line is the whole text, already shared.
            let line = if raw.len() == text.len() {
                text.clone()
            } else {
                raw.to_owned().into()
            };
            let shaped = window
                .text_system()
                .shape_text(line, font_size, &runs, wrap_width, None)
                .ok()
                .and_then(|mut lines| lines.pop())
                .unwrap_or_default();
            lines.push((offset, shaped));
            offset += raw.len() + 1;
        }

        let height: Pixels = lines
            .iter()
            .map(|(_, line)| row_height(line, line_height))
            .sum();
        let focused = input.focus_handle.is_focused(window);
        let caret_visible = focused && input.cursor_visible;
        let selected = input.selected_range.clone();
        let cursor_offset = input.cursor_offset();

        // Positions are computed against the layout just shaped, not the stored one.
        let position = |target: usize| {
            if is_empty {
                Some(point(Pixels::ZERO, Pixels::ZERO))
            } else {
                position_in(&lines, line_height, input.shown_at(target))
            }
        };

        // A single line longer than the field slides sideways, as little as keeps the caret
        // in it. A line editor's doesn't: it scrolls with the lines around it, in the box it's
        // `scrolled_by`, so it stays lined up with them.
        let scroll_x = match (input.multiline, lines.first()) {
            (false, Some((_, line))) if !input.line_editor => {
                let room = bounds.size.width - theme.scaled(2.);
                let caret = position(cursor_offset).map_or(Pixels::ZERO, |at| at.x);
                let slack = (line.size(line_height).width - room).max(Pixels::ZERO);
                (input.scroll_x.max(caret - room).min(caret)).clamp(Pixels::ZERO, slack)
            }
            _ => Pixels::ZERO,
        };
        let origin = bounds.origin - point(scroll_x, Pixels::ZERO);

        let mut selections = Vec::new();
        let mut cursor = None;
        if selected.is_empty() {
            if caret_visible && let Some(at) = position(cursor_offset) {
                cursor = Some(fill(
                    Bounds::new(origin + at, size(theme.scaled(1.5), line_height)),
                    theme.pop,
                ));
            }
        } else if let (Some(start), Some(end)) = (position(selected.start), position(selected.end))
        {
            // A diff line being edited selects as the diff's lines do, so a selection over
            // more lines is one color; on a green line pop's would read as green.
            let selection_color = match input.line_editor {
                true => theme.highlight(Highlight::Words),
                false => theme.highlight(match focused {
                    true => Highlight::Selected,
                    false => Highlight::SelectedUnfocused,
                }),
            };
            let mut row_top = start.y;
            while row_top <= end.y {
                let left = if row_top == start.y {
                    start.x
                } else {
                    Pixels::ZERO
                };
                let right = if row_top == end.y {
                    end.x
                } else {
                    bounds.size.width
                };
                selections.push(fill(
                    Bounds::from_corners(
                        origin + point(left, row_top),
                        origin + point(right.max(left + theme.scaled(4.)), row_top + line_height),
                    ),
                    selection_color,
                ));
                row_top += line_height;
            }
        }

        if input.wanted_height != Some(height) {
            let input = self.input.clone();
            window.on_next_frame(move |_, cx| {
                input.update(cx, |input, cx| {
                    input.wanted_height = Some(height);
                    cx.notify();
                })
            });
        }

        Prepaint {
            lines,
            line_height,
            cursor,
            selections,
            scroll_x,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // The text as slid sideways, which mouse positions and the IME go by too.
        let scroll_x = prepaint.scroll_x;
        let text_bounds = Bounds::new(
            bounds.origin - point(scroll_x, Pixels::ZERO),
            size(bounds.size.width + scroll_x, bounds.size.height),
        );
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(text_bounds, self.input.clone()),
            cx,
        );
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for selection in prepaint.selections.drain(..) {
                window.paint_quad(selection);
            }
            let mut y = bounds.top();
            for (_, line) in &prepaint.lines {
                line.paint(
                    point(text_bounds.left(), y),
                    prepaint.line_height,
                    gpui::TextAlign::Left,
                    Some(text_bounds),
                    window,
                    cx,
                )
                .ok();
                y += row_height(line, prepaint.line_height);
            }
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });
        let line_height = prepaint.line_height;
        let lines = std::mem::take(&mut prepaint.lines);
        // How far past the caret a line editor's box keeps in view.
        let room = Theme::get(cx).scaled(24.);
        self.input.update(cx, |input, _cx| {
            // Once the caret moved, scroll the box just enough to show it.
            let caret = position_in(&lines, line_height, input.shown_at(input.cursor_offset()));
            let focused = input.focus_handle.is_focused(window);
            if let Some((scroll, shown)) = &mut input.scroll
                && focused
                && *shown != input.selected_range
                && let Some(caret) = caret
            {
                *shown = input.selected_range.clone();
                let (view, mut offset) = (scroll.bounds(), scroll.offset());
                // Inside the box's padding, as far as the text sits from its top unscrolled.
                let inset = bounds.top() - offset.y - view.top();
                let top = bounds.top() + caret.y;
                let bottom = top + line_height;
                if bottom > view.bottom() - inset {
                    offset.y -= bottom - (view.bottom() - inset);
                } else if top < view.top() + inset {
                    offset.y += view.top() + inset - top;
                }
                // A line editor's box scrolls sideways, with the lines around it: as little as
                // shows the caret, a few characters' room past it.
                if input.line_editor {
                    let x = bounds.left() + caret.x;
                    if x + room > view.right() {
                        offset.x -= x + room - view.right();
                    } else if x < view.left() + room {
                        offset.x = (offset.x + view.left() + room - x).min(Pixels::ZERO);
                    }
                }
                if offset != scroll.offset() {
                    scroll.set_offset(offset);
                    // Painting can't ask for another frame; the next one can.
                    window.on_next_frame(|window, _| window.refresh());
                }
            }
            input.last_layout = lines;
            input.last_bounds = Some(text_bounds);
            input.scroll_x = scroll_x;
            input.last_line_height = line_height;
        });
    }
}

/// The height a laid-out line takes; an empty one still takes a row.
fn row_height(line: &WrappedLine, line_height: Pixels) -> Pixels {
    line.size(line_height).height.max(line_height)
}

/// Bytes of a line editor's text as drawn, and how they look.
pub type SpanStyle = (Range<usize>, gpui::HighlightStyle);

/// How a line editor draws a tab, as the diff's lines do, and the bytes that adds.
const TAB: &str = "    ";
const TAB_EXTRA: usize = TAB.len() - 1;

/// The byte of `text` at byte `shown` of it drawn with tabs as [`TAB`], the nearer end of a
/// tab drawn there.
fn raw_for(text: &str, shown: usize) -> usize {
    let mut at = 0;
    for (ix, c) in text.char_indices() {
        let width = match c {
            '\t' => TAB.len(),
            c => c.len_utf8(),
        };
        if shown < at + width {
            return match c == '\t' && shown - at > width / 2 {
                true => ix + 1,
                false => ix,
            };
        }
        at += width;
    }
    text.len()
}

fn tabs_before(text: &str, at: usize) -> usize {
    (text.char_indices())
        .take_while(|(ix, _)| *ix < at)
        .filter(|(_, c)| *c == '\t')
        .count()
}

/// Runs over `len` bytes like `base`, with `styles` over theirs, as a diff line's syntax: the
/// first style over a byte wins, and what none covers stays as `base`.
fn styled_runs(len: usize, base: &TextRun, styles: &[SpanStyle]) -> Vec<TextRun> {
    let mut runs = Vec::new();
    let mut at = 0;
    let push = |runs: &mut Vec<TextRun>, len: usize, style: Option<&gpui::HighlightStyle>| {
        if len == 0 {
            return;
        }
        let mut run = TextRun {
            len,
            ..base.clone()
        };
        if let Some(style) = style {
            run.color = style.color.unwrap_or(run.color);
            run.background_color = style.background_color.or(run.background_color);
            run.font.weight = style.font_weight.unwrap_or(run.font.weight);
            run.font.style = style.font_style.unwrap_or(run.font.style);
        }
        runs.push(run);
    };
    let mut sorted: Vec<&SpanStyle> = styles.iter().collect();
    sorted.sort_by_key(|(range, _)| range.start);
    for (range, style) in sorted {
        let (start, end) = (range.start.max(at), range.end.min(len));
        if start >= end {
            continue;
        }
        push(&mut runs, start - at, None);
        push(&mut runs, end - start, Some(style));
        at = end;
    }
    push(&mut runs, len.saturating_sub(at), None);
    runs
}

/// Where byte `offset` falls in `lines`, from their top left.
fn position_in(
    lines: &[(usize, WrappedLine)],
    line_height: Pixels,
    offset: usize,
) -> Option<Point<Pixels>> {
    let mut y = Pixels::ZERO;
    for (start, line) in lines {
        if offset <= start + line.len() {
            let local = line.position_for_index(offset - start, line_height)?;
            return Some(point(local.x, local.y + y));
        }
        y += row_height(line, line_height);
    }
    None
}

/// `text` as a field holds it: a single line without the line breaks it ends with, as a pasted
/// token has, and spaces for the rest; several lines with `\n` for `\r\n`.
fn cleaned(text: &str, multiline: bool) -> String {
    match multiline {
        // An old Mac's line ending alone too, which would be kept unseen.
        true => text.replace("\r\n", "\n").replace('\r', "\n"),
        false => text
            .trim_end_matches(['\r', '\n'])
            .replace(['\r', '\n', '\t'], " "),
    }
}

#[cfg(test)]
mod tests {
    use super::{TAB, cleaned, raw_for, styled_runs, tabs_before};

    #[test]
    fn a_tab_drawn_as_spaces_maps_back_to_its_byte() {
        let text = "\t\tx = 1";
        assert_eq!(tabs_before(text, 2), 2, "both tabs are before the x");
        let x = 2 * TAB.len();
        assert_eq!(raw_for(text, x), 2, "the x is drawn after two tabs' spaces");
        assert_eq!(raw_for(text, 1), 0, "near a tab's start is before it");
        assert_eq!(raw_for(text, 3), 1, "near its end is after it");
        assert_eq!(raw_for(text, 99), text.len(), "past the end is the end");
    }

    #[test]
    fn styles_split_the_line_into_runs() {
        let base = gpui::TextRun {
            len: 10,
            font: gpui::font("Zed Mono"),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let red = gpui::HighlightStyle {
            color: Some(gpui::red()),
            ..Default::default()
        };
        let runs = styled_runs(10, &base, &[(2..5, red)]);
        let lens: Vec<usize> = runs.iter().map(|run| run.len).collect();
        assert_eq!(lens, [2, 3, 5], "plain, styled, plain");
        assert_eq!(
            runs[1].color,
            gpui::red(),
            "the styled run takes its colour"
        );
        assert_eq!(runs[2].color, gpui::black(), "the rest keeps the base");
    }

    #[test]
    fn pasted_line_breaks_fit_the_field() {
        assert_eq!(
            cleaned("ghp_token\n", false),
            "ghp_token",
            "a token copied from a terminal is sent as it is, not with a space"
        );
        assert_eq!(
            cleaned("two\nlines\tand a tab", false),
            "two lines and a tab",
            "one line, still readable"
        );
        assert_eq!(
            cleaned("title\r\n\r\nbody\r\nold\rmac", true),
            "title\n\nbody\nold\nmac",
            "a message pasted from Windows keeps no carriage returns"
        );
    }
}
