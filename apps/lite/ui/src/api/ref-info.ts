import { encodeRefName, refNamesEqual } from "#ui/api/ref-name.ts";
import { type Commit, type RefInfo, type RelativeTo, type Segment } from "@gitbutler/but-sdk";

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
}): Commit | null => {
	for (const stack of headInfo.stacks)
		for (const segment of stack.segments) {
			const commit = segment.commits.find((candidate) => candidate.id === commitId);
			if (!commit) continue;

			return commit;
		}

	return null;
};

export const findCommitStackId = (headInfo: RefInfo, commitId: string): string | null => {
	for (const stack of headInfo.stacks) {
		if (stack.id === null) continue;

		for (const segment of stack.segments)
			if (segment.commits.some((commit) => commit.id === commitId)) return stack.id;
	}

	return null;
};

export const findSegmentByBranchRef = ({
	headInfo,
	branchRef,
}: {
	headInfo: RefInfo;
	branchRef: Array<number> | null;
}): Segment | null => {
	for (const stack of headInfo.stacks)
		for (const segment of stack.segments)
			if (refNamesEqual(segment.refName?.fullNameBytes ?? null, branchRef)) return segment;

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
				if (!segment.refName || !refNamesEqual(segment.refName.fullNameBytes, branchRef))
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
	headInfo,
	relativeTo,
}: {
	headInfo: RefInfo;
	relativeTo: RelativeTo;
}): string | null => {
	switch (relativeTo.type) {
		case "commit":
			return relativeTo.subject;
		case "referenceBytes":
			return (
				findSegmentByBranchRef({ headInfo, branchRef: relativeTo.subject })?.commits[0]?.id ?? null
			);
		case "reference":
			return (
				findSegmentByBranchRef({ headInfo, branchRef: encodeRefName(relativeTo.subject) })
					?.commits[0]?.id ?? null
			);
	}
};
