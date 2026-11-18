//! The places between status lines where commit, move, cherry-pick and branch mode can land
//! what they hold, and how the cursor steps through them.
//!
//! Every place is drawn as an extension line next to the line it belongs to, so `j` and `k` reach
//! every place, including new branches above and below a stack's branches and a new stack.

use crate::{
    CliId,
    command::legacy::status::{
        StatusOutputLine,
        output::StatusOutputLineData,
        tui::graph_extension::{ExtensionDirection, lane_depth},
    },
    id::{BranchId, CommitId, LaneId},
};

/// A place between status lines where a commit, changes or a branch can land.
///
/// Each stop belongs to exactly one line, the one it's drawn next to, as [`stops_at`] says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    /// On top of the branch. Drawn below the branch line.
    BranchTip(BranchId),
    /// Right below the commit, in its branch. Drawn below the commit line.
    ///
    /// There is no stop above a commit: above a branch's top commit is the branch tip, and above
    /// any other commit is below the one before it.
    BelowCommit(CommitId),
    /// In a new branch stacked right above the branch. Drawn above the branch line.
    NewBranchAbove(BranchId),
    /// In a new branch stacked below the branch, the bottom branch of its stack. Drawn below the
    /// stack's last line.
    NewBranchBelow(BranchId),
    /// In a new branch in a stack of its own. Drawn above the merge base.
    NewStack,
}

impl Stop {
    /// Whether landing here creates a branch in a stack, which its line draws as a branch head.
    pub fn creates_stacked_branch(&self) -> bool {
        match self {
            Stop::NewBranchAbove(_) | Stop::NewBranchBelow(_) => true,
            Stop::BranchTip(_) | Stop::BelowCommit(_) | Stop::NewStack => false,
        }
    }

    /// Which side of its line the stop is drawn on.
    pub fn direction(&self) -> ExtensionDirection {
        match self {
            Stop::BranchTip(_) | Stop::BelowCommit(_) | Stop::NewBranchBelow(_) => {
                ExtensionDirection::Below
            }
            Stop::NewBranchAbove(_) | Stop::NewStack => ExtensionDirection::Above,
        }
    }
}

/// The stops drawn next to line `idx`, in the order they're drawn.
pub fn stops_at(lines: &[StatusOutputLine], idx: usize) -> Vec<Stop> {
    let Some(line) = lines.get(idx) else {
        return Vec::new();
    };
    match &line.data {
        StatusOutputLineData::Branch { cli_id, .. } => {
            let CliId::Branch(branch) = &**cli_id else {
                return Vec::new();
            };
            let mut stops = Vec::from([
                Stop::NewBranchAbove(branch.clone()),
                Stop::BranchTip(branch.clone()),
            ]);
            if ends_stack(lines, idx, &branch.lane) {
                stops.push(Stop::NewBranchBelow(branch.clone()));
            }
            stops
        }
        StatusOutputLineData::Commit { cli_id, .. } => {
            let CliId::Commit { commit, id: _ } = &**cli_id else {
                return Vec::new();
            };
            let mut stops = Vec::from([Stop::BelowCommit(commit.clone())]);
            if let Some(branch) = branch_of_line(lines, idx)
                && ends_stack(lines, idx, &branch.lane)
            {
                stops.push(Stop::NewBranchBelow(branch.clone()));
            }
            stops
        }
        StatusOutputLineData::MergeBase => Vec::from([Stop::NewStack]),
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UncommittedChanges { .. }
        | StatusOutputLineData::WorktreeUncommitted { .. }
        | StatusOutputLineData::UncommittedFile { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::File { .. }
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => Vec::new(),
    }
}

/// The named branch that line `idx` is in, as [`branch_line_of`] finds it.
fn branch_of_line(lines: &[StatusOutputLine], idx: usize) -> Option<&BranchId> {
    match &lines[branch_line_of(lines, idx)?].data {
        StatusOutputLineData::Branch { cli_id, .. } => match &**cli_id {
            CliId::Branch(branch) => Some(branch),
            _ => None,
        },
        _ => None,
    }
}

/// The line of the branch that line `idx` is in: the nearest branch line above it at its own
/// nesting depth, so a commit below a linked worktree's lane belongs to the branch the lane forks
/// from.
pub(crate) fn branch_line_of(lines: &[StatusOutputLine], idx: usize) -> Option<usize> {
    let depth = nesting_depth(&lines[idx]);
    (0..idx).rev().find(|line| {
        matches!(lines[*line].data, StatusOutputLineData::Branch { .. })
            && nesting_depth(&lines[*line]) == depth
    })
}

/// Whether line `idx`, in the stack in `lane`, is that stack's last branch or commit line: no
/// commit the cursor can select follows it before the stack ends, and no branch of the stack
/// does either. Lanes nested deeper, such as a linked worktree forking from one of the stack's
/// commits, are part of it.
fn ends_stack(lines: &[StatusOutputLine], idx: usize, lane: &LaneId) -> bool {
    let depth = nesting_depth(&lines[idx]);
    for line in &lines[idx + 1..] {
        match &line.data {
            StatusOutputLineData::Commit { .. } if !line.is_selectable() => {}
            StatusOutputLineData::Commit { .. } => return nesting_depth(line) < depth,
            StatusOutputLineData::Branch { cli_id, .. } => match nesting_depth(line).cmp(&depth) {
                std::cmp::Ordering::Greater => {}
                std::cmp::Ordering::Equal => return cli_id.lane() != Some(lane),
                std::cmp::Ordering::Less => return true,
            },
            StatusOutputLineData::MergeBase => return true,
            _ => {}
        }
    }
    true
}

fn nesting_depth(line: &StatusOutputLine) -> usize {
    line.connector.as_deref().map_or(0, lane_depth)
}

/// A mode that lands what it holds at stops, which `j` and `k` step through.
pub(crate) trait StopMode {
    /// What landing at `stop` does, as its extension line says, or `None` when the mode can't
    /// land there.
    fn stop_label(&self, stop: &Stop) -> Option<String>;

    /// Extra words after every label, such as how a commit gets its message.
    fn stop_label_suffix(&self) -> Option<&'static str> {
        None
    }

    /// Whether `id` is what the mode lands. Its line is a position of its own, where confirming
    /// does nothing.
    fn is_source(&self, id: &CliId) -> bool;

    /// The stop selected with `j` and `k`. It only counts while the cursor is on its line, so
    /// selecting a line any other way lands on the line itself, as [`current_position`] says.
    fn selected_stop(&self) -> Option<&SelectedStop>;
}

/// A stop selected among the stops of line `line`.
#[derive(Debug, Clone)]
pub struct SelectedStop {
    pub line: usize,
    pub stop: Stop,
}

/// Where the cursor can be in a mode that lands at stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StopPosition {
    /// On a line holding what the mode lands, where confirming does nothing.
    Source,
    At(Stop),
}

/// The positions the cursor can take on line `idx`, in drawing order. A source line is a
/// position of its own, and keeps the new branch below its stack, which what lands there can
/// still go to. A line the cursor can't select, such as a commit only on the remote, has none.
pub(crate) fn positions_at(
    lines: &[StatusOutputLine],
    idx: usize,
    mode: &dyn StopMode,
) -> Vec<StopPosition> {
    let Some(line) = lines.get(idx).filter(|line| line.is_selectable()) else {
        return Vec::new();
    };
    let stops = stops_at(lines, idx)
        .into_iter()
        .filter(|stop| mode.stop_label(stop).is_some());
    let is_source = line.data.cli_id().is_some_and(|id| mode.is_source(id));
    if is_source {
        std::iter::once(StopPosition::Source)
            .chain(
                stops
                    .filter(|stop| matches!(stop, Stop::NewBranchBelow(_)))
                    .map(StopPosition::At),
            )
            .collect()
    } else {
        stops.map(StopPosition::At).collect()
    }
}

/// The position the cursor at line `idx` is at, if the line has any.
pub(crate) fn current_position(
    lines: &[StatusOutputLine],
    idx: usize,
    mode: &dyn StopMode,
) -> Option<StopPosition> {
    let positions = positions_at(lines, idx, mode);
    let at = current_index(&positions, idx, mode);
    positions.into_iter().nth(at)
}

/// Which of `positions`, those of line `idx`, the cursor is at: the selected stop, or when that
/// isn't on this line, the stop that lands on the line itself, a branch's tip or right below a
/// commit, so selecting a line by clicking or by entering a mode on it commits, moves or picks
/// onto it. Else the line's first position.
fn current_index(positions: &[StopPosition], idx: usize, mode: &dyn StopMode) -> usize {
    let selected = mode.selected_stop().filter(|selected| selected.line == idx);
    positions
        .iter()
        .position(|position| match (position, selected) {
            (StopPosition::At(stop), Some(selected)) => *stop == selected.stop,
            (StopPosition::At(_) | StopPosition::Source, _) => false,
        })
        .or_else(|| {
            positions.iter().position(|position| {
                matches!(
                    position,
                    StopPosition::At(Stop::BranchTip(_) | Stop::BelowCommit(_))
                )
            })
        })
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepDirection {
    Up,
    Down,
}

/// The line and position one position up or down from the cursor at line `idx`, or `None` when
/// there is none.
pub(crate) fn step(
    lines: &[StatusOutputLine],
    idx: usize,
    mode: &dyn StopMode,
    direction: StepDirection,
) -> Option<(usize, StopPosition)> {
    let here = positions_at(lines, idx, mode);
    let at = current_index(&here, idx, mode);
    match direction {
        StepDirection::Down => {
            if !here.is_empty() && at + 1 < here.len() {
                return Some((idx, here[at + 1].clone()));
            }
            (idx + 1..lines.len()).find_map(|line| {
                let first = positions_at(lines, line, mode).into_iter().next()?;
                Some((line, first))
            })
        }
        StepDirection::Up => {
            if !here.is_empty() && at > 0 {
                return Some((idx, here[at - 1].clone()));
            }
            (0..idx.min(lines.len())).rev().find_map(|line| {
                let last = positions_at(lines, line, mode).into_iter().next_back()?;
                Some((line, last))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use but_core::ref_metadata::StackId;
    use ratatui::text::Span;
    use snapbox::str;

    use super::{SelectedStop, StepDirection, Stop, StopMode, StopPosition, step, stops_at};
    use crate::{
        CliId,
        command::legacy::status::{
            CommitClassification, StatusOutputLine,
            output::{StatusOutputContent, StatusOutputLineData},
        },
        id::{BranchId, CommitId, LaneId},
    };

    fn line(data: StatusOutputLineData) -> StatusOutputLine {
        StatusOutputLine {
            connector: None,
            content: StatusOutputContent::Plain(Vec::new()),
            data,
        }
    }

    fn drawn_with(connector: &str, mut line: StatusOutputLine) -> StatusOutputLine {
        line.connector = Some(Vec::from([Span::raw(connector.to_owned())]));
        line
    }

    fn branch(name: &str, lane: &LaneId) -> BranchId {
        BranchId {
            name: name.to_owned(),
            id: name.to_owned(),
            lane: lane.clone(),
        }
    }

    fn branch_line(name: &str, lane: &LaneId) -> StatusOutputLine {
        line(StatusOutputLineData::Branch {
            cli_id: Arc::new(CliId::Branch(branch(name, lane))),
            is_merged_upstream: false,
        })
    }

    fn commit(byte: u8) -> CommitId {
        CommitId {
            commit_id: gix::ObjectId::from_bytes_or_panic(&[byte; 20]),
            change_id: None,
        }
    }

    fn commit_line(byte: u8) -> StatusOutputLine {
        line(StatusOutputLineData::Commit {
            cli_id: Arc::new(CliId::Commit {
                commit: commit(byte),
                id: byte.to_string(),
            }),
            stack_id: None,
            classification: CommitClassification::LocalOnly,
        })
    }

    fn stack(n: u128) -> LaneId {
        LaneId::Stack(Some(StackId::from_number_for_testing(n)))
    }

    /// Two stacks: `top` stacked on `bottom` with a commit each, and `lone` with no commits.
    fn lines() -> Vec<StatusOutputLine> {
        let (one, two) = (stack(1), stack(2));
        Vec::from([
            line(StatusOutputLineData::UncommittedChanges {
                cli_id: Arc::new(CliId::UncommittedArea {
                    id: "@".into(),
                    source: crate::ChangeSourceId::Head,
                }),
            }),
            line(StatusOutputLineData::Connector),
            branch_line("top", &one),
            commit_line(1),
            line(StatusOutputLineData::Connector),
            branch_line("bottom", &one),
            commit_line(2),
            line(StatusOutputLineData::Connector),
            branch_line("lone", &two),
            line(StatusOutputLineData::Connector),
            line(StatusOutputLineData::MergeBase),
        ])
    }

    fn describe(stop: &Stop) -> String {
        match stop {
            Stop::BranchTip(branch) => format!("top of {}", branch.name),
            Stop::BelowCommit(commit) => format!("below {}", commit.commit_id.to_hex_with_len(2)),
            Stop::NewBranchAbove(branch) => format!("new branch above {}", branch.name),
            Stop::NewBranchBelow(branch) => format!("new branch below {}", branch.name),
            Stop::NewStack => "new stack".to_owned(),
        }
    }

    fn describe_all(lines: &[StatusOutputLine]) -> String {
        (0..lines.len())
            .flat_map(|idx| {
                stops_at(lines, idx)
                    .into_iter()
                    .map(move |stop| format!("{idx}: {}\n", describe(&stop)))
            })
            .collect()
    }

    #[test]
    fn every_branch_takes_a_new_branch_above_and_each_stack_one_below_its_last_line() {
        // A stack's bottom branch takes a new branch below its last commit, or below its own line
        // when it has none. The new stack goes above the merge base.
        snapbox::assert_data_eq!(
            describe_all(&lines()),
            str![[r#"
2: new branch above top
2: top of top
3: below 01
5: new branch above bottom
5: top of bottom
6: below 02
6: new branch below bottom
8: new branch above lone
8: top of lone
8: new branch below lone
10: new stack

"#]]
        );
    }

    #[test]
    fn a_linked_worktree_forking_from_a_commit_stays_part_of_its_stack() {
        let (stack, worktree) = (stack(1), LaneId::Worktree("wt".into()));
        let lines = Vec::from([
            drawn_with("┊╭┄", branch_line("A", &stack)),
            drawn_with("┊┊╭┄", line(StatusOutputLineData::Connector)),
            drawn_with("┊┊├┄", branch_line("wt-branch", &worktree)),
            drawn_with("┊┊●   ", commit_line(1)),
            drawn_with("┊├╯", line(StatusOutputLineData::Connector)),
            drawn_with("┊●   ", commit_line(2)),
            drawn_with("├╯", line(StatusOutputLineData::Connector)),
            line(StatusOutputLineData::MergeBase),
        ]);
        // A's own commit, below the worktree's lane, is the last line of A's stack. The
        // worktree's lane ends at its own commit.
        snapbox::assert_data_eq!(
            describe_all(&lines),
            str![[r#"
0: new branch above A
0: top of A
2: new branch above wt-branch
2: top of wt-branch
3: below 01
3: new branch below wt-branch
5: below 02
5: new branch below A
7: new stack

"#]]
        );
    }

    /// Lands the uncommitted changes, or with `source`, that commit.
    struct Landing {
        source: Option<CommitId>,
        selected: Option<SelectedStop>,
    }

    impl StopMode for Landing {
        fn stop_label(&self, _stop: &Stop) -> Option<String> {
            Some("land".to_owned())
        }

        fn is_source(&self, id: &CliId) -> bool {
            match (id, &self.source) {
                (CliId::Commit { commit, .. }, Some(source)) => commit == source,
                (CliId::UncommittedArea { .. }, None) => true,
                _ => false,
            }
        }

        fn selected_stop(&self) -> Option<&SelectedStop> {
            self.selected.as_ref()
        }
    }

    fn walk(
        lines: &[StatusOutputLine],
        source: Option<CommitId>,
        direction: StepDirection,
        start: usize,
    ) -> String {
        let mut mode = Landing {
            source,
            selected: None,
        };
        let mut idx = start;
        let mut out = String::new();
        while let Some((line, position)) = step(lines, idx, &mode, direction) {
            let described = match &position {
                StopPosition::Source => "source".to_owned(),
                StopPosition::At(stop) => describe(stop),
            };
            out.push_str(&format!("{line}: {described}\n"));
            idx = line;
            mode.selected = match position {
                StopPosition::Source => None,
                StopPosition::At(stop) => Some(SelectedStop { line, stop }),
            };
        }
        out
    }

    #[test]
    fn moving_a_stacks_last_commit_keeps_the_new_branch_below_its_stack() {
        // The commit stays where it is at its own line, but it can still go below its stack.
        snapbox::assert_data_eq!(
            walk(&lines(), Some(commit(2)), StepDirection::Down, 6),
            str![[r#"
6: new branch below bottom
8: new branch above lone
8: top of lone
8: new branch below lone
10: new stack

"#]]
        );
    }

    #[test]
    fn a_commit_only_on_the_remote_is_stepped_over() {
        let lane = stack(1);
        let lines = Vec::from([
            branch_line("A", &lane),
            line(StatusOutputLineData::Commit {
                cli_id: Arc::new(CliId::Commit {
                    commit: commit(9),
                    id: "9".into(),
                }),
                stack_id: None,
                classification: CommitClassification::Upstream,
            }),
            commit_line(1),
            line(StatusOutputLineData::MergeBase),
        ]);
        snapbox::assert_data_eq!(
            walk(&lines, None, StepDirection::Down, 0),
            str![[r#"
2: below 01
2: new branch below A
3: new stack

"#]]
        );
    }

    #[test]
    fn stepping_down_from_the_source_reaches_every_stop_in_drawing_order() {
        snapbox::assert_data_eq!(
            walk(&lines(), None, StepDirection::Down, 0),
            str![[r#"
2: new branch above top
2: top of top
3: below 01
5: new branch above bottom
5: top of bottom
6: below 02
6: new branch below bottom
8: new branch above lone
8: top of lone
8: new branch below lone
10: new stack

"#]]
        );
    }

    #[test]
    fn stepping_up_from_the_merge_base_reaches_every_stop_in_reverse() {
        snapbox::assert_data_eq!(
            walk(&lines(), None, StepDirection::Up, 10),
            str![[r#"
8: new branch below lone
8: top of lone
8: new branch above lone
6: new branch below bottom
6: below 02
5: top of bottom
5: new branch above bottom
3: below 01
2: top of top
2: new branch above top
0: source

"#]]
        );
    }
}
