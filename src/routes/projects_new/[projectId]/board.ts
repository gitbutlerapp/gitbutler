export type Hunk = {
	id: string;
	name: string;
	modified: Date;
};

export type File = {
	id: string;
	path: string;
	hunks: Hunk[];
};

export type BranchLane = {
	id: string;
	name: string;
	active: boolean;
	files: File[];
};
