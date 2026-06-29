import { encodeBytes, bytesEqual } from "#ui/api/bytes.ts";
import { type BranchOperand } from "#ui/operands.ts";
import {
	type Commit,
	type RefInfo,
	type RelativeTo,
	type Segment,
	type Stack,
} from "@gitbutler/but-sdk";

export type HeadInfoIndex = {
	stackContextById: (stackId: string) => { stack: Stack } | undefined;
	branchContextByRefBytes: (ref: Array<number>) => { stack: Stack; segment: Segment } | undefined;
	commitContextById: (
		commitId: string,
	) => { stack: Stack; segment: Segment; commit: Commit } | undefined;
};

const headInfoIndexCache = new WeakMap<RefInfo, HeadInfoIndex>();

const buildHeadInfoIndex = (headInfo: RefInfo): HeadInfoIndex => {
	const stackContextById = new Map<string, { stack: Stack }>();
	const branchContextByRef = new Map<string, { stack: Stack; segment: Segment }>();
	const commitContextById = new Map<string, { stack: Stack; segment: Segment; commit: Commit }>();

	const branchRefKey = (ref: Array<number>): string => ref.join(",");

	for (const stack of headInfo.stacks) {
		if (stack.id !== null) stackContextById.set(stack.id, { stack });

		for (const segment of stack.segments) {
			if (segment.refName) {
				const key = branchRefKey(segment.refName.fullNameBytes);
				if (!branchContextByRef.has(key)) branchContextByRef.set(key, { stack, segment });
			}

			for (const commit of segment.commits)
				if (!commitContextById.has(commit.id))
					commitContextById.set(commit.id, { stack, segment, commit });
		}
	}

	return {
		stackContextById: (id: string) => stackContextById.get(id),
		branchContextByRefBytes: (ref: Array<number>) => branchContextByRef.get(branchRefKey(ref)),
		commitContextById: (id: string) => commitContextById.get(id),
	};
};

export const getHeadInfoIndex = (headInfo: RefInfo): HeadInfoIndex => {
	const cached = headInfoIndexCache.get(headInfo);
	if (cached) return cached;

	const index = buildHeadInfoIndex(headInfo);
	headInfoIndexCache.set(headInfo, index);
	return index;
};

export const findCommitStackId = (headInfo: RefInfo, commitId: string): string | null => {
	for (const stack of headInfo.stacks) {
		if (stack.id === null) continue;

		for (const segment of stack.segments)
			if (segment.commits.some((commit) => commit.id === commitId)) return stack.id;
	}

	return null;
};

export const findBranchOperandByRef = ({
	headInfo,
	branchRef,
}: {
	headInfo: RefInfo;
	branchRef: Array<number>;
}): BranchOperand | null => {
	for (const stack of headInfo.stacks) {
		if (stack.id === null) continue;

		for (const segment of stack.segments)
			if (segment.refName && bytesEqual(segment.refName.fullNameBytes, branchRef))
				return { stackId: stack.id, branchRef };
	}

	return null;
};

export const renameBranchInHeadInfo = ({
	headInfo,
	stackId,
	branchRef,
	newName,
	newBranchRef,
}: {
	headInfo: RefInfo;
	stackId: string;
	branchRef: Array<number>;
	newName: string;
	newBranchRef: Array<number>;
}): RefInfo => ({
	...headInfo,
	stacks: headInfo.stacks.map((stack) => {
		if (stack.id !== stackId) return stack;

		return {
			...stack,
			segments: stack.segments.map((segment) => {
				if (!segment.refName || !bytesEqual(segment.refName.fullNameBytes, branchRef))
					return segment;

				return {
					...segment,
					refName: {
						...segment.refName,
						displayName: newName,
						fullNameBytes: newBranchRef,
					},
				};
			}),
		};
	}),
});

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
