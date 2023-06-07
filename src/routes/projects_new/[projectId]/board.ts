export type Hunk = {
	id: string;
	name: string;
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
