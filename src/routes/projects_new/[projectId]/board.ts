export type FileCard = {
	id: number;
	name: string;
};

export type Hunk = {
	id: string;
	updated: Date;
	description: string;
};

export type File = {
	filePath: string;
	hunks: Hunk[];
};

export type BranchLane = {
	id: string;
	name: string;
	items: FileCard[];
	uncommittedFiles: File[];
};
