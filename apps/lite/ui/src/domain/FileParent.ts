/** @public */
export type CommitFileParent = { stackId: string; commitId: string };

/** @public */
export type BranchFileParent = { stackId: string; branchRef: Array<number> };

export type FileParent =
	| ({ _tag: "Commit" } & CommitFileParent)
	| { _tag: "Changes" }
	| ({ _tag: "Branch" } & BranchFileParent);

/** @public */
export const commitFileParent = ({ stackId, commitId }: CommitFileParent): FileParent => ({
	_tag: "Commit",
	stackId,
	commitId,
});

/** @public */
export const changesFileParent: FileParent = {
	_tag: "Changes",
};

/** @public */
export const branchFileParent = ({ stackId, branchRef }: BranchFileParent): FileParent => ({
	_tag: "Branch",
	stackId,
	branchRef,
});
