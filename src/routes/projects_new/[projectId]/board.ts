export type FileCard = {
	id: number;
	name: string;
};

export type BranchLane = {
	id: string;
	name: string;
	active: boolean;
	items: FileCard[];
};
