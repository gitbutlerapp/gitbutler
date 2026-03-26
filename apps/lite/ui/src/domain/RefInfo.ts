import { Segment, type RefInfo } from "@gitbutler/but-sdk";

export const getCommonBaseCommitId = (headInfo: RefInfo): string | undefined => {
	const bases = headInfo.stacks
		.map((stack) => stack.base)
		.filter((base): base is string => base !== null);
	const first = bases[0];
	if (first === undefined) return undefined;
	return bases.every((base) => base === first) ? first : undefined;
};

export const getBranchNameByCommitId = (headInfo: RefInfo): Map<string, string> => {
	const byCommitId = new Map<string, string>();

	for (const stack of headInfo.stacks)
		for (const segment of stack.segments) {
			const branchName = segment.refName?.displayName ?? "Untitled";
			for (const commit of segment.commits) byCommitId.set(commit.id, branchName);
		}

	return byCommitId;
};

export const getStackIdsByCommitId = (headInfo: RefInfo): Map<string, Set<string>> => {
	const byCommitId = new Map<string, Set<string>>();

	for (const stack of headInfo.stacks) {
		if (stack.id == null) continue;

		for (const segment of stack.segments)
			for (const commit of segment.commits) {
				const stackIds = byCommitId.get(commit.id) ?? new Set<string>();
				stackIds.add(stack.id);
				byCommitId.set(commit.id, stackIds);
			}
	}

	return byCommitId;
};

export const getSegmentBranchRef = (segment: Segment): string | null =>
	segment.refName ? `refs/heads/${segment.refName.displayName}` : null;

export const getBranchRefsByStackId = (headInfo: RefInfo): Map<string, Set<string>> => {
	const refsByStackId = new Map<string, Set<string>>();

	for (const stack of headInfo.stacks) {
		if (stack.id == null) continue;

		const branchRefs = new Set<string>();
		for (const segment of stack.segments) {
			const branchRef = getSegmentBranchRef(segment);
			if (branchRef !== null) branchRefs.add(branchRef);
		}

		refsByStackId.set(stack.id, branchRefs);
	}

	return refsByStackId;
};
