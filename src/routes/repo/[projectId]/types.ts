export type BranchData = {
	sha: string;
	branch: string;
	name: string;
	description: string;
	lastCommitTs: number;
	firstCommitTs: number;
	ahead: number;
	behind: number;
	upstream: string;
	authors: string[];
	mergeable: boolean;
	mergeConflicts: string[];
};

export interface Target {
	sha: string;
	name: string;
	remote: string;
	behind: number;
}
