//! Folds that open and close over a moment rather than at once, as ui-react's sections fold
//! in `--transition-medium`: what a fold holds grows down from its heading as one curtain, or
//! shrinks back up into it, its chevron turning as it goes. What's closing stays drawn till
//! it's shut, taking no clicks. Panes beside the others slide open and shut over the same
//! moment ([`Slide`]), and a list's rows slide to where a reorder puts them ([`Reorder`]).
//! Under reduced motion, all of them get there at once.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::f32::consts::FRAC_PI_2;
use std::hash::Hash;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnimationExt as _, AnyElement, App, Div, ElementId, Hsla, Length, Pixels,
    SpringAnimationElement, Svg, Transformation, div, prelude::*, radians,
};

use super::{Icon, fixed, small_icon, switch_spring};

/// How long a fold takes to open or close: design-core's `--transition-medium`.
pub const FOLD: Duration = Duration::from_millis(150);

/// The most rows a list's fold moves, which is more than a window shows: those past them
/// join or leave once it's done, so a branch of a thousand commits opens as quickly as one of
/// twenty, and the list draws no more rows a frame than it can show.
pub const MOVING_ROWS: usize = 100;

/// A view's folds moving now, by what they fold.
pub struct Folds<K> {
    moving: HashMap<K, Move>,
    /// The frame being drawn, which every row of a fold reads its openness at.
    now: Instant,
}

#[derive(Clone)]
struct Move {
    open: bool,
    since: Instant,
    /// What it holds as tall as it was last drawn, for one that isn't in a list's rows.
    height: Rc<Cell<Option<Pixels>>>,
}

/// Where a row of a flat list is in a fold moving now: the fold, how many rows down it, and
/// how many rows it moves.
pub struct Place<K> {
    fold: K,
    at: usize,
    of: usize,
}

impl<K: Eq + Hash> Default for Folds<K> {
    fn default() -> Self {
        Self {
            moving: HashMap::new(),
            now: Instant::now(),
        }
    }
}

impl<K: Eq + Hash> Folds<K> {
    /// `key` just opened, or shut: it moves there from where it is, so one turned back partway
    /// goes back from there.
    pub fn start(&mut self, key: K, open: bool, cx: &App) {
        match cx.reduce_motion() {
            true => self.stop(&key),
            false => self.start_at(key, open, Instant::now()),
        }
    }

    fn start_at(&mut self, key: K, open: bool, now: Instant) {
        let shut = match open {
            true => 0.,
            false => 1.,
        };
        let at = (self.moving.get(&key)).map_or(shut, |m| m.openness(now));
        let since = set_out(open, at, now);
        let height = (self.moving.remove(&key)).map_or_else(Rc::default, |m| m.height);
        self.moving.insert(
            key,
            Move {
                open,
                since,
                height,
            },
        );
    }

    /// Stop `key` where it's going, as when what it holds went.
    pub fn stop(&mut self, key: &K) {
        self.moving.remove(key);
    }

    /// Stop every fold, as when the rows are rebuilt from scratch.
    pub fn stop_all(&mut self) {
        self.moving.clear();
    }

    /// How far open `key` is, from 0 shut to 1 open, while it moves; `None` at rest.
    pub fn openness(&self, key: &K) -> Option<f32> {
        Some(self.moving.get(key)?.openness(self.now))
    }

    /// Whether `key` is closing, so what it holds is still drawn.
    pub fn closing(&self, key: &K) -> bool {
        self.moving.get(key).is_some_and(|m| !m.open)
    }

    /// Whether `key` shows what it holds: `open`, or still closing.
    pub fn shows(&self, key: &K, open: bool) -> bool {
        open || self.closing(key)
    }

    /// Whether any fold moves.
    pub fn any(&self) -> bool {
        !self.moving.is_empty()
    }

    /// Go on to the frame being drawn, dropping the folds that got where they were going, and
    /// say whether any did, for a view whose rows then change. While [`Self::any`] still
    /// moves, the view asks for the frame after.
    pub fn frame(&mut self) -> bool {
        self.frame_at(Instant::now())
    }

    fn frame_at(&mut self, now: Instant) -> bool {
        self.now = now;
        let moving = self.moving.len();
        (self.moving).retain(|_, m| now.saturating_duration_since(m.since) < FOLD);
        self.moving.len() < moving
    }

    /// `content`, what fold `key` holds outside a list, cut to as far as it's open while it
    /// moves, by its height as last drawn. The first frame of one opening shows none of it
    /// while that's measured, and one closing all.
    pub fn reveal(&self, key: &K, content: impl IntoElement) -> AnyElement {
        let Some(m) = self.moving.get(key) else {
            return content.into_any_element();
        };
        let open = m.openness(self.now);
        let measured = m.height.clone();
        let inner = div().relative().child(content).child(
            gpui::canvas(
                move |bounds, _, _| measured.set(Some(bounds.size.height)),
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        );
        let height = match m.height.get() {
            Some(height) => height * open,
            None if open < 0.5 => Pixels::ZERO,
            None => return inner.into_any_element(),
        };
        clipped(inner, height, !m.open).into_any_element()
    }
}

impl<K: Eq + Hash + Clone> Folds<K> {
    /// `rows`, a flat list's, each with its place in the fold moving it, if one is: a row
    /// `heads` says heads a moving fold holds the rows after it that are `under` it. Only as
    /// many of them move as `fit` under the head where the list last drew it, given its index;
    /// the rest are left out till it's there. A fold inside one that moves moves with it.
    pub fn place<R>(
        &self,
        rows: Vec<R>,
        heads: impl Fn(&R) -> Option<K>,
        under: impl Fn(&R, &R) -> bool,
        fit: impl Fn(usize) -> usize,
    ) -> (Vec<R>, Vec<Option<Place<K>>>) {
        if self.moving.is_empty() {
            return (rows, Vec::new());
        }
        // The moving folds' heads, by index, and how many rows each holds.
        let mut heading = Vec::new();
        let mut ix = 0;
        while let Some(row) = rows.get(ix) {
            ix += 1;
            if let Some(fold) = heads(row).filter(|fold| self.moving.contains_key(fold)) {
                let held = (rows.iter().skip(ix)).take_while(|r| under(row, r)).count();
                heading.push((ix - 1, fold, held));
                ix += held;
            }
        }
        let mut heading = heading.into_iter().peekable();
        let (mut kept, mut places) = (Vec::with_capacity(rows.len()), Vec::new());
        // The fold the rows are in now: it, its first row, and how many it holds and moves.
        let mut inside: Option<(K, usize, usize, usize)> = None;
        for (ix, row) in rows.into_iter().enumerate() {
            let place = (inside.as_ref())
                .filter(|(_, from, held, _)| ix < from + held)
                .map(|(fold, from, _, of)| Place {
                    fold: fold.clone(),
                    at: ix - from,
                    of: *of,
                });
            if place.as_ref().is_some_and(|p| p.at >= p.of) {
                continue;
            }
            if let Some((_, fold, held)) = heading.next_if(|(head, ..)| *head == ix) {
                let of = held.min(fit(kept.len())).min(MOVING_ROWS);
                inside = Some((fold, ix + 1, held, of));
            }
            kept.push(row);
            places.push(place);
        }
        (kept, places)
    }

    /// How the fold a flat list's row is `placed` in moves it; `None` at rest.
    pub fn moved(&self, placed: Option<&Place<K>>) -> Option<Moved> {
        let Place { fold, at, of } = placed?;
        Some(Moved {
            open: self.openness(fold)?,
            at: *at,
            of: *of,
            closing: self.closing(fold),
        })
    }
}

/// A flat list's row in a moving fold: how far open that is, how many rows down it the row is
/// and how many it moves, and whether it's closing.
#[derive(Clone, Copy)]
pub struct Moved {
    open: f32,
    at: usize,
    of: usize,
    pub closing: bool,
}

impl Moved {
    /// How much of the row shows, each of the fold's rows `height` tall.
    pub fn shown(&self, height: f32) -> f32 {
        curtain(
            self.open,
            self.of as f32 * height,
            self.at as f32 * height,
            height,
        )
    }
}

impl Move {
    fn openness(&self, now: Instant) -> f32 {
        openness(self.open, self.since, now)
    }
}

/// A pane sliding open or shut beside the others, as the navigator and the file list do, over
/// [`FOLD`] as a fold moves. It keeps its width and its place as it goes, and the edge of the
/// pane beside it moves across it, so neither lays out its rows again at every width. One
/// turned back partway goes back from where it is. Under reduced motion, it opens and closes
/// at once.
#[derive(Default)]
pub struct Slide(Option<Sliding>);

#[derive(Clone, Copy)]
struct Sliding {
    open: bool,
    since: Instant,
}

impl Slide {
    /// The pane just opened, or shut: it moves there from where it is.
    pub fn start(&mut self, open: bool, cx: &App) {
        match cx.reduce_motion() {
            true => self.0 = None,
            false => self.start_at(open, Instant::now()),
        }
    }

    fn start_at(&mut self, open: bool, now: Instant) {
        let shut = match open {
            true => 0.,
            false => 1.,
        };
        let at = self.openness(now).unwrap_or(shut);
        let since = set_out(open, at, now);
        self.0 = Some(Sliding { open, since });
    }

    /// How far open the pane is at `now`, from 0 shut to 1 open, while it moves; `None` at rest.
    pub fn openness(&self, now: Instant) -> Option<f32> {
        let Sliding { open, since } = self.0?;
        (now.saturating_duration_since(since) < FOLD).then(|| openness(open, since, now))
    }

    /// Whether the pane is closing at `now`, so it's still drawn.
    pub fn closing(&self, now: Instant) -> bool {
        self.openness(now).is_some() && self.0.is_some_and(|s| !s.open)
    }

    /// How much of the pane shows at `now`, from 0 to 1: as far as it's open while it moves,
    /// else all of it while it's `open`.
    pub fn share(&self, open: bool, now: Instant) -> f32 {
        match (self.openness(now), open) {
            (Some(at), _) => at,
            (None, true) => 1.,
            (None, false) => 0.,
        }
    }
}

/// A hairline along the edge of a pane sliding over another, as the border the other has at
/// rest: placed at the edge by the caller.
pub fn edge(color: Hsla) -> Div {
    div().absolute().top_0().bottom_0().w(fixed(1.)).bg(color)
}

/// When a move toward `open` that's `at` open by `now` set out, so it goes on from there.
fn set_out(open: bool, at: f32, now: Instant) -> Instant {
    // How far along its way there a move that's at `at` now is.
    let along = match open {
        true => unease(at),
        false => unease(1. - at),
    };
    (now.checked_sub(FOLD.mul_f32(along))).unwrap_or(now)
}

/// How far open a move toward `open` that set out at `since` is at `now`, from 0 to 1.
fn openness(open: bool, since: Instant, now: Instant) -> f32 {
    let along = now.saturating_duration_since(since).as_secs_f32() / FOLD.as_secs_f32();
    let eased = ease(along.clamp(0., 1.));
    match open {
        true => eased,
        false => 1. - eased,
    }
}

/// How much of a list's row shows while the fold holding it is `open` that far: the fold's
/// rows, `extent` tall together, open as one curtain from the top, so this one, `top` down
/// them and `height` tall, shows only once the curtain's past its top.
pub fn curtain(open: f32, extent: f32, top: f32, height: f32) -> f32 {
    (open * extent - top).clamp(0., height)
}

/// `row` cut to `height`, as much of it as a moving fold shows, and while `closing`, covered so
/// it takes no clicks, drops or hovers: what's under the pointer as it closes is the row that
/// will be there.
pub fn clipped(row: impl IntoElement, height: impl Into<Length> + Clone, closing: bool) -> Div {
    inert(div().h(height).overflow_y_hidden().child(row), closing)
}

/// `el`, on its way out while `closing`, covered so it takes no clicks, drops or hovers.
pub fn inert(el: Div, closing: bool) -> Div {
    el.when(closing, |el| {
        el.relative().child(div().absolute().inset_0().occlude())
    })
}

/// A fold's chevron, in `color`: pointing right, or down while `open`, turning between the
/// two on the spring a switch's knob slides on, which settles in about [`FOLD`]. One clicked
/// again mid-turn turns back from where it is.
pub fn chevron(id: impl Into<ElementId>, open: bool, color: Hsla) -> SpringAnimationElement<Svg> {
    turned(id, open, Icon::ChevronRight, FRAC_PI_2, color)
}

/// `icon` in `color`, turned `by` radians while `on`, turning as [`chevron`] does.
pub fn turned(
    id: impl Into<ElementId>,
    on: bool,
    icon: Icon,
    by: f32,
    color: Hsla,
) -> SpringAnimationElement<Svg> {
    small_icon(icon)
        .text_color(color)
        .with_spring(id, switch_spring(on), move |el, phase| {
            el.with_transformation(Transformation::rotate(radians(
                phase.interpolate_clamped(0., by),
            )))
        })
}

/// CSS's `ease`, which design-core's transitions ride: a cubic Bézier through (0.25, 0.1) and
/// (0.25, 1), at `x` of the way through its time.
fn ease(x: f32) -> f32 {
    let t = solve(x, |t| bezier(t, 0.25, 0.25));
    bezier(t, 0.1, 1.)
}

/// How far through its time [`ease`] is at `y`, as it climbs all the way.
fn unease(y: f32) -> f32 {
    let t = solve(y, |t| bezier(t, 0.1, 1.));
    bezier(t, 0.25, 0.25)
}

/// A coordinate of a cubic Bézier from 0 to 1 at `t`, its control points' at `a` and `b`.
fn bezier(t: f32, a: f32, b: f32) -> f32 {
    let u = 1. - t;
    3. * u * u * t * a + 3. * u * t * t * b + t * t * t
}

/// Where between 0 and 1 `f`, which climbs, reaches `target`, by halving.
fn solve(target: f32, f: impl Fn(f32) -> f32) -> f32 {
    let (mut low, mut high) = (0., 1.);
    for _ in 0..24 {
        let mid = (low + high) / 2.;
        match f(mid) < target {
            true => low = mid,
            false => high = mid,
        }
    }
    (low + high) / 2.
}

/// A list's rows sliding to where a reorder put them, over [`FOLD`] as a fold moves, by their
/// keys. Rows reordered again partway slide on from where they're drawn. Under reduced
/// motion, rows move at once.
pub struct Reorder<K> {
    from: HashMap<K, f32>,
    since: Instant,
    /// The frame being drawn, which every row reads its offset at.
    now: Instant,
}

impl<K> Default for Reorder<K> {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            from: HashMap::new(),
            since: now,
            now,
        }
    }
}

impl<K: Eq + Hash + Clone> Reorder<K> {
    /// Slide the rows a reorder moved, each from as far from where it's laid out as `offsets`
    /// says, as [`reordered`] works out.
    pub fn start(&mut self, offsets: HashMap<K, f32>, cx: &App) {
        match cx.reduce_motion() {
            true => self.from.clear(),
            false => self.start_at(offsets, Instant::now()),
        }
    }

    fn start_at(&mut self, offsets: HashMap<K, f32>, now: Instant) {
        if offsets.is_empty() {
            return;
        }
        let mut from: HashMap<K, f32> = (self.from.keys())
            .map(|key| (key.clone(), self.offset_at(key, now)))
            .collect();
        for (key, by) in offsets {
            *from.entry(key).or_default() += by;
        }
        from.retain(|_, by| *by != 0.);
        (self.from, self.since, self.now) = (from, now, now);
    }

    /// How far row `key` is drawn from where it's laid out, in this frame.
    pub fn offset(&self, key: &K) -> f32 {
        self.offset_at(key, self.now)
    }

    fn offset_at(&self, key: &K, now: Instant) -> f32 {
        let left = 1. - openness(true, self.since, now);
        self.from.get(key).map_or(0., |by| by * left)
    }

    /// Whether any row slides.
    pub fn any(&self) -> bool {
        !self.from.is_empty()
    }

    /// Go on to the frame being drawn, letting the rows that got there rest. While
    /// [`Self::any`] still slides, the view asks for the frame after.
    pub fn frame(&mut self) {
        self.frame_at(Instant::now());
    }

    fn frame_at(&mut self, now: Instant) {
        self.now = now;
        if now.saturating_duration_since(self.since) >= FOLD {
            self.from.clear();
        }
    }
}

/// How far each row of a list sits from where it's laid out, as it starts sliding there,
/// after the rows went from `old` to `new`: each by its key, if it keeps one, and its height.
/// The rows whose order changed slide from where they were. The rest slide as far as those
/// moved them, as rows only added or removed move nothing, and rows without a key stay put.
pub fn reordered<'a, K: Eq + Hash + Clone>(
    old: &'a [(Option<K>, f32)],
    new: &'a [(Option<K>, f32)],
) -> HashMap<K, f32> {
    let counts = |rows: &'a [(Option<K>, f32)]| {
        let mut counts: HashMap<&K, usize> = HashMap::new();
        for key in rows.iter().filter_map(|(key, _)| key.as_ref()) {
            *counts.entry(key).or_default() += 1;
        }
        counts
    };
    let (in_old, in_new) = (counts(old), counts(new));
    let kept = |key: &'a K| in_old.get(key) == Some(&1) && in_new.get(key) == Some(&1);
    let rank: HashMap<&K, usize> = (old.iter().filter_map(|(key, _)| key.as_ref()))
        .filter(|key| kept(key))
        .enumerate()
        .map(|(rank, key)| (key, rank))
        .collect();
    let along_new: Vec<&K> = (new.iter().filter_map(|(key, _)| key.as_ref()))
        .filter(|key| kept(key))
        .collect();
    let ranks: Vec<usize> = along_new
        .iter()
        .filter_map(|key| rank.get(key).copied())
        .collect();
    let stayed: HashSet<&K> = (longest_rise(&ranks).into_iter())
        .filter_map(|ix| along_new.get(ix).copied())
        .collect();
    // Where each kept row starts, and how tall the rows only on that side above it are.
    let places = |rows: &'a [(Option<K>, f32)]| {
        let (mut top, mut alone) = (0., 0.);
        let mut places: HashMap<&K, (f32, f32)> = HashMap::new();
        for (key, height) in rows {
            match key {
                Some(key) if kept(key) => _ = places.insert(key, (top, alone)),
                Some(_) => alone += height,
                None => {}
            }
            top += height;
        }
        places
    };
    let (was, is) = (places(old), places(new));
    (is.iter())
        .filter_map(|(key, (top, added))| {
            let (was_top, removed) = was.get(key)?;
            let by = match stayed.contains(key) {
                true => was_top - top + added - removed,
                false => was_top - top,
            };
            (by != 0.).then(|| ((*key).clone(), by))
        })
        .collect()
}

/// The indexes of a longest strictly rising run of `seq`, in order.
fn longest_rise(seq: &[usize]) -> Vec<usize> {
    let value = |ix: usize| seq.get(ix).copied().unwrap_or_default();
    let (mut tails, mut before): (Vec<usize>, Vec<Option<usize>>) = (Vec::new(), Vec::new());
    for (ix, &x) in seq.iter().enumerate() {
        let at = tails.partition_point(|&tail| value(tail) < x);
        before.push(at.checked_sub(1).and_then(|at| tails.get(at).copied()));
        match tails.get_mut(at) {
            Some(tail) => *tail = ix,
            None => tails.push(ix),
        }
    }
    let mut run = Vec::with_capacity(tails.len());
    let mut at = tails.last().copied();
    while let Some(ix) = at {
        run.push(ix);
        at = before.get(ix).copied().flatten();
    }
    run.reverse();
    run
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reordered_rows_slide_on_from_where_they_are() {
        let mut reorder = Reorder::default();
        let start = Instant::now();
        reorder.start_at(HashMap::from([("a", 30.)]), start);
        assert_eq!(reorder.offset(&"a"), 30., "it starts where it was");
        reorder.frame_at(start + FOLD / 2);
        let halfway = reorder.offset(&"a");
        assert!(halfway > 0. && halfway < 30., "halfway, it's partly there");
        reorder.start_at(HashMap::from([("a", -10.), ("b", 10.)]), reorder.now);
        assert!(
            (reorder.offset(&"a") - (halfway - 10.)).abs() < 1e-3,
            "moved again, it goes on from where it's drawn"
        );
        reorder.frame_at(reorder.now + FOLD);
        assert!(!reorder.any(), "then rests");
        assert_eq!(reorder.offset(&"b"), 0., "where it's laid out");
    }

    #[test]
    fn rows_slide_only_where_their_order_changed() {
        let rows = |keys: &[&'static str]| -> Vec<(Option<&'static str>, f32)> {
            keys.iter()
                .map(|key| ((*key != "-").then_some(*key), 10.))
                .collect()
        };
        let sorted = |offsets: HashMap<&'static str, f32>| {
            let mut offsets: Vec<_> = offsets.into_iter().collect();
            offsets.sort_by(|a, b| a.0.cmp(b.0));
            offsets
        };
        assert_eq!(
            sorted(reordered(
                &rows(&["a1", "a2", "-", "b1", "b2"]),
                &rows(&["b1", "b2", "-", "a1", "a2"])
            )),
            [("a1", -30.), ("a2", -30.), ("b1", 30.), ("b2", 30.)],
            "a swap slides both sections from where they were, the keyless row between snapping"
        );
        assert!(
            reordered(&rows(&["a", "b"]), &rows(&["x", "a", "b"])).is_empty(),
            "a row added above moves nothing"
        );
        assert_eq!(
            sorted(reordered(
                &rows(&["a", "b", "c"]),
                &rows(&["x", "c", "a", "b"])
            )),
            [("a", -10.), ("b", -10.), ("c", 10.)],
            "the moved row slides from its old place, and those it passed by its height alone"
        );
    }

    #[test]
    fn ease_runs_from_shut_to_open_as_css_does() {
        assert!(ease(0.).abs() < 1e-4, "it starts where it was");
        assert!((ease(1.) - 1.).abs() < 1e-4, "it ends where it's going");
        // CSS's `ease` is most of the way there by half its time.
        assert!(
            (ease(0.5) - 0.8024).abs() < 1e-3,
            "it sets out quickly and eases in"
        );
    }

    #[test]
    fn unease_finds_where_ease_is() {
        for x in [0.1, 0.3, 0.5, 0.9] {
            assert!(
                (unease(ease(x)) - x).abs() < 1e-3,
                "a move turned back finds where it was along its way"
            );
        }
    }

    #[test]
    fn a_curtain_shows_the_rows_above_its_edge() {
        // Three rows 10 tall, a third of the way open.
        let shown = [0., 10., 20.].map(|top| curtain(1. / 3., 30., top, 10.));
        assert_eq!(
            shown,
            [10., 0., 0.],
            "the first row shows whole, the rest not yet"
        );
        let shown = [0., 10., 20.].map(|top| curtain(0.5, 30., top, 10.));
        assert_eq!(
            shown,
            [10., 5., 0.],
            "the curtain's edge cuts the second row in half"
        );
    }

    #[test]
    fn a_moving_fold_places_the_rows_under_its_head() {
        let mut folds = Folds::default();
        folds.start_at('b', true, Instant::now());
        // Rows by depth: `b` heads the two deeper rows after it, not `c` beside it.
        let rows = vec![('a', 0), ('b', 0), ('x', 1), ('y', 2), ('c', 0)];
        let (rows, places) = folds.place(
            rows,
            |&(name, _)| Some(name),
            |&(_, head), &(_, depth)| depth > head,
            |_| MOVING_ROWS,
        );
        let placed: Vec<_> = (rows.iter().zip(&places))
            .map(|((name, _), place)| (*name, place.as_ref().map(|p| (p.fold, p.at, p.of))))
            .collect();
        assert_eq!(
            placed,
            [
                ('a', None),
                ('b', None),
                ('x', Some(('b', 0, 2))),
                ('y', Some(('b', 1, 2))),
                ('c', None)
            ],
            "only what `b` holds moves with it"
        );
    }

    #[test]
    fn a_moving_fold_leaves_out_what_the_list_doesnt_show() {
        let mut folds = Folds::default();
        folds.start_at(0, true, Instant::now());
        let rows: Vec<usize> = (0..20).collect();
        let (rows, places) = folds.place(rows, |&row| Some(row), |_, _| true, |head| 5 + head);
        assert_eq!(rows.len(), 1 + 5, "the head and the rows that fit under it");
        assert!(
            places
                .last()
                .and_then(Option::as_ref)
                .is_some_and(|p| p.of == 5),
            "the rest join once it's open"
        );
    }

    #[test]
    fn a_fold_turned_back_partway_goes_back_from_where_it_is() {
        let mut folds = Folds::default();
        let start = Instant::now();
        folds.start_at(1, true, start);
        folds.now = start + FOLD / 3;
        let open = folds.openness(&1).unwrap_or_default();
        assert!(
            open > 0.1 && open < 0.9,
            "a third of the way, it's partly open"
        );
        folds.start_at(1, false, folds.now);
        let back = folds.openness(&1).unwrap_or_default();
        assert!(
            (back - open).abs() < 1e-3,
            "it closes from as far open as it got"
        );
        folds.now += FOLD / 10;
        assert!(
            folds.openness(&1).is_some_and(|now| now < open),
            "and goes on closing from there"
        );
    }

    #[test]
    fn a_slide_turned_back_partway_goes_back_from_where_it_is() {
        let mut slide = Slide::default();
        let start = Instant::now();
        slide.start_at(false, start);
        let at = start + FOLD / 3;
        let open = slide.share(false, at);
        assert!(
            open > 0.1 && open < 0.9 && slide.closing(at),
            "a third of the way, it's partly shut and still drawn"
        );
        slide.start_at(true, at);
        assert!(
            (slide.share(true, at) - open).abs() < 1e-3,
            "it opens from as far shut as it got"
        );
        assert!(!slide.closing(at), "and is no longer on its way out");
    }

    #[test]
    fn a_slide_rests_once_it_gets_there() {
        let mut slide = Slide::default();
        assert_eq!(
            (
                slide.share(true, Instant::now()),
                slide.share(false, Instant::now())
            ),
            (1., 0.),
            "one that never moved shows as it is"
        );
        let start = Instant::now();
        slide.start_at(false, start);
        assert!(
            slide.openness(start + FOLD / 2).is_some(),
            "halfway, it's moving"
        );
        let there = start + FOLD;
        assert!(
            slide.openness(there).is_none() && !slide.closing(there),
            "there, it rests"
        );
        assert_eq!(slide.share(false, there), 0., "and is drawn shut");
    }

    #[test]
    fn a_fold_rests_once_it_gets_there() {
        let mut folds = Folds::default();
        let start = Instant::now();
        folds.start_at(1, false, start);
        assert!(
            !folds.frame_at(start + FOLD / 2) && folds.any(),
            "halfway, it's moving"
        );
        assert!(
            folds.shows(&1, false),
            "so the view draws what it holds, though it's shut"
        );
        assert!(folds.frame_at(start + FOLD), "there, it says it arrived");
        assert!(
            !folds.any() && !folds.shows(&1, false),
            "and it's drawn shut"
        );
    }
}
