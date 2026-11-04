//! How a navigator row lays out the detail: its home tile, the page the row picks, divided
//! with the tiles pinned beside it.

use std::collections::HashMap;

use gpui::{Context, Entity};

use super::{ProjectId, Selection, Store, StoreEvent};
use crate::backend::Checkout;
use crate::terminal::{Terminal, TerminalId};

/// A navigator row, by what stays the same while agents rewrite it: a commit by its change ID,
/// anything else by its selection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RowKey {
    Commit(ProjectId, String),
    Row(Selection),
}

/// Which way a division lays out its parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Side by side, left to right.
    Across,
    /// One above the other, top to bottom.
    Down,
}

/// What a tile shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tile {
    /// The page of the row the tiling is for, which follows the navigator. It closes only as a
    /// terminal, whose place a terminal pinned beside it takes: see [`Tiling::promote`].
    Home,
    /// Another row's page, whatever the navigator selects: a terminal's is the terminal.
    Pinned(RowKey),
}

/// The tiles a row lays out, in a tree of divisions.
#[derive(Debug, Clone, PartialEq)]
pub enum Tiling {
    Tile(Tile),
    /// Two or more parts along `axis`, each with its share of the room, the shares adding to 1.
    Divided {
        axis: Axis,
        parts: Vec<(f32, Tiling)>,
    },
}

/// Where a tile goes, in shares of the room the tiling has, from its top left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

/// A gap between cards being dragged: between two tiles, by their division's path and the
/// part it's after, or between the navigator and the detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DividerDrag {
    Tiles(Vec<usize>, usize),
    /// Where the lines between tiles meet, by each line's division and the part it's after.
    Junction(Vec<(Vec<usize>, usize)>),
    Navigator,
}

/// The line between two parts of a division, which dragging moves.
#[derive(Debug, Clone, PartialEq)]
pub struct Divider {
    /// The division, by the part each division above it is in, from the top.
    pub path: Vec<usize>,
    /// It's after this part, before the next.
    pub after: usize,
    pub axis: Axis,
    /// The room the division has.
    pub room: Area,
    /// How far along `axis` of all the room the line is: from the left across, from the top
    /// down.
    pub at: f32,
}

/// A side of a tile, or of the room all the tiles share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

/// Where a tile dragged by its bar goes as it's let go.
#[derive(Debug, Clone, PartialEq)]
pub enum Landing {
    /// Back where it was.
    Stay,
    /// On a side of another tile, taking its room from that tile alone.
    Beside(Tile, Side),
    /// Along a side of the whole room, as long as it is.
    Edge(Side),
    /// Out of the row it's in, below all of it.
    BelowRow,
}

/// Which way `area` is from `own`, where it's right beside it: past it, the side of `area` away
/// from `own`.
fn beyond(own: Area, area: Area) -> Option<Side> {
    let ends = |area: Area| (area.left + area.width, area.top + area.height);
    let ((own_right, own_bottom), (right, bottom)) = (ends(own), ends(area));
    let across = area.top < own_bottom && bottom > own.top;
    let down = area.left < own_right && right > own.left;
    match () {
        () if across && area.left >= own_right - 1e-4 => Some(Side::Right),
        () if across && right <= own.left + 1e-4 => Some(Side::Left),
        () if down && area.top >= own_bottom - 1e-4 => Some(Side::Bottom),
        () if down && bottom <= own.top + 1e-4 => Some(Side::Top),
        () => None,
    }
}

/// Where over the tiles a dragged tile lands where, in shares of the room: see
/// [`Tiling::landing_at`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zones {
    /// How far in from the room's left, and from its bottom, the pointer lands a tile along
    /// that side.
    pub edge: (f32, f32),
    /// How far in from the room's right the pointer lands a tile along it: further than the
    /// others, as past its left are the navigator and past its bottom the status bar, and past
    /// its right only the window's edge.
    pub right: f32,
    /// How far down a tile its bar's row reaches, which lands one dragged beside it or in its
    /// place, rather than above or below it.
    pub bar: f32,
}

/// How dragging a tile by its bar feels, tried out on Settings' Developer page until it's
/// settled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileDragFeel {
    /// How far the pointer moves before the tile comes off, in design pixels.
    pub lift: f32,
    /// How long the pointer rests over a landing before the tiles make room there, in
    /// milliseconds.
    pub wait: f32,
    /// [`Zones::edge`], in design pixels.
    pub edge: f32,
    /// [`Zones::right`], in design pixels.
    pub right_edge: f32,
    /// [`Zones::bar`], in design pixels.
    pub bar: f32,
    /// How long the other tiles take to move, in milliseconds.
    pub reflow: f32,
    /// How long the dragged tile takes to grow or shrink to the room it'd land in, in
    /// milliseconds.
    pub resize: f32,
    /// How far in from the left and right of all the tiles a row dragged from the navigator
    /// lands as a tile, in design pixels.
    pub row_zone: f32,
    /// How long the pointer rests in one of those zones before the row's tile starts to slide
    /// in, in milliseconds, so a drag on its way to a terminal passes through.
    pub row_wait: f32,
    /// How far in the row's tile slides while it's held there, in design pixels.
    pub row_peek: f32,
}

impl Default for TileDragFeel {
    fn default() -> Self {
        Self {
            lift: 4.,
            wait: 100.,
            edge: 32.,
            right_edge: 96.,
            bar: 48.,
            reflow: 150.,
            resize: 110.,
            row_zone: 96.,
            row_wait: 150.,
            row_peek: 64.,
        }
    }
}

/// The selected row's tiling as it's drawn in a frame while its tiles move.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawn {
    /// Each tile and where it's drawn.
    pub areas: Vec<(Tile, Area)>,
    /// The lines between them, as they're drawn.
    pub dividers: Vec<Divider>,
    /// The tiling at rest, or as a tile dragged shows it would land, where each tile's page
    /// lays out.
    pub rest: Tiling,
    /// The tile dragged by its bar or zoomed, drawn over the others.
    pub lifted: Option<Tile>,
    /// Whether that tile is zoomed, whose page lays out as it's drawn.
    pub zoomed: bool,
}

/// Where lines between tiles of both ways meet, which dragging moves together.
#[derive(Debug, Clone, PartialEq)]
pub struct Junction {
    /// How far across and down all the room it is.
    pub at: (f32, f32),
    pub dividers: Vec<Divider>,
}

impl Junction {
    /// Whether a line runs from it toward `side`.
    pub fn reaches(&self, side: Side) -> bool {
        let (x, y) = self.at;
        self.dividers.iter().any(|divider| {
            let room = divider.room;
            let past = JUNCTION_NEAR;
            match (divider.axis, side) {
                (Axis::Across, Side::Top) => room.top < y - past,
                (Axis::Across, Side::Bottom) => room.top + room.height > y + past,
                (Axis::Down, Side::Left) => room.left < x - past,
                (Axis::Down, Side::Right) => room.left + room.width > x + past,
                (Axis::Across, Side::Left | Side::Right)
                | (Axis::Down, Side::Top | Side::Bottom) => false,
            }
        })
    }
}

/// How near lines are, of all the room, to meet at one junction, as the gaps between tiles make
/// them look one.
const JUNCTION_NEAR: f32 = 0.004;

/// Where `dividers` meet: each line across that touches one down, and those lined up with them.
pub fn junctions(dividers: &[Divider]) -> Vec<Junction> {
    let along = |axis| dividers.iter().filter(move |divider| divider.axis == axis);
    let mut junctions: Vec<Junction> = Vec::new();
    for across in along(Axis::Across) {
        for down in along(Axis::Down) {
            let (x, y) = (across.at, down.at);
            let within = |at: f32, start: f32, length: f32| {
                at >= start - JUNCTION_NEAR && at <= start + length + JUNCTION_NEAR
            };
            if !within(x, down.room.left, down.room.width)
                || !within(y, across.room.top, across.room.height)
            {
                continue;
            }
            let near = junctions.iter_mut().find(|junction| {
                (junction.at.0 - x).abs() < JUNCTION_NEAR
                    && (junction.at.1 - y).abs() < JUNCTION_NEAR
            });
            match near {
                Some(junction) => {
                    for divider in [across, down] {
                        if !junction.dividers.contains(divider) {
                            junction.dividers.push(divider.clone());
                        }
                    }
                }
                None => junctions.push(Junction {
                    at: (x, y),
                    dividers: vec![across.clone(), down.clone()],
                }),
            }
        }
    }
    junctions
}

impl Side {
    fn axis(self) -> Axis {
        match self {
            Side::Left | Side::Right => Axis::Across,
            Side::Top | Side::Bottom => Axis::Down,
        }
    }

    /// Whether a tile on this side comes after what it's beside, in its division.
    fn after(self) -> bool {
        matches!(self, Side::Right | Side::Bottom)
    }

    fn opposite(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
        }
    }
}

impl Area {
    /// All the room there is.
    pub const WHOLE: Area = Area {
        left: 0.,
        top: 0.,
        width: 1.,
        height: 1.,
    };

    /// `at` of the way from it to `to`.
    pub fn toward(self, to: Area, at: f32) -> Area {
        let mix = |from: f32, to: f32| from + (to - from) * at;
        Area {
            left: mix(self.left, to.left),
            top: mix(self.top, to.top),
            width: mix(self.width, to.width),
            height: mix(self.height, to.height),
        }
    }

    fn holds(&self, (x, y): (f32, f32)) -> bool {
        x >= self.left && x < self.left + self.width && y >= self.top && y < self.top + self.height
    }

    fn along(&self, axis: Axis) -> f32 {
        match axis {
            Axis::Across => self.width,
            Axis::Down => self.height,
        }
    }

    fn same_size(&self, other: &Area) -> bool {
        (self.width - other.width).abs() < 1e-4 && (self.height - other.height).abs() < 1e-4
    }

    /// Whether it reaches all of `other`.
    fn covers(&self, other: &Area) -> bool {
        self.left <= other.left + 1e-4
            && self.top <= other.top + 1e-4
            && self.left + self.width >= other.left + other.width - 1e-4
            && self.top + self.height >= other.top + other.height - 1e-4
    }
}

impl Default for Tiling {
    fn default() -> Self {
        Tiling::Tile(Tile::Home)
    }
}

impl Tiling {
    /// Whether it's the home tile alone, as a row is until something is pinned beside it.
    pub fn is_home_only(&self) -> bool {
        *self == Tiling::Tile(Tile::Home)
    }

    /// Divide `at`'s room along `axis` with `pinned`, which takes the half after it. Beside
    /// another part of a division along `axis`, it's a part of that one, so three across take
    /// a third each once divided again, as tmux's panes do.
    pub fn pin(&mut self, at: &Tile, axis: Axis, pinned: RowKey) {
        let new = Tiling::Tile(Tile::Pinned(pinned.clone()));
        match self {
            Tiling::Tile(tile) if tile == at => {
                let old = std::mem::take(self);
                *self = Tiling::Divided {
                    axis,
                    parts: vec![(0.5, old), (0.5, new)],
                };
            }
            Tiling::Tile(_) => {}
            Tiling::Divided { axis: along, parts } => {
                let here =
                    (parts.iter()).position(|(_, part)| matches!(part, Tiling::Tile(t) if t == at));
                match here {
                    Some(ix) if *along == axis => {
                        let n = parts.len() as f32;
                        for (share, _) in parts.iter_mut() {
                            *share *= n / (n + 1.);
                        }
                        parts.insert(ix + 1, (1. / (n + 1.), new));
                    }
                    _ => {
                        for (_, part) in parts.iter_mut() {
                            part.pin(at, axis, pinned.clone());
                        }
                    }
                }
            }
        }
    }

    /// Take away the tiles `keep` refuses, the parts beside each growing into its room. The
    /// home tile always stays.
    pub fn retain(&mut self, keep: &impl Fn(&RowKey) -> bool) {
        self.retain_tiles(&|tile| match tile {
            Tile::Home => true,
            Tile::Pinned(pinned) => keep(pinned),
        });
    }

    /// Take the home tile away, its room going to the parts beside it, and make `pinned`'s
    /// tile the home one where it is: the tiling of `pinned`'s row now.
    pub fn promote(&mut self, pinned: &RowKey) {
        self.retain_tiles(&|tile| *tile != Tile::Home);
        self.replace(&Tile::Pinned(pinned.clone()), Tile::Home);
    }

    /// As it was with `row` in the home tile's place, the home tile being in `heir`'s: what
    /// [`Self::promote`] took away, as a tile that can slide shut in the tiling of `heir`'s row.
    pub fn before_promoting(&self, row: &RowKey, heir: &RowKey) -> Tiling {
        let mut before = self.clone();
        before.replace(&Tile::Home, Tile::Pinned(row.clone()));
        before.replace(&Tile::Pinned(heir.clone()), Tile::Home);
        before
    }

    /// With `pinned` along `side` of all the room, as tall or wide as it is, in half the room of
    /// what's there: the part at that end of a division along `side`, else everything. Not
    /// where it has a tile already, which stays where it is.
    fn pinned_along(&self, side: Side, pinned: RowKey) -> Option<Tiling> {
        let tile = Tile::Pinned(pinned);
        if self.has(&tile) {
            return None;
        }
        let mut tiling = self.clone();
        tiling.put_along(side, Tiling::Tile(tile), 0.5);
        Some(tiling)
    }

    /// Where each tile is drawn with `tile` growing over the others to take all the room, `at`
    /// of the way there, last, to go on top. The others stay as they are, and once it's there
    /// only the home tile is left under it, for the page and terminal it lays out to stay put.
    pub fn zoomed(&self, tile: &Tile, at: f32) -> Vec<(Tile, Area)> {
        let (grown, others): (Vec<_>, Vec<_>) = self
            .areas()
            .into_iter()
            .partition(|(other, _)| other == tile);
        let others = others
            .into_iter()
            .filter(|(other, _)| at < 1. || *other == Tile::Home);
        let grown = (grown.into_iter()).map(|(tile, area)| (tile, area.toward(Area::WHOLE, at)));
        others.chain(grown).collect()
    }

    /// `at` of the way to `to`, which has the same divisions with other shares.
    pub fn toward(&self, to: &Tiling, at: f32) -> Tiling {
        let mut drawn = self.clone();
        drawn.move_toward(to, at);
        drawn
    }

    fn move_toward(&mut self, to: &Tiling, at: f32) {
        if let (Tiling::Divided { parts, .. }, Tiling::Divided { parts: to, .. }) = (self, to) {
            for ((share, part), (to_share, to_part)) in parts.iter_mut().zip(to) {
                *share += (to_share - *share) * at;
                part.move_toward(to_part, at);
            }
        }
    }

    /// As it's drawn while `pinned`'s tile, just pinned, opens out of the split: `open` of its
    /// share, from 0 to 1, the parts beside it keeping the rest in theirs' proportions.
    pub fn opening(&self, pinned: &RowKey, open: f32) -> Tiling {
        let mut drawn = self.clone();
        drawn.open_part(&Tile::Pinned(pinned.clone()), open);
        drawn
    }

    fn open_part(&mut self, tile: &Tile, open: f32) {
        let Tiling::Divided { parts, .. } = self else {
            return;
        };
        let here =
            (parts.iter()).position(|(_, part)| matches!(part, Tiling::Tile(t) if t == tile));
        let Some((share, _)) = here.and_then(|ix| parts.get(ix)) else {
            for (_, part) in parts.iter_mut() {
                part.open_part(tile, open);
            }
            return;
        };
        let (opened, rest) = (*share * open, 1. - *share);
        for (ix, (share, _)) in parts.iter_mut().enumerate() {
            *share = match Some(ix) == here {
                true => opened,
                false if rest > 0. => *share / rest * (1. - opened),
                false => *share,
            };
        }
    }

    fn retain_tiles(&mut self, keep: &impl Fn(&Tile) -> bool) {
        let Tiling::Divided { parts, .. } = self else {
            return;
        };
        for (_, part) in parts.iter_mut() {
            part.retain_tiles(keep);
        }
        parts.retain(|(_, part)| match part {
            Tiling::Tile(tile) => keep(tile),
            Tiling::Divided { .. } => true,
        });
        let total: f32 = parts.iter().map(|(share, _)| share).sum();
        for (share, _) in parts.iter_mut() {
            *share /= total;
        }
        if parts.len() == 1
            && let Some((_, only)) = parts.pop()
        {
            *self = only;
        }
    }

    fn replace(&mut self, old: &Tile, new: Tile) {
        match self {
            Tiling::Tile(tile) if tile == old => *tile = new,
            Tiling::Tile(_) => {}
            Tiling::Divided { parts, .. } => {
                for (_, part) in parts.iter_mut() {
                    part.replace(old, new.clone());
                }
            }
        }
    }

    /// Each tile and where it goes.
    pub fn areas(&self) -> Vec<(Tile, Area)> {
        let mut areas = Vec::new();
        self.lay_out(Area::WHOLE, &mut areas);
        areas
    }

    fn lay_out(&self, room: Area, areas: &mut Vec<(Tile, Area)>) {
        match self {
            Tiling::Tile(tile) => areas.push((tile.clone(), room)),
            Tiling::Divided { axis, parts } => {
                let mut at = 0.;
                for (share, part) in parts {
                    let area = match axis {
                        Axis::Across => Area {
                            left: room.left + room.width * at,
                            width: room.width * share,
                            ..room
                        },
                        Axis::Down => Area {
                            top: room.top + room.height * at,
                            height: room.height * share,
                            ..room
                        },
                    };
                    part.lay_out(area, areas);
                    at += share;
                }
            }
        }
    }

    /// The lines between the parts of each division, which move as they're dragged.
    pub fn dividers(&self) -> Vec<Divider> {
        let mut dividers = Vec::new();
        self.divide(Area::WHOLE, &mut Vec::new(), &mut dividers);
        dividers
    }

    fn divide(&self, room: Area, path: &mut Vec<usize>, dividers: &mut Vec<Divider>) {
        let Tiling::Divided { axis, parts } = self else {
            return;
        };
        let (start, length) = match axis {
            Axis::Across => (room.left, room.width),
            Axis::Down => (room.top, room.height),
        };
        let mut at = 0.;
        for (ix, (share, part)) in parts.iter().enumerate() {
            let area = match axis {
                Axis::Across => Area {
                    left: start + length * at,
                    width: length * share,
                    ..room
                },
                Axis::Down => Area {
                    top: start + length * at,
                    height: length * share,
                    ..room
                },
            };
            path.push(ix);
            part.divide(area, path, dividers);
            path.pop();
            at += share;
            if ix + 1 < parts.len() {
                dividers.push(Divider {
                    path: path.clone(),
                    after: ix,
                    axis: *axis,
                    room,
                    at: start + length * at,
                });
            }
        }
    }

    /// Move the line after part `after` of the division at `path` to `at` of the division's
    /// room, from its start, keeping both parts beside it at least `min` of it.
    pub fn move_divider(&mut self, path: &[usize], after: usize, at: f32, min: f32) {
        let Some(Tiling::Divided { parts, .. }) = self.division(path) else {
            return;
        };
        let start: f32 = parts.iter().take(after).map(|(share, _)| share).sum();
        let (Some((before, _)), Some((next, _))) = (parts.get(after), parts.get(after + 1)) else {
            return;
        };
        let both = before + next;
        // Where they're too small for `min` already, as a narrow window leaves them, they stay.
        let min = min.min(both / 2.);
        let before = (at - start).clamp(min, both - min);
        if let Some((share, _)) = parts.get_mut(after) {
            *share = before;
        }
        if let Some((share, _)) = parts.get_mut(after + 1) {
            *share = both - before;
        }
    }

    /// Give each part of the division at `path` the same room.
    pub fn even_out(&mut self, path: &[usize]) {
        if let Some(Tiling::Divided { parts, .. }) = self.division(path) {
            let n = parts.len() as f32;
            for (share, _) in parts.iter_mut() {
                *share = 1. / n;
            }
        }
    }

    /// The path of each of its divisions, for [`Self::even_out`].
    pub fn divisions(&self) -> Vec<Vec<usize>> {
        let Tiling::Divided { parts, .. } = self else {
            return Vec::new();
        };
        let inside = parts.iter().enumerate().flat_map(|(ix, (_, part))| {
            (part.divisions().into_iter()).map(move |mut path| {
                path.insert(0, ix);
                path
            })
        });
        std::iter::once(Vec::new()).chain(inside).collect()
    }

    fn division(&mut self, path: &[usize]) -> Option<&mut Tiling> {
        let Some((first, rest)) = path.split_first() else {
            return Some(self);
        };
        match self {
            Tiling::Divided { parts, .. } => parts.get_mut(*first)?.1.division(rest),
            Tiling::Tile(_) => None,
        }
    }

    /// Whether `tile` is one of its tiles.
    pub fn has(&self, tile: &Tile) -> bool {
        self.areas().iter().any(|(other, _)| other == tile)
    }

    /// Where the home tile goes, if it's there.
    pub fn home(&self) -> Option<Area> {
        self.area_of(&Tile::Home)
    }

    /// Where `tile` goes, if it's there.
    pub fn area_of(&self, tile: &Tile) -> Option<Area> {
        (self.areas().into_iter())
            .find(|(other, _)| other == tile)
            .map(|(_, area)| area)
    }

    /// Where `tile`, dragged by its bar, lands with the pointer at `pointer`, in shares of the
    /// room from its top left corner, as it was as it came off. With the pointer in
    /// `zones.edge` of the room's left or bottom side, `zones.right` of its right, or past its
    /// top, where the tiles' bars are, along that side, unless it's along it already. Else by
    /// the tile the pointer's over, or nearest, which holds it by its bar, as it would put the bar there: over its own place, it stays,
    /// or in the lower half leaves its row; over another's bar, `zones.bar` tall, on its left or
    /// right, by the half of the bar; lower, above it in its upper half and below it in its
    /// lower, where it would part the tile. Alone beside the tile, the side it's on already
    /// gives way to the other, as does a side that leaves the tiling as it is, and it stays
    /// where both do. With only one other tile, what only lands it along another side of the
    /// room, as dragging to that side does, moves it past that tile instead, but where it's as
    /// the lower half of its own place has it.
    pub fn landing_at(&self, tile: &Tile, pointer: (f32, f32), zones: Zones) -> Option<Landing> {
        let (x, y) = pointer;
        let (across, down) = zones.edge;
        let sides = [
            (across - x, Side::Left),
            (x - 1. + zones.right, Side::Right),
            (-y, Side::Top),
            (y - 1. + down, Side::Bottom),
        ];
        // Not a side it's along already, which goes by the tile under the pointer as anywhere.
        let nearest = (sides.into_iter())
            .filter(|(past, side)| {
                *past > 0. && self.landed(tile, &Landing::Edge(*side), (0., 0.)) != *self
            })
            .max_by(|(a, _), (b, _)| a.total_cmp(b));
        if let Some((_, side)) = nearest {
            return Some(Landing::Edge(side));
        }
        let (x, y) = (x.clamp(0., 0.9999), y.clamp(0., 0.9999));
        let pointer = (x, y);
        let (over, area) = (self.areas().into_iter()).find(|(_, area)| area.holds(pointer))?;
        if over == *tile {
            // Dragged straight down, out of its row.
            let down = y >= area.top + area.height / 2.;
            let moves = || self.landed(tile, &Landing::BelowRow, (0., 0.)) != *self;
            return Some(match down && moves() {
                true => Landing::BelowRow,
                false => Landing::Stay,
            });
        }
        let own = self.area_of(tile)?;
        let side = match y < area.top + zones.bar {
            true => match x < area.left + area.width / 2. {
                true => Side::Left,
                false => Side::Right,
            },
            false => match y < area.top + area.height / 2. {
                true => Side::Top,
                false => Side::Bottom,
            },
        };
        // Alone beside it, the side it's on already is no place to go.
        let side = match self.alone_beside(tile, own, area) {
            Some(away) => match side.axis() == away.axis() {
                true => away,
                false => side,
            },
            None => side,
        };
        // As where it is, as beside the tile next to its own place, the other side, if that
        // moves it.
        let moves = |side: Side| {
            let landing = Landing::Beside(over.clone(), side);
            (self.landed(tile, &landing, (0., 0.)) != *self).then_some(landing)
        };
        let landing = moves(side).or_else(|| moves(side.opposite()));
        let two = self.areas().len() == 2;
        let past = beyond(own, area).filter(|_| two).filter(|away| {
            let Some(landing) = &landing else {
                return false;
            };
            let landed = self.landed(tile, landing, (0., 0.));
            let as_edge = [Side::Left, Side::Right, Side::Top, Side::Bottom]
                .into_iter()
                .filter(|edge| edge != away)
                .any(|edge| self.landed(tile, &Landing::Edge(edge), (0., 0.)) == landed);
            as_edge && self.landed(tile, &Landing::BelowRow, (0., 0.)) != landed
        });
        Some(past.and_then(moves).or(landing).unwrap_or(Landing::Stay))
    }

    /// The side of `area` away from `tile`, at `own`, where it's alone beside that area: beside
    /// it along the way its own division goes, and as long as it the other way, so its own
    /// column or row is only it there. Not so a tile of a column of two beside another column,
    /// which can go into that one's either side.
    fn alone_beside(&self, tile: &Tile, own: Area, area: Area) -> Option<Side> {
        let along = self.axis_holding(tile)?;
        let spans = |start: f32, length: f32, over: f32, over_length: f32| {
            start <= over + 1e-4 && start + length >= over + over_length - 1e-4
        };
        beyond(own, area).filter(|away| {
            away.axis() == along
                && match along {
                    Axis::Across => spans(own.top, own.height, area.top, area.height),
                    Axis::Down => spans(own.left, own.width, area.left, area.width),
                }
        })
    }

    /// The axis of the division with `tile` as one of its parts.
    fn axis_holding(&self, tile: &Tile) -> Option<Axis> {
        let Tiling::Divided { axis, parts } = self else {
            return None;
        };
        match (parts.iter()).any(|(_, part)| matches!(part, Tiling::Tile(t) if t == tile)) {
            true => Some(*axis),
            false => parts.iter().find_map(|(_, part)| part.axis_holding(tile)),
        }
    }

    /// As it is once `tile` lands at `landing`, each tile taking at least `least` of the room's
    /// width and height where it can. Moved along its own division, the tiles keep their room;
    /// moved elsewhere, its room goes to the part next to it that holds the tile it lands
    /// beside, else to the parts beside it, as closing it does, and it keeps what it had along
    /// the way it's laid out with the tile it lands beside, up to half of that tile's.
    pub fn landed(&self, tile: &Tile, landing: &Landing, least: (f32, f32)) -> Tiling {
        let mut landed = self.clone();
        let (beside, side) = match landing {
            Landing::Stay => return landed,
            Landing::Beside(beside, side) => (Some(beside), *side),
            Landing::Edge(side) => (None, *side),
            Landing::BelowRow => return self.below_row(tile, least),
        };
        if landed.reorder(tile, beside, side) {
            return landed;
        }
        let axis = side.axis();
        let Some(had) = self.area_of(tile).map(|area| area.along(axis)) else {
            return landed;
        };
        let least = match axis {
            Axis::Across => least.0,
            Axis::Down => least.1,
        };
        if let Some(beside) = beside {
            landed.give_room(tile, beside);
        }
        landed.retain_tiles(&|other| other != tile);
        let room = match beside {
            Some(beside) => landed.area_of(beside).map(|area| area.along(axis)),
            None => Some(landed.end(side).map_or(1., |area| area.along(axis))),
        };
        let Some(room) = room.filter(|room| *room > 0.) else {
            return self.clone();
        };
        // Of the room it lands in, where it can leave the rest at least the least.
        let share = (had.min(room / 2.)).max(least.min(room / 2.)) / room;
        let new = Tiling::Tile(tile.clone());
        match beside {
            Some(beside) => landed.put_beside(beside, side, new, share),
            None => landed.put_along(side, new, share),
        }
        landed
    }

    /// As it's drawn while `tile` is dragged to land at `landing`: as it lands, unless that
    /// would move or shrink a tile it doesn't land beside, which would then be off where the
    /// pointer lands it by. Then only what it lands beside gives it room, up to half, and the
    /// room it left stays empty until it's let go.
    pub fn shown_landing(&self, tile: &Tile, landing: &Landing, least: (f32, f32)) -> Tiling {
        let mut landed = self.landed(tile, landing, least);
        let (beside, side) = match landing {
            Landing::Stay => return landed,
            Landing::Beside(beside, side) => (vec![beside.clone()], *side),
            Landing::Edge(side) => (landed.next_to(tile), *side),
            Landing::BelowRow => (landed.next_to(tile), Side::Bottom),
        };
        let left_be = |(other, area): &(Tile, Area)| {
            *other == *tile
                || beside.contains(other)
                || self
                    .area_of(other)
                    .is_some_and(|was| was.same_size(area) || area.covers(&was))
        };
        if landed.areas().iter().all(left_be) {
            return landed;
        }
        let Some(slot) = landed.area_of(tile) else {
            return landed;
        };
        let mut shown = self.clone();
        shown.leave_room(tile);
        let Some(path) = shown.path_around(&beside) else {
            return landed;
        };
        let room = shown.area_at(&path, Area::WHOLE).along(side.axis());
        let share = (slot.along(side.axis()).min(room / 2.)) / room.max(1e-4);
        if let Some(part) = shown.division(&path) {
            part.wrap(side, Tiling::Tile(tile.clone()), share);
        }
        shown
    }

    /// Give the room of `tile`, about to go, to the part next to it that holds `to`, if one
    /// does.
    fn give_room(&mut self, tile: &Tile, to: &Tile) {
        let Some((_, parts)) = self.holding(tile) else {
            return;
        };
        let Some(ix) =
            (parts.iter()).position(|(_, part)| matches!(part, Tiling::Tile(t) if t == tile))
        else {
            return;
        };
        let Some(share) = parts.get(ix).map(|(share, _)| *share) else {
            return;
        };
        let near = [ix.checked_sub(1), Some(ix + 1)]
            .into_iter()
            .flatten()
            .find(|near| parts.get(*near).is_some_and(|(_, part)| part.has(to)));
        if let Some((had, _)) = near.and_then(|near| parts.get_mut(near)) {
            *had += share;
        }
    }

    /// The tiles of the part before `tile` in its division, or after it where it's first.
    fn next_to(&mut self, tile: &Tile) -> Vec<Tile> {
        let Some((_, parts)) = self.holding(tile) else {
            return Vec::new();
        };
        let Some(ix) =
            (parts.iter()).position(|(_, part)| matches!(part, Tiling::Tile(t) if t == tile))
        else {
            return Vec::new();
        };
        let near = ix.checked_sub(1).unwrap_or(ix + 1);
        (parts.get(near).iter())
            .flat_map(|(_, part)| part.areas())
            .map(|(tile, _)| tile)
            .collect()
    }

    /// Leave the room `tile` has in place with nothing in it: a division of no parts.
    fn leave_room(&mut self, tile: &Tile) {
        match self {
            Tiling::Tile(t) if t == tile => {
                *self = Tiling::Divided {
                    axis: Axis::Across,
                    parts: Vec::new(),
                }
            }
            Tiling::Tile(_) => {}
            Tiling::Divided { parts, .. } => {
                for (_, part) in parts.iter_mut() {
                    part.leave_room(tile);
                }
            }
        }
    }

    /// The path to the smallest part that holds all of `tiles`.
    fn path_around(&self, tiles: &[Tile]) -> Option<Vec<usize>> {
        if tiles.is_empty() || !tiles.iter().all(|tile| self.has(tile)) {
            return None;
        }
        if let Tiling::Divided { parts, .. } = self {
            for (ix, (_, part)) in parts.iter().enumerate() {
                if let Some(mut path) = part.path_around(tiles) {
                    path.insert(0, ix);
                    return Some(path);
                }
            }
        }
        Some(Vec::new())
    }

    /// As it is once `tile` leaves the row it's in for below it, keeping its height, up to half
    /// the row's, as [`Self::landed`] keeps a tile's room. Where only a column's left of the
    /// row, under the last tile of that, which gives up the room, as below that tile. As it is,
    /// where it's in no row.
    fn below_row(&self, tile: &Tile, least: (f32, f32)) -> Tiling {
        let mut landed = self.clone();
        let Some(path) = self.path_holding(tile) else {
            return landed;
        };
        let (Some(had), Some(Tiling::Divided { axis, .. })) =
            (self.area_of(tile), landed.division(&path))
        else {
            return landed;
        };
        if *axis != Axis::Across {
            return landed;
        }
        if let Some(row) = landed.division(&path) {
            row.retain_tiles(&|other| other != tile);
        }
        let stacked = (landed.division(&path))
            .and_then(|left| left.end(Side::Bottom))
            .map(|end| end.height);
        let room = landed.area_at(&path, Area::WHOLE).height * stacked.unwrap_or(1.);
        if room <= 0. {
            return self.clone();
        }
        let share = (had.height.min(room / 2.)).max(least.1.min(room / 2.)) / room;
        let new = Tiling::Tile(tile.clone());
        if stacked.is_some()
            && let Some(left) = landed.division(&path)
        {
            left.put_along(Side::Bottom, new, share);
            return landed;
        }
        let Some((ix, column)) = path.split_last() else {
            landed.wrap(Side::Bottom, new, share);
            return landed;
        };
        match landed.division(column) {
            Some(Tiling::Divided {
                axis: Axis::Down,
                parts,
            }) => {
                let Some((had, _)) = parts.get_mut(*ix) else {
                    return self.clone();
                };
                let taken = *had * share;
                *had -= taken;
                parts.insert(ix + 1, (taken, new));
            }
            _ => {
                if let Some(row) = landed.division(&path) {
                    let old = std::mem::take(row);
                    *row = Tiling::Divided {
                        axis: Axis::Down,
                        parts: vec![(1. - share, old), (share, new)],
                    };
                }
            }
        }
        landed
    }

    /// The path to the division with `tile` as one of its parts, by the part each division
    /// above it is in.
    fn path_holding(&self, tile: &Tile) -> Option<Vec<usize>> {
        let Tiling::Divided { parts, .. } = self else {
            return None;
        };
        if (parts.iter()).any(|(_, part)| matches!(part, Tiling::Tile(t) if t == tile)) {
            return Some(Vec::new());
        }
        parts.iter().enumerate().find_map(|(ix, (_, part))| {
            let mut path = part.path_holding(tile)?;
            path.insert(0, ix);
            Some(path)
        })
    }

    /// Where the part at `path` goes in `room`.
    fn area_at(&self, path: &[usize], room: Area) -> Area {
        let (Some((first, rest)), Tiling::Divided { axis, parts }) = (path.split_first(), self)
        else {
            return room;
        };
        let before: f32 = parts.iter().take(*first).map(|(share, _)| share).sum();
        let Some((share, part)) = parts.get(*first) else {
            return room;
        };
        let area = match axis {
            Axis::Across => Area {
                left: room.left + room.width * before,
                width: room.width * share,
                ..room
            },
            Axis::Down => Area {
                top: room.top + room.height * before,
                height: room.height * share,
                ..room
            },
        };
        part.area_at(rest, area)
    }

    /// Move `tile` to `side` of `beside`, or of the whole room without it, in the division
    /// along that side's axis that holds both, keeping each part's share. Whether it did.
    fn reorder(&mut self, tile: &Tile, beside: Option<&Tile>, side: Side) -> bool {
        let is = |part: &Tiling, tile: &Tile| matches!(part, Tiling::Tile(t) if t == tile);
        let holding = match (beside, &mut *self) {
            (Some(_), tiling) => tiling.holding(tile),
            (None, Tiling::Divided { axis, parts }) => Some((*axis, parts)),
            (None, Tiling::Tile(_)) => None,
        };
        let Some((_, parts)) = holding.filter(|(axis, _)| *axis == side.axis()) else {
            return false;
        };
        let Some(from) = parts.iter().position(|(_, part)| is(part, tile)) else {
            return false;
        };
        if beside.is_some_and(|beside| !parts.iter().any(|(_, part)| is(part, beside))) {
            return false;
        }
        let part = parts.remove(from);
        let to = match beside {
            Some(beside) => (parts.iter().position(|(_, part)| is(part, beside)))
                .map(|ix| ix + usize::from(side.after())),
            None if side.after() => Some(parts.len()),
            None => Some(0),
        };
        parts.insert(to.unwrap_or(from), part);
        true
    }

    /// The division with `tile` as one of its parts, and its axis.
    fn holding(&mut self, tile: &Tile) -> Option<(Axis, &mut Vec<(f32, Tiling)>)> {
        let Tiling::Divided { axis, parts } = self else {
            return None;
        };
        if (parts.iter()).any(|(_, part)| matches!(part, Tiling::Tile(t) if t == tile)) {
            return Some((*axis, parts));
        }
        parts.iter_mut().find_map(|(_, part)| part.holding(tile))
    }

    /// Put `new` on `side` of `beside`, with `share` of its room, which `beside` keeps the
    /// rest of.
    fn put_beside(&mut self, beside: &Tile, side: Side, new: Tiling, share: f32) {
        let axis = side.axis();
        if let Some((along, parts)) = self.holding(beside)
            && along == axis
        {
            let Some(ix) =
                (parts.iter()).position(|(_, part)| matches!(part, Tiling::Tile(t) if t == beside))
            else {
                return;
            };
            let Some((had, _)) = parts.get_mut(ix) else {
                return;
            };
            let taken = *had * share;
            *had -= taken;
            parts.insert(ix + usize::from(side.after()), (taken, new));
            return;
        }
        self.divide_tile(beside, side, new, share);
    }

    /// Divide `tile`'s room along `side`'s axis, `new` taking `share` of it on that side.
    fn divide_tile(&mut self, tile: &Tile, side: Side, new: Tiling, share: f32) {
        match self {
            Tiling::Tile(t) if t == tile => {
                let old = std::mem::take(self);
                let parts = match side.after() {
                    true => vec![(1. - share, old), (share, new)],
                    false => vec![(share, new), (1. - share, old)],
                };
                *self = Tiling::Divided {
                    axis: side.axis(),
                    parts,
                };
            }
            Tiling::Tile(_) => {}
            Tiling::Divided { parts, .. } => {
                for (_, part) in parts.iter_mut() {
                    part.divide_tile(tile, side, new.clone(), share);
                }
            }
        }
    }

    /// Put `new` along `side` of the whole room, with `share` of it.
    /// Put `new` along `side` of the whole room, with `share` of the part at that end of it, as
    /// [`Self::end`] finds it, which keeps the rest, or of all of it.
    fn put_along(&mut self, side: Side, new: Tiling, share: f32) {
        let Tiling::Divided { axis, parts } = self else {
            return self.wrap(side, new, share);
        };
        let at = if side.after() { parts.len() } else { 0 };
        let end = if side.after() {
            parts.len().checked_sub(1)
        } else {
            Some(0)
        };
        match end
            .and_then(|ix| parts.get_mut(ix))
            .filter(|_| *axis == side.axis())
        {
            Some((had, _)) => {
                let taken = *had * share;
                *had -= taken;
                parts.insert(at, (taken, new));
            }
            None => self.wrap(side, new, share),
        }
    }

    /// Where the part at `side` of the whole room goes, the room being divided along it, which
    /// a tile put along that side takes its room from.
    fn end(&self, side: Side) -> Option<Area> {
        let Tiling::Divided { axis, parts } = self else {
            return None;
        };
        let ix = match side.after() {
            true => parts.len().checked_sub(1)?,
            false => 0,
        };
        (*axis == side.axis()).then(|| self.area_at(&[ix], Area::WHOLE))
    }

    /// Divide all the room along `side`'s axis, `new` taking `share` of it on that side.
    fn wrap(&mut self, side: Side, new: Tiling, share: f32) {
        let old = std::mem::take(self);
        let parts = match side.after() {
            true => vec![(1. - share, old), (share, new)],
            false => vec![(share, new), (1. - share, old)],
        };
        *self = Tiling::Divided {
            axis: side.axis(),
            parts,
        };
    }

    /// The tiles drawn in text, `columns` by `rows` characters, each boxed with what `label`
    /// calls it in its middle and its share of the room's width and height under that: to paste
    /// where a picture doesn't go.
    pub fn diagram(&self, label: impl Fn(&Tile) -> String, columns: usize, rows: usize) -> String {
        let mut grid = vec![vec![' '; columns + 1]; rows + 1];
        // Where lines cross, or a box's corner sits on another's side, a corner.
        let mut draw = |x: usize, y: usize, line: char| {
            if let Some(cell) = grid.get_mut(y).and_then(|row| row.get_mut(x)) {
                *cell = match (*cell, line) {
                    (' ', line) => line,
                    (was, line) if was == line => line,
                    _ => '+',
                };
            }
        };
        let at = |share: f32, of: usize| (share * of as f32).round() as usize;
        let boxes: Vec<_> = (self.areas().into_iter())
            .map(|(tile, area)| {
                let (left, top) = (at(area.left, columns), at(area.top, rows));
                let right = at(area.left + area.width, columns);
                let bottom = at(area.top + area.height, rows);
                (tile, area, left, top, right, bottom)
            })
            .collect();
        for (_, _, left, top, right, bottom) in &boxes {
            for x in *left..=*right {
                draw(x, *top, '-');
                draw(x, *bottom, '-');
            }
            for y in *top..=*bottom {
                draw(*left, y, '|');
                draw(*right, y, '|');
            }
            for (x, y) in [(left, top), (right, top), (left, bottom), (right, bottom)] {
                draw(*x, *y, '+');
            }
        }
        for (tile, area, left, top, right, bottom) in &boxes {
            let room = right.saturating_sub(*left + 1);
            let share = |share: f32| (share * 100.).round();
            let size = format!("{}% x {}%", share(area.width), share(area.height));
            let middle = (top + bottom) / 2;
            for (y, text) in [(middle, label(tile)), (middle + 1, size)] {
                let Some(row) = grid.get_mut(y).filter(|_| y > *top && y < *bottom) else {
                    continue;
                };
                let text: Vec<char> = text.chars().take(room).collect();
                let from = left + 1 + (room - text.len()) / 2;
                for (cell, c) in row.iter_mut().skip(from).zip(text) {
                    *cell = c;
                }
            }
        }
        (grid.into_iter())
            .map(|row| row.into_iter().collect::<String>().trim_end().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Store {
    /// `selection`'s row, as its tiling is kept by.
    pub fn row_key(&self, selection: &Selection) -> RowKey {
        if let Selection::Commit(project, id) = selection
            && let Some((_, commit)) = self.workspace(project).and_then(|ws| ws.find_commit(*id))
        {
            return RowKey::Commit(project.clone(), commit.key.clone());
        }
        RowKey::Row(selection.clone())
    }

    /// The page `row` shows: a commit found by its change ID, as an agent may have rewritten
    /// it, while the workspace has it.
    pub fn selection_of(&self, row: &RowKey) -> Option<Selection> {
        match row {
            RowKey::Row(selection) => Some(selection.clone()),
            RowKey::Commit(project, key) => {
                let workspace = self.workspace(project)?;
                let commit = (workspace.all_branches())
                    .flat_map(|branch| &branch.commits)
                    .find(|commit| commit.key == *key)?;
                Some(Selection::Commit(project.clone(), commit.id))
            }
        }
    }

    /// How the selected row lays out the detail, while it has tiles pinned beside its page.
    pub fn tiling(&self) -> Option<&Tiling> {
        // Before looking a commit's change ID up, as the detail asks for it as it draws.
        if self.tilings.is_empty() {
            return None;
        }
        let selection = self.selection.as_ref()?;
        self.tilings.get(&self.row_key(selection))
    }

    /// Each tile of the selected row's tiling and where it's drawn in this frame: partway while
    /// a tile slides open or shut or is dragged, else where it is.
    pub fn drawn_areas(&self) -> Option<Vec<(Tile, Area)>> {
        match &self.drawn {
            Some(drawn) => Some(drawn.areas.clone()),
            None => self.tiling().map(Tiling::areas),
        }
    }

    /// Where the home tile of the selected row's tiling is drawn in this frame, while it has
    /// one.
    pub fn drawn_home(&self) -> Option<Area> {
        match &self.drawn {
            Some(drawn) => (drawn.areas.iter())
                .find(|(tile, _)| *tile == Tile::Home)
                .map(|(_, area)| *area),
            None => self.tiling().and_then(Tiling::home),
        }
    }

    /// The lines between the selected row's tiles, where they're drawn in this frame: none
    /// while a tile's dragged.
    pub fn drawn_dividers(&self) -> Vec<Divider> {
        match &self.drawn {
            Some(drawn) => drawn.dividers.clone(),
            None => self.tiling().map(Tiling::dividers).unwrap_or_default(),
        }
    }

    /// The tile of the selected row being dragged by its bar, drawn over the others.
    pub fn lifted_tile(&self) -> Option<&Tile> {
        self.drawn.as_ref()?.lifted.as_ref()
    }

    /// The selected row's tiling at rest, with the tile sliding open or shut in it, or as the
    /// tile dragged would land: where each tile's page lays out as it moves.
    pub fn resting_tiling(&self) -> Option<&Tiling> {
        match &self.drawn {
            Some(drawn) => Some(&drawn.rest),
            None => self.tiling(),
        }
    }

    /// Where `tile`'s page lays out: where it's at rest, which it keeps as it slides, but as
    /// it's drawn while it's zoomed.
    pub fn page_area(&self, tile: &Tile) -> Option<Area> {
        let zoomed = (self.drawn.as_ref())
            .filter(|drawn| drawn.zoomed && drawn.lifted.as_ref() == Some(tile));
        match zoomed {
            Some(drawn) => (drawn.areas.iter())
                .find(|(other, _)| other == tile)
                .map(|(_, area)| *area),
            None => self.resting_tiling()?.area_of(tile),
        }
    }

    /// Draw the selected row's tiling as `drawn` while its tiles move, until it's set back to
    /// `None`. It draws nothing again: the app view sets it as it draws. Whether it changed.
    pub fn set_drawn(&mut self, drawn: Option<Drawn>) -> bool {
        let changed = self.drawn != drawn;
        self.drawn = drawn;
        changed
    }

    /// Open a terminal in a tile pinned beside `at` in the selected row's tiling, along
    /// `axis`, once its shell has started: on the checkout the selection is in, the main one
    /// when it's in none.
    pub fn pin_terminal(&mut self, at: Tile, axis: Axis, cx: &mut Context<Self>) {
        let Some(selection) = self.selection.clone() else {
            return;
        };
        let checkout = self.checkout_of(&selection).unwrap_or(Checkout::Main);
        let row = self.row_key(&selection);
        let project = selection.project().clone();
        self.open_terminal_then(project, checkout, Some((row, at, axis)), cx);
    }

    /// Pin `pinned` beside `at` in the selected row's tiling, along `axis`, unless it's
    /// there already.
    pub fn pin(&mut self, at: Tile, axis: Axis, pinned: RowKey, cx: &mut Context<Self>) {
        let Some(selection) = &self.selection else {
            return;
        };
        let row = self.row_key(selection);
        self.pin_into((row, at, axis), pinned, cx);
    }

    /// Pin `pinned` beside `at` in `row`'s tiling, along `axis`, unless `at` is gone
    /// meanwhile, as a terminal is once its shell exits, or `pinned` is there already. Whether
    /// it did.
    pub(super) fn pin_into(
        &mut self,
        (row, at, axis): (RowKey, Tile, Axis),
        pinned: RowKey,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut tiling = self.tilings.get(&row).cloned().unwrap_or_default();
        let tiles: Vec<Tile> = tiling.areas().into_iter().map(|(tile, _)| tile).collect();
        let pinned_there = tiles.contains(&Tile::Pinned(pinned.clone()));
        if !tiles.contains(&at) || pinned_there || pinned == row {
            return false;
        }
        tiling.pin(&at, axis, pinned.clone());
        self.tilings.insert(row, tiling);
        self.show_terminals(cx);
        cx.emit(StoreEvent::TilingChanged);
        cx.emit(StoreEvent::TilePinned(pinned));
        cx.notify();
        true
    }

    /// Move a divider of the selected row's tiling, as [`Tiling::move_divider`] does.
    pub fn move_divider(
        &mut self,
        path: &[usize],
        after: usize,
        at: f32,
        min: f32,
        cx: &mut Context<Self>,
    ) {
        self.change_tiling(cx, |tiling| tiling.move_divider(path, after, at, min));
    }

    /// `tiling`, the selected row's, with a new tile for `pinned` along `side`, as a row dragged
    /// there lands. Not where it's on show already: as the selected row's own page, its home
    /// tile, or in a tile of its own, which moves only by its bar.
    pub fn tiling_with(
        &self,
        tiling: Option<&Tiling>,
        pinned: &RowKey,
        side: Side,
    ) -> Option<Tiling> {
        if *pinned == self.row_key(self.selection.as_ref()?) {
            return None;
        }
        tiling
            .cloned()
            .unwrap_or_default()
            .pinned_along(side, pinned.clone())
    }

    /// Lay `row`'s tiles out as `tiling`, or with `None` leave it alone again, as a tile tried
    /// out while a row is held over a side of the tiles comes and goes.
    pub fn try_tiling(&mut self, row: RowKey, tiling: Option<Tiling>, cx: &mut Context<Self>) {
        match tiling {
            Some(tiling) => self.tilings.insert(row, tiling),
            None => self.tilings.remove(&row),
        };
        self.show_terminals(cx);
        cx.emit(StoreEvent::TilingChanged);
        cx.notify();
    }

    /// The tile a row dragged in was let go as is there to stay, to use.
    pub fn row_landed(&mut self, pinned: RowKey, cx: &mut Context<Self>) {
        cx.emit(StoreEvent::TilePinned(pinned));
    }

    /// A row's drag ended, let go anywhere or stopped with Escape.
    pub(crate) fn row_drag_ended(&mut self, cx: &mut Context<Self>) {
        self.dragged_row = None;
        self.tile_zone = None;
        cx.emit(StoreEvent::RowDragEnded);
        cx.notify();
    }

    /// Lay the selected row's tiles out as `tiling`, as a tile dragged by its bar lands.
    pub fn land_tiling(&mut self, tiling: Tiling, cx: &mut Context<Self>) {
        self.change_tiling(cx, |was| *was = tiling);
    }

    /// Give each part of a division of the selected row's tiling the same room.
    pub fn even_out_tiles(&mut self, path: &[usize], cx: &mut Context<Self>) {
        self.change_tiling(cx, |tiling| tiling.even_out(path));
    }

    fn change_tiling(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Tiling)) {
        let Some(selection) = &self.selection else {
            return;
        };
        let row = self.row_key(selection);
        let Some(tiling) = self.tilings.get_mut(&row) else {
            return;
        };
        let before = tiling.clone();
        change(tiling);
        if *tiling != before {
            cx.emit(StoreEvent::TilingChanged);
            cx.notify();
        }
    }

    /// The tiles of `selection`'s tiling and where they go, in the order they're laid out.
    pub fn tiles_of(&self, selection: &Selection) -> Vec<(Tile, Area)> {
        // Before looking a commit's change ID up, as the navigator asks for each row it draws.
        if self.tilings.is_empty() {
            return Vec::new();
        }
        (self.tilings.get(&self.row_key(selection)).iter())
            .flat_map(|tiling| tiling.areas())
            .collect()
    }

    /// What's pinned in the selected row's tiling.
    pub fn pinned(&self) -> Vec<RowKey> {
        (self.tiling().iter().flat_map(|tiling| tiling.areas()))
            .filter_map(|(tile, _)| match tile {
                Tile::Pinned(pinned) => Some(pinned),
                Tile::Home => None,
            })
            .collect()
    }

    /// Note that `tile` of the selected row's tiling has the keys, for
    /// [`Self::last_tile_with_keys`].
    pub fn note_tile_with_keys(&mut self, tile: &Tile) {
        let Some(selection) = &self.selection else {
            return;
        };
        let row = self.row_key(selection);
        let shown = match tile {
            Tile::Home => row.clone(),
            Tile::Pinned(pinned) => pinned.clone(),
        };
        let recent = self.tiles_with_keys.entry(row).or_default();
        if recent.last() != Some(&shown) {
            recent.retain(|other| *other != shown);
            recent.push(shown);
        }
    }

    /// The tile of the selected row's tiling that had the keys last, of those it has.
    pub fn last_tile_with_keys(&self) -> Option<Tile> {
        let tiling = self.tiling()?;
        let row = self.row_key(self.selection.as_ref()?);
        let recent = self.tiles_with_keys.get(&row)?;
        (recent.iter().rev())
            .map(|shown| tile_showing(&row, shown))
            .find(|tile| tiling.has(tile))
    }

    /// Whether the tile of the selected row that had the keys last is gone, which it forgets
    /// then, so keys lost later go where they would without it.
    pub fn forget_gone_tile_with_keys(&mut self) -> bool {
        let Some(selection) = &self.selection else {
            return false;
        };
        let row = self.row_key(selection);
        let last = (self.tiles_with_keys.get(&row)).and_then(|recent| recent.last());
        let gone = (last.map(|last| tile_showing(&row, last))).is_some_and(|tile| {
            tile != Tile::Home && self.tiling().is_none_or(|tiling| !tiling.has(&tile))
        });
        if gone && let Some(recent) = self.tiles_with_keys.get_mut(&row) {
            recent.pop();
        }
        gone
    }

    /// The terminals pinned in the selected row's tiling, in the order its tiles are laid out,
    /// but for `closing`: what could take the home tile's place, while it's a terminal.
    pub fn home_heirs(&self, closing: &[TerminalId]) -> Vec<RowKey> {
        if !matches!(self.selection, Some(Selection::Terminal(..))) {
            return Vec::new();
        }
        (self.pinned().into_iter())
            .filter(|pinned| {
                matches!(pinned, RowKey::Row(Selection::Terminal(_, id)) if !closing.contains(id))
            })
            .collect()
    }

    /// Take the tile showing `pinned` away from the selected row's tiling, the tiles beside it
    /// growing into its room. What it showed stays open, as a terminal does in the navigator,
    /// but for a terminal opened in the tile, which closes with it.
    pub fn close_tile(&mut self, pinned: &RowKey, cx: &mut Context<Self>) {
        if let RowKey::Row(Selection::Terminal(_, id)) = pinned
            && self.terminals.tiled(*id)
        {
            return self.close_terminal(*id, cx);
        }
        let Some(selection) = &self.selection else {
            return;
        };
        let row = self.row_key(selection);
        let Some(tiling) = self.tilings.get_mut(&row) else {
            return;
        };
        let before = tiling.clone();
        tiling.retain(&|tile| tile != pinned);
        if tiling.is_home_only() {
            self.tilings.remove(&row);
        }
        self.show_terminals(cx);
        // Before the tiling changes, so the app view keeps what it drew there to draw it going.
        cx.emit(StoreEvent::TileClosed {
            before,
            tile: pinned.clone(),
            terminal: None,
        });
        cx.emit(StoreEvent::TilingChanged);
        cx.notify();
    }

    /// Take away the tiles of terminals `closed`, and the tilings of their rows.
    pub(super) fn unpin_terminals(
        &mut self,
        closed: &HashMap<TerminalId, Entity<Terminal>>,
        cx: &mut Context<Self>,
    ) {
        let ids: Vec<TerminalId> = closed.keys().copied().collect();
        let ids = ids.as_slice();
        let closed_tile = |pinned: &RowKey| matches!(pinned, RowKey::Row(Selection::Terminal(_, id)) if ids.contains(id));
        let before = self.tilings.clone();
        self.tilings.retain(|row, tiling| {
            if let RowKey::Row(Selection::Terminal(_, id)) = row
                && ids.contains(id)
            {
                return false;
            }
            tiling.retain(&|pinned| !closed_tile(pinned));
            !tiling.is_home_only()
        });
        if self.tilings == before {
            return;
        }
        // One of the selected row's tiles going, drawn going as its tile's closed by hand is.
        let row = (self.selection.as_ref()).map(|selection| self.row_key(selection));
        let shown = row.and_then(|row| before.get(&row));
        let gone = (shown.into_iter().flat_map(Tiling::areas)).find_map(|(tile, _)| match tile {
            Tile::Pinned(pinned @ RowKey::Row(Selection::Terminal(_, id))) => {
                Some((pinned, closed.get(&id)?.clone()))
            }
            _ => None,
        });
        if let (Some(before), Some((tile, terminal))) = (shown, gone) {
            cx.emit(StoreEvent::TileClosed {
                before: before.clone(),
                tile,
                terminal: Some(terminal),
            });
        }
        cx.emit(StoreEvent::TilingChanged);
    }
}

/// The tile of `row`'s tiling that shows `shown`: its home tile, showing the row itself.
fn tile_showing(row: &RowKey, shown: &RowKey) -> Tile {
    match shown == row {
        true => Tile::Home,
        false => Tile::Pinned(shown.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal(n: u64) -> RowKey {
        RowKey::Row(Selection::Terminal("project".into(), TerminalId(n)))
    }

    fn area(left: f32, top: f32, width: f32, height: f32) -> Area {
        Area {
            left,
            top,
            width,
            height,
        }
    }

    #[test]
    fn pinning_beside_the_home_tile_halves_its_room() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        assert_eq!(
            tiling.areas(),
            vec![
                (Tile::Home, area(0., 0., 0.5, 1.)),
                (Tile::Pinned(terminal(1)), area(0.5, 0., 0.5, 1.)),
            ],
            "the pinned tile takes the half after the home tile"
        );
    }

    #[test]
    fn pinning_along_a_division_shares_its_room_evenly() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Home, Axis::Across, terminal(2));
        let widths: Vec<f32> = (tiling.areas().iter())
            .map(|(_, area)| (area.width * 300.).round())
            .collect();
        assert_eq!(
            widths,
            vec![100., 100., 100.],
            "three across take a third each"
        );
        let tiles: Vec<Tile> = tiling.areas().into_iter().map(|(tile, _)| tile).collect();
        assert_eq!(
            tiles,
            vec![
                Tile::Home,
                Tile::Pinned(terminal(2)),
                Tile::Pinned(terminal(1))
            ],
            "the newer one goes right after the tile it was pinned beside"
        );
    }

    #[test]
    fn pinning_across_a_division_divides_that_tile_alone() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        assert_eq!(
            tiling.areas(),
            vec![
                (Tile::Home, area(0., 0., 0.5, 1.)),
                (Tile::Pinned(terminal(1)), area(0.5, 0., 0.5, 0.5)),
                (Tile::Pinned(terminal(2)), area(0.5, 0.5, 0.5, 0.5)),
            ],
            "the terminal's half is divided top and bottom, the home tile's isn't"
        );
    }

    #[test]
    fn closing_a_tile_gives_its_room_back_and_folds_divisions_of_one() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        tiling.retain(&|pinned| *pinned != terminal(2));
        assert_eq!(
            tiling.areas(),
            vec![
                (Tile::Home, area(0., 0., 0.5, 1.)),
                (Tile::Pinned(terminal(1)), area(0.5, 0., 0.5, 1.)),
            ],
            "the terminal above takes the room the closed one had"
        );
        tiling.retain(&|pinned| *pinned != terminal(1));
        assert!(
            tiling.is_home_only(),
            "with nothing pinned, the home tile is all there is"
        );
    }

    #[test]
    fn promoting_a_tile_closes_the_home_one_and_keeps_it_in_place() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        tiling.promote(&terminal(1));
        assert_eq!(
            tiling.areas(),
            vec![
                (Tile::Home, area(0., 0., 1., 0.5)),
                (Tile::Pinned(terminal(2)), area(0., 0.5, 1., 0.5)),
            ],
            "the terminal above is the home tile where it was, the column taking the old one's room"
        );
        tiling.promote(&terminal(2));
        assert_eq!(
            tiling,
            Tiling::default(),
            "the last terminal promoted is the home tile alone"
        );
    }

    #[test]
    fn before_promoting_is_the_tiling_with_the_closing_tile_still_in_it() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let before = tiling.before_promoting(&terminal(9), &terminal(1));
        assert_eq!(
            rounded(&before),
            vec![
                (Tile::Pinned(terminal(9)), [0, 0, 50, 100]),
                (Tile::Home, [50, 0, 50, 50]),
                (Tile::Pinned(terminal(2)), [50, 50, 50, 50]),
            ],
            "the tile closing is where the home tile was, the heir at home where it is"
        );
        let mut promoted = tiling.clone();
        promoted.promote(&terminal(1));
        let mut shut = before;
        shut.retain(&|pinned| *pinned != terminal(9));
        assert_eq!(shut, promoted, "shut, it's the tiling promoted");
    }

    #[test]
    fn a_tiling_moves_toward_another_by_the_shares_of_its_parts() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        let mut to = tiling.clone();
        tiling.move_divider(&[], 0, 0.2, 0.1);
        to.move_divider(&[], 0, 0.6, 0.1);
        assert_eq!(
            rounded(&tiling.toward(&to, 0.5)),
            vec![
                (Tile::Home, [0, 0, 40, 100]),
                (Tile::Pinned(terminal(1)), [40, 0, 60, 100]),
            ],
            "halfway, the line's halfway"
        );
    }

    #[test]
    fn an_opening_tile_takes_its_share_from_the_parts_beside_it() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Across, terminal(2));
        let widths = |tiling: &Tiling| -> Vec<f32> {
            (tiling.areas().iter())
                .map(|(_, area)| (area.width * 1000.).round() / 10.)
                .collect()
        };
        assert_eq!(
            widths(&tiling.opening(&terminal(2), 0.)),
            vec![50., 50., 0.],
            "shut, the others have the room they had before it opened"
        );
        assert_eq!(
            widths(&tiling.opening(&terminal(2), 0.5)),
            vec![41.7, 41.7, 16.7],
            "halfway, half its share, the others shrinking alike"
        );
        assert_eq!(
            tiling.opening(&terminal(2), 1.),
            tiling,
            "open, it's the tiling as it is"
        );
    }

    #[test]
    fn dividers_sit_between_the_parts_of_each_division() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let dividers: Vec<(Vec<usize>, usize, Axis, f32)> = (tiling.dividers().into_iter())
            .map(|divider| (divider.path, divider.after, divider.axis, divider.at))
            .collect();
        assert_eq!(
            dividers,
            vec![
                (vec![], 0, Axis::Across, 0.5),
                (vec![1], 0, Axis::Down, 0.5)
            ],
            "one down the middle, and one across the terminals' half"
        );
    }

    #[test]
    fn moving_a_divider_keeps_both_sides_at_least_the_least_they_take() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.move_divider(&[], 0, 0.7, 0.1);
        let widths = |tiling: &Tiling| -> Vec<f32> {
            (tiling.areas().iter())
                .map(|(_, area)| (area.width * 100.).round())
                .collect()
        };
        assert_eq!(
            widths(&tiling),
            vec![70., 30.],
            "the line goes where it's dragged"
        );
        tiling.move_divider(&[], 0, 0.99, 0.1);
        assert_eq!(
            widths(&tiling),
            vec![90., 10.],
            "the terminal keeps the least it takes"
        );
        tiling.even_out(&[]);
        assert_eq!(
            widths(&tiling),
            vec![50., 50.],
            "evened out, they share the room"
        );
    }

    #[test]
    fn closing_one_of_three_shares_its_room_by_the_others_shares() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Across, terminal(2));
        tiling.retain(&|pinned| *pinned != terminal(1));
        let widths: Vec<f32> = (tiling.areas().iter())
            .map(|(_, area)| (area.width * 100.).round())
            .collect();
        assert_eq!(widths, vec![50., 50.], "the two left take half each");
    }

    /// Each tile's area, in hundredths of the room, to compare without float noise.
    fn rounded(tiling: &Tiling) -> Vec<(Tile, [i32; 4])> {
        let r = |x: f32| (x * 100.).round() as i32;
        (tiling.areas().into_iter())
            .map(|(tile, a)| (tile, [r(a.left), r(a.top), r(a.width), r(a.height)]))
            .collect()
    }

    const LEAST: (f32, f32) = (0.1, 0.1);

    const ZONES: Zones = Zones {
        edge: (0.03, 0.04),
        right: 0.03,
        bar: 0.06,
    };

    #[test]
    fn moving_a_tile_along_its_division_keeps_every_tiles_room() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Across, terminal(2));
        tiling.move_divider(&[], 0, 0.5, 0.1);
        tiling.move_divider(&[], 1, 0.8, 0.1);
        let landing = Landing::Beside(Tile::Home, Side::Right);
        assert_eq!(
            rounded(&tiling.landed(&Tile::Pinned(terminal(2)), &landing, LEAST)),
            vec![
                (Tile::Home, [0, 0, 50, 100]),
                (Tile::Pinned(terminal(2)), [50, 0, 20, 100]),
                (Tile::Pinned(terminal(1)), [70, 0, 30, 100]),
            ],
            "the last one goes between the others, all as wide as they were"
        );
        let landing = Landing::Edge(Side::Left);
        assert_eq!(
            rounded(&tiling.landed(&Tile::Pinned(terminal(2)), &landing, LEAST)),
            vec![
                (Tile::Pinned(terminal(2)), [0, 0, 20, 100]),
                (Tile::Home, [20, 0, 50, 100]),
                (Tile::Pinned(terminal(1)), [70, 0, 30, 100]),
            ],
            "along the room's left edge, it goes first, as wide as it was"
        );
    }

    #[test]
    fn a_row_dragged_in_takes_half_of_what_is_next_to_it_and_no_more() {
        let tile = |n| Tile::Pinned(terminal(n));
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&tile(1), Axis::Across, terminal(2));
        let landed =
            (tiling.pinned_along(Side::Right, terminal(3))).expect("terminal 3 has no tile yet");
        let width = |tiling: &Tiling, tile: &Tile| tiling.area_of(tile).map(|area| area.width);
        assert_eq!(
            (width(&landed, &Tile::Home), width(&landed, &tile(1))),
            (width(&tiling, &Tile::Home), width(&tiling, &tile(1))),
            "tiles not next to the zone keep their size"
        );
        let (was, now) = (width(&tiling, &tile(2)), width(&landed, &tile(2)));
        assert_eq!(
            now.map(|width| width * 2.),
            was,
            "the tile next to the zone gives up half"
        );
        assert_eq!(
            width(&landed, &tile(3)),
            now,
            "the new tile has what it gave up"
        );
    }

    #[test]
    fn a_row_with_a_tile_already_has_nowhere_to_be_dragged_in() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        for side in [Side::Left, Side::Right] {
            assert_eq!(
                tiling.pinned_along(side, terminal(1)),
                None,
                "its tile stays where it is, and moves only by its bar"
            );
        }
    }

    #[test]
    fn evening_out_every_division_reaches_those_inside_others() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        tiling.pin(&Tile::Pinned(terminal(2)), Axis::Across, terminal(3));
        assert_eq!(
            tiling.divisions(),
            vec![vec![], vec![1], vec![1, 1]],
            "the whole room, the column, and the row at its bottom"
        );
        tiling.move_divider(&[], 0, 0.2, 0.);
        tiling.move_divider(&[1], 0, 0.8, 0.);
        tiling.move_divider(&[1, 1], 0, 0.1, 0.);
        let mut evened = tiling.clone();
        for path in evened.divisions() {
            evened.even_out(&path);
        }
        let width = |tile: Tile| evened.area_of(&tile).map(|area| area.width);
        assert_eq!(
            (width(Tile::Home), width(Tile::Pinned(terminal(1)))),
            (Some(0.5), Some(0.5)),
            "each part of the whole room has the same"
        );
        assert_eq!(
            width(Tile::Pinned(terminal(2))),
            width(Tile::Pinned(terminal(3))),
            "and so do the parts of a division inside it"
        );
    }

    #[test]
    fn a_zoomed_tile_grows_over_the_others_which_stay_where_they_are() {
        let tile = Tile::Pinned(terminal(2));
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Across, terminal(2));
        let (areas, zoomed) = (tiling.areas(), tiling.zoomed(&tile, 0.5));
        assert_eq!(
            zoomed.last().map(|(top, _)| top),
            Some(&tile),
            "it's drawn last, over the others"
        );
        for (other, area) in zoomed.iter().filter(|(other, _)| *other != tile) {
            assert!(
                areas.contains(&(other.clone(), *area)),
                "{other:?} neither moves nor shrinks"
            );
        }
        let was = tiling.area_of(&tile).unwrap_or(Area::WHOLE);
        assert_eq!(
            zoomed.last().map(|(_, area)| *area),
            Some(was.toward(Area::WHOLE, 0.5)),
            "halfway, it's halfway to all the room"
        );
        assert_eq!(
            tiling.zoomed(&tile, 1.),
            vec![
                (Tile::Home, tiling.home().unwrap_or(Area::WHOLE)),
                (tile.clone(), Area::WHOLE)
            ],
            "there, only the home tile stays under it, where it was"
        );
    }

    #[test]
    fn a_row_dragged_beside_stacked_tiles_is_as_tall_as_all_of_them() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Down, terminal(1));
        let landed =
            (tiling.pinned_along(Side::Left, terminal(2))).expect("terminal 2 has no tile yet");
        assert_eq!(
            landed.area_of(&Tile::Pinned(terminal(2))),
            Some(area(0., 0., 0.5, 1.)),
            "a whole column down the left, beside both"
        );
    }

    #[test]
    fn dragging_a_tile_below_the_one_beside_it_stacks_them() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        let landing = Landing::Beside(Tile::Home, Side::Bottom);
        assert_eq!(
            rounded(&tiling.landed(&Tile::Pinned(terminal(1)), &landing, LEAST)),
            vec![
                (Tile::Home, [0, 0, 100, 50]),
                (Tile::Pinned(terminal(1)), [0, 50, 100, 50]),
            ],
            "as tall as the room before, it takes half of the home tile's, the most it takes"
        );
        let landing = Landing::Beside(Tile::Pinned(terminal(1)), Side::Left);
        assert_eq!(
            rounded(&tiling.landed(&Tile::Home, &landing, LEAST)),
            vec![
                (Tile::Home, [0, 0, 50, 100]),
                (Tile::Pinned(terminal(1)), [50, 0, 50, 100]),
            ],
            "the home tile dragged back where it is changes nothing"
        );
    }

    #[test]
    fn a_tile_moved_elsewhere_keeps_its_size_from_the_tile_it_lands_beside() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        tiling.move_divider(&[1], 0, 0.7, 0.1);
        let landing = Landing::Beside(Tile::Home, Side::Bottom);
        assert_eq!(
            rounded(&tiling.landed(&Tile::Pinned(terminal(2)), &landing, LEAST)),
            vec![
                (Tile::Home, [0, 0, 50, 70]),
                (Tile::Pinned(terminal(2)), [0, 70, 50, 30]),
                (Tile::Pinned(terminal(1)), [50, 0, 50, 100]),
            ],
            "it keeps its height under the home tile, and the terminal it left grows into its room"
        );
        let landing = Landing::Beside(Tile::Pinned(terminal(1)), Side::Left);
        let landed = tiling.landed(&Tile::Pinned(terminal(2)), &landing, (0.3, 0.1));
        assert_eq!(
            rounded(&landed),
            vec![
                (Tile::Home, [0, 0, 50, 100]),
                (Tile::Pinned(terminal(2)), [50, 0, 25, 100]),
                (Tile::Pinned(terminal(1)), [75, 0, 25, 100]),
            ],
            "half as wide as the room, it takes half the terminal's, as much as it takes, \
             no more than the least"
        );
    }

    #[test]
    fn a_tile_along_the_rooms_edge_runs_all_of_it() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let landing = Landing::Edge(Side::Bottom);
        assert_eq!(
            rounded(&tiling.landed(&Tile::Pinned(terminal(2)), &landing, LEAST)),
            vec![
                (Tile::Home, [0, 0, 50, 50]),
                (Tile::Pinned(terminal(1)), [50, 0, 50, 50]),
                (Tile::Pinned(terminal(2)), [0, 50, 100, 50]),
            ],
            "it's as wide as the room, the two left side by side above it"
        );
    }

    #[test]
    fn along_a_side_takes_room_from_the_part_at_that_end_as_beside_it_does() {
        // The home tile above a terminal, another dragged in below.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Down, terminal(1));
        tiling.pin(&Tile::Home, Axis::Across, terminal(2));
        let dragged = Tile::Pinned(terminal(2));
        let below = Landing::Beside(Tile::Pinned(terminal(1)), Side::Bottom);
        let along = Landing::Edge(Side::Bottom);
        assert_eq!(
            tiling.landed(&dragged, &along, LEAST),
            tiling.landed(&dragged, &below, LEAST),
            "along the bottom is below the terminal at the bottom, as big"
        );
        assert_eq!(
            rounded(&tiling.landed(&dragged, &along, LEAST)),
            vec![
                (Tile::Home, [0, 0, 100, 50]),
                (Tile::Pinned(terminal(1)), [0, 50, 100, 25]),
                (Tile::Pinned(terminal(2)), [0, 75, 100, 25]),
            ],
            "the home tile keeps its room, the terminal gives half of its"
        );
    }

    #[test]
    fn a_dragged_tile_lands_by_where_its_bar_is_over_the_tiles() {
        // Three side by side, the middle one dragged by its bar.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(2));
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        let dragged = Tile::Pinned(terminal(1));
        // Held a tenth of the room in from its left, along its top.
        let at = |x: f32, y: f32| tiling.landing_at(&dragged, (x, y), ZONES);
        assert_eq!(
            at(0.5, 0.3),
            Some(Landing::Stay),
            "over its own place, it stays"
        );
        assert_eq!(
            at(0.5, 0.6),
            Some(Landing::BelowRow),
            "dragged down into the bottom half of it, it leaves the row for below it"
        );
        assert_eq!(
            at(0.1, 0.02),
            Some(Landing::Beside(Tile::Home, Side::Left)),
            "on the left half of the home tile's bar, before it"
        );
        assert_eq!(
            at(0.25, 0.02),
            Some(Landing::Beside(Tile::Home, Side::Left)),
            "all along its bar, as it comes from its right"
        );
        assert_eq!(
            at(0.15, 0.3),
            Some(Landing::Beside(Tile::Home, Side::Top)),
            "its bar in the upper half of the home tile, above it"
        );
        assert_eq!(
            at(0.15, 0.8),
            Some(Landing::Beside(Tile::Home, Side::Bottom)),
            "in the lower half, below it, though it hangs off the bottom"
        );
        assert_eq!(
            at(0.72, 0.9),
            Some(Landing::Beside(Tile::Pinned(terminal(2)), Side::Bottom)),
            "likewise below the tile on the right"
        );
        assert_eq!(
            at(0.5, 0.98),
            Some(Landing::Edge(Side::Bottom)),
            "with the pointer near the room's bottom, along all of it"
        );
        assert_eq!(
            at(0.5, -0.01),
            Some(Landing::Edge(Side::Top)),
            "past its top, where the bars are, along that"
        );
    }

    #[test]
    fn a_diagram_boxes_each_tile_with_its_name_and_size() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let label = |tile: &Tile| match tile {
            Tile::Home => "htop".to_owned(),
            Tile::Pinned(pinned) if *pinned == terminal(1) => "nvim".to_owned(),
            Tile::Pinned(_) => "shell".to_owned(),
        };
        // Side by side, the right one divided top and bottom, each named and sized.
        snapbox::assert_data_eq!(
            tiling.diagram(label, 40, 8),
            snapbox::str![[r#"
+-------------------+-------------------+
|                   |                   |
|                   |       nvim        |
|                   |     50% x 50%     |
|       htop        +-------------------+
|    50% x 100%     |                   |
|                   |       shell       |
|                   |     50% x 50%     |
+-------------------+-------------------+
"#]]
        );
    }

    #[test]
    fn a_tile_dragged_down_its_own_place_leaves_its_row_for_below_it() {
        // The home tile above a row of two terminals.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Down, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Across, terminal(2));
        let dragged = Tile::Pinned(terminal(2));
        let at = |y: f32| tiling.landing_at(&dragged, (0.75, y), ZONES);
        assert_eq!(
            at(0.6),
            Some(Landing::Stay),
            "in the top of its place, it stays"
        );
        assert_eq!(
            at(0.85),
            Some(Landing::BelowRow),
            "in the bottom of it, it leaves its row"
        );
        assert_eq!(
            rounded(&tiling.landed(&dragged, &Landing::BelowRow, LEAST)),
            vec![
                (Tile::Home, [0, 0, 100, 50]),
                (Tile::Pinned(terminal(1)), [0, 50, 100, 25]),
                (Tile::Pinned(terminal(2)), [0, 75, 100, 25]),
            ],
            "below the other terminal, with half the row's room, the home tile as it was"
        );
        let mut column = Tiling::default();
        column.pin(&Tile::Home, Axis::Down, terminal(1));
        assert_eq!(
            column.landing_at(&Tile::Pinned(terminal(1)), (0.5, 0.9), ZONES),
            Some(Landing::Stay),
            "in a column, in no row, it stays"
        );
    }

    #[test]
    fn over_another_tile_its_bar_is_its_left_or_right_and_the_rest_above_or_below_it() {
        // Layouts made by pinning and dragging, from a fixed seed.
        let mut seed: u64 = 7;
        let mut next = move |n: usize| {
            seed = (seed.wrapping_mul(6364136223846793005)).wrapping_add(1442695040888963407);
            ((seed >> 33) as usize) % n.max(1)
        };
        let least = (0.16, 0.2);
        for layout in 0..150 {
            let mut tiling = Tiling::default();
            for step in 0..(2 + next(5)) {
                let tiles: Vec<Tile> = tiling.areas().into_iter().map(|(t, _)| t).collect();
                let Some(at) = tiles.get(next(tiles.len())).cloned() else {
                    continue;
                };
                if step < 3 || next(2) == 0 {
                    let axis = [Axis::Across, Axis::Down][next(2)];
                    tiling.pin(&at, axis, terminal(step as u64 + 1));
                } else {
                    let pointer = (next(100) as f32 / 100., next(100) as f32 / 100.);
                    if let Some(landing) = tiling.landing_at(&at, pointer, ZONES) {
                        tiling = tiling.landed(&at, &landing, least);
                    }
                }
            }
            let areas = tiling.areas();
            for (dragged, _) in &areas {
                for (x, y) in (1..30).flat_map(|x| (1..30).map(move |y| (x, y))) {
                    let (x, y) = (x as f32 / 30., y as f32 / 30.);
                    let near_side =
                        x < ZONES.edge.0 || x > 1. - ZONES.right || y > 1. - ZONES.edge.1;
                    let over = areas.iter().find(|(_, area)| area.holds((x, y)));
                    let Some((over, area)) = over.filter(|(over, _)| over != dragged && !near_side)
                    else {
                        continue;
                    };
                    let landing = tiling.landing_at(dragged, (x, y), ZONES);
                    let sides = match y < area.top + ZONES.bar {
                        true => [Side::Left, Side::Right],
                        false => [Side::Top, Side::Bottom],
                    };
                    let expected = sides.map(|side| Some(Landing::Beside(over.clone(), side)));
                    // Past the only other tile, where those only land it along the room's sides.
                    let own = tiling.area_of(dragged).unwrap_or(Area::WHOLE);
                    let past = beyond(own, *area)
                        .filter(|_| areas.len() == 2)
                        .map(|away| Some(Landing::Beside(over.clone(), away)))
                        .filter(|past| *past == landing);
                    assert!(
                        expected.contains(&landing) || past.is_some(),
                        "layout {layout}: dragging {dragged:?} at ({x}, {y}) over {over:?} lands \
                         {landing:?}: on its bar beside it, else above or below it\n{tiling:?}"
                    );
                    let moved = landing
                        .is_some_and(|landing| tiling.landed(dragged, &landing, least) != tiling);
                    assert!(
                        moved,
                        "layout {layout}: dragging {dragged:?} at ({x}, {y}) over {over:?} \
                         changes nothing\n{tiling:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn above_and_below_a_tile_part_where_it_would_be_split() {
        // Two stacked beside a tall one, the lower of the two dragged.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Home, Axis::Down, terminal(2));
        let dragged = Tile::Pinned(terminal(2));
        let tall = Tile::Pinned(terminal(1));
        let at = |y: f32| tiling.landing_at(&dragged, (0.75, y), ZONES);
        assert_eq!(
            at(0.45),
            Some(Landing::Beside(tall.clone(), Side::Top)),
            "above the middle of the tall tile, above it"
        );
        assert_eq!(
            at(0.55),
            Some(Landing::Beside(tall, Side::Bottom)),
            "below it, though the tile dragged comes from lower down"
        );
    }

    #[test]
    fn a_side_the_tile_is_along_already_is_no_zone_of_its_own() {
        // Two side by side, the right one dragged.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        let dragged = Tile::Pinned(terminal(1));
        let at = |x: f32, y: f32| tiling.landing_at(&dragged, (x, y), ZONES);
        assert_eq!(
            at(0.99, 0.8),
            Some(Landing::BelowRow),
            "in the right side's zone, it's as in the lower half of its own place"
        );
        assert_eq!(
            at(1.05, 0.8),
            Some(Landing::BelowRow),
            "and past the side, as over the place nearest"
        );
        assert_eq!(
            at(0.99, 0.3),
            Some(Landing::Stay),
            "in the upper half, it stays"
        );
    }

    #[test]
    fn beside_a_column_each_of_its_tiles_keeps_its_above_and_below() {
        // A tall tile dragged, beside two stacked.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let at = |y: f32| tiling.landing_at(&Tile::Home, (0.75, y), ZONES);
        let (top, bottom) = (Tile::Pinned(terminal(1)), Tile::Pinned(terminal(2)));
        assert_eq!(
            [at(0.15), at(0.4), at(0.6), at(0.85)],
            [
                Some(Landing::Beside(top.clone(), Side::Top)),
                Some(Landing::Beside(top, Side::Bottom)),
                Some(Landing::Beside(bottom.clone(), Side::Top)),
                Some(Landing::Beside(bottom, Side::Bottom)),
            ],
            "above the column and below it are places of their own, not past its tiles"
        );
    }

    #[test]
    fn below_its_row_beside_a_column_is_below_that_column() {
        // A tall tile dragged, beside two stacked.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let below = Landing::Beside(Tile::Pinned(terminal(2)), Side::Bottom);
        assert_eq!(
            tiling.landed(&Tile::Home, &Landing::BelowRow, LEAST),
            tiling.landed(&Tile::Home, &below, LEAST),
            "out of its row, it's below the last of the column, as big"
        );
        assert_eq!(
            tiling.landed(&Tile::Home, &Landing::BelowRow, LEAST),
            tiling.landed(&Tile::Home, &Landing::Edge(Side::Bottom), LEAST),
            "which is along the room's bottom"
        );
        assert_eq!(
            rounded(&tiling.landed(&Tile::Home, &Landing::BelowRow, LEAST)),
            vec![
                (Tile::Pinned(terminal(1)), [0, 0, 100, 50]),
                (Tile::Pinned(terminal(2)), [0, 50, 100, 25]),
                (Tile::Home, [0, 75, 100, 25]),
            ],
            "the last of the column gives up half its room, the first keeps its own"
        );
        // The same with a tile over all of it, so its row isn't the whole room.
        let mut nested = Tiling::default();
        nested.pin(&Tile::Home, Axis::Down, terminal(3));
        nested.pin(&Tile::Pinned(terminal(3)), Axis::Across, terminal(1));
        nested.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let dragged = Tile::Pinned(terminal(3));
        assert_eq!(
            nested.landed(&dragged, &Landing::BelowRow, LEAST),
            nested.landed(&dragged, &below, LEAST),
            "below the column in a row of its own, likewise"
        );
    }

    #[test]
    fn alone_beside_a_tile_the_side_it_is_on_is_no_place_to_go() {
        // A tall tile dragged, beside two stacked.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let at = |x: f32, y: f32| tiling.landing_at(&Tile::Home, (x, y), ZONES);
        let (top, bottom) = (Tile::Pinned(terminal(1)), Tile::Pinned(terminal(2)));
        assert_eq!(
            [at(0.55, 0.02), at(0.9, 0.02)],
            [
                Some(Landing::Beside(top.clone(), Side::Right)),
                Some(Landing::Beside(top, Side::Right)),
            ],
            "all of the top one's bar puts it to that one's right, as it's on its left"
        );
        assert_eq!(
            [at(0.55, 0.52), at(0.9, 0.52)],
            [
                Some(Landing::Beside(bottom.clone(), Side::Right)),
                Some(Landing::Beside(bottom, Side::Right)),
            ],
            "likewise the bottom one's"
        );
        // Turned round: a wide tile dragged, above two side by side.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Down, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Across, terminal(2));
        let at = |y: f32| tiling.landing_at(&Tile::Home, (0.25, y), ZONES);
        let left = Tile::Pinned(terminal(1));
        assert_eq!(
            [at(0.6), at(0.9)],
            [
                Some(Landing::Beside(left.clone(), Side::Bottom)),
                Some(Landing::Beside(left, Side::Bottom)),
            ],
            "all of the body of the one under it puts it below that one, as it's above it"
        );
    }

    #[test]
    fn a_tile_moved_beside_its_neighbour_gives_it_its_room() {
        // Three stacked, the last dragged to the left of the middle one.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Down, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let dragged = Tile::Pinned(terminal(2));
        let landing = Landing::Beside(Tile::Pinned(terminal(1)), Side::Left);
        assert_eq!(
            rounded(&tiling.landed(&dragged, &landing, LEAST)),
            vec![
                (Tile::Home, [0, 0, 100, 33]),
                (dragged.clone(), [0, 33, 50, 67]),
                (Tile::Pinned(terminal(1)), [50, 33, 50, 67]),
            ],
            "the two share the room they had, the top one keeping its own"
        );
        assert_eq!(
            tiling.shown_landing(&dragged, &landing, LEAST),
            tiling.landed(&dragged, &landing, LEAST),
            "it shows as it lands, as the top one stays where it is"
        );
    }

    #[test]
    fn with_only_the_tile_it_lands_beside_it_shows_as_it_lands() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        let other = Tile::Pinned(terminal(1));
        for landing in [
            Landing::Beside(other.clone(), Side::Right),
            Landing::Beside(other.clone(), Side::Top),
            Landing::Beside(other, Side::Bottom),
        ] {
            assert_eq!(
                tiling.shown_landing(&Tile::Home, &landing, LEAST),
                tiling.landed(&Tile::Home, &landing, LEAST),
                "side by side, {landing:?} shows what letting go does"
            );
        }
    }

    #[test]
    fn a_tile_that_would_move_stays_put_and_the_room_left_stays_empty() {
        // Three stacked, the last dragged beside the first, not next to it.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Down, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let dragged = Tile::Pinned(terminal(2));
        let landing = Landing::Beside(Tile::Home, Side::Left);
        assert_eq!(
            rounded(&tiling.shown_landing(&dragged, &landing, LEAST)),
            vec![
                (dragged, [0, 0, 50, 33]),
                (Tile::Home, [50, 0, 50, 33]),
                (Tile::Pinned(terminal(1)), [0, 33, 100, 33]),
            ],
            "the middle one, which would move down as it lands, stays until it's let go"
        );
    }

    #[test]
    fn landings_that_lay_the_tiles_out_alike_show_alike() {
        // A tall tile dragged, beside two stacked.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let below = Landing::Beside(Tile::Pinned(terminal(2)), Side::Bottom);
        let shown = tiling.shown_landing(&Tile::Home, &below, LEAST);
        assert_eq!(
            shown,
            tiling.landed(&Tile::Home, &below, LEAST),
            "as it lands, the top of the column only stretching into the room it leaves"
        );
        for landing in [Landing::Edge(Side::Bottom), Landing::BelowRow] {
            assert_eq!(
                tiling.shown_landing(&Tile::Home, &landing, LEAST),
                shown,
                "{landing:?} lands it there too"
            );
        }
    }

    #[test]
    fn lines_of_both_ways_meet_at_junctions() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        assert_eq!(
            junctions(&tiling.dividers()),
            Vec::new(),
            "one line meets nothing"
        );
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let found = junctions(&tiling.dividers());
        assert_eq!(
            found
                .iter()
                .map(|j| (j.at, j.dividers.len()))
                .collect::<Vec<_>>(),
            vec![((0.5, 0.5), 2)],
            "the line down the middle, and the one across the right half"
        );
        let reaches = [Side::Top, Side::Right, Side::Bottom, Side::Left]
            .map(|side| found.iter().all(|junction| junction.reaches(side)));
        assert_eq!(
            reaches,
            [true, true, true, false],
            "a T on its side: up, down and right, with no line to the left"
        );
        tiling.pin(&Tile::Home, Axis::Down, terminal(3));
        let found = junctions(&tiling.dividers());
        assert_eq!(
            found
                .iter()
                .map(|j| (j.at, j.dividers.len()))
                .collect::<Vec<_>>(),
            vec![((0.5, 0.5), 3)],
            "with the left half divided too, in a line with it, all three meet at the middle"
        );
        tiling.move_divider(&[0], 0, 0.3, 0.1);
        let found = junctions(&tiling.dividers());
        assert_eq!(
            found
                .iter()
                .map(|j| (j.at, j.dividers.len()))
                .collect::<Vec<_>>(),
            vec![((0.5, 0.3), 2), ((0.5, 0.5), 2)],
            "out of line, each meets the one down the middle at a junction of its own"
        );
    }

    #[test]
    fn a_bar_parts_in_its_middle_beside_the_tile_dragged() {
        // Two columns of two, the bottom right dragged.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Home, Axis::Down, terminal(2));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(3));
        let dragged = Tile::Pinned(terminal(3));
        let at = |x: f32, y: f32| tiling.landing_at(&dragged, (x, y), ZONES);
        assert_eq!(
            at(0.24, 0.52),
            Some(Landing::Beside(Tile::Pinned(terminal(2)), Side::Left)),
            "the bar of the tile beside it, left of its middle, to its left"
        );
        assert_eq!(
            at(0.26, 0.52),
            Some(Landing::Beside(Tile::Pinned(terminal(2)), Side::Right)),
            "right of its middle, to its right"
        );
    }

    #[test]
    fn a_tile_dragged_into_the_one_beside_it_goes_past_it() {
        // Side by side, the left one dragged right.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        let at = |x: f32, y: f32| tiling.landing_at(&Tile::Home, (x, y), ZONES);
        let past = Some(Landing::Beside(Tile::Pinned(terminal(1)), Side::Right));
        assert_eq!(
            at(0.75, 0.3),
            past,
            "the upper half of it, which would put it along the top, as past the room's top \
             does, puts it past it"
        );
        assert_eq!(
            at(0.75, 0.8),
            Some(Landing::Beside(Tile::Pinned(terminal(1)), Side::Bottom)),
            "the lower half, below it, as the lower half of its own place has it"
        );
        // One above the other, the top one dragged down.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Down, terminal(1));
        let at = |x: f32, y: f32| tiling.landing_at(&Tile::Home, (x, y), ZONES);
        let below = Some(Landing::Beside(Tile::Pinned(terminal(1)), Side::Bottom));
        assert_eq!(
            [at(0.2, 0.52), at(0.8, 0.52), at(0.5, 0.8)],
            [below.clone(), below.clone(), below],
            "its bar, which would put it along the room's left or right, puts it below it too"
        );
    }

    #[test]
    fn a_bar_parts_in_its_middle_wherever_the_tile_dragged_was() {
        // Two side by side above a third.
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Down, terminal(2));
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        let at = |x: f32, y: f32| tiling.landing_at(&Tile::Home, (x, y), ZONES);
        assert_eq!(
            at(0.6, 0.02),
            Some(Landing::Beside(Tile::Pinned(terminal(1)), Side::Right)),
            "all of the bar of the tile beside it moves it to that one's right"
        );
        assert_eq!(
            at(0.45, 0.52),
            Some(Landing::Beside(Tile::Pinned(terminal(2)), Side::Left)),
            "on the bar below, left of its middle, to that one's left, past where it was"
        );
        assert_eq!(
            at(0.55, 0.52),
            Some(Landing::Beside(Tile::Pinned(terminal(2)), Side::Right)),
            "right of its middle, to its right"
        );
    }

    #[test]
    fn a_tile_dragged_to_the_far_side_of_a_column_goes_along_that_side() {
        let mut tiling = Tiling::default();
        tiling.pin(&Tile::Home, Axis::Across, terminal(1));
        tiling.pin(&Tile::Pinned(terminal(1)), Axis::Down, terminal(2));
        let landing = tiling.landing_at(&Tile::Home, (0.98, 0.02), ZONES);
        assert_eq!(
            landing,
            Some(Landing::Edge(Side::Right)),
            "the home tile's bar near the room's right side lands it along it"
        );
        assert_eq!(
            rounded(&tiling.landed(&Tile::Home, &landing.unwrap_or(Landing::Stay), LEAST)),
            vec![
                (Tile::Pinned(terminal(1)), [0, 0, 50, 50]),
                (Tile::Pinned(terminal(2)), [0, 50, 50, 50]),
                (Tile::Home, [50, 0, 50, 100]),
            ],
            "the column of terminals goes to the left, as wide as the home tile was"
        );
    }
}
