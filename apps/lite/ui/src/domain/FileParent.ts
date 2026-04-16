/** @public */
export type CommitFileParent = { commitId: string };
/** @public */
export type ChangesSectionFileParent = {};

export type FileParent =
	| ({ _tag: "Commit" } & CommitFileParent)
	| ({ _tag: "ChangesSection" } & ChangesSectionFileParent);

/** @public */
export const commitFileParent = ({ commitId }: CommitFileParent): FileParent => ({
	_tag: "Commit",
	commitId,
});

/** @public */
export const changesSectionFileParent = (_x: ChangesSectionFileParent): FileParent => ({
	_tag: "ChangesSection",
});
