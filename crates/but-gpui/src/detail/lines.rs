//! Parts of hunks to cut, drag or take out of where they are, as files and hunks are: lines
//! picked by their numbers, and the run of changed lines a gutter grip takes.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use gpui::{App, Context, Entity, Modifiers, Pixels, Point, SharedString, WeakEntity, Window};

use super::Detail;
use crate::backend::{FileDiff, Hunk, HunkKey, LineKind, Selected};
use crate::state::Cut;
use crate::transfer::DragPreview;

/// Lines picked in one file of the diff.
pub(super) struct LinePicks {
    /// The file, by path, and its patch's fingerprint: picks go by line numbers, which the
    /// file changed since no longer means.
    path: String,
    fingerprint: u64,
    /// By their hunk and index in it.
    lines: HashSet<(HunkKey, usize)>,
    /// What was picked before the run from `anchor` began, which ⇧ adds to.
    kept: HashSet<(HunkKey, usize)>,
    /// Where ⇧-click, and dragging on down the numbers, run from.
    anchor: (HunkKey, usize),
}

/// How a press on a line's number picks it.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Pick {
    /// Alone, or no longer when it's all that's picked.
    Only,
    /// Along with the others, or not any more: ⌘-click.
    Toggle,
    /// With the lines between it and where the pick began: ⇧-click, or dragging on.
    Run,
}

impl Pick {
    pub(super) fn from(modifiers: Modifiers) -> Self {
        if modifiers.shift {
            Pick::Run
        } else if modifiers.secondary() {
            Pick::Toggle
        } else {
            Pick::Only
        }
    }
}

/// The changed lines of `file` in its order, by hunk and index in it.
fn changed(file: &FileDiff) -> impl Iterator<Item = (HunkKey, usize)> + '_ {
    file.hunks().iter().flat_map(|hunk| {
        (hunk.lines.iter().enumerate())
            .filter(|(_, line)| line.kind != LineKind::Context)
            .map(move |(ix, _)| (hunk.key, ix))
    })
}

impl Detail {
    /// Whether line `line` of the hunk with `key` in the file at `path` is picked.
    pub(super) fn line_picked(&self, path: &str, key: HunkKey, line: usize) -> bool {
        (self.line_picks.as_ref())
            .is_some_and(|picks| picks.path == path && picks.lines.contains(&(key, line)))
    }

    /// Pick line `line` of hunk `hunk` of the loaded diff's file at `file`, as `how` says.
    pub(super) fn pick_line(
        &mut self,
        (file, hunk, line): (usize, usize, usize),
        how: Pick,
        cx: &mut Context<Self>,
    ) {
        // The hunk keys act where the lines are picked.
        let cursor = super::Cursor {
            file,
            hunk: Some(hunk),
        };
        let Some(file) = self.diff.as_ref().and_then(|diff| diff.files.get(file)) else {
            return;
        };
        let Some(hunk) = file.hunks().get(hunk) else {
            return;
        };
        if hunk
            .lines
            .get(line)
            .is_none_or(|l| l.kind == LineKind::Context)
        {
            return;
        }
        let item = (hunk.key, line);
        let path = &file.file.path;
        let before = self.line_picks.as_ref().map(|picks| picks.lines.clone());
        let picks = (self.line_picks.take())
            .filter(|picks| picks.path == *path && picks.fingerprint == file.fingerprint);
        self.line_picks =
            match (how, picks) {
                (Pick::Run, Some(mut picks)) => {
                    let order: Vec<_> = changed(file).collect();
                    let at = |item| order.iter().position(|it| *it == item);
                    if let (Some(from), Some(to)) = (at(picks.anchor), at(item)) {
                        let run = order.iter().skip(from.min(to)).take(from.abs_diff(to) + 1);
                        picks.lines = picks.kept.iter().copied().chain(run.copied()).collect();
                    }
                    Some(picks)
                }
                (Pick::Toggle, Some(mut picks)) => {
                    if !picks.lines.remove(&item) {
                        picks.lines.insert(item);
                    }
                    picks.kept = picks.lines.clone();
                    picks.anchor = item;
                    (!picks.lines.is_empty()).then_some(picks)
                }
                // Alone, unless it's all that was picked, which lets it go.
                (_, picks) => (picks.is_none_or(|picks| picks.lines != HashSet::from([item])))
                    .then(|| LinePicks {
                        path: path.clone(),
                        fingerprint: file.fingerprint,
                        lines: HashSet::from([item]),
                        kept: HashSet::from([item]),
                        anchor: item,
                    }),
            };
        // Dragging on over the same line picks nothing new.
        if self.line_picks.as_ref().map(|picks| &picks.lines) != before.as_ref() {
            self.place_cursor(cursor);
            self.refresh_grip();
            cx.notify();
        }
    }

    /// Work out again what the grip under the pointer takes, as the picks or the lines changed.
    pub(super) fn refresh_grip(&mut self) {
        self.grip_over = (self.grip_over.take()).map(|(at, _)| (at, self.grip_takes(at)));
    }

    /// How many lines are picked.
    pub fn picked_line_count(&self) -> usize {
        self.line_picks
            .as_ref()
            .map_or(0, |picks| picks.lines.len())
    }

    /// A plain press in the diff off the line numbers: it lets the picked lines go, as a
    /// click elsewhere lets a selection go, and ends picking by dragging over the numbers.
    pub(super) fn press_off_picks(&mut self, cx: &mut Context<Self>) {
        self.picking_lines = false;
        self.clear_line_picks(cx);
    }

    /// The preview of a drag of `label` from the diff or its files, which lets the picked
    /// lines go as it starts unless it `carries_picks`: dragging anything else passes them
    /// over.
    pub(super) fn drag_preview(
        &self,
        label: SharedString,
        carries_picks: bool,
        detail: WeakEntity<Detail>,
    ) -> impl Fn(&Cut, Point<Pixels>, &mut Window, &mut App) -> Entity<DragPreview> + 'static {
        let preview = crate::transfer::drag_preview(self.store.clone(), label);
        move |payload, offset, window, cx| {
            // Nor does it pick the lines whose numbers it passes over, nor keep the grip it
            // started from, which hears nothing more while dragging.
            (detail.update(cx, |detail, cx| {
                detail.picking_lines = false;
                detail.let_go(cx);
                if !carries_picks {
                    detail.clear_line_picks(cx);
                }
            }))
            .ok();
            preview(payload, offset, window, cx)
        }
    }

    /// Let the picked lines go. Whether any were.
    pub fn clear_line_picks(&mut self, cx: &mut Context<Self>) -> bool {
        let had = self.line_picks.take().is_some();
        if had {
            self.refresh_grip();
            cx.notify();
        }
        had
    }

    /// Keep the picked lines while the loaded diff has their file as it was, and else let
    /// them go: a line's place in a changed file may be another line by now.
    pub(super) fn settle_line_picks(&mut self) {
        let diff = self.diff.as_ref();
        self.line_picks.take_if(|picks| {
            let file = diff.and_then(|diff| diff.files.iter().find(|f| f.file.path == picks.path));
            file.is_none_or(|file| file.fingerprint != picks.fingerprint)
        });
    }

    /// What the grip on line `line` of hunk `hunk` of the file at `file` takes, by hunk and
    /// line index: the picked lines when it's one of them, else the run of changed lines
    /// around it.
    pub(super) fn grip_takes(
        &self,
        (file, hunk, line): (usize, usize, usize),
    ) -> Vec<(usize, usize)> {
        let Some(diff_file) = self.diff.as_ref().and_then(|diff| diff.files.get(file)) else {
            return Vec::new();
        };
        let Some((key, lines)) = diff_file.hunks().get(hunk).map(|h| (h.key, &h.lines)) else {
            return Vec::new();
        };
        if self.line_picked(&diff_file.file.path, key, line) {
            return self.picked_lines(file);
        }
        let changed = |ix: &usize| lines.get(*ix).is_some_and(|l| l.kind != LineKind::Context);
        let start = (0..=line).rev().take_while(changed).last().unwrap_or(line);
        let end = (line..lines.len())
            .take_while(changed)
            .last()
            .unwrap_or(line);
        (start..=end).map(|ix| (hunk, ix)).collect()
    }

    /// The picked lines of the loaded diff's file at `file`, by hunk and line index, in order.
    pub(super) fn picked_lines(&self, file: usize) -> Vec<(usize, usize)> {
        let (Some(picks), Some(file)) = (
            self.line_picks.as_ref(),
            self.diff.as_ref().and_then(|diff| diff.files.get(file)),
        ) else {
            return Vec::new();
        };
        if file.file.path != picks.path {
            return Vec::new();
        }
        let mut lines: Vec<(usize, usize)> = (picks.lines.iter())
            .filter_map(|(key, line)| {
                let hunk = file.hunks().iter().position(|hunk| hunk.key == *key)?;
                Some((hunk, *line))
            })
            .collect();
        lines.sort();
        lines
    }

    /// The loaded diff's file the lines are picked in.
    fn line_picks_file(&self) -> Option<usize> {
        let picks = self.line_picks.as_ref()?;
        (self.diff.as_ref()?.files.iter()).position(|file| file.file.path == picks.path)
    }

    /// The picked lines, to cut, drag or take out.
    pub(super) fn line_picks_cut(&self, cx: &App) -> Option<Cut> {
        let file = self.line_picks_file()?;
        self.lines_cut(file, self.picked_lines(file), cx)
    }

    /// `lines` of the loaded diff's file at `file`, by hunk and line index, as they'd be cut.
    pub(super) fn lines_cut(
        &self,
        file: usize,
        lines: impl IntoIterator<Item = (usize, usize)>,
        cx: &App,
    ) -> Option<Cut> {
        let diff_file = self.diff.as_ref()?.files.get(file)?;
        let mut by_hunk: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for (hunk, line) in lines {
            by_hunk.entry(hunk).or_default().insert(line);
        }
        let keys: Vec<HunkKey> = (by_hunk.iter())
            .filter_map(|(hunk, lines)| Some(self.part_keys(diff_file.hunks().get(*hunk)?, lines)))
            .flatten()
            .collect();
        // No keys would read as the whole file.
        if keys.is_empty() {
            return None;
        }
        self.owned_cut(vec![diff_file.part(Some(keys))], cx)
    }

    /// The keys taking `lines` of `hunk` out of the loaded diff: the hunk itself when they're
    /// all of its changes, else one per run, shaped for what reads them. Lines a commit gives
    /// up name the hunk they leave; uncommitted ones name just themselves, as a commit takes
    /// them, and a discard finds their hunk.
    pub(super) fn part_keys(&self, hunk: &Hunk, lines: &BTreeSet<usize>) -> Vec<HunkKey> {
        let all = (hunk.lines.iter().enumerate())
            .filter(|(_, line)| line.kind != LineKind::Context)
            .all(|(ix, _)| lines.contains(&ix));
        let uncommitted = (self.diff.as_ref()).is_some_and(|diff| diff.key.source.live());
        match (all, uncommitted) {
            (true, _) => vec![hunk.key],
            (false, true) => hunk.picked(lines),
            (false, false) => hunk.picked_out(lines),
        }
    }

    /// What dragged `lines` of the file at `file` say they are: "+2 −1 · run.rs".
    pub(super) fn lines_label(&self, file: usize, lines: &[(usize, usize)]) -> SharedString {
        let Some(diff_file) = self.diff.as_ref().and_then(|diff| diff.files.get(file)) else {
            return SharedString::default();
        };
        let kinds = (lines.iter())
            .filter_map(|(hunk, line)| Some(diff_file.hunks().get(*hunk)?.lines.get(*line)?.kind));
        let (added, removed) = kinds.fold((0, 0), |(a, r), kind| match kind {
            LineKind::Added => (a + 1, r),
            LineKind::Removed => (a, r + 1),
            LineKind::Context => (a, r),
        });
        let name = (diff_file.file.path.rsplit('/').next()).unwrap_or(&diff_file.file.path);
        let counts = match (added, removed) {
            (0, r) => format!("−{r}"),
            (a, 0) => format!("+{a}"),
            (a, r) => format!("+{a} −{r}"),
        };
        format!("{counts} · {name}").into()
    }

    /// Whether `files` take parts of hunks rather than whole ones: lines picked, or a run.
    pub(super) fn takes_lines(&self, files: &[Selected]) -> bool {
        let Some(diff) = &self.diff else {
            return false;
        };
        files.iter().any(|part| {
            let file = diff.files.iter().find(|f| f.file.path == part.file.path);
            let whole: HashSet<HunkKey> =
                file.map_or_else(HashSet::new, |f| f.hunks().iter().map(|h| h.key).collect());
            (part.hunks.iter().flatten()).any(|key| !whole.contains(key))
        })
    }

    /// Whether `files` take every hunk of every file in the diff.
    pub(super) fn takes_all(&self, files: &[Selected]) -> bool {
        self.diff.as_ref().is_some_and(|diff| {
            diff.files.iter().all(|file| {
                files.iter().any(|part| {
                    part.file.path == file.file.path
                        && (part.hunks.as_ref()).is_none_or(|keys| {
                            file.hunks().iter().all(|hunk| keys.contains(&hunk.key))
                        })
                })
            })
        })
    }
}
