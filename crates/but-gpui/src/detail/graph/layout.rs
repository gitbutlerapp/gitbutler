//! Where each of the Graph page's commits goes. Rows count up from the oldest, each commit a
//! row above the highest of its parents, so commits that fork from one sit side by side.
//! Lines of first parents run straight up: the target's in the middle, the workspace's lanes
//! centred over the commit they fork from, so lanes on the target's newest commit fan out
//! straight above it, and the rest of the history either side of the line it forks from. Each
//! line goes as near where it forks from as what's beside it allows, from just above that
//! commit to just below the one it merges into: a pill takes a pill's width, a merge's circle
//! less, and a stretch of line between commits hardly any, so a branch tucks in beside a line
//! with nothing of its own there.
//!
//! Where edges turn from one line into another, they cross the gap between two rows, each on
//! a track of its own where it would overlap another: turns out of a commit low in the gap, the
//! widest lowest, and turns into one high, the widest highest, so they nest rather than cross.

use std::cmp::Reverse;
use std::collections::HashMap;

/// A commit as the layout sees it: its parents by their place in the list, first parent
/// first, `None` for one the graph doesn't have. One with several is a merge.
#[derive(Debug, Clone, Default)]
pub struct Input {
    pub parents: Vec<Option<usize>>,
}

/// Where a line goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// In the middle, as the target's.
    Middle,
    /// Centred over the commit it forks from with the others forking from it, in their order,
    /// as the workspace's lanes.
    Over,
    /// Beside the line it forks from, on the side with room nearer, taking turns when both
    /// are as near, as the rest of the history.
    Beside,
}

/// A line laid out before the rest, from `tip` down its first parents, in the order given.
/// Every other line goes beside the one it forks from, the newest first.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub tip: usize,
    pub side: Side,
}

/// How wide things are across, in the units positions come out in.
#[derive(Debug, Clone, Copy)]
pub struct Widths {
    pub pill: f32,
    /// A merge's circle.
    pub merge: f32,
    /// A line running past a row it has no commit on.
    pub line: f32,
    /// Between neighbours.
    pub gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// From the bottom, the oldest commits at 0.
    pub row: u32,
    /// Its middle, from the target's line, the left's below 0.
    pub x: f32,
}

/// How an edge runs from a parent up to its child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Straight up the line they're both on.
    Straight,
    /// Out of the parent's top into the line the child starts, then up it.
    Fork,
    /// Up the line the parent ends, then into the child it merged into.
    Merge,
    /// From one to the other directly, as a merge of a commit midway up a line.
    Across,
}

/// Where a turn crosses the gap between two rows: on track `at` of `of`, counted up from the
/// gap's bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Track {
    pub at: u16,
    pub of: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub child: usize,
    pub parent: usize,
    pub route: Route,
    /// Its turns' tracks: out of the parent, across the gap above it, and into the child,
    /// across the gap below it. A fork turns only out, a merge only in, and an edge across
    /// both, unless its ends are a row apart.
    pub tracks: [Option<Track>; 2],
    /// How far from the child an edge across both goes up, in the gap beside it towards the
    /// parent.
    pub aside: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Layout {
    /// Each commit's, in the order they came in.
    pub places: Vec<Place>,
    pub edges: Vec<Edge>,
}

/// A line of first parents.
struct Chain {
    /// Top first.
    commits: Vec<usize>,
    side: Side,
    /// The commit it forks from, on a line laid out before it.
    fork: Option<usize>,
    /// The child its top merged into, the lowest when several did.
    merge: Option<usize>,
    /// How wide it is on each row it takes, from just above the commit it forks from to just
    /// below the one it merges into.
    needs: Vec<(u32, f32)>,
}

pub fn layout(commits: &[Input], lines: &[Line], widths: Widths) -> Layout {
    let n = commits.len();
    let rows = rows(commits);
    let mut children: Vec<Vec<usize>> = std::iter::repeat_n(Vec::new(), n).collect();
    for (child, commit) in commits.iter().enumerate() {
        for parent in commit.parents.iter().flatten() {
            if let Some(children) = children.get_mut(*parent) {
                children.push(child);
            }
        }
    }
    let wide: Vec<f32> = (commits.iter())
        .map(|commit| match commit.parents.len() > 1 {
            true => widths.merge,
            false => widths.pill,
        })
        .collect();
    let mut rest: Vec<usize> = (0..n).collect();
    rest.sort_by_key(|commit| Reverse(row(&rows, *commit)));
    let tips = (lines.iter().filter(|line| line.tip < n))
        .map(|line| (line.tip, line.side))
        .chain(rest.into_iter().map(|commit| (commit, Side::Beside)));

    let mut chain_of: Vec<Option<usize>> = std::iter::repeat_n(None, n).collect();
    let mut chains: Vec<Chain> = Vec::new();
    for (tip, side) in tips {
        let id = chains.len();
        let mut line = Vec::new();
        let mut at = Some(tip);
        while let Some(slot) = at.and_then(|commit| chain_of.get_mut(commit))
            && slot.is_none()
        {
            *slot = Some(id);
            line.extend(at);
            at = (at.and_then(|commit| commits.get(commit)))
                .and_then(|commit| commit.parents.first().copied().flatten());
        }
        // Laid out already, as the tip of a line laid out before it was.
        let (Some(&top), Some(&bottom)) = (line.first(), line.last()) else {
            continue;
        };
        // Where the loop stopped is the commit it forks from, on a line laid out already.
        let fork = at;
        let merge = (children.get(top).into_iter().flatten().copied())
            .filter(|child| first_parent(commits, *child) != Some(top))
            .min_by_key(|child| row(&rows, *child));
        let low = fork.map_or(row(&rows, bottom), |fork| row(&rows, fork) + 1);
        let high = merge.map_or(row(&rows, top), |merge| row(&rows, merge).saturating_sub(1));
        let on: HashMap<u32, usize> = (line.iter())
            .map(|commit| (row(&rows, *commit), *commit))
            .collect();
        let needs = (low..=high.max(low))
            .map(|at| {
                let width = (on.get(&at)).map_or(widths.line, |commit| {
                    wide.get(*commit).copied().unwrap_or(widths.pill)
                });
                (at, width)
            })
            .collect();
        chains.push(Chain {
            commits: line,
            side,
            fork,
            merge,
            needs,
        });
    }

    let mut xs: Vec<f32> = Vec::with_capacity(chains.len());
    let mut taken: HashMap<u32, Vec<(f32, f32)>> = HashMap::new();
    // How many lines went beside on the left and on the right, to even them out.
    let mut beside = (0, 0);
    for (id, line) in chains.iter().enumerate() {
        let fork_x = (line.fork.and_then(|fork| chain(&chain_of, fork)))
            .and_then(|chain| xs.get(chain).copied());
        let x = match line.side {
            Side::Middle => clear(&taken, &line.needs, 0., 1., widths.gap),
            Side::Over => {
                // Side by side with the others forking from the same commit, centred over it.
                let siblings: Vec<(usize, f32)> = (chains.iter().enumerate())
                    .filter(|(_, other)| other.side == Side::Over && other.fork == line.fork)
                    .map(|(sibling, other)| (sibling, widest(&other.needs)))
                    .collect();
                let across: f32 = siblings.iter().map(|(_, width)| width + widths.gap).sum();
                let mut wanted = fork_x.unwrap_or(0.) - (across - widths.gap) / 2.;
                for (sibling, width) in &siblings {
                    if *sibling == id {
                        wanted += width / 2.;
                        break;
                    }
                    wanted += width + widths.gap;
                }
                clear(&taken, &line.needs, wanted, 1., widths.gap)
            }
            Side::Beside => {
                let centre = fork_x.unwrap_or(0.);
                let left = clear(&taken, &line.needs, centre, -1., widths.gap);
                let right = clear(&taken, &line.needs, centre, 1., widths.gap);
                let (left_by, right_by) = (centre - left, right - centre);
                let goes_right = right_by < left_by || (right_by == left_by && beside.1 < beside.0);
                match goes_right {
                    true => {
                        beside.1 += 1;
                        right
                    }
                    false => {
                        beside.0 += 1;
                        left
                    }
                }
            }
        };
        for (at, width) in &line.needs {
            taken
                .entry(*at)
                .or_default()
                .push((x - width / 2., x + width / 2.));
        }
        xs.push(x);
    }

    let at_x: Vec<f32> = (0..n)
        .map(|commit| {
            (chain(&chain_of, commit).and_then(|chain| xs.get(chain).copied())).unwrap_or(0.)
        })
        .collect();
    let places = (at_x.iter().enumerate())
        .map(|(commit, x)| Place {
            row: row(&rows, commit),
            x: *x,
        })
        .collect();
    let mut edges = Vec::new();
    for (child, commit) in commits.iter().enumerate() {
        for (nth, parent) in commit.parents.iter().enumerate() {
            let Some(parent) = *parent else {
                continue;
            };
            let (child_chain, parent_chain) = (chain(&chain_of, child), chain(&chain_of, parent));
            let parent_ends =
                (parent_chain.and_then(|chain| chains.get(chain))).is_some_and(|chain| {
                    chain.commits.first() == Some(&parent) && chain.merge == Some(child)
                });
            let route = if nth == 0 && child_chain == parent_chain {
                Route::Straight
            } else if nth == 0 {
                Route::Fork
            } else if parent_ends {
                Route::Merge
            } else {
                Route::Across
            };
            // An edge across goes up beside the child, between it and the parent.
            let aside = match (route, at_x.get(child), at_x.get(parent)) {
                (Route::Across, Some(to), Some(from)) => {
                    let room =
                        wide.get(child).copied().unwrap_or(widths.pill) / 2. + widths.gap / 2.;
                    match from < to {
                        true => -room,
                        false => room,
                    }
                }
                (Route::Straight | Route::Fork | Route::Merge, ..) | (Route::Across, ..) => 0.,
            };
            edges.push(Edge {
                child,
                parent,
                route,
                tracks: [None, None],
                aside,
            });
        }
    }
    lay_tracks(&mut edges, &rows, &at_x);
    Layout { places, edges }
}

/// How wide a line is at its widest.
fn widest(needs: &[(u32, f32)]) -> f32 {
    needs.iter().map(|(_, width)| *width).fold(0., f32::max)
}

/// Where a line needing `needs` goes from `start`, moving that way along `step`'s sign until
/// it's clear of everything `taken` on its rows by `gap`.
fn clear(
    taken: &HashMap<u32, Vec<(f32, f32)>>,
    needs: &[(u32, f32)],
    start: f32,
    step: f32,
    gap: f32,
) -> f32 {
    let mut x = start;
    // Each move takes it past something in the way, so it ends.
    loop {
        let mut moved = false;
        for (at, width) in needs {
            for (low, high) in taken.get(at).into_iter().flatten() {
                let half = width / 2. + gap;
                if x - half < *high && *low < x + half {
                    x = match step < 0. {
                        true => low - half,
                        false => high + half,
                    };
                    moved = true;
                }
            }
        }
        if !moved {
            return x;
        }
    }
}

fn chain(chain_of: &[Option<usize>], commit: usize) -> Option<usize> {
    chain_of.get(commit).copied().flatten()
}

fn x_of(xs: &[f32], commit: usize) -> f32 {
    xs.get(commit).copied().unwrap_or(0.)
}

fn row(rows: &[u32], commit: usize) -> u32 {
    rows.get(commit).copied().unwrap_or(0)
}

fn first_parent(commits: &[Input], commit: usize) -> Option<usize> {
    commits.get(commit)?.parents.first().copied().flatten()
}

/// A turn across the gap above row `gap`, from one place across to another.
struct Run {
    edge: usize,
    /// Out of the edge's parent, rather than into its child.
    out: bool,
    gap: u32,
    across: (f32, f32),
}

/// Each turn's track in the gap it crosses, `xs` being where each commit is across.
fn lay_tracks(edges: &mut [Edge], rows: &[u32], xs: &[f32]) {
    let mut runs: Vec<Run> = Vec::new();
    for (ix, edge) in edges.iter().enumerate() {
        let (from, to) = (x_of(xs, edge.parent), x_of(xs, edge.child));
        let (parent_row, child_row) = (row(rows, edge.parent), row(rows, edge.child));
        let out = Run {
            edge: ix,
            out: true,
            gap: parent_row,
            across: (from.min(to), from.max(to)),
        };
        let into = Run {
            out: false,
            gap: child_row.saturating_sub(1),
            ..out
        };
        match edge.route {
            Route::Straight => {}
            Route::Fork => runs.push(out),
            Route::Merge => runs.push(into),
            Route::Across if child_row <= parent_row + 1 => runs.push(out),
            Route::Across => {
                let between = to + edge.aside;
                runs.push(Run {
                    across: (from.min(between), from.max(between)),
                    ..out
                });
                runs.push(Run {
                    across: (between.min(to), between.max(to)),
                    ..into
                });
            }
        }
    }
    let mut gaps: HashMap<u32, Vec<Run>> = HashMap::new();
    for run in runs {
        gaps.entry(run.gap).or_default().push(run);
    }
    for (_, mut gap) in gaps {
        // Out of commits first, the widest lowest, then into them, the widest highest.
        gap.sort_by(|a, b| {
            let (wide_a, wide_b) = (a.across.1 - a.across.0, b.across.1 - b.across.0);
            let by_width = match (a.out, b.out) {
                (true, true) => wide_b.total_cmp(&wide_a),
                (false, false) => wide_a.total_cmp(&wide_b),
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
            };
            by_width.then(a.across.0.total_cmp(&b.across.0))
        });
        let mut laid: Vec<((f32, f32), u16)> = Vec::with_capacity(gap.len());
        for run in &gap {
            let at = (laid.iter())
                .filter(|(across, _)| across.0 < run.across.1 && run.across.0 < across.1)
                .map(|(_, at)| at + 1)
                .max()
                .unwrap_or(0);
            laid.push((run.across, at));
        }
        let of = laid.iter().map(|(_, at)| at + 1).max().unwrap_or(1);
        for (run, (_, at)) in gap.iter().zip(laid) {
            if let Some(edge) = edges.get_mut(run.edge) {
                let end = match run.out {
                    true => 0,
                    false => 1,
                };
                if let Some(track) = edge.tracks.get_mut(end) {
                    *track = Some(Track { at, of });
                }
            }
        }
    }
}

/// Each commit's row: 0 for one with no parents in the graph, else one above its highest
/// parent's.
fn rows(commits: &[Input]) -> Vec<u32> {
    let n = commits.len();
    let mut rows: Vec<Option<u32>> = std::iter::repeat_n(None, n).collect();
    let mut entered = std::iter::repeat_n(false, n).collect::<Vec<_>>();
    for start in 0..n {
        // Parents first, without recursing, as a line of history can be thousands long.
        let mut stack = Vec::from([(start, false)]);
        while let Some((commit, parents_done)) = stack.pop() {
            let (Some(None), Some(input)) = (rows.get(commit), commits.get(commit)) else {
                continue;
            };
            let parents = input.parents.iter().flatten();
            if parents_done {
                let row = (parents.filter_map(|parent| rows.get(*parent).copied().flatten()))
                    .map(|row| row + 1)
                    .max()
                    .unwrap_or(0);
                if let Some(slot) = rows.get_mut(commit) {
                    *slot = Some(row);
                }
                continue;
            }
            // Only a cycle, which Git's history can't have, enters one twice.
            let Some(entered) = entered.get_mut(commit).filter(|entered| !**entered) else {
                continue;
            };
            *entered = true;
            stack.push((commit, true));
            stack.extend(
                parents
                    .filter(|parent| rows.get(**parent).is_some_and(Option::is_none))
                    .map(|parent| (*parent, false)),
            );
        }
    }
    rows.into_iter().map(|row| row.unwrap_or(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::{Edge, Input, Line, Place, Route, Side, Track, Widths, lay_tracks, layout};

    /// Sizes whose sums come out exact: a pill or a merge 2 across, a passing line ½, and ½
    /// between neighbours.
    const WIDTHS: Widths = Widths {
        pill: 2.,
        merge: 2.,
        line: 0.5,
        gap: 0.5,
    };

    fn input(parents: &[usize]) -> Input {
        Input {
            parents: parents.iter().copied().map(Some).collect(),
        }
    }

    fn place(row: u32, x: f32) -> Place {
        Place { row, x }
    }

    fn middle(tip: usize) -> Line {
        Line {
            tip,
            side: Side::Middle,
        }
    }

    fn over(tip: usize) -> Line {
        Line {
            tip,
            side: Side::Over,
        }
    }

    #[test]
    fn a_line_of_first_parents_is_one_column() {
        // Newest first, as the backend lists them.
        let commits = [input(&[1]), input(&[2]), input(&[])];
        let layout = layout(&commits, &[middle(0)], WIDTHS);
        assert_eq!(
            layout.places,
            Vec::from([place(2, 0.), place(1, 0.), place(0, 0.)]),
            "each commit is a row above its parent, in the middle"
        );
        assert!(
            (layout.edges.iter()).all(|edge| edge.route == Route::Straight),
            "a line is drawn straight up"
        );
    }

    #[test]
    fn a_lane_forking_off_the_target_goes_beside_it() {
        // D, C, B and A on the target, and B' on a lane forked from B.
        let (d, c, b, a, b2) = (0, 1, 2, 3, 4);
        let commits = [
            input(&[c]),
            input(&[b]),
            input(&[a]),
            input(&[]),
            input(&[b]),
        ];
        let layout = layout(&commits, &[middle(d), over(b2)], WIDTHS);
        assert_eq!(
            layout.places[b2],
            place(2, 2.5),
            "B' sits beside C, as both are a row above B, a pill and a gap out"
        );
        let fork = layout.edges.iter().find(|edge| edge.child == b2);
        assert_eq!(
            fork.map(|edge| (edge.parent, edge.route)),
            Some((b, Route::Fork)),
            "B' starts its line out of B"
        );
    }

    #[test]
    fn a_merged_branch_goes_beside_the_line_and_merges_back() {
        // M merges X into the target's P; both forked from B.
        let (m, p, x, b) = (0, 1, 2, 3);
        let commits = [input(&[p, x]), input(&[b]), input(&[b]), input(&[])];
        let layout = layout(&commits, &[middle(m)], WIDTHS);
        assert_eq!(
            layout.places[x],
            place(1, -2.5),
            "the merged branch clears P beside it, the left first"
        );
        let routes: Vec<_> = (layout.edges.iter())
            .map(|edge| (edge.child, edge.parent, edge.route))
            .collect();
        assert_eq!(
            routes,
            Vec::from([
                (m, p, Route::Straight),
                (m, x, Route::Merge),
                (p, b, Route::Straight),
                (x, b, Route::Fork),
            ]),
            "the branch forks out of B and merges back into M"
        );
    }

    #[test]
    fn a_branch_tucks_in_beside_a_bare_stretch_of_line() {
        // X forks from B and merges into M two rows up, with no target commit beside it.
        let (m, x, b) = (0, 1, 2);
        let commits = [input(&[b, x]), input(&[b]), input(&[])];
        let layout = layout(&commits, &[middle(m)], WIDTHS);
        assert_eq!(
            layout.places[x].x, -1.75,
            "half a line, a gap and half a pill out, rather than a pill's width"
        );
    }

    #[test]
    fn merged_branches_take_turns_either_side() {
        // Two branches merged one after the other, each forked off right after the last.
        let (m2, x2, m1, x1, b) = (0, 1, 2, 3, 4);
        let commits = [
            input(&[m1, x2]),
            input(&[m1]),
            input(&[b, x1]),
            input(&[b]),
            input(&[]),
        ];
        let layout = layout(&commits, &[middle(m2)], WIDTHS);
        assert_eq!(
            (layout.places[x2].x, layout.places[x1].x),
            (-1.75, 1.75),
            "with room as near either side, they even the sides out"
        );
    }

    #[test]
    fn branches_open_at_once_fan_out_both_sides() {
        // Three branches forked from B, merged into the target one after another.
        let (m3, m2, m1, x3, x2, x1, p, b) = (0, 1, 2, 3, 4, 5, 6, 7);
        let commits = [
            input(&[m2, x3]),
            input(&[m1, x2]),
            input(&[p, x1]),
            input(&[b]),
            input(&[b]),
            input(&[b]),
            input(&[b]),
            input(&[]),
        ];
        let layout = layout(&commits, &[middle(m3)], WIDTHS);
        let xs: Vec<f32> = [x3, x2, x1]
            .iter()
            .map(|branch| layout.places[*branch].x)
            .collect();
        assert_eq!(
            xs,
            Vec::from([-2.5, 2.5, -5.]),
            "each takes the nearest room, on the side with fewer when both are as near"
        );
    }

    #[test]
    fn two_lanes_on_the_targets_tip_sit_either_side_above_it() {
        let (target, one, two) = (0, 1, 2);
        let commits = [input(&[]), input(&[target]), input(&[target])];
        let layout = layout(&commits, &[middle(target), over(one), over(two)], WIDTHS);
        assert_eq!(
            (layout.places[one].x, layout.places[two].x),
            (-1.25, 1.25),
            "they centre over the target, which ends below them, in the order given"
        );
    }

    #[test]
    fn three_lanes_on_the_targets_tip_fan_out_from_straight_above_it() {
        let (target, one, two, three) = (0, 1, 2, 3);
        let commits = [
            input(&[]),
            input(&[target]),
            input(&[target]),
            input(&[target]),
        ];
        let lines = [middle(target), over(one), over(two), over(three)];
        let layout = layout(&commits, &lines, WIDTHS);
        let xs: Vec<f32> = [one, two, three]
            .iter()
            .map(|lane| layout.places[*lane].x)
            .collect();
        assert_eq!(
            xs,
            Vec::from([-2.5, 0., 2.5]),
            "the middle one carries straight on up the target's line"
        );
    }

    #[test]
    fn a_parent_outside_the_graph_is_where_it_stops() {
        let commits = [Input {
            parents: Vec::from([None]),
        }];
        let layout = layout(&commits, &[], WIDTHS);
        assert_eq!(
            layout.places,
            Vec::from([place(0, 0.)]),
            "it starts the bottom row, with nothing beside it to move it aside"
        );
        assert!(
            layout.edges.is_empty(),
            "there's no parent to draw an edge to"
        );
    }

    fn turn(child: usize, parent: usize, route: Route) -> Edge {
        Edge {
            child,
            parent,
            route,
            tracks: [None, None],
            aside: 0.,
        }
    }

    fn track(at: u16, of: u16) -> Option<Track> {
        Some(Track { at, of })
    }

    #[test]
    fn turns_out_of_one_commit_nest_the_widest_lowest() {
        // P forks into A a little across and B further, both a row up.
        let (p, a, b) = (0, 1, 2);
        let mut edges = [turn(a, p, Route::Fork), turn(b, p, Route::Fork)];
        lay_tracks(&mut edges, &[0, 1, 1], &[0., 2., 4.]);
        assert_eq!(
            edges.map(|edge| edge.tracks),
            [[track(1, 2), None], [track(0, 2), None]],
            "B turns off below A, so A's turn up never crosses B's way across"
        );
    }

    #[test]
    fn turns_into_one_commit_nest_the_widest_highest() {
        // X a little across and Y further both merge into C, a row up.
        let (c, x, y) = (0, 1, 2);
        let mut edges = [turn(c, x, Route::Merge), turn(c, y, Route::Merge)];
        lay_tracks(&mut edges, &[1, 0, 0], &[0., 2., 4.]);
        assert_eq!(
            edges.map(|edge| edge.tracks),
            [[None, track(0, 2)], [None, track(1, 2)]],
            "Y crosses above where X turns up, so X's way up never crosses it"
        );
    }

    #[test]
    fn a_turn_in_and_one_out_cross_a_gap_apart() {
        // Across the same stretch of one gap, P forks into A and X merges into C.
        let (p, a, x, c) = (0, 1, 2, 3);
        let mut edges = [turn(a, p, Route::Fork), turn(c, x, Route::Merge)];
        lay_tracks(&mut edges, &[0, 1, 0, 1], &[0., -2., -2., 0.]);
        assert_eq!(
            edges.map(|edge| edge.tracks),
            [[track(0, 2), None], [None, track(1, 2)]],
            "the turn out goes low and the turn in high, rather than one over the other"
        );
    }

    #[test]
    fn turns_that_dont_share_a_stretch_share_a_track() {
        // P forks into A on its left and B on its right.
        let (p, a, b) = (0, 1, 2);
        let mut edges = [turn(a, p, Route::Fork), turn(b, p, Route::Fork)];
        lay_tracks(&mut edges, &[0, 1, 1], &[0., -2., 2.]);
        assert_eq!(
            edges.map(|edge| edge.tracks),
            [[track(0, 1), None], [track(0, 1), None]],
            "they meet only where both leave P"
        );
    }

    #[test]
    fn an_edge_across_rows_turns_out_and_in() {
        // C, on a lane two rows above P and off to the right, merges P from the middle of the
        // target's line.
        let (g, p, q, a, b, c) = (0, 1, 2, 3, 4, 5);
        let commits = [
            input(&[]),
            input(&[g]),
            input(&[p]),
            input(&[g]),
            input(&[a]),
            input(&[b, p]),
        ];
        let layout = layout(&commits, &[middle(q), over(c)], WIDTHS);
        let across = (layout.edges.iter()).find(|edge| edge.child == c && edge.parent == p);
        assert_eq!(
            across.map(|edge| (edge.route, edge.tracks, edge.aside)),
            Some((Route::Across, [track(0, 1), track(0, 1)], -1.25)),
            "it turns out above P, goes up beside C on P's side, and turns in below C"
        );
    }
}
