import { encodeBytes } from "#ui/api/bytes.ts";
import { assert } from "#ui/assert.ts";
import {
	type DownstackPushStatus,
	downstackPushStatusesFromSegments,
	emptyDownstackPushStatus,
} from "#ui/segment.ts";
import type { Commit, RefInfo, RelativeTo, Segment, Stack } from "@gitbutler/but-sdk";

type StackIndex = {
	stack: Stack;
	stackIndex: number;
};

type SegmentIndex = {
	segment: Segment;
	segmentIndex: number;
};

type CommitIndex = {
	commit: Commit;
	commitIndex: number;
};

/** A branch of a workspace stack or of a linked worktree's lane. */
type LaneBranch = {
	segment: Segment;
	/** What a push from the branch covers: it, the branches below it, and what its lane rests on. */
	downstack: DownstackPushStatus;
	/** The linked worktree whose lane holds the branch, `null` for a workspace stack. */
	worktree: string | null;
};

export type HeadInfoIndex = {
	/** Whether the workspace holds a branch at this ref — appliedness. */
	isApplied: (ref: Array<number>) => boolean;
	branchContextByRefBytes: (ref: Array<number>) => (StackIndex & SegmentIndex) | undefined;
	laneBranchByRefBytes: (ref: Array<number>) => LaneBranch | undefined;
	commitContextByCommitId: (
		commitId: string,
	) => (StackIndex & SegmentIndex & CommitIndex) | undefined;
	commitContextsByChangeId: (
		changeId: string,
	) =>
		| [StackIndex & SegmentIndex & CommitIndex, ...Array<StackIndex & SegmentIndex & CommitIndex>]
		| undefined;
};

const headInfoIndexCache = new WeakMap<RefInfo, HeadInfoIndex>();

const buildHeadInfoIndex = (headInfo: RefInfo): HeadInfoIndex => {
	const branchContextByRef = new Map<string, StackIndex & SegmentIndex>();
	const commitContextByCommitId = new Map<string, StackIndex & SegmentIndex & CommitIndex>();
	const commitContextsByChangeId = new Map<
		string,
		[StackIndex & SegmentIndex & CommitIndex, ...Array<StackIndex & SegmentIndex & CommitIndex>]
	>();

	const branchRefKey = (ref: Array<number>): string => ref.join(",");

	for (const [stackIndex, stack] of headInfo.stacks.entries()) {
		for (const [segmentIndex, segment] of stack.segments.entries()) {
			if (segment.refName) {
				branchContextByRef.set(branchRefKey(segment.refName.fullNameBytes), {
					stack,
					stackIndex,
					segment,
					segmentIndex,
				});
			}

			for (const [commitIndex, commit] of segment.commits.entries()) {
				const ctx = {
					stack,
					stackIndex,
					segment,
					segmentIndex,
					commit,
					commitIndex,
				};
				commitContextByCommitId.set(commit.id, ctx);

				const prev = commitContextsByChangeId.get(commit.changeId);
				if (prev) prev.push(ctx);
				else commitContextsByChangeId.set(commit.changeId, [ctx]);
			}
		}
	}

	const laneBranchByRef = new Map<string, LaneBranch>();
	const downstackByCommitId = new Map<string, DownstackPushStatus>();
	const indexLane = (
		segments: Array<Segment>,
		beneath: DownstackPushStatus,
		worktree: string | null,
	) => {
		const downstacks = downstackPushStatusesFromSegments(segments, beneath);
		for (const [segmentIndex, segment] of segments.entries()) {
			const downstack = assert(downstacks[segmentIndex]);
			for (const commit of segment.commits) downstackByCommitId.set(commit.id, downstack);
			if (segment.refName) {
				laneBranchByRef.set(branchRefKey(segment.refName.fullNameBytes), {
					segment,
					downstack,
					worktree,
				});
			}
		}
	};
	for (const stack of headInfo.stacks) indexLane(stack.segments, emptyDownstackPushStatus, null);
	// A worktree only rests on a lane listed before it, so what it rests on is already indexed.
	for (const worktree of headInfo.worktrees) {
		const beneath =
			worktree.base?.type === "InWorkspace"
				? downstackByCommitId.get(worktree.base.subject)
				: undefined;
		indexLane(worktree.segments, beneath ?? emptyDownstackPushStatus, worktree.name);
	}

	return {
		isApplied: (ref: Array<number>) => branchContextByRef.has(branchRefKey(ref)),
		branchContextByRefBytes: (ref: Array<number>) => branchContextByRef.get(branchRefKey(ref)),
		laneBranchByRefBytes: (ref: Array<number>) => laneBranchByRef.get(branchRefKey(ref)),
		commitContextByCommitId: (commitId: string) => commitContextByCommitId.get(commitId),
		commitContextsByChangeId: (changeId: string) => commitContextsByChangeId.get(changeId),
	};
};

export const getHeadInfoIndex = (headInfo: RefInfo): HeadInfoIndex => {
	const cached = headInfoIndexCache.get(headInfo);
	if (cached) return cached;

	const index = buildHeadInfoIndex(headInfo);
	headInfoIndexCache.set(headInfo, index);
	return index;
};

/**
 * The review number the projection recorded on the segment. The projection
 * only ever associates display-worthy reviews — an open one, a merge still
 * awaiting integration detection, or an integrated branch's landed identity —
 * so this is safe to render as-is; callers needing the review's actual state
 * must fetch it.
 */
export const recordedPullRequest = (segment: Segment): number | null =>
	segment.metadata?.review.pullRequest ?? null;

export const resolveRelativeTo = ({
	headInfoIndex,
	relativeTo,
}: {
	headInfoIndex: HeadInfoIndex;
	relativeTo: RelativeTo;
}): string | null => {
	switch (relativeTo.type) {
		case "commit":
			return relativeTo.subject;
		case "referenceBytes":
			return (
				headInfoIndex.branchContextByRefBytes(relativeTo.subject)?.segment.commits[0]?.id ?? null
			);
		case "reference":
			return (
				headInfoIndex.branchContextByRefBytes(encodeBytes(relativeTo.subject))?.segment.commits[0]
					?.id ?? null
			);
	}
};
