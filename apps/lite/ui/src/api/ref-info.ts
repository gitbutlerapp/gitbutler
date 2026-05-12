import { type Commit, type RefInfo, type Segment } from "@gitbutler/but-sdk";

export const getCommonBaseCommitId = (headInfo: RefInfo): string | undefined => {
	const bases = headInfo.stacks
		.map((stack) => stack.base)
		.filter((base): base is string => base !== null);
	const first = bases[0];
	if (first === undefined) return undefined;
	return bases.every((base) => base === first) ? first : undefined;
};

export const getBranchNameByCommitId = (headInfo: RefInfo): Map<string, string | undefined> => {
	const byCommitId = new Map<string, string | undefined>();

	for (const stack of headInfo.stacks)
		for (const segment of stack.segments) {
			const branchName = segment.refName?.displayName;
			for (const commit of segment.commits) byCommitId.set(commit.id, branchName);
		}

	return byCommitId;
};

export const findCommit = ({
	headInfo,
	commitId,
}: {
	headInfo: RefInfo;
	commitId: string;
}): Commit | undefined => {
	for (const stack of headInfo.stacks)
		for (const segment of stack.segments) {
			const commit = segment.commits.find((candidate) => candidate.id === commitId);
			if (!commit) continue;

			return commit;
		}

	return undefined;
};

const branchRefsEqual = (
	left: Array<number> | undefined,
	right: Array<number> | undefined,
): boolean =>
	left === right ||
	(left !== undefined &&
		right !== undefined &&
		left.length === right.length &&
		left.every((value, index) => value === right[index]));

export const findSegmentByBranchRef = ({
	headInfo,
	branchRef,
}: {
	headInfo: RefInfo;
	branchRef: Array<number> | undefined;
}): Segment | undefined => {
	for (const stack of headInfo.stacks)
		for (const segment of stack.segments)
			if (branchRefsEqual(segment.refName?.fullNameBytes, branchRef)) return segment;

	return undefined;
};
