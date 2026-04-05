import { BranchIdentity, BranchListing } from "@gitbutler/but-sdk";

type BranchSelection = { branchName: BranchIdentity };

type CommitMode = { _tag: "Summary" } | { _tag: "Details"; path?: string };
type CommitSelection = BranchSelection & { commitId: string; mode: CommitMode };

export type Selection =
	| ({ _tag: "Branch" } & BranchSelection)
	| ({ _tag: "Commit" } & CommitSelection);

export const normalizeBranchSelection = (
	selection: Selection,
	branches: Array<BranchListing>,
): Selection | null => {
	const branch = branches.find((branch) => branch.name === selection.branchName);
	if (!branch) return null;
	return selection;
};

export const getDefaultSelection = (branches: Array<BranchListing>): Selection | null => {
	const firstBranch = branches[0];
	if (!firstBranch) return null;
	return {
		_tag: "Branch",
		branchName: firstBranch.name,
	};
};
