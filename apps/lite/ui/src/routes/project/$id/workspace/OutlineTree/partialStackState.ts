import { PushStatus, Segment } from "@gitbutler/but-sdk";
import { initNonEmpty, scanRight } from "effect/Array";

export type PartialStackState = {
	requiresPush: boolean;
	pushWithForce: boolean;
	hasConflicts: boolean;
	branchCount: number;
};

const pushStatusRequiresPush = (pushStatus: PushStatus): boolean =>
	pushStatus === "unpushedCommits" ||
	pushStatus === "unpushedCommitsRequiringForce" ||
	pushStatus === "completelyUnpushed";

const emptyPartialStackState: PartialStackState = {
	requiresPush: false,
	pushWithForce: false,
	hasConflicts: false,
	branchCount: 0,
};

const addSegmentToPartialStackState = (
	state: PartialStackState,
	segment: Segment,
): PartialStackState => ({
	requiresPush: state.requiresPush || pushStatusRequiresPush(segment.pushStatus),
	pushWithForce: state.pushWithForce || segment.pushStatus === "unpushedCommitsRequiringForce",
	hasConflicts: state.hasConflicts || segment.commits.some((commit) => commit.hasConflicts),
	branchCount: segment.refName ? state.branchCount + 1 : state.branchCount,
});

export const partialStackPushDisabled = (partialStackState: PartialStackState): boolean =>
	!partialStackState.requiresPush || partialStackState.hasConflicts;

export const partialStackStateFromSegments = (segments: Array<Segment>): PartialStackState =>
	segments.reduce(addSegmentToPartialStackState, emptyPartialStackState);

export const partialStackStatesFromSegments = (
	segments: Array<Segment>,
): Array<PartialStackState> =>
	initNonEmpty(scanRight(segments, emptyPartialStackState, addSegmentToPartialStackState));
