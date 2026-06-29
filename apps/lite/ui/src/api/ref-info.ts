import { encodeBytes, bytesEqual } from "#ui/api/bytes.ts";
import { type BranchOperand } from "#ui/operands.ts";
import {
	type Commit,
	type RefInfo,
	type RelativeTo,
	type Segment,
	type Stack,
} from "@gitbutler/but-sdk";

export const branchRefKey = (branchRef: Array<number>): string => branchRef.join(",");

export type HeadInfoIndex = {
	// files tree
	branchNameByCommitId: Map<string, string | undefined>;
	commitById: Map<string, Commit>;
	// outline & label. label could probably get by w/o it
	segmentByBranchRef: Map<string, Segment>;
	// outline
	stackById: Map<string, Stack>;
};

const headInfoIndexCache = new WeakMap<RefInfo, HeadInfoIndex>();

const buildHeadInfoIndex = (headInfo: RefInfo): HeadInfoIndex => {
	const branchNameByCommitId = new Map<string, string | undefined>();
	const commitById = new Map<string, Commit>();
	const segmentByBranchRef = new Map<string, Segment>();
	const stackById = new Map<string, Stack>();

	for (const stack of headInfo.stacks) {
		if (stack.id !== null) stackById.set(stack.id, stack);

		for (const segment of stack.segments) {
			if (segment.refName) {
				const key = branchRefKey(segment.refName.fullNameBytes);
				if (!segmentByBranchRef.has(key)) segmentByBranchRef.set(key, segment);
			}

			const branchName = segment.refName?.displayName;
			for (const commit of segment.commits) {
				branchNameByCommitId.set(commit.id, branchName);
				if (!commitById.has(commit.id)) commitById.set(commit.id, commit);
			}
		}
	}

	return {
		branchNameByCommitId,
		commitById,
		segmentByBranchRef,
		stackById,
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
				headInfoIndex.segmentByBranchRef.get(branchRefKey(relativeTo.subject))?.commits[0]?.id ??
				null
			);
		case "reference":
			return (
				headInfoIndex.segmentByBranchRef.get(branchRefKey(encodeBytes(relativeTo.subject)))
					?.commits[0]?.id ?? null
			);
	}
};
