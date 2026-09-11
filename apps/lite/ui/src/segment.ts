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

/**
 * Whether the update-from-remote flow has anything to do: an upstream with
 * commits the branch lacks, or rewritten history it still holds.
 */
export const canIntegrateUpstream = (segment: Segment): boolean =>
	segment.remoteTrackingRefName !== null &&
	(segment.commitsOnRemote.length > 0 || segment.pushStatus === "unpushedCommitsRequiringForce");

export type DownstackPushStatus = {
	anyRequiresPush: boolean;
	anyPushRequiresForce: boolean;
	anyHasConflicts: boolean;
	downstackBranches: number;
};

export const emptyDownstackPushStatus: DownstackPushStatus = {
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

// Nothing pushes a segment without a branch, so it adds nothing to what rests on it.
const toDownstackPushStatus = (segment: Segment): DownstackPushStatus =>
	segment.refName === null
		? emptyDownstackPushStatus
		: {
				anyRequiresPush: pushStatusRequiresPush(segment.pushStatus),
				anyPushRequiresForce: segment.pushStatus === "unpushedCommitsRequiringForce",
				anyHasConflicts: segment.commits.some((commit) => commit.hasConflicts),
				downstackBranches: 1,
			};

export const downstackPushStatusDisabled = (dps: DownstackPushStatus): boolean =>
	!dps.anyRequiresPush || dps.anyHasConflicts;

export const downstackPushStatusFromSegments = (segments: Array<Segment>): DownstackPushStatus =>
	segments.reduce(
		(acc, segment) => concatDownstackPushStatus(acc, toDownstackPushStatus(segment)),
		emptyDownstackPushStatus,
	);

/**
 * Per segment, what a push from it covers: itself, the segments below, and
 * `beneath`, which is what the last segment rests on. A stack rests on the
 * target, a worktree lane on a commit of another lane whose push it also makes.
 */
export const downstackPushStatusesFromSegments = (
	segments: Array<Segment>,
	beneath: DownstackPushStatus = emptyDownstackPushStatus,
): Array<DownstackPushStatus> =>
	segments.reduceRight((acc, segment, idx) => {
		acc[idx] = concatDownstackPushStatus(acc[idx + 1] ?? beneath, toDownstackPushStatus(segment));
		return acc;
	}, [] as Array<DownstackPushStatus>);

export const downstackPushLabel = (dps: DownstackPushStatus): string =>
	dps.downstackBranches > 1
		? dps.anyPushRequiresForce
			? "Force Push With Branches Below"
			: "Push With Branches Below"
		: dps.anyPushRequiresForce
			? "Force Push Branch"
			: "Push Branch";
