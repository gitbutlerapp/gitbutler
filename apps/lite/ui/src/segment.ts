import type { PushStatus, Segment, Stack } from "@gitbutler/but-sdk";

export const canRemoveBranchReference = (stack: Stack, segmentIndex: number): boolean => {
	const segment = stack.segments[segmentIndex];
	if (!segment?.refName) return false;
	if (segment.commits.length === 0) return true;

	// We disallow deleting the top (non-empty) branch reference inside a stack of multiple branches
	// because (1) the backend misbehaves (2) and we want to discourage users from creating branchless
	// segments. See discussion in https://github.com/gitbutlerapp/gitbutler/pull/14059.
	const topBranchIndex = stack.segments.findIndex((segment) => segment.refName !== null);
	return segmentIndex !== topBranchIndex;
};

export type DownstackPushStatus = {
	anyRequiresPush: boolean;
	anyPushRequiresForce: boolean;
	anyHasConflicts: boolean;
	downstackBranches: number;
};

const emptyDownstackPushStatus: DownstackPushStatus = {
	anyRequiresPush: false,
	anyPushRequiresForce: false,
	anyHasConflicts: false,
	downstackBranches: 0,
};

const pushStatusRequiresPush = (pushStatus: PushStatus): boolean =>
	pushStatus === "unpushedCommits" ||
	pushStatus === "unpushedCommitsRequiringForce" ||
	pushStatus === "completelyUnpushed";

const concatDownstackPushStatus = (
	x: DownstackPushStatus,
	y: DownstackPushStatus,
): DownstackPushStatus => ({
	anyRequiresPush: x.anyRequiresPush || y.anyRequiresPush,
	anyPushRequiresForce: x.anyPushRequiresForce || y.anyPushRequiresForce,
	anyHasConflicts: x.anyHasConflicts || y.anyHasConflicts,
	downstackBranches: x.downstackBranches + y.downstackBranches,
});

const toDownstackPushStatus = (segment: Segment): DownstackPushStatus => ({
	anyRequiresPush: pushStatusRequiresPush(segment.pushStatus),
	anyPushRequiresForce: segment.pushStatus === "unpushedCommitsRequiringForce",
	anyHasConflicts: segment.commits.some((commit) => commit.hasConflicts),
	downstackBranches: segment.refName ? 1 : 0,
});

export const downstackPushStatusDisabled = (dps: DownstackPushStatus): boolean =>
	!dps.anyRequiresPush || dps.anyHasConflicts;

export const downstackPushStatusFromSegments = (segments: Array<Segment>): DownstackPushStatus =>
	segments.reduce(
		(acc, segment) => concatDownstackPushStatus(acc, toDownstackPushStatus(segment)),
		emptyDownstackPushStatus,
	);

export const downstackPushStatusesFromSegments = (
	segments: Array<Segment>,
): Array<DownstackPushStatus> =>
	segments.reduceRight<Array<DownstackPushStatus>>((acc, segment, idx) => {
		acc[idx] = concatDownstackPushStatus(
			acc[idx + 1] ?? emptyDownstackPushStatus,
			toDownstackPushStatus(segment),
		);
		return acc;
	}, []);
