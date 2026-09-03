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

/**
 * What a folded stacks panel is standing in for: how many branches it holds,
 * how many of those still have commits to push, and whether any of them is
 * conflicted.
 *
 * Branch-wise rather than stack-wise, because a branch is what the folded rows
 * would have shown one of, and a stack of five branches with one unpushed is
 * not "one unpushed stack" to anybody reading the number.
 */
type WorkspaceStacksSummary = {
	branches: number;
	unpushedBranches: number;
	hasConflicts: boolean;
};

export const workspaceStacksSummary = (stacks: Array<Stack>): WorkspaceStacksSummary => {
	const summary: WorkspaceStacksSummary = {
		branches: 0,
		unpushedBranches: 0,
		hasConflicts: false,
	};

	for (const stack of stacks) {
		for (const segment of stack.segments) {
			// Branchless segments are not rows of their own, so they are not counted
			// — but their commits still belong to the branch below them, and a
			// conflict in one is still a conflict this panel is hiding.
			if (segment.refName) {
				summary.branches += 1;
				if (pushStatusRequiresPush(segment.pushStatus)) summary.unpushedBranches += 1;
			}
			if (segment.commits.some((commit) => commit.hasConflicts)) summary.hasConflicts = true;
		}
	}

	return summary;
};

export const downstackPushStatusesFromSegments = (
	segments: Array<Segment>,
): Array<DownstackPushStatus> =>
	segments.reduceRight((acc, segment, idx) => {
		acc[idx] = concatDownstackPushStatus(
			acc[idx + 1] ?? emptyDownstackPushStatus,
			toDownstackPushStatus(segment),
		);
		return acc;
	}, [] as Array<DownstackPushStatus>);
