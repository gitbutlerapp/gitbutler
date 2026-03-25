import { BranchIdentity, BranchListing } from "@gitbutler/but-sdk";

type BranchSelection = { branchName: BranchIdentity };

type CommitMode =
	| {
			_tag: "Summary";
	  }
	| {
			_tag: "Details";
			path?: string;
	  };
type CommitSelection = { branchName: BranchIdentity; commitId: string; mode: CommitMode };

export type Selection =
	| ({ _tag: "Branch" } & BranchSelection)
	| ({ _tag: "Commit" } & CommitSelection);

export const toggleBranchSelection = (
	selection: Selection | null,
	branchName: BranchIdentity,
): Selection | null =>
	selection?._tag === "Branch" && selection.branchName === branchName
		? null
		: {
				_tag: "Branch",
				branchName,
			};

export const toggleCommitSelection = (
	selection: Selection | null,
	branchName: BranchIdentity,
	commitId: string,
): Selection | null =>
	selection?._tag === "Commit" &&
	selection.branchName === branchName &&
	selection.commitId === commitId &&
	selection.mode._tag !== "Details"
		? {
				_tag: "Branch",
				branchName,
			}
		: {
				_tag: "Commit",
				branchName,
				commitId,
				mode: {
					_tag: "Summary",
				},
			};

export const toggleCommitFileSelection = (
	selection: Selection | null,
	branchName: BranchIdentity,
	commitId: string,
	path: string,
): Selection | null =>
	selection?._tag === "Commit" &&
	selection.branchName === branchName &&
	selection.commitId === commitId &&
	selection.mode._tag === "Details" &&
	selection.mode.path === path
		? {
				_tag: "Commit",
				branchName,
				commitId,
				mode: {
					_tag: "Summary",
				},
			}
		: {
				_tag: "Commit",
				branchName,
				commitId,
				mode: {
					_tag: "Details",
					path,
				},
			};

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
