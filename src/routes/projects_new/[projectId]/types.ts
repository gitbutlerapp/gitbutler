export type Hunk = {
	id: string;
	name: string;
	modifiedAt: Date;
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

export type Commit = {
	id: string;
	description: string;
	committedAt: Date;
	files: File[];
	kind: string;
};

export type Branch = {
	id: string;
	name: string;
	active: boolean;
	commits: Commit[];
	kind: string;
};
