/** @public */
export type BranchFileParent = { stackId: string; branchRef: Array<number> };
/** @public */
export type CommitFileParent = { stackId: string; commitId: string };

export type FileParent =
	| { _tag: "Changes" }
	| ({ _tag: "Branch" } & BranchFileParent)
	| ({ _tag: "Commit" } & CommitFileParent);

/** @public */
export const branchFileParent = ({ stackId, branchRef }: BranchFileParent): FileParent => ({
	_tag: "Branch",
	stackId,
	branchRef,
});

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
