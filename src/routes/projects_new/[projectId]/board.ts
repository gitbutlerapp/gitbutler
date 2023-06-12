export type Hunk = {
	id: string;
	name: string;
	modified: Date;
	kind: string;
	filePath: string;
	isDndShadowItem?: boolean;
};

export type File = {
	id: string;
	path: string;
	hunks: Hunk[];
	kind: string;
	isDndShadowItem?: boolean;
};

export type BranchLane = {
	id: string;
	name: string;
	active: boolean;
	files: File[];
	kind: string;
};
