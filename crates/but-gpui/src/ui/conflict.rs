//! A conflict inside its file's card, as Lite and but.dev draw one: what to keep above it, the
//! new base's lines and the commit's below that, and once something is picked, what's kept.

use std::rc::Rc;

use gpui::{AnyElement, App, ElementId, Entity, SharedString, Window, div, prelude::*};

use super::diff::{
    DiffRow, Layout, Look, OnToggle, card, digit_width, line_card, line_text, look, number_cell,
    text_cell,
};
use super::text_input::TextInput;
use super::{
    ButtonStyle, Icon, Tooltip, button, dp, field, h_flex, icon_button, segmented, small_icon,
    v_flex,
};
use crate::backend::{Conflict, DiffLine, LineKind};
use crate::theme::Theme;

/// One of a conflict's rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Part {
    /// What to keep, above the conflict.
    Bar,
    /// An unchanged line before the conflict, or after it.
    Before(usize),
    After(usize),
    /// A side's name, above its lines.
    Label(Side),
    Line(Side, usize),
    /// Shows or hides the lines before either side changed them.
    AncestorToggle,
    /// What's kept, or the editor writing it.
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Side {
    NewBase,
    Commit,
    Ancestor,
}

/// How a conflict lays out: its sides, or what's kept in their place, and whether the lines
/// before either side changed them show.
#[derive(Debug, Clone, Copy, Default)]
pub struct View {
    pub settled: bool,
    pub ancestor: bool,
}

/// A side to keep, or both, the new base's first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    NewBase,
    Commit,
    Both,
}

/// What's kept of a conflict, as its rows draw it.
pub enum Kept<'a> {
    Pick(Pick),
    /// Written by hand, or suggested; `writing` while the suggestion still streams in.
    Text {
        text: &'a str,
        suggested: bool,
        writing: bool,
    },
    Editing(Entity<TextInput>),
}

pub type OnPick = Rc<dyn Fn(Pick, &mut Window, &mut App)>;

/// What the rows of a conflict act with.
pub struct Actions {
    pub pick: OnPick,
    pub edit: OnToggle,
    /// Asks the AI set up in Settings for a merge.
    pub suggest: OnToggle,
    /// Back to the two sides.
    pub change: OnToggle,
    pub ancestor: OnToggle,
    pub done: OnToggle,
    pub cancel: OnToggle,
}

/// A side's name, as a button says it and as a sentence does, like "This commit" and "this
/// commit".
#[derive(Clone, Copy)]
pub struct Name<'a> {
    pub label: &'a str,
    pub phrase: &'a str,
}

/// What a conflict's rows draw from.
pub struct Row<'a> {
    pub conflict: &'a Conflict,
    /// Its place among the conflicts shown, from 1, and how many there are.
    pub number: usize,
    pub total: usize,
    /// What the two sides are called, alone and in a sentence: what the commit or the
    /// changes now sit on, like the target, and the commit or the changes themselves.
    pub sides: [Name<'a>; 2],
    /// What marks each side's name.
    pub icons: [Icon; 2],
    pub kept: Option<Kept<'a>>,
    /// The keyboard's picks go to it.
    pub current: bool,
    /// Its editor holds the focus.
    pub focused: bool,
    /// The lines before either side changed them show.
    pub ancestor: bool,
    pub last: bool,
    pub layout: &'a Layout,
    pub actions: Actions,
}

/// Add the rows of `conflict`, file `file`'s `conflict.index`th, laid out as `view` says;
/// with `context`, the unchanged lines around it too.
pub fn rows(rows: &mut Vec<DiffRow>, file: usize, conflict: &Conflict, view: View, context: bool) {
    let mut part = |part| {
        rows.push(DiffRow::Conflict {
            file,
            conflict: conflict.index,
            part,
        })
    };
    part(Part::Bar);
    if context {
        (0..conflict.before.len()).for_each(|ix| part(Part::Before(ix)));
    }
    if view.settled {
        part(Part::Result);
    } else {
        for (side, lines) in [
            (Side::NewBase, &conflict.new_base),
            (Side::Commit, &conflict.commit),
        ] {
            part(Part::Label(side));
            (0..lines.len()).for_each(|ix| part(Part::Line(side, ix)));
        }
        if let Some(ancestor) = &conflict.ancestor {
            part(Part::AncestorToggle);
            if view.ancestor {
                (0..ancestor.len()).for_each(|ix| part(Part::Line(Side::Ancestor, ix)));
            }
        }
    }
    if context {
        (0..conflict.after.len()).for_each(|ix| part(Part::After(ix)));
    }
}

/// The row `part` of a conflict.
pub fn row(part: Part, row: Row, theme: &Theme) -> AnyElement {
    let id = |name: &'static str, ix: usize| -> ElementId {
        (name, row.conflict.at as usize * 4096 + ix).into()
    };
    match part {
        Part::Bar => bar(&row, theme),
        Part::Before(ix) => {
            let Some(line) = row.conflict.before.get(ix) else {
                return div().into_any_element();
            };
            plain_line(id("conflict-before", ix), line, &row, theme)
        }
        Part::After(ix) => {
            // After what's kept, which can be more or fewer lines than the new base's.
            let Some(line) = row.conflict.after.get(ix) else {
                return div().into_any_element();
            };
            let renumbered = kept(&row).map(|(_, kept)| DiffLine {
                new: Some(row.conflict.at + (kept.len() + ix) as u32),
                ..line.clone()
            });
            plain_line(
                id("conflict-after", ix),
                renumbered.as_ref().unwrap_or(line),
                &row,
                theme,
            )
        }
        Part::Label(side) => label(side, &row, theme),
        Part::Line(side, ix) => {
            let Some(line) = side_lines(row.conflict, side).get(ix) else {
                return div().into_any_element();
            };
            side_line(
                id(side_id(side), ix),
                line,
                side_look(side, row.layout.plain, theme),
                row.layout,
                row.last,
                theme,
            )
        }
        Part::AncestorToggle => ancestor_toggle(&row, theme),
        Part::Result => result(&row, theme),
    }
}

/// The lines kept in the conflict's place, as they'd stand in the file, and the line saying
/// which, once something is.
fn kept(row: &Row) -> Option<(String, Vec<DiffLine>)> {
    let (c, sides) = (row.conflict, &row.sides);
    Some(match row.kept.as_ref()? {
        Kept::Pick(Pick::NewBase) => (
            format!("Keeping the lines from {}", sides[0].phrase),
            c.new_base.clone(),
        ),
        Kept::Pick(Pick::Commit) => (
            format!("Keeping the lines from {}", sides[1].phrase),
            c.commit.clone(),
        ),
        Kept::Pick(Pick::Both) => (
            format!("Keeping both, {} first", sides[0].phrase),
            renumbered(c.new_base.iter().chain(&c.commit).cloned(), c.at),
        ),
        Kept::Text {
            text,
            suggested,
            writing,
        } => (
            match (suggested, writing) {
                (true, true) => "Suggesting…".into(),
                (true, false) => "Keeping the suggested merge".into(),
                (false, _) => "Keeping your edit".into(),
            },
            renumbered(text.lines().map(plain), c.at),
        ),
        Kept::Editing(_) => return None,
    })
}

fn side_lines(conflict: &Conflict, side: Side) -> &[DiffLine] {
    match side {
        Side::NewBase => &conflict.new_base,
        Side::Commit => &conflict.commit,
        Side::Ancestor => conflict.ancestor.as_deref().unwrap_or_default(),
    }
}

fn side_id(side: Side) -> &'static str {
    match side {
        Side::NewBase => "conflict-new-base",
        Side::Commit => "conflict-commit",
        Side::Ancestor => "conflict-ancestor",
    }
}

/// The new base as the diff library tints the current side, the commit as it tints the
/// incoming one, and the lines before both as unchanged ones.
fn side_look(side: Side, plain: bool, theme: &Theme) -> Look {
    match side {
        Side::NewBase => look(LineKind::Added, plain, theme),
        Side::Commit => Look {
            bg: (!plain).then_some(theme.blue.opacity(0.1)),
            number: theme.blue,
            emph: theme.blue.opacity(0.2),
            bar: Some(theme.blue.into()),
        },
        Side::Ancestor => look(LineKind::Context, plain, theme),
    }
}

/// A slice of the card, its content lined up with the code, past the line numbers.
fn strip(layout: &Layout, last: bool, theme: &Theme) -> gpui::Div {
    let numbers = digit_width(theme) * (layout.digits.max(3) + 3) as f32;
    card(h_flex(), theme)
        .when(last, |el| el.border_b_1().rounded_b(dp(6.)))
        .pl(dp(numbers.round() + 8.))
        .pr(dp(8.))
        .gap(dp(6.))
        .text_size(dp(12.))
}

/// The conflict's place, and what to keep: a side, both, an edit or a suggestion.
fn bar(row: &Row, theme: &Theme) -> AnyElement {
    let picked = match &row.kept {
        Some(Kept::Pick(pick)) => Some(*pick),
        _ => None,
    };
    let actions = &row.actions;
    let choice = |id: &'static str, label: SharedString, tip: String, pick: Pick| {
        let act = actions.pick.clone();
        let segment = (div().id(id).child(label))
            .tooltip(Tooltip::keyed(tip, Some(pick_key(pick).into())))
            .on_click(move |_, window, cx| act(pick, window, cx));
        (segment, picked == Some(pick))
    };
    let edit = actions.edit.clone();
    let suggest = actions.suggest.clone();
    // Once a pick or a finished text is kept, the conflict only waits to be applied.
    let handled = matches!(
        row.kept,
        Some(Kept::Pick(_) | Kept::Text { writing: false, .. })
    );
    let (mark, color, ground) = match handled {
        true => (Icon::Tick, theme.safe, theme.text_1.opacity(0.04)),
        false => (Icon::Warning, theme.warn, theme.warn.opacity(0.06)),
    };
    card(h_flex(), theme)
        .h(dp(36.))
        .px(dp(8.))
        .gap(dp(6.))
        .bg(ground)
        .when(row.current, |el| el.bg(ground.opacity(ground.a * 2.)))
        .text_size(dp(12.))
        .child(small_icon(mark).text_color(color))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(color)
                .child(format!("Conflict {} of {}", row.number, row.total)),
        )
        .child(segmented(
            true,
            theme,
            [
                choice(
                    "keep-new-base",
                    row.sides[0].label.to_owned().into(),
                    format!("Keep the lines from {}", row.sides[0].phrase),
                    Pick::NewBase,
                ),
                choice(
                    "keep-commit",
                    row.sides[1].label.to_owned().into(),
                    format!("Keep the lines from {}", row.sides[1].phrase),
                    Pick::Commit,
                ),
                choice(
                    "keep-both",
                    "Both".into(),
                    format!("Keep both, {} first", row.sides[0].phrase),
                    Pick::Both,
                ),
            ],
        ))
        .child(
            button("conflict-edit", "Edit")
                .small()
                .style(ButtonStyle::Ghost)
                .tooltip_with_key("Write what to keep", Some("↩".into()))
                .on_click(move |_, window, cx| edit(window, cx)),
        )
        .child(
            icon_button("conflict-suggest", Icon::Ai)
                .small()
                .tooltip_with_key("Suggest a merge", Some("A".into()))
                .on_click(move |_, window, cx| suggest(window, cx)),
        )
        .into_any_element()
}

fn pick_key(pick: Pick) -> &'static str {
    match pick {
        Pick::NewBase => "1",
        Pick::Commit => "2",
        Pick::Both => "3",
    }
}

/// A side's name over its lines, or what it says when it has none.
fn label(side: Side, row: &Row, theme: &Theme) -> AnyElement {
    let (glyph, name, color) = match side {
        Side::NewBase => (row.icons[0], row.sides[0], theme.diff_added),
        Side::Commit => (row.icons[1], row.sides[1], theme.blue),
        // Under its toggle, not a label.
        Side::Ancestor => return div().into_any_element(),
    };
    let empty = side_lines(row.conflict, side).is_empty();
    let text = match empty {
        true => format!("Nothing from {}", name.phrase),
        false => name.label.to_owned(),
    };
    strip(row.layout, row.last && empty, theme)
        .h(dp(24.))
        .text_color(theme.text_2)
        .child(small_icon(glyph).text_color(color))
        .child(text)
        .into_any_element()
}

/// What was there before either side changed it: nothing, or a button that shows it.
fn ancestor_toggle(row: &Row, theme: &Theme) -> AnyElement {
    let lines = row.conflict.ancestor.as_ref().map_or(0, Vec::len);
    let toggle = row.actions.ancestor.clone();
    let strip = strip(row.layout, row.last, theme).h(dp(28.));
    if lines == 0 {
        return strip
            .text_color(theme.text_3)
            .child("Both sides added these lines")
            .into_any_element();
    }
    strip
        .child(
            button(
                "conflict-ancestor",
                match (row.ancestor, lines) {
                    (true, _) => "Hide how it was before".to_owned(),
                    (false, 1) => "Show the line before either changed it".to_owned(),
                    (false, n) => format!("Show the {n} lines before either changed them"),
                },
            )
            .small()
            .style(ButtonStyle::Ghost)
            .on_click(move |_, window, cx| toggle(window, cx)),
        )
        .into_any_element()
}

/// An unchanged line around the conflict.
fn plain_line(id: ElementId, line: &DiffLine, row: &Row, theme: &Theme) -> AnyElement {
    let look = look(LineKind::Context, row.layout.plain, theme);
    side_line(id, line, look, row.layout, row.last, theme)
}

fn side_line(
    id: ElementId,
    line: &DiffLine,
    look: Look,
    layout: &Layout,
    last: bool,
    theme: &Theme,
) -> AnyElement {
    line_card(id.clone(), last, layout.wrap, theme)
        .when_some(look.bg, |el, bg| el.bg(bg))
        .child(number_cell(
            line.new,
            layout.digits,
            &look,
            None,
            None,
            theme,
        ))
        .child(text_cell(
            id,
            line_text(line, look.emph, Vec::new(), None, theme),
            layout.wrap,
            &layout.sideways[1],
            theme,
        ))
        .into_any_element()
}

/// What's kept in the conflict's place: the picked lines, the text written or suggested, or
/// the editor writing it, under a line saying which.
fn result(row: &Row, theme: &Theme) -> AnyElement {
    if let Some(Kept::Editing(input)) = &row.kept {
        return editor(input.clone(), row, theme);
    }
    let conflict = row.conflict;
    let (says, lines) = kept(row).unwrap_or_default();
    let change = row.actions.change.clone();
    let edit = row.actions.edit.clone();
    let suggested = matches!(
        row.kept,
        Some(Kept::Text {
            suggested: true,
            ..
        })
    );
    let kept_look = Look {
        bg: None,
        number: theme.diff_muted,
        emph: theme.bg_1,
        bar: Some(theme.safe.into()),
    };
    let last = row.last;
    v_flex()
        .w_full()
        .child(
            strip(row.layout, last && lines.is_empty(), theme)
                .h(dp(28.))
                .text_color(theme.text_2)
                .child(small_icon(Icon::Tick).text_color(theme.safe))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(match lines.is_empty() {
                            true => format!("{says}: nothing, so these lines go"),
                            false => says,
                        }),
                )
                .when(suggested, |el| {
                    el.child(
                        button("conflict-result-edit", "Edit")
                            .small()
                            .style(ButtonStyle::Ghost)
                            .on_click(move |_, window, cx| edit(window, cx)),
                    )
                })
                .child(
                    button("conflict-change", "Change")
                        .small()
                        .style(ButtonStyle::Ghost)
                        .tooltip("Pick again")
                        .on_click(move |_, window, cx| change(window, cx)),
                ),
        )
        .children(lines.iter().enumerate().map(|(ix, line)| {
            let id: ElementId = ("conflict-result", conflict.at as usize * 4096 + ix).into();
            let last = last && ix + 1 == lines.len();
            side_line(id, line, kept_look.clone(), row.layout, last, theme)
        }))
        .into_any_element()
}

/// `lines` numbered from `at` on, as they'd stand in the file.
fn renumbered(lines: impl Iterator<Item = DiffLine>, at: u32) -> Vec<DiffLine> {
    lines
        .zip(at..)
        .map(|(line, new)| DiffLine {
            new: Some(new),
            ..line
        })
        .collect()
}

fn plain(text: &str) -> DiffLine {
    DiffLine::new(LineKind::Context, None, None, text)
}

/// The lines being written, with what keeps them and what drops the edit.
fn editor(input: Entity<TextInput>, row: &Row, theme: &Theme) -> AnyElement {
    let (done, cancel) = (row.actions.done.clone(), row.actions.cancel.clone());
    card(v_flex(), theme)
        .when(row.last, |el| el.border_b_1().rounded_b(dp(6.)))
        // Clicks in it stay off the diff, which would take the keys back to its shortcuts.
        .occlude()
        .p(dp(8.))
        .gap(dp(6.))
        .child(
            field(&input, row.focused, theme)
                .p(dp(6.))
                .rounded(dp(6.))
                .font_family(theme.mono_font.clone())
                .text_size(dp(theme.code_text))
                .line_height(dp(theme.code_line)),
        )
        .child(
            h_flex()
                .gap(dp(6.))
                .text_size(dp(12.))
                .child(
                    div()
                        .flex_1()
                        .text_color(theme.text_2)
                        .child("Leaving it empty removes these lines."),
                )
                .child(
                    button("conflict-cancel", "Cancel")
                        .small()
                        .style(ButtonStyle::Ghost)
                        .tooltip_with_key("Drop the edit", Some("esc".into()))
                        .on_click(move |_, window, cx| cancel(window, cx)),
                )
                .child(
                    button("conflict-done", "Keep")
                        .small()
                        .style(ButtonStyle::Gray)
                        .tooltip_with_key("Keep these lines", Some("⌘↩".into()))
                        .on_click(move |_, window, cx| done(window, cx)),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{Part, Side, View};
    use crate::backend::{
        Conflict, DiffBody, DiffLine, FileChange, FileDiff, Hunk, HunkKey, LineKind,
    };
    use crate::ui::diff::{DiffRow, rows};

    fn line(new: u32) -> DiffLine {
        DiffLine::new(LineKind::Context, Some(new), Some(new), "x")
    }

    /// A conflict of one line each side, with a line of context either way.
    fn conflict(index: usize, at: u32) -> Conflict {
        Conflict {
            id: format!("c{index}"),
            index,
            at,
            before: vec![line(at - 1)],
            new_base: vec![line(at)],
            commit: vec![line(at)],
            ancestor: None,
            after: vec![line(at + 1)],
            new_base_text: "x\n".into(),
            commit_text: "y\n".into(),
            labels: Default::default(),
        }
    }

    /// A file with one changed line at 30, and conflicts at 12 and 50.
    fn file() -> FileDiff {
        let header = but_core::HunkHeader {
            old_start: 30,
            old_lines: 1,
            new_start: 30,
            new_lines: 1,
        };
        FileDiff {
            file: FileChange {
                conflicted: true,
                ..FileChange::bare("a.rs")
            },
            body: DiffBody::Hunks {
                hunks: vec![Hunk {
                    key: HunkKey(header),
                    header: String::new(),
                    lines: vec![line(30)],
                    scope: Vec::new(),
                    lead: Vec::new(),
                    tail: Vec::new(),
                }],
                added: 0,
                removed: 0,
                widest: [1, 1],
                conflicts: vec![conflict(0, 12), conflict(1, 50)],
            },
            fingerprint: 0,
            annotated: true,
            scope_lines: None,
            drawing: None,
            commit: None,
        }
    }

    /// The rows of [`file`], shown whole to line `whole` when that's given, each conflict laid
    /// out as `view`.
    fn file_rows(whole: Option<u32>, view: View) -> Vec<DiffRow> {
        let none = |_: &FileDiff| false;
        rows(
            &[file()],
            none,
            none,
            none,
            |_, _| false,
            |_, _, _| false,
            |_| whole,
            false,
            |_, _| view,
        )
    }

    fn parts(rows: &[DiffRow], of: usize) -> Vec<Part> {
        (rows.iter())
            .filter_map(|row| match *row {
                DiffRow::Conflict { conflict, part, .. } if conflict == of => Some(part),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn conflicts_sit_among_the_hunks_by_line() {
        let shown = file_rows(None, View::default());
        let first_bar = shown.iter().position(|row| {
            matches!(
                row,
                DiffRow::Conflict {
                    conflict: 0,
                    part: Part::Bar,
                    ..
                }
            )
        });
        let hunk = shown
            .iter()
            .position(|row| matches!(row, DiffRow::Hunk { .. }));
        let last_bar = shown.iter().position(|row| {
            matches!(
                row,
                DiffRow::Conflict {
                    conflict: 1,
                    part: Part::Bar,
                    ..
                }
            )
        });
        assert!(
            first_bar < hunk && hunk < last_bar,
            "the conflict at 12 comes before the hunk at 30, the one at 50 after it"
        );
        assert_eq!(
            parts(&shown, 0),
            [
                Part::Bar,
                Part::Before(0),
                Part::Label(Side::NewBase),
                Part::Line(Side::NewBase, 0),
                Part::Label(Side::Commit),
                Part::Line(Side::Commit, 0),
                Part::After(0),
            ],
            "an open conflict shows both sides between its context"
        );
    }

    #[test]
    fn a_settled_conflict_shows_what_it_keeps_and_a_whole_file_its_own_context() {
        let settled = View {
            settled: true,
            ancestor: false,
        };
        let shown = file_rows(None, settled);
        assert_eq!(
            parts(&shown, 0),
            [Part::Bar, Part::Before(0), Part::Result, Part::After(0)],
            "what's kept stands in for the two sides"
        );
        let whole = file_rows(Some(60), settled);
        assert_eq!(
            parts(&whole, 0),
            [Part::Bar, Part::Result],
            "a file shown whole has the unchanged lines around the conflict already"
        );
        let unchanged: Vec<u32> = (whole.iter())
            .filter_map(|row| match *row {
                DiffRow::Context { new, .. } => Some(new),
                _ => None,
            })
            .collect();
        assert!(
            unchanged.contains(&11) && !unchanged.contains(&12) && unchanged.contains(&13),
            "the new base's line at 12 shows only in the conflict"
        );
    }
}
