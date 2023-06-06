export type FileCard = {
	id: number;
	name: string;
};

export type BranchLane = {
	id: string;
	name: string;
	items: FileCard[];
};
