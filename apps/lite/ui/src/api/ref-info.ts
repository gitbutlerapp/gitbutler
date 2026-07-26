import { encodeBytes } from "#ui/api/bytes.ts";
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

export type HeadInfoIndex = {
	branchContextByRefBytes: (ref: Array<number>) => (StackIndex & SegmentIndex) | undefined;
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

	return {
		branchContextByRefBytes: (ref: Array<number>) => branchContextByRef.get(branchRefKey(ref)),
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
