import { bytesEqual } from "#ui/api/bytes.ts";
import type { Commit, RefInfo, Segment, Stack } from "@gitbutler/but-sdk";

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

export type HeadInfoIndex = {
	stackContextById: (stackId: string) => StackIndex | undefined;
	branchContextByRefBytes: (ref: Array<number>) => (StackIndex & SegmentIndex) | undefined;
	commitContextById: (
		changeOrCommitId: string,
	) => (StackIndex & SegmentIndex & CommitIndex) | undefined;
};

const headInfoIndexCache = new WeakMap<RefInfo, HeadInfoIndex>();

const buildHeadInfoIndex = (headInfo: RefInfo): HeadInfoIndex => {
	const stackContextById = new Map<string, StackIndex>();
	const branchContextByRef = new Map<string, StackIndex & SegmentIndex>();
	const commitContextById = new Map<string, StackIndex & SegmentIndex & CommitIndex>();

	const branchRefKey = (ref: Array<number>): string => ref.join(",");

	for (const [stackIndex, stack] of headInfo.stacks.entries()) {
		if (stack.id !== null) stackContextById.set(stack.id, { stack, stackIndex });

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

				commitContextById.set(commit.changeId, ctx);
				commitContextById.set(commit.id, ctx);
			}
		}
	}

	return {
		stackContextById: (stackId: string) => stackContextById.get(stackId),
		branchContextByRefBytes: (ref: Array<number>) => branchContextByRef.get(branchRefKey(ref)),
		commitContextById: (changeOrCommitId: string) => commitContextById.get(changeOrCommitId),
	};
};

export const getHeadInfoIndex = (headInfo: RefInfo): HeadInfoIndex => {
	const cached = headInfoIndexCache.get(headInfo);
	if (cached) return cached;

	const index = buildHeadInfoIndex(headInfo);
	headInfoIndexCache.set(headInfo, index);
	return index;
};

export const renameBranchInHeadInfo = ({
	headInfo,
	branchRef,
	newName,
	newBranchRef,
}: {
	headInfo: RefInfo;
	branchRef: Array<number>;
	newName: string;
	newBranchRef: Array<number>;
}): RefInfo => ({
	...headInfo,
	stacks: headInfo.stacks.map((stack) => ({
		...stack,
		segments: stack.segments.map((segment) => {
			if (!segment.refName || !bytesEqual(segment.refName.fullNameBytes, branchRef)) return segment;

			return {
				...segment,
				refName: {
					...segment.refName,
					displayName: newName,
					fullNameBytes: newBranchRef,
				},
			};
		}),
	})),
});
