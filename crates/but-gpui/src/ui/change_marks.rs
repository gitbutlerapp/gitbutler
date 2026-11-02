//! Marks for a diff's added and removed lines beside its scrollbar, as deltoids draws them:
//! the whole diff fitted to the track's height, so a glance shows where it changes, however far
//! down that is.

use std::rc::Rc;

use gpui::{Bounds, Hsla, Pixels, canvas, fill, point, prelude::*, size};

use super::fixed;
use crate::theme::Theme;

/// How tall a mark's smallest step is, in the track's pixels.
const CELL: f32 = 3.;

/// How wide the strip at the diff's right edge is, which the marks and the scrollbar's thumb
/// share, as deltoids' does.
pub const RAIL: f32 = 14.;

/// How wide the marks are, and how far right of them the scrollbar's thumb starts at rest.
const WIDTH: Pixels = fixed(5.);
const RIGHT: Pixels = fixed(8.);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Added,
    Removed,
}

/// A run of changed rows, measured from the top of the diff.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Span {
    top: f32,
    height: f32,
    change: Change,
}

/// The changed runs of a diff's rows, in logical pixels before the theme's scale.
#[derive(Debug, Default, PartialEq)]
pub struct ChangeMarks {
    spans: Vec<Span>,
    height: f32,
}

impl ChangeMarks {
    /// Add a row `height` tall below the others, with the changes it shows: a side-by-side
    /// row can show a removed line beside an added one.
    pub fn push(&mut self, height: f32, changes: [Option<Change>; 2]) {
        for change in changes.into_iter().flatten() {
            // A run goes on through the rows of one kind, and past rows of the other kind
            // beside it.
            match (self.spans.iter_mut().rev())
                .take_while(|span| span.top + span.height >= self.height)
                .find(|span| span.change == change)
            {
                Some(span) => span.height = self.height + height - span.top,
                None => self.spans.push(Span {
                    top: self.height,
                    height,
                    change,
                }),
            }
        }
        self.height += height;
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
}

/// One step of the track, which can hold both kinds of change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Cell {
    added: bool,
    removed: bool,
}

/// `spans` of a diff `content` tall on a track `track` tall in steps `cell` tall, each at least
/// `least` tall, as a line is, so a lone changed line in a long file still shows. A diff that
/// fits maps one to one, so the marks line up with the rows beside them. A taller one is
/// fitted so its first and last rows land on the first and last steps, which keeps a change
/// at either end in sight wherever the diff is scrolled.
fn cells(spans: &[Span], content: f32, track: f32, cell: f32, least: f32) -> Vec<Cell> {
    let count = match cell > 0. && track > 0. {
        true => (track / cell).floor() as usize,
        false => 0,
    };
    let mut cells = vec![Cell::default(); count];
    if count == 0 || content <= 0. {
        return cells;
    }
    let last = count - 1;
    let fits = content <= track;
    let cell_at = |y: f32| {
        let y = y.clamp(0., (content - 1.).max(0.));
        let ix = match (fits, content <= 1.) {
            (true, _) => (y / cell).floor(),
            (false, true) => 0.,
            (false, false) => (y * last as f32 / (content - 1.)).floor(),
        };
        (ix as usize).min(last)
    };
    let least = ((least / cell).ceil() as usize).clamp(1, count);
    for span in spans.iter().filter(|span| span.height > 0.) {
        let mut first = cell_at(span.top);
        let mut end = cell_at(span.top + span.height - 1.).max(first);
        // Grown down from its place, or up from the track's end.
        if end + 1 - first < least {
            end = (first + least - 1).min(last);
            first = end + 1 - least;
        }
        for cell in cells.get_mut(first..=end).into_iter().flatten() {
            match span.change {
                Change::Added => cell.added = true,
                Change::Removed => cell.removed = true,
            }
        }
    }
    cells
}

/// `marks` along the right edge of a positioned element the size of the diff's list, just left
/// of where its scrollbar's thumb shows.
pub fn change_marks(marks: Rc<ChangeMarks>, theme: &Theme) -> impl IntoElement + use<> {
    let (added, removed, scale) = (theme.diff_added, theme.diff_removed, theme.scale);
    let line = theme.code_line * scale;
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let track = f32::from(bounds.size.height);
            let content = marks.height * scale;
            let spans: Vec<Span> = (marks.spans.iter())
                .map(|span| Span {
                    top: span.top * scale,
                    height: span.height * scale,
                    change: span.change,
                })
                .collect();
            let left = bounds.right() - RIGHT - WIDTH;
            let mut paint = |y: f32, height: f32, color: Hsla| {
                let at = point(left, bounds.top() + fixed(y));
                window.paint_quad(fill(Bounds::new(at, size(WIDTH, fixed(height))), color));
            };
            // A block for each run of steps alike. One with both kinds splits in two, removed
            // above added, as unified lines are.
            let cells = cells(&spans, content, track, CELL, line);
            let mut start = 0;
            for run in cells.chunk_by(|a, b| a == b) {
                let (y, height) = (start as f32 * CELL, run.len() as f32 * CELL);
                start += run.len();
                match run.first().map(|cell| (cell.added, cell.removed)) {
                    Some((true, true)) => {
                        paint(y, (height / 2.).round(), removed);
                        paint(
                            y + (height / 2.).round(),
                            height - (height / 2.).round(),
                            added,
                        );
                    }
                    Some((true, false)) => paint(y, height, added),
                    Some((false, true)) => paint(y, height, removed),
                    _ => {}
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(rows: &[[Option<Change>; 2]]) -> ChangeMarks {
        let mut marks = ChangeMarks::default();
        for changes in rows {
            marks.push(1., *changes);
        }
        marks
    }

    const ADDED: [Option<Change>; 2] = [Some(Change::Added), None];
    const REMOVED: [Option<Change>; 2] = [Some(Change::Removed), None];
    const NONE: [Option<Change>; 2] = [None, None];

    #[test]
    fn rows_of_one_kind_make_one_run() {
        let marks = marks(&[REMOVED, REMOVED, ADDED, ADDED, NONE, ADDED]);
        let runs: Vec<_> = (marks.spans.iter())
            .map(|span| (span.top, span.height, span.change))
            .collect();
        assert_eq!(
            runs,
            [
                (0., 2., Change::Removed),
                (2., 2., Change::Added),
                (5., 1., Change::Added)
            ]
        );
    }

    #[test]
    fn side_by_side_rows_keep_both_runs_going() {
        let pair = [Some(Change::Removed), Some(Change::Added)];
        let marks = marks(&[pair, pair, ADDED]);
        let runs: Vec<_> = (marks.spans.iter())
            .map(|span| (span.top, span.height, span.change))
            .collect();
        assert_eq!(runs, [(0., 2., Change::Removed), (0., 3., Change::Added)]);
    }

    #[test]
    fn changes_at_either_end_of_a_tall_diff_stay_on_the_track() {
        let mut rows = vec![NONE; 101];
        (rows[0], rows[50], rows[100]) = (REMOVED, ADDED, REMOVED);
        let marks = marks(&rows);
        let cells = cells(&marks.spans, marks.height, 8., 1., 1.);
        assert!(cells[0].removed, "the first row is on the first step");
        assert!(cells[3].added, "the middle row is halfway down");
        assert!(cells[7].removed, "the last row is on the last step");
    }

    #[test]
    fn crowded_changes_keep_both_kinds() {
        let marks = marks(&[
            ADDED, REMOVED, NONE, NONE, NONE, NONE, NONE, NONE, NONE, NONE,
        ]);
        let cells = cells(&marks.spans, marks.height, 3., 1., 1.);
        assert_eq!(
            cells[0],
            Cell {
                added: true,
                removed: true
            }
        );
    }

    #[test]
    fn a_lone_line_in_a_long_diff_is_a_line_tall() {
        let mut rows = vec![NONE; 1000];
        (rows[500], rows[999]) = (ADDED, REMOVED);
        let marks = marks(&rows);
        let cells = cells(&marks.spans, marks.height, 30., 1., 4.);
        let added: Vec<_> = (0..30).filter(|&ix| cells[ix].added).collect();
        let removed: Vec<_> = (0..30).filter(|&ix| cells[ix].removed).collect();
        assert_eq!(added, [14, 15, 16, 17], "grown down from where it is");
        assert_eq!(removed, [26, 27, 28, 29], "grown up from the track's end");
    }

    #[test]
    fn a_diff_that_fits_lines_up_with_its_rows() {
        let marks = marks(&[ADDED, NONE, NONE, REMOVED]);
        let cells = cells(&marks.spans, marks.height, 9., 1., 1.);
        assert!(cells[0].added && !cells[1].added && cells[3].removed);
        assert_eq!(cells[4], Cell::default(), "nothing past the diff's end");
    }
}
