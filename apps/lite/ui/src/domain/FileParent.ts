/** @public */
export type CommitFileParent = { commitId: string };

/** @public */
export type BranchFileParent = { branchRef: Array<number> };

export type FileParent =
	| ({ _tag: "Commit" } & CommitFileParent)
	| { _tag: "Change" }
	| ({ _tag: "Branch" } & BranchFileParent);

/** @public */
export const commitFileParent = ({ commitId }: CommitFileParent): FileParent => ({
	_tag: "Commit",
	commitId,
});

/** @public */
export const changeFileParent: FileParent = {
	_tag: "Change",
};

/** @public */
export const branchFileParent = ({ branchRef }: BranchFileParent): FileParent => ({
	_tag: "Branch",
	branchRef,
});
