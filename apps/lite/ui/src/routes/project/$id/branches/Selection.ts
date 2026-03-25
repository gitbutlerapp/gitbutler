import { BranchIdentity, BranchListing } from "@gitbutler/but-sdk";

export type Selection =
	| {
			_tag: "Branch";
			branchName: BranchIdentity;
	  }
	| {
			_tag: "Commit";
			branchName: BranchIdentity;
			commitId: string;
	  }
	| {
			_tag: "CommitFile";
			branchName: BranchIdentity;
			commitId: string;
			path: string;
	  };

export const isBranchSelected = (
	selection: Selection | null,
	branchName: BranchIdentity,
): boolean => selection?._tag === "Branch" && selection.branchName === branchName;

export const isBranchSelectedWithin = (
	selection: Selection | null,
	branchName: BranchIdentity,
): boolean =>
	(selection?._tag === "Commit" || selection?._tag === "CommitFile") &&
	selection.branchName === branchName;

export const isCommitSelected = (
	selection: Selection | null,
	branchName: BranchIdentity,
	commitId: string,
): boolean =>
	selection?._tag === "Commit" &&
	selection.branchName === branchName &&
	selection.commitId === commitId;

export const isCommitSelectedWithin = (
	selection: Selection | null,
	branchName: BranchIdentity,
	commitId: string,
): boolean =>
	selection?._tag === "CommitFile" &&
	selection.branchName === branchName &&
	selection.commitId === commitId;

export const isCommitFileSelected = (
	selection: Selection | null,
	branchName: BranchIdentity,
	commitId: string,
	path: string,
): boolean =>
	selection?._tag === "CommitFile" &&
	selection.branchName === branchName &&
	selection.commitId === commitId &&
	selection.path === path;

export const toggleBranchSelection = (
	selection: Selection | null,
	branchName: BranchIdentity,
): Selection | null =>
	isBranchSelected(selection, branchName) ? null : { _tag: "Branch", branchName };

export const toggleCommitSelection = (
	selection: Selection | null,
	branchName: BranchIdentity,
	commitId: string,
): Selection | null =>
	isCommitSelected(selection, branchName, commitId)
		? { _tag: "Branch", branchName }
		: { _tag: "Commit", branchName, commitId };

export const toggleCommitFileSelection = (
	selection: Selection | null,
	branchName: BranchIdentity,
	commitId: string,
	path: string,
): Selection | null =>
	isCommitFileSelected(selection, branchName, commitId, path)
		? { _tag: "Commit", branchName, commitId }
		: { _tag: "CommitFile", branchName, commitId, path };

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
	return { _tag: "Branch", branchName: firstBranch.name };
};
