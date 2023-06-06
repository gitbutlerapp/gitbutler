export type Hunk = {
	id: string;
	filePath: string;
	description: string;
	diff: string;
	modified: Date;
	commitId: string | null;
};

export type BranchLane = {
	id: string;
	name: string;
	hunks: Hunk[];
};
