export type EditingCommit = {
	stackId: string;
	segmentIndex: number;
	branchName: string | null;
	branchRef: string | null;
	commitId: string;
};

export type RenamingBranch = {
	stackId: string;
	segmentIndex: number;
};

export type Editing =
	| { _tag: "CommitMessage"; subject: EditingCommit }
	| { _tag: "BranchName"; subject: RenamingBranch };
