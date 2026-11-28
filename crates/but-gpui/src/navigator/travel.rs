//! A project going between the cards and the hidden projects' card, as but-ui-react's
//! HideAndShowMotion story carries it: a copy of it floats over the cards from where it was to
//! where it lands, while the room it leaves closes and the room it takes opens, as cards trading
//! places do. The hidden projects are a list of their own, so no slide of one list can carry it
//! across, and the two lists can take it in different frames, so each one's room moves from when
//! that list does.

use std::cell::Cell;
use std::collections::HashSet;
use std::ops::Range;
use std::time::{Duration, Instant};

use gpui::{AnyElement, App, Bounds, Context, Pixels, anchored, deferred, div, point, prelude::*};

use super::rows::{CARD_PADDING, Drawer, Row, span_height};
use super::{CARD_GAP, HIDDEN_HEAD, Navigator, RowStyle, card_of, part_ends};
use crate::clock;
use crate::state::{ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::fold::{REORDER, openness_over, travel_time};
use crate::ui::{dp, repo_item};

/// How far through its way the rows of a card under its heading start coming in.
const BODY_FROM: f32 = 0.5;

/// How far through its way a copy going to the hidden projects stops carrying its card's rows
/// and carries the row it lands as: shrunk all but to a row by then, eased as it is, so the
/// row it leaves at the end is the one under it.
const ROW_FROM: f32 = 0.75;

/// Above the rows and their slides, below menus and tooltips.
const TRAVEL_PRIORITY: usize = 1;

pub(super) struct Travel {
    project: ProjectId,
    /// Going to the hidden projects' card, rather than up among the cards.
    hiding: bool,
    /// Where it was drawn, in the window: its card, or its part of the hidden projects' card.
    from: Bounds<Pixels>,
    /// When it set out: once where it lands is drawn, as it waits where it was till then.
    since: Cell<Instant>,
    /// How long it takes, once where it lands is drawn and so how far it goes is known.
    lasting: Cell<Option<Duration>>,
    /// The cards closing over the room it left or opening for it, from when the cards took it:
    /// how far the hidden projects' card starts from where the list lays it out, so it slides
    /// with them.
    cards: Option<(f32, Instant)>,
    /// The hidden projects' rows opening or closing for it, from when they took it: the first
    /// row past its room, and how tall that room is.
    drawer: Option<(usize, f32, Instant)>,
    /// The hidden projects' card coming with the first project hidden, or going with the last
    /// one shown: `true` while it comes.
    footer_comes: Option<bool>,
    /// Hiding, the rows of the card it leaves, and how far in each sits, as they were drawn:
    /// the copy carries them, folding away into it as it shrinks to a row.
    left_card: Option<(Vec<Row>, Vec<f32>)>,
}

impl Travel {
    fn lasting(&self) -> Duration {
        self.lasting.get().unwrap_or(REORDER)
    }

    /// How far along its way it is, eased as the navigator's reorders are.
    fn along(&self, now: Instant) -> f32 {
        openness_over(true, self.since.get(), now, self.lasting())
    }

    /// How much of its time has gone, from 0 to 1, rather than of its way.
    fn gone(&self, now: Instant) -> f32 {
        let lasting = self.lasting().as_secs_f32();
        (now.saturating_duration_since(self.since.get())
            .as_secs_f32()
            / lasting)
            .clamp(0., 1.)
    }

    /// Whether where it lands is drawn yet: once it's there, under it, for the one frame before
    /// it goes. Drawn sooner, it would show past the copy's edge as the copy eases in, or through
    /// the copy as it faded.
    fn landed_drawn(&self, now: Instant) -> bool {
        self.gone(now) >= 1.
    }

    /// How far in the rows of its card under its heading are, coming in under it from halfway
    /// along its way, so the card fills in round its heading rather than shows up at once.
    fn body_in(&self, now: Instant) -> f32 {
        ((self.gone(now) - BODY_FROM) / (1. - BODY_FROM)).clamp(0., 1.)
    }
}

/// How a project crossed in one change of the lists, worked out before they change.
pub(super) struct Crossing {
    project: ProjectId,
    hiding: bool,
    /// Where it was drawn, if the list it leaves still drew it.
    from: Option<Bounds<Pixels>>,
    /// The cards took it or let it go: as far as the hidden projects' card moves.
    cards: Option<f32>,
    /// The hidden projects' rows let it go, from where and how tall its part was.
    drawer_left: Option<(usize, f32)>,
    /// The hidden projects' rows took it.
    drawer_took: bool,
    /// The hidden projects' card comes with it, or goes.
    footer_comes: Option<bool>,
    /// Hiding, the rows of the card it leaves, as [`Travel::left_card`].
    left_card: Option<(Vec<Row>, Vec<f32>)>,
}

/// The projects `rows` lists rows of.
fn projects(rows: &[Row]) -> HashSet<&ProjectId> {
    (rows.iter())
        .filter_map(|row| match row {
            Row::Item(Selection::Project(id)) => Some(id),
            _ => None,
        })
        .collect()
}

/// The rows of the hidden projects' card that are `project`'s part of it.
fn part_of(rows: &[Row], project: &ProjectId) -> Option<Range<usize>> {
    let at = (rows.iter())
        .position(|row| matches!(row, Row::Item(Selection::Project(id)) if id == project))?;
    let end = (at + 1..rows.len())
        .find(|ix| part_ends(rows, *ix).0)
        .unwrap_or(rows.len());
    Some(at..end)
}

/// The bounds from the top of `first` to the bottom of `last`.
fn spanning(first: Bounds<Pixels>, last: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::from_corners(first.origin, last.bottom_right())
}

fn lerp(from: Bounds<Pixels>, to: Bounds<Pixels>, along: f32) -> Bounds<Pixels> {
    let at = |a: Pixels, b: Pixels| a + (b - a) * along;
    Bounds {
        origin: point(at(from.left(), to.left()), at(from.top(), to.top())),
        size: gpui::size(
            at(from.size.width, to.size.width),
            at(from.size.height, to.size.height),
        ),
    }
}

/// How much of a move of the room that set out at `since` is still to go, from 1 to 0, as the
/// cards' own slides go.
fn left_since(since: Instant) -> f32 {
    1. - openness_over(true, since, clock::now(), REORDER)
}

impl Navigator {
    /// The project that `rows` and `drawer` move between the cards and the hidden projects'
    /// card, if one does, and where it was: read before the lists change. The hidden projects'
    /// count can change a frame or two before either list moves it, and it sets off then.
    pub(super) fn crossing(&self, rows: &[Row], drawer: &Drawer, cx: &App) -> Option<Crossing> {
        if cx.reduce_motion() || self.filtering(cx) {
            return None;
        }
        let total_changed = drawer.total != self.built.drawer.total;
        let footer_comes = match (self.built.drawer.total, drawer.total) {
            (0, 1..) => Some(true),
            (1.., 0) => Some(false),
            _ => None,
        };
        let drawer = &drawer.rows;
        let store = self.store.read(cx);
        let (was_carded, carded) = (projects(&self.built.rows), projects(rows));
        let (was_hidden, hidden) = (projects(&self.built.drawer.rows), projects(drawer));
        let moved = |id: &&ProjectId| {
            was_carded.contains(id) != carded.contains(id)
                || was_hidden.contains(id) != hidden.contains(id)
        };
        // Only a project still there that changed sides: one added or removed isn't crossing.
        let project = (was_carded.iter().chain(&carded))
            .chain(was_hidden.iter().chain(&hidden))
            .filter(|id| moved(id))
            .find(|id| {
                (was_carded.contains(*id) || was_hidden.contains(*id))
                    && (carded.contains(*id) || hidden.contains(*id))
            })
            .map(|id| (*id).clone())
            .or_else(|| {
                // Hidden, or shown, but still where it was.
                let stays = |rows: &HashSet<&ProjectId>, expanded: bool| {
                    (rows.iter())
                        .find(|id| store.project(id).is_some_and(|e| e.expanded == expanded))
                        .map(|id| (*id).clone())
                };
                total_changed
                    .then(|| stays(&carded, false).or_else(|| stays(&hidden, true)))
                    .flatten()
            })?;
        let hiding = !store.project(&project)?.expanded;
        let item = Selection::Project(project.clone());
        // Hiding, it leaves the cards; showing, the hidden projects' card.
        let from = match hiding {
            true => card_of(&self.built.rows, &item).and_then(|card| self.card_bounds(card, cx)),
            false => {
                part_of(&self.built.drawer.rows, &project).and_then(|p| self.part_bounds(p, cx))
            }
        };
        let cards = (was_carded.contains(&project) != carded.contains(&project)).then(|| {
            span_height(&self.built.rows, 0..self.built.rows.len(), self.row)
                - span_height(rows, 0..rows.len(), self.row)
        });
        let drawer_left = (!hiding && was_hidden.contains(&project) && !hidden.contains(&project))
            .then(|| part_of(&self.built.drawer.rows, &project))
            .flatten()
            .map(|part| (part.start, part.map(|ix| self.drawer_row_height(ix)).sum()));
        let drawer_took = hiding && !was_hidden.contains(&project) && hidden.contains(&project);
        let left_card = (hiding && was_carded.contains(&project))
            .then(|| card_of(&self.built.rows, &item))
            .flatten()
            .map(|card| {
                let rows = self
                    .built
                    .rows
                    .get(card.clone())
                    .unwrap_or_default()
                    .to_vec();
                let insets = self.built.insets.get(card).unwrap_or_default().to_vec();
                (rows, insets)
            });
        Some(Crossing {
            project,
            hiding,
            from,
            cards,
            drawer_left,
            drawer_took,
            footer_comes,
            left_card,
        })
    }

    /// Set the project off, or carry on with it, once the hidden projects' rows are the new
    /// ones, saying whether the cards took it or let it go in this change, so they slide around
    /// the room.
    pub(super) fn cross(&mut self, crossing: Crossing) -> bool {
        let now = clock::now();
        let same = (self.travel.as_ref())
            .is_some_and(|t| t.project == crossing.project && t.hiding == crossing.hiding);
        if !same {
            let Some(from) = crossing.from else {
                self.travel = None;
                return false;
            };
            self.travel = Some(Travel {
                project: crossing.project.clone(),
                hiding: crossing.hiding,
                from,
                since: Cell::new(now),
                lasting: Cell::new(None),
                cards: None,
                drawer: None,
                footer_comes: None,
                left_card: crossing.left_card,
            });
        }
        let drawer = match (crossing.drawer_left, crossing.drawer_took) {
            (Some((at, room)), _) => Some((at, room, now)),
            (None, true) => Some(match part_of(&self.built.drawer.rows, &crossing.project) {
                Some(part) => {
                    let room = part.clone().map(|ix| self.drawer_row_height(ix)).sum();
                    (part.end, room, now)
                }
                // Into the card folded: only its bar shows.
                None => (usize::MAX, 0., now),
            }),
            (None, false) => None,
        };
        let Some(travel) = self.travel.as_mut() else {
            return false;
        };
        if drawer.is_some() {
            travel.drawer = drawer;
        }
        if let Some(by) = crossing.cards {
            travel.cards = Some((by, now));
        }
        if crossing.footer_comes.is_some() {
            travel.footer_comes = crossing.footer_comes;
        }
        crossing.cards.is_some()
    }

    /// Whether a project is on its way, and the frame after this one should be drawn: ends the
    /// one that got there, and whose room is done moving.
    pub(super) fn travel_frame(&mut self) -> bool {
        let now = clock::now();
        let done =
            |since: Instant, lasting: Duration| now.saturating_duration_since(since) >= lasting;
        let over = (self.travel.as_ref()).is_some_and(|t| {
            done(t.since.get(), t.lasting())
                && t.cards.is_none_or(|(_, since)| done(since, REORDER))
                && t.drawer.is_none_or(|(_, _, since)| done(since, REORDER))
        });
        if over {
            self.travel = None;
        }
        self.travel.is_some()
    }

    /// How far the hidden projects' card is drawn from where the list lays it out.
    pub(super) fn footer_offset(&self) -> f32 {
        (self.travel.as_ref())
            .and_then(|t| t.cards)
            .map_or(0., |(by, since)| by * left_since(since))
    }

    /// How opaque the hidden projects' card is drawn: fading in with the first project hidden,
    /// out with the last one shown, as the room does.
    pub(super) fn footer_opacity(&self) -> f32 {
        (self.travel.as_ref()).map_or(1., |t| match t.footer_comes {
            Some(comes) => {
                let left = left_since(t.since.get());
                if comes { 1. - left } else { left }
            }
            None => 1.,
        })
    }

    /// How much taller the hidden projects' card is drawn than its rows: less while the room a
    /// project takes opens, more while the room it left closes.
    pub(super) fn footer_room(&self) -> f32 {
        (self.travel.as_ref()).map_or(0., |t| {
            let left = (t.drawer).map_or(0., |(_, room, since)| room * left_since(since));
            if t.hiding { -left } else { left }
        })
    }

    /// How far the hidden projects' row `ix` is drawn from where it's laid out, and how opaque,
    /// when it's the project on its way: unseen where it leaves, coming in where it lands as its
    /// copy hands over.
    pub(super) fn drawer_travel(&self, ix: usize) -> (f32, Option<f32>) {
        let Some(travel) = &self.travel else {
            return (0., None);
        };
        let own =
            part_of(&self.built.drawer.rows, &travel.project).is_some_and(|p| p.contains(&ix));
        let shift = match travel.drawer {
            Some((after, _, _)) if ix >= after => self.footer_room(),
            _ => 0.,
        };
        let opacity = match travel.hiding && travel.landed_drawn(clock::now()) {
            true => 1.,
            false => 0.,
        };
        (shift, own.then_some(opacity))
    }

    /// How opaque the card's row `ix` is drawn, when it's of the project on its way: unseen where
    /// it leaves; where it lands, its heading under the copy once that hands over, and the rows
    /// under it coming in from halfway.
    pub(super) fn travelling_in_cards(&self, ix: usize) -> Option<f32> {
        let travel = self.travel.as_ref()?;
        let item = Selection::Project(travel.project.clone());
        let card = card_of(&self.built.rows, &item)?;
        if !card.contains(&ix) {
            return None;
        }
        let now = clock::now();
        let heading = matches!(self.built.rows.get(ix), Some(Row::Item(it)) if *it == item);
        Some(match (travel.hiding, heading) {
            (true, _) => 0.,
            (false, true) => f32::from(u8::from(travel.landed_drawn(now))),
            (false, false) => travel.body_in(now),
        })
    }

    /// Where `item`'s card heading in `rows` is drawn in the window, between the card's margins:
    /// the copy goes from and to it alone, and the card's other rows come and go around it.
    fn heading_bounds(&self, rows: &[Row], item: &Selection, cx: &App) -> Option<Bounds<Pixels>> {
        let at = (rows.iter()).position(|row| matches!(row, Row::Item(it) if it == item))?;
        self.card_bounds(at..at + 1, cx)
    }

    /// Where the card's rows `card` are drawn in the window, between the card's margins, if
    /// its first row is.
    fn card_bounds(&self, card: Range<usize>, cx: &App) -> Option<Bounds<Pixels>> {
        let top = crate::ui::shown_row(&self.list, card.start)?;
        let last = (card.rev())
            .find_map(|ix| self.list.bounds_for_item(ix))
            .unwrap_or(top);
        let theme = Theme::get(cx);
        let (left, right) = self.card_margins();
        let bounds = spanning(top, last);
        Some(Bounds::from_corners(
            point(bounds.left() + theme.scaled(left), bounds.top()),
            point(bounds.right() - theme.scaled(right), bounds.bottom()),
        ))
    }

    /// Where the hidden projects' rows `part` are drawn in the window, if its first is, out to
    /// the card's edges, its bottom too as the card's last, so what's in it lines up with the
    /// copy, whose own edge is there.
    fn part_bounds(&self, part: Range<usize>, cx: &App) -> Option<Bounds<Pixels>> {
        let last_part = part.end >= self.built.drawer.rows.len();
        let top = crate::ui::shown_row(&self.drawer_list, part.start)?;
        let last = (part.rev())
            .find_map(|ix| self.drawer_list.bounds_for_item(ix))
            .unwrap_or(top);
        let edge = Theme::get(cx).scaled(1.);
        let bounds = spanning(top, last);
        Some(Bounds::from_corners(
            point(bounds.left() - edge, bounds.top()),
            point(
                bounds.right() + edge,
                bounds.bottom() + if last_part { edge } else { Pixels::ZERO },
            ),
        ))
    }

    /// Where the project's copy is going, and whether it fades as it gets there: its part of
    /// the hidden projects' card, or the card's bar while that's folded, or its card.
    fn travel_to(&self, travel: &Travel, cx: &App) -> Option<(Bounds<Pixels>, bool)> {
        let item = Selection::Project(travel.project.clone());
        if !travel.hiding {
            return self
                .heading_bounds(&self.built.rows, &item, cx)
                .map(|b| (b, false));
        }
        if let Some(part) = part_of(&self.built.drawer.rows, &travel.project)
            && let Some(bounds) = self.part_bounds(part, cx)
        {
            return Some((bounds, false));
        }
        let footer = self.list.bounds_for_item(self.built.rows.len())?;
        let theme = Theme::get(cx);
        let (left, right) = self.card_margins();
        let top = footer.top() + theme.scaled(CARD_GAP);
        Some((
            Bounds::from_corners(
                point(footer.left() + theme.scaled(left), top),
                point(
                    footer.right() - theme.scaled(right),
                    top + theme.scaled(HIDDEN_HEAD),
                ),
            ),
            true,
        ))
    }

    /// The project's row as it lands: as its part of the hidden projects' card draws it, or as
    /// its card's heading.
    fn travelling_row(&self, travel: &Travel, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let item = Selection::Project(travel.project.clone());
        let at = |rows: &[Row]| {
            (rows.iter()).position(|row| matches!(row, Row::Item(it) if *it == item))
        };
        // Hiding, the card it leaves, all its rows, cut off as the copy shrinks to a row, till
        // it's all but a row.
        if travel.hiding
            && travel.gone(clock::now()) < ROW_FROM
            && let Some((rows, insets)) = &travel.left_card
        {
            let drawn = rows.iter().enumerate().map(|(ix, row)| match row {
                Row::Item(_) => {
                    let inset = insets.get(ix).copied().unwrap_or_default();
                    // Drawn without their guides, so each commit keeps its glyph.
                    self.render_item(rows, &[], ix, inset, theme, cx)
                }
                Row::HeaderRule(_) => super::header_rule(theme),
                _ => div().h(dp(row.height(self.row))).into_any_element(),
            });
            return div().flex().flex_col().children(drawn).into_any_element();
        }
        if travel.hiding
            && let Some(ix) = at(&self.built.drawer.rows)
        {
            let inset = (self.built.drawer.insets.get(ix).copied()).unwrap_or_default();
            self.drawing_drawer.set(true);
            let drawer = &self.built.drawer;
            let row = self.render_item(&drawer.rows, &drawer.tips, ix, inset, theme, cx);
            self.drawing_drawer.set(false);
            return row;
        }
        if let Some(ix) = at(&self.built.rows) {
            let inset = self.built.insets.get(ix).copied().unwrap_or_default();
            return self.render_item(&self.built.rows, &self.built.tips, ix, inset, theme, cx);
        }
        let name = (self.store.read(cx).project(&travel.project))
            .map(|entry| entry.project.name.clone())
            .unwrap_or_default();
        RowStyle::plain(theme, 0.)
            .draw(repo_item("travelling", name, theme), 0.)
            .into_any_element()
    }

    /// The project on its way, over the cards, as the terminal crossing to another card floats:
    /// from where it was to where it lands, as tall as it is along the way between the two.
    /// Edged all the way, as a part of the hidden projects' card is edged by the card and the
    /// line over it: hiding, only its corners square off as it lands; showing, it's a card.
    pub(super) fn travelling(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let travel = self.travel.as_ref()?;
        let theme = Theme::get(cx);
        let landing = self.travel_to(travel, cx);
        // How far it goes is known once where it lands is drawn, and holds from then on.
        // It sets out then too: till where it lands is drawn, it waits where it was, rather than
        // being further along its way than it's shown once it's known.
        let now = clock::now();
        if travel.lasting.get().is_none()
            && let Some((to, _)) = landing
        {
            let distance = f32::from(to.top() - travel.from.top()) / theme.scale;
            travel.lasting.set(Some(travel_time(distance)));
            travel.since.set(now);
        }
        let along = travel.along(now);
        // Whole till it's there and what it lands as is drawn under it, and gone the frame after,
        // so neither shows through the other. Only into the hidden projects' bar does it fade.
        let (to, fades) = landing.unwrap_or((travel.from, false));
        let at = lerp(travel.from, to, along);
        let carded = match travel.hiding {
            true => 1. - along,
            false => 1.,
        };
        // Into the hidden projects' card's last part, its bottom corners are the card's.
        let last_part = travel.hiding
            && part_of(&self.built.drawer.rows, &travel.project)
                .is_some_and(|part| part.end >= self.built.drawer.rows.len());
        let radius = theme.shape.card;
        let copy = div()
            .w(at.size.width)
            .h(at.size.height)
            .overflow_hidden()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border_subtle)
            // All but a row in the hidden projects' card, its bottom edge would rule a second
            // line over the card's or the next part's as the room under it closes: what's
            // there draws that line.
            .when(travel.hiding && travel.gone(now) >= ROW_FROM, |el| {
                el.border_b_0()
            })
            .rounded_t(dp(radius * carded))
            .rounded_b(dp(if last_part { radius } else { radius * carded }))
            // Its row sits as a card's first row and a part's do, a card's padding under its edge,
            // and sets itself in.
            .pt(dp(CARD_PADDING))
            .opacity(if fades { 1. - along } else { 1. })
            .child(self.travelling_row(travel, cx));
        Some(
            deferred(anchored().position(at.origin).child(copy))
                .with_priority(TRAVEL_PRIORITY)
                .into_any_element(),
        )
    }
}
