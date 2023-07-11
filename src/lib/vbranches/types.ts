import 'reflect-metadata';

export interface Hunk {
	id: string;
	name: string;
	diff: string;
	modifiedAt: Date;
	filePath: string;
}

export interface File {
	id: string;
	path: string;
	hunks: Hunk[];
	expanded: boolean;
	modifiedAt: Date;
}

export interface Branch {
	id: string;
	name: string;
	active: boolean;
	files: File[];
	commits: Commit[];
	description: string;
	mergeable: boolean;
	mergeConflicts: string[];
	order: number;
	upstream: string;
}

export interface Commit {
	id: string;
	authorEmail: string;
	authorName: string;
	description: string;
	createdAt: Date;
	isRemote: boolean;
}

export class BranchData {
	sha!: string;
	branch!: string;
	name!: string;
	description!: string;
	lastCommitTs!: number;
	firstCommitTs!: number;
	ahead!: number;
	behind!: number;
	upstream!: string;
	authors!: string[];
	mergeable!: boolean;
	mergeConflicts!: string[];
}

export class Target {
	sha!: string;
	name!: string;
	remote!: string;
	behind!: number;
}
