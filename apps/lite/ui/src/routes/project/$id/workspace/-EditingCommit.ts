export type EditingCommit = {
	stackId: string;
	segmentIndex: number;
	branchName: string | null;
	branchRef: string | null;
	commitId: string;
};
